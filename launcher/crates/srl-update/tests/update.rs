//! The update against a local stand-in for GitHub: the release JSON, the checksum asset and the
//! zip, each served from 127.0.0.1 the way GitHub's API and download host answer.
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use sha2::{Digest, Sha256};
use srl_update::{CHECKSUM_NAME, Check, EXE_NAME, ErrorKind, Updater, ZIP_NAME, swap};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// What the stand-in serves, by path, and the paths it was asked for.
#[derive(Default)]
struct Server {
    files: HashMap<String, (u16, Vec<u8>)>,
    asked: Vec<String>,
}

async fn serve(server: Arc<Mutex<Server>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let server = server.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let path = loop {
                    let Ok(n) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let mut headers = [httparse::EMPTY_HEADER; 32];
                    let mut req = httparse::Request::new(&mut headers);
                    if let Ok(httparse::Status::Complete(_)) = req.parse(&buf) {
                        break req.path.unwrap_or_default().to_string();
                    }
                };
                let (status, body) = {
                    let mut s = server.lock().unwrap();
                    s.asked.push(path.clone());
                    s.files
                        .get(&path)
                        .cloned()
                        .unwrap_or((404, b"{\"message\":\"Not Found\"}".to_vec()))
                };
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    base
}

fn release_zip() -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for (name, data) in [
        (EXE_NAME, b"MZ version 0.2.0".as_slice()),
        ("README.txt", b"Speedrunning Launcher 0.2.0"),
        ("THIRD_PARTY_NOTICES.md", b"# notices"),
        ("VERSION.txt", b"version 0.2.0"),
        ("licenses/LICENSE-lucide.txt", b"ISC"),
    ] {
        w.start_file(name, opts).unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn release_json(base: &str, tag: &str, zip_size: usize) -> Vec<u8> {
    serde_json::json!({
        "tag_name": tag,
        "html_url": format!("{base}/releases/tag/{tag}"),
        "draft": false,
        "prerelease": false,
        "assets": [
            {"name": CHECKSUM_NAME, "size": 100,
             "browser_download_url": format!("{base}/download/{CHECKSUM_NAME}")},
            {"name": ZIP_NAME, "size": zip_size,
             "browser_download_url": format!("{base}/download/{ZIP_NAME}")},
        ]
    })
    .to_string()
    .into_bytes()
}

/// A stand-in serving `tag` with the zip and a checksum: the right one, or `bad_sum`.
async fn github(tag: &str, bad_sum: bool) -> (String, Arc<Mutex<Server>>) {
    let server = Arc::new(Mutex::new(Server::default()));
    let base = serve(server.clone()).await;
    let zip = release_zip();
    let sum = if bad_sum {
        "0".repeat(64)
    } else {
        hex::encode(Sha256::digest(&zip))
    };
    {
        let mut s = server.lock().unwrap();
        s.files.insert(
            "/repos/x/releases/latest".into(),
            (200, release_json(&base, tag, zip.len())),
        );
        s.files.insert(
            format!("/download/{CHECKSUM_NAME}"),
            (200, format!("{sum}  {ZIP_NAME}\n").into_bytes()),
        );
        s.files.insert(format!("/download/{ZIP_NAME}"), (200, zip));
    }
    (base, server)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("srl-update-it-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn updater(base: &str, current: &str, dir: &Path) -> Updater {
    Updater::new(
        srl_core::Downloader::new().unwrap(),
        format!("{base}/repos/x/releases/latest"),
        current,
        dir.join("updates"),
    )
    .unwrap()
}

#[tokio::test]
async fn a_newer_release_is_downloaded_verified_and_staged() {
    let (base, server) = github("v0.2.0", false).await;
    let dir = temp_dir("newer");
    let up = updater(&base, "0.1.0", &dir);
    let Check::Newer(offer) = up.check().await.unwrap() else {
        panic!("no update offered")
    };
    assert_eq!(offer.version.to_string(), "0.2.0");
    assert_eq!(offer.page, format!("{base}/releases/tag/v0.2.0"));
    let staged = up.download(&offer).await.unwrap();
    assert_eq!(staged.dir, dir.join("updates/0.2.0/staged"));
    assert_eq!(
        std::fs::read(staged.exe()).unwrap(),
        b"MZ version 0.2.0".as_slice()
    );
    assert_eq!(staged.files.len(), 5);

    // And swapped in for an installed launcher.
    let install = dir.join("install");
    std::fs::create_dir_all(&install).unwrap();
    let exe = install.join(EXE_NAME);
    std::fs::write(&exe, b"MZ version 0.1.0").unwrap();
    let done = swap::install(&staged, &exe).unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), b"MZ version 0.2.0".as_slice());
    assert_eq!(
        std::fs::read(&done.old).unwrap(),
        b"MZ version 0.1.0".as_slice()
    );
    assert!(install.join("licenses/LICENSE-lucide.txt").is_file());
    up.forget(&staged.version);
    assert!(!dir.join("updates/0.2.0").exists());

    let asked = server.lock().unwrap().asked.clone();
    assert_eq!(
        asked,
        [
            "/repos/x/releases/latest".to_string(),
            format!("/download/{CHECKSUM_NAME}"),
            format!("/download/{ZIP_NAME}"),
        ]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn the_same_version_downloads_nothing() {
    let (base, server) = github("v0.2.0", false).await;
    let dir = temp_dir("same");
    for current in ["0.2.0", "0.3.0"] {
        let up = updater(&base, current, &dir);
        assert_eq!(
            up.check().await.unwrap(),
            Check::UpToDate {
                latest: Some("v0.2.0".into())
            }
        );
    }
    assert_eq!(server.lock().unwrap().asked.len(), 2);
    assert!(!dir.join("updates").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn a_bad_checksum_is_rejected() {
    let (base, _server) = github("v0.2.0", true).await;
    let dir = temp_dir("badsum");
    let up = updater(&base, "0.1.0", &dir);
    let Check::Newer(offer) = up.check().await.unwrap() else {
        panic!("no update offered")
    };
    let err = up.download(&offer).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Checksum, "{err}");
    // Neither the zip nor anything staged is left.
    let vdir = dir.join("updates/0.2.0");
    assert!(!vdir.join(ZIP_NAME).exists());
    assert!(!vdir.join("staged").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn no_release_rate_limits_and_bad_answers() {
    let server = Arc::new(Mutex::new(Server::default()));
    let base = serve(server.clone()).await;
    let dir = temp_dir("answers");
    let up = updater(&base, "0.1.0", &dir);
    // Nothing published yet: GitHub answers 404.
    assert_eq!(up.check().await.unwrap(), Check::UpToDate { latest: None });
    let latest = "/repos/x/releases/latest".to_string();
    server.lock().unwrap().files.insert(
        latest.clone(),
        (403, b"{\"message\":\"API rate limit exceeded\"}".to_vec()),
    );
    assert_eq!(up.check().await.unwrap_err().kind, ErrorKind::RateLimited);
    server.lock().unwrap().files.insert(
        latest.clone(),
        (200, b"<html>captive portal</html>".to_vec()),
    );
    assert_eq!(up.check().await.unwrap_err().kind, ErrorKind::Other);
    // A newer release whose assets are missing.
    server.lock().unwrap().files.insert(
        latest,
        (200, br#"{"tag_name":"v0.2.0","assets":[]}"#.to_vec()),
    );
    let err = up.check().await.unwrap_err();
    assert!(err.detail.contains(ZIP_NAME), "{err}");
    // Nobody listening: offline.
    let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gone = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let err = updater(&gone, "0.1.0", &dir).check().await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Offline, "{err}");
    std::fs::remove_dir_all(&dir).unwrap();
}
