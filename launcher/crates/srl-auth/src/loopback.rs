//! The loopback redirect of the browser flow (RFC 8252 section 7.3): a one-shot HTTP listener on an
//! ephemeral port, which receives `?code=...&state=...` from the browser and answers with a "you can
//! close this tab" page.
//!
//! Adapted from Pandora Launcher's `crates/auth/src/serve_redirect.rs` (MIT). Changes: an ephemeral
//! port instead of the fixed 3160 (which fails when the port is taken), an IPv6 listener on the
//! same port, `state` checked before anything else in the query and required (Pandora accepts a
//! missing one), connections served side by side with a read deadline each, and our own page.
//!
//! Other local processes can reach the port, so nothing they send may end or stall the flow: a
//! request without our state gets the "not started here" page and the listener keeps waiting for
//! the real redirect (PKCE already makes a stolen code useless; this keeps the real one coming in).
use std::{io, sync::Arc, time::Duration};

use reqwest::Url;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
    time::Instant,
};

use crate::{error::AuthError, pkce::same, secret::Secret};

/// A request bigger than this isn't a redirect from a browser.
const MAX_REQUEST: usize = 16 * 1024;
/// A connection that sends nothing useful in this long is dropped, so a stuck or hostile client
/// can't hold up the browser's real request.
const READ_DEADLINE: Duration = Duration::from_secs(5);
/// Connections served at once. More wait in the OS backlog until one finishes (at most
/// `READ_DEADLINE`), so a flood can delay the redirect by seconds, not stop it.
const MAX_CONNECTIONS: usize = 64;
/// Ports tried before giving up when someone else holds [::1] on the port we got for 127.0.0.1.
const BIND_TRIES: usize = 16;

pub struct Loopback {
    v4: TcpListener,
    v6: Option<TcpListener>,
    port: u16,
}

enum Outcome {
    /// Not the redirect (a favicon, a port scan): keep listening.
    Ignore,
    /// A redirect without our state (a stale tab, another program): keep listening, but report
    /// `StateMismatch` instead of `TimedOut` if nothing better arrives.
    Mismatch,
    Done(Result<Secret, AuthError>),
}

/// The IPv6 side of a port.
enum Pair {
    Ready(Loopback),
    /// Another process has [::1] on this port. Browsers try ::1 first for `localhost`, so the
    /// redirect would reach it instead of us: take another port.
    V6Taken(io::Error),
}

impl Loopback {
    /// Binds 127.0.0.1 on a port the OS picks, then [::1] on the same port (browsers may resolve
    /// `localhost` to either). If [::1] is taken on that port, tries another port; only when the
    /// machine has no IPv6 loopback at all does it listen on 127.0.0.1 alone (browsers then fall
    /// back to it, since nothing answers on ::1).
    pub async fn bind() -> io::Result<Self> {
        let mut last = None;
        for _ in 0..BIND_TRIES {
            let v4 = TcpListener::bind(("127.0.0.1", 0)).await?;
            match pair(v4).await? {
                Pair::Ready(lb) => return Ok(lb),
                Pair::V6Taken(e) => last = Some(e),
            }
        }
        Err(last.expect("BIND_TRIES > 0"))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `http://<host>:<port>`, which must match the registered `http://<host>` but for the port.
    pub fn redirect_uri(&self, host: &str) -> String {
        format!("http://{host}:{}", self.port)
    }

    /// Serves until the redirect with our `state` arrives, or `timeout` passes (`TimedOut`, or
    /// `StateMismatch` if only redirects with someone else's state came in). Returns the code
    /// only if `state` matches ours.
    pub async fn wait_for_code(self, state: &str, timeout: Duration) -> Result<Secret, AuthError> {
        let deadline = Instant::now() + timeout;
        let state: Arc<str> = state.into();
        let mut tasks = JoinSet::new();
        let mut mismatch = false;
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(if mismatch {
                        AuthError::StateMismatch
                    } else {
                        AuthError::TimedOut
                    });
                }
                Some(done) = tasks.join_next(), if !tasks.is_empty() => match done {
                    Ok(Outcome::Done(result)) => return result,
                    Ok(Outcome::Mismatch) => mismatch = true,
                    Ok(Outcome::Ignore) | Err(_) => {}
                },
                accepted = accept(&self.v4, self.v6.as_ref()), if tasks.len() < MAX_CONNECTIONS => {
                    match accepted {
                        Ok(stream) => {
                            let state = state.clone();
                            tasks.spawn(async move {
                                match tokio::time::timeout(READ_DEADLINE, handle(stream, &state)).await {
                                    Ok(Ok(outcome)) => outcome,
                                    Ok(Err(_)) | Err(_) => Outcome::Ignore,
                                }
                            });
                        }
                        // Out of file descriptors and the like: don't spin, the browser can wait.
                        Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                    }
                }
            }
        }
    }
}

