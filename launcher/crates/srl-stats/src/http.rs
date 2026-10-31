//! HTTP for the stats APIs: one GET with a short deadline, errors the UI can word, and a
//! client-side rate limit so the launcher can never be the reason a player gets throttled.
use std::{
    collections::VecDeque,
    fmt,
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

/// The longest pause a 429 can impose. A server, proxy or captive portal can send any number in
/// `Retry-After`; past an hour a retry when the player next opens the page serves them better, and
/// a huge value must not overflow `Instant + Duration`.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(3600);

/// Per request, connect included. The APIs answer in well under a second; the page shows cached
/// data while it waits, so a slow network should fail fast rather than hang the refresh.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// Why fresh data could not be had. Cloneable and comparable so it can sit in UI state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiError {
    /// The name cannot be a Minecraft name (or UUID); nothing was sent.
    InvalidName,
    /// The API says there is no such player.
    NotFound,
    /// The API (429) or our own limiter said to wait this many seconds.
    RateLimited { retry_after_s: u64 },
    /// No connection, DNS failure or timeout.
    Offline(String),
    /// Any other HTTP status.
    Http(u16),
    /// The answer was not what the API documents.
    BadResponse(String),
    /// Only set on `Cached::refresh_error`: the caller asked for the cache only
    /// (`Refresh::Never`, the `*_cached` methods) and the value is older than its TTL. Nothing
    /// failed; the value is just not current, so the UI labels it by `Cached::age()`.
    NotRefreshed,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::InvalidName => write!(f, "not a valid player name"),
            ApiError::NotFound => write!(f, "no such player"),
            ApiError::RateLimited { retry_after_s } => {
                write!(f, "too many requests, try again in {retry_after_s} s")
            }
            ApiError::Offline(e) => write!(f, "offline: {e}"),
            ApiError::Http(code) => write!(f, "HTTP {code}"),
            ApiError::BadResponse(e) => write!(f, "unexpected answer: {e}"),
            ApiError::NotRefreshed => write!(f, "not refreshed yet"),
        }
    }
}

impl std::error::Error for ApiError {}

/// A sliding-window limit on our own requests, plus the pause a 429 asks for.
pub(crate) struct Limiter {
    window: Duration,
    max: usize,
    state: Mutex<LimiterState>,
}

#[derive(Default)]
struct LimiterState {
    sent: VecDeque<Instant>,
    blocked_until: Option<Instant>,
}

impl Limiter {
    pub(crate) fn new(max: usize, window: Duration) -> Self {
        Limiter {
            window,
            max,
            state: Mutex::default(),
        }
    }

    /// Takes a slot, or says how long to wait. Never sleeps: the caller shows cached data instead.
    pub(crate) fn acquire(&self) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut s = self.lock();
        if let Some(until) = s.blocked_until {
            if until > now {
                return Err(ApiError::RateLimited {
                    retry_after_s: (until - now).as_secs().max(1),
                });
            }
            s.blocked_until = None;
        }
        while s.sent.front().is_some_and(|&t| now - t >= self.window) {
            s.sent.pop_front();
        }
        if s.sent.len() >= self.max {
            let oldest = *s.sent.front().unwrap();
            let wait = self.window.saturating_sub(now - oldest);
            return Err(ApiError::RateLimited {
                retry_after_s: wait.as_secs().max(1),
            });
        }
        s.sent.push_back(now);
        Ok(())
    }

    /// Pauses every request for `d`, at most `MAX_RETRY_AFTER`.
    fn block_for(&self, d: Duration) {
        let until = Instant::now().checked_add(d.min(MAX_RETRY_AFTER));
        let mut s = self.lock();
        s.blocked_until = until.or(s.blocked_until);
    }

    /// The state, even after a panic elsewhere poisoned the lock: it is two plain values that are
    /// always valid, and a poisoned limiter must not turn every later call into a panic.
    fn lock(&self) -> std::sync::MutexGuard<'_, LimiterState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// GET `url`; `Ok((status, body))` for every status except 429, which becomes `RateLimited` and
/// pauses the limiter for as long as the server asks.
pub(crate) async fn get(
    client: &reqwest::Client,
    limiter: &Limiter,
    url: &str,
) -> Result<(u16, bytes::Bytes), ApiError> {
    limiter.acquire()?;
    let resp = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(net_error)?;
    let status = resp.status().as_u16();
    if status == 429 {
        let wait = retry_after(resp.headers())
            .unwrap_or(60)
            .min(MAX_RETRY_AFTER.as_secs());
        limiter.block_for(Duration::from_secs(wait));
        return Err(ApiError::RateLimited {
            retry_after_s: wait,
        });
    }
    let body = resp.bytes().await.map_err(net_error)?;
    Ok((status, body))
}

