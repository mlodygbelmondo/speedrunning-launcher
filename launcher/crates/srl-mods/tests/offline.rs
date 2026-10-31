//! `ModMeta::load` when the network is down: it falls back to the last good copies. Its own test
//! binary, because it points the process's proxy at a dead port to simulate being offline.
use std::path::{Path, PathBuf};

use srl_core::{Downloader, Mode, Paths};
use srl_mods::{ModMeta, Os, ResolveOptions, meta::cache_dir, resolve};

fn home(name: &str) -> Paths {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("srl-mods-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    Paths::new(dir)
}

#[tokio::test]
async fn offline_load_uses_the_cache_and_fails_without_one() {
    // SAFETY: the only test in this binary, set before any thread reads the environment.
    unsafe {
        std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
        std::env::remove_var("NO_PROXY");
    }
    let dl = Downloader::new().unwrap();

    let empty = home("offline-empty");
    let err = ModMeta::load(&empty, &dl, true).await.unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("mods.json: offline"), "{msg}");

    let cached = home("offline-cached");
    let dir = cache_dir(&cached);
    std::fs::create_dir_all(&dir).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/meta");
    for f in std::fs::read_dir(&fixtures).unwrap() {
        let f = f.unwrap();
        std::fs::copy(f.path(), dir.join(f.file_name())).unwrap();
    }
    let meta = ModMeta::load(&cached, &dl, true).await.unwrap();
    assert_eq!(meta.notes.len(), 7, "{:?}", meta.notes);
    assert!(meta.notes.iter().all(|n| n.contains("offline")));
    // The cache is untouched and still resolves.
    let set = resolve(&meta, Mode::Ranked, Os::Linux, &ResolveOptions::default()).unwrap();
    assert_eq!(set.get("mcsrranked").unwrap().version, "6.0.4");

    // Without refresh, a full cache needs no network at all: no notes.
    let meta = ModMeta::load(&cached, &dl, false).await.unwrap();
    assert!(meta.notes.is_empty(), "{:?}", meta.notes);
}
