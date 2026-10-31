//! `ModMeta::load` on a network that never answers (a dead VPN, Wi-Fi without upstream): with a
//! cached copy, each list gets one short try and the cache is used within seconds. Its own test
//! binary, because it points the process's proxy at a local socket that accepts and stays silent.
use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use srl_core::{Downloader, Paths, download::REFRESH_DEADLINE};
use srl_mods::{ModMeta, meta::cache_dir};

#[tokio::test]
async fn a_silent_network_falls_back_to_the_cache_in_seconds() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    // Accept every connection and never write a byte, like a proxy whose upstream is gone.
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for s in listener.incoming() {
            held.push(s);
        }
    });
    // SAFETY: the only test in this binary, set before any thread reads the environment.
    unsafe {
        std::env::set_var("HTTPS_PROXY", format!("http://{addr}"));
        std::env::set_var("HTTP_PROXY", format!("http://{addr}"));
        std::env::remove_var("NO_PROXY");
    }
    let dl = Downloader::new().unwrap();

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("srl-mods-silent");
    let _ = std::fs::remove_dir_all(&dir);
    let paths = Paths::new(&dir);
    let cache = cache_dir(&paths);
    std::fs::create_dir_all(&cache).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meta");
    for f in std::fs::read_dir(&fixtures).unwrap() {
        let f = f.unwrap();
        std::fs::copy(f.path(), cache.join(f.file_name())).unwrap();
    }

    let started = Instant::now();
    let meta = ModMeta::load(&paths, &dl, true).await.unwrap();
    let took = started.elapsed();
    assert!(
        took < REFRESH_DEADLINE + Duration::from_secs(2),
        "took {took:?}"
    );
    assert_eq!(meta.notes.len(), 7, "{:?}", meta.notes);
    assert!(meta.notes.iter().all(|n| n.contains("offline")));
    std::fs::remove_dir_all(&dir).unwrap();
}