async fn pair(v4: TcpListener) -> io::Result<Pair> {
    let port = v4.local_addr()?.port();
    match TcpListener::bind(("::1", port)).await {
        Ok(v6) => Ok(Pair::Ready(Loopback {
            v4,
            v6: Some(v6),
            port,
        })),
        // Taken by someone else (Windows says "access denied" for an exclusive bind).
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied
            ) =>
        {
            Ok(Pair::V6Taken(e))
        }
        // No IPv6 loopback on this machine: nobody can be on ::1 either.
        Err(_) => Ok(Pair::Ready(Loopback { v4, v6: None, port })),
    }
}

/// The next connection on either listener. Cancel-safe, like `TcpListener::accept`.
async fn accept(v4: &TcpListener, v6: Option<&TcpListener>) -> io::Result<TcpStream> {
    let (stream, _) = match v6 {
        Some(v6) => tokio::select! {
            r = v4.accept() => r,
            r = v6.accept() => r,
        },
        None => v4.accept().await,
    }?;
    Ok(stream)
}

async fn handle(mut stream: TcpStream, state: &str) -> io::Result<Outcome> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let target = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(Outcome::Ignore);
        }
        buf.extend_from_slice(&chunk[..n]);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut headers);
        match req.parse(&buf) {
            Ok(httparse::Status::Complete(_)) => {
                if req.method != Some("GET") {
                    respond(&mut stream, "405 Method Not Allowed", None).await?;
                    return Ok(Outcome::Ignore);
                }
                break req.path.unwrap_or("/").to_string();
            }
            Ok(httparse::Status::Partial) if buf.len() < MAX_REQUEST => continue,
            _ => {
                respond(&mut stream, "400 Bad Request", None).await?;
                return Ok(Outcome::Ignore);
            }
        }
    };
    let Ok(url) = Url::parse(&format!("http://loopback{target}")) else {
        respond(&mut stream, "400 Bad Request", None).await?;
        return Ok(Outcome::Ignore);
    };
    if url.path() != "/" {
        respond(&mut stream, "404 Not Found", None).await?;
        return Ok(Outcome::Ignore);
    }
    let (mut code, mut got_state, mut error) = (None, None, None);
    for (k, v) in url.query_pairs() {
        match &*k {
            "code" => code = Some(Secret::new(v.into_owned())),
            "state" => got_state = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            _ => {}
        }
    }
    if code.is_none() && got_state.is_none() && error.is_none() {
        respond(&mut stream, "404 Not Found", None).await?;
        return Ok(Outcome::Ignore);
    }
    // The state first: without our state, nothing in the request is trusted, not even an error,
    // and it doesn't end the flow either (any local process can send one).
    if !got_state.is_some_and(|s| same(&s, state)) {
        respond(&mut stream, "400 Bad Request", Some(Page::Mismatch)).await?;
        return Ok(Outcome::Mismatch);
    }
    if let Some(error) = error {
        respond(&mut stream, "200 OK", Some(Page::Failed)).await?;
        let err = match error.as_str() {
            "access_denied" | "consent_required" | "interaction_required" => AuthError::Declined,
            other => AuthError::Service {
                service: "Microsoft login",
                status: 400,
                detail: crate::error::clip(other),
            },
        };
        return Ok(Outcome::Done(Err(err)));
    }
    match code {
        Some(code) => {
            respond(&mut stream, "200 OK", Some(Page::Done)).await?;
            Ok(Outcome::Done(Ok(code)))
        }
        None => {
            respond(&mut stream, "400 Bad Request", Some(Page::Failed)).await?;
            Ok(Outcome::Ignore)
        }
    }
}

#[derive(Clone, Copy)]
enum Page {
    Done,
    Failed,
    Mismatch,
}

/// The page the browser shows. Both languages, since the launcher's language isn't known here and
/// the page is seen for a second.
fn page(p: Page) -> String {
    let (en, pl) = match p {
        Page::Done => (
            "Signed in. You can close this tab and go back to Speedrunning Launcher.",
            "Zalogowano. Możesz zamknąć tę kartę i wrócić do Speedrunning Launchera.",
        ),
        Page::Failed => (
            "The sign-in did not finish. Go back to Speedrunning Launcher for details.",
            "Logowanie się nie udało. Szczegóły są w Speedrunning Launcherze.",
        ),
        Page::Mismatch => (
            "This sign-in was not started by this launcher window. Start it again from the launcher.",
            "To logowanie nie zostało rozpoczęte w tym oknie launchera. Zacznij je jeszcze raz.",
        ),
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Speedrunning Launcher</title>\
         <style>body{{font-family:system-ui,sans-serif;background:#16181d;color:#e8e6e3;\
         display:grid;place-items:center;height:100vh;margin:0}}main{{max-width:32rem;padding:2rem}}\
         p{{line-height:1.5}}.pl{{opacity:.7}}</style></head><body><main>\
         <p>{en}</p><p class=\"pl\">{pl}</p></main></body></html>"
    )
}