fn net_error(e: reqwest::Error) -> ApiError {
    if e.is_decode() {
        ApiError::BadResponse(e.to_string())
    } else {
        ApiError::Offline(e.to_string())
    }
}

/// `Retry-After: <seconds>`, else the IETF draft `RateLimit: ...; t=<seconds>` header the MCSR
/// Ranked API sends (seen live: `ratelimit: "500-in-10min"; r=497; t=598`).
fn retry_after(h: &reqwest::header::HeaderMap) -> Option<u64> {
    if let Some(s) = h.get("retry-after").and_then(|v| v.to_str().ok())
        && let Ok(n) = s.trim().parse()
    {
        return Some(n);
    }
    let v = h.get("ratelimit")?.to_str().ok()?;
    v.split(';')
        .filter_map(|p| p.trim().strip_prefix("t="))
        .find_map(|t| t.trim().parse().ok())
}

/// A Minecraft name (1 to 16 of `A-Z a-z 0-9 _`; old accounts can be shorter than today's
/// minimum of 3) or a UUID with or without dashes. Checked before it goes into a URL path.
pub fn is_lookup_name(name: &str) -> bool {
    let simple =
        |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    let uuid = name.len() == 36 && name.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-');
    (simple(name) && name.len() <= 16) || uuid || (name.len() == 32 && simple(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_counts_and_blocks() {
        let l = Limiter::new(2, Duration::from_secs(600));
        assert!(l.acquire().is_ok());
        assert!(l.acquire().is_ok());
        assert!(
            matches!(l.acquire(), Err(ApiError::RateLimited { retry_after_s }) if retry_after_s > 500)
        );
        let l = Limiter::new(10, Duration::from_secs(600));
        l.block_for(Duration::from_secs(30));
        assert!(matches!(l.acquire(), Err(ApiError::RateLimited { .. })));
    }

    #[test]
    fn a_huge_retry_after_neither_overflows_nor_poisons() {
        let l = Limiter::new(10, Duration::from_secs(600));
        l.block_for(Duration::from_secs(u64::MAX));
        for _ in 0..2 {
            match l.acquire() {
                Err(ApiError::RateLimited { retry_after_s }) => {
                    assert!(retry_after_s <= MAX_RETRY_AFTER.as_secs())
                }
                other => panic!("{other:?}"),
            }
        }
        // A panic while the lock is held (anywhere) leaves the limiter usable.
        let l = std::sync::Arc::new(Limiter::new(10, Duration::from_secs(600)));
        let l2 = l.clone();
        let _ = std::thread::spawn(move || {
            let _held = l2.state.lock().unwrap();
            panic!("poisoning the limiter on purpose");
        })
        .join();
        assert!(l.state.is_poisoned());
        assert!(l.acquire().is_ok());
    }

    /// A real 429 over HTTP from a local server, with a Retry-After no `Instant` can hold.
    #[tokio::test]
    async fn a_429_with_a_huge_retry_after_over_http() {
        use std::io::{Read, Write};
        // A std server thread: the workspace's tokio has no `net` feature of its own.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf);
            let _ = sock.write_all(
                b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 18446744073709551615\r\n\
                  Content-Length: 0\r\nConnection: close\r\n\r\n",
            );
        });
        let client = reqwest::Client::new();
        let l = Limiter::new(10, Duration::from_secs(600));
        assert_eq!(
            get(&client, &l, &url).await.unwrap_err(),
            ApiError::RateLimited {
                retry_after_s: MAX_RETRY_AFTER.as_secs()
            }
        );
        // The next call is refused by the limiter, without a request and without a panic.
        assert!(matches!(
            get(&client, &l, &url).await,
            Err(ApiError::RateLimited { .. })
        ));
        server.join().unwrap();
    }

    #[test]
    fn retry_after_headers() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert("ratelimit", "\"500-in-10min\"; r=0; t=598".parse().unwrap());
        assert_eq!(retry_after(&h), Some(598));
        h.insert("retry-after", "12".parse().unwrap());
        assert_eq!(retry_after(&h), Some(12));
    }

    #[test]
    fn lookup_names() {
        for ok in [
            "Violet_Runner",
            "ab",
            "x",
            "bbc886da1b024739b4b80f1542e9f61d",
        ] {
            assert!(is_lookup_name(ok), "{ok}");
        }
        assert!(is_lookup_name("bbc886da-1b02-4739-b4b8-0f1542e9f61d"));
        for bad in [
            "",
            "no spaces",
            "../users",
            "a?b=c",
            "seventeen_chars_x",
            "ä",
        ] {
            assert!(!is_lookup_name(bad), "{bad}");
        }
    }
}