async fn respond(stream: &mut TcpStream, status: &str, body: Option<Page>) -> io::Result<()> {
    let body = body.map(page).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_secs(10);

    async fn get(port: u16, path_and_query: &str) -> u16 {
        let resp = reqwest::get(format!("http://127.0.0.1:{port}{path_and_query}"))
            .await
            .unwrap();
        resp.status().as_u16()
    }

    #[tokio::test]
    async fn returns_the_code_when_state_matches() {
        let lb = Loopback::bind().await.unwrap();
        let port = lb.port();
        assert_ne!(port, 0);
        assert_eq!(
            lb.redirect_uri("localhost"),
            format!("http://localhost:{port}")
        );
        let wait = tokio::spawn(async move { lb.wait_for_code("good-state", WAIT).await });
        // Noise first: other paths and empty queries are ignored, the listener keeps going.
        assert_eq!(get(port, "/favicon.ico").await, 404);
        assert_eq!(get(port, "/").await, 404);
        assert_eq!(get(port, "/?code=the-code&state=good-state").await, 200);
        let code = wait.await.unwrap().unwrap();
        assert_eq!(code.expose(), "the-code");
    }

    /// Requests without our state (any local process can send them) are refused, but don't end
    /// the flow: the real redirect still gets through.
    #[tokio::test]
    async fn a_foreign_state_is_refused_and_the_real_redirect_still_works() {
        let lb = Loopback::bind().await.unwrap();
        let port = lb.port();
        let wait = tokio::spawn(async move { lb.wait_for_code("good-state", WAIT).await });
        for probe in [
            "/?code=stolen&state=evil",
            "/?code=stolen",
            "/?state=x",
            "/?error=access_denied&state=evil",
            "/?error=access_denied",
        ] {
            assert_eq!(get(port, probe).await, 400, "{probe}");
        }
        assert_eq!(get(port, "/?code=real&state=good-state").await, 200);
        assert_eq!(wait.await.unwrap().unwrap().expose(), "real");
    }

    #[tokio::test]
    async fn only_foreign_states_end_as_a_mismatch_at_the_timeout() {
        let lb = Loopback::bind().await.unwrap();
        let port = lb.port();
        let wait = tokio::spawn(async move {
            lb.wait_for_code("good-state", Duration::from_millis(500))
                .await
        });
        assert_eq!(get(port, "/?code=stolen").await, 400);
        assert_eq!(wait.await.unwrap().unwrap_err(), AuthError::StateMismatch);

        let lb = Loopback::bind().await.unwrap();
        let quiet = lb.wait_for_code("s", Duration::from_millis(100)).await;
        assert_eq!(quiet.unwrap_err(), AuthError::TimedOut);
    }

    /// Idle connections don't hold up the real redirect: each is served on its own.
    #[tokio::test]
    async fn idle_connections_dont_delay_the_redirect() {
        let lb = Loopback::bind().await.unwrap();
        let port = lb.port();
        let wait = tokio::spawn(async move { lb.wait_for_code("s", WAIT).await });
        let mut idle = Vec::new();
        for _ in 0..3 {
            idle.push(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
        }
        let started = std::time::Instant::now();
        assert_eq!(get(port, "/?code=c&state=s").await, 200);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(wait.await.unwrap().unwrap().expose(), "c");
        drop(idle);
    }

    /// Someone on [::1] at the port we got for 127.0.0.1 would receive the browser's redirect
    /// (browsers try ::1 first for `localhost`): that port is not used.
    #[tokio::test]
    async fn a_port_taken_on_ipv6_is_not_used() {
        let Ok(squatter) = TcpListener::bind(("::1", 0)).await else {
            eprintln!("skipped: no IPv6 loopback here");
            return;
        };
        let port = squatter.local_addr().unwrap().port();
        let Ok(v4) = TcpListener::bind(("127.0.0.1", port)).await else {
            eprintln!("skipped: 127.0.0.1:{port} is taken too");
            return;
        };
        assert!(matches!(pair(v4).await.unwrap(), Pair::V6Taken(_)));
        // bind() never ends on a port someone holds on ::1, and listens on both.
        let lb = Loopback::bind().await.unwrap();
        assert_ne!(lb.port(), port);
        assert!(lb.v6.is_some());
    }

    #[tokio::test]
    async fn a_declined_consent_ends_the_flow() {
        let lb = Loopback::bind().await.unwrap();
        let port = lb.port();
        let wait = tokio::spawn(async move { lb.wait_for_code("s", WAIT).await });
        assert_eq!(
            get(port, "/?error=access_denied&error_description=no&state=s").await,
            200
        );
        assert_eq!(wait.await.unwrap().unwrap_err(), AuthError::Declined);
    }
}
