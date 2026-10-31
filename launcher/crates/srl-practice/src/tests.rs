//! The install, reset, update and uninstall flows against a temporary data folder. Map zips are
//! built here and put into the store (or served by a tiny local HTTP server), so no test touches
//! the network.
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha512};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;
use crate::{archive::tests::make_zip, level::tests::level_dat};

struct Env {
    root: PathBuf,
    paths: Paths,
}

impl Env {
    fn new(tag: &str) -> Self {
        let root = crate::archive::tests::temp_dir(tag);
        Self {
            paths: Paths::new(&root),
            root,
        }
    }
    fn saves(&self) -> PathBuf {
        self.paths.practice_instance().join("saves")
    }
    fn practice(&self, maps: Vec<MapEntry>) -> Practice {
        let cat = Catalogue {
            schema: 1,
            checked: "test".into(),
            maps,
        };
        Practice::with_catalogue(self.paths.clone(), Downloader::new().unwrap(), cat)
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A map zip with one world folder and a loadout file, as Mescht's maps ship them.
fn map_zip(dir: &Path, folder: &str, level_name: &str, loadout: &[u8]) -> PathBuf {
    let path = dir.join(format!("{folder}.zip"));
    let level = level_dat(2567, level_name, 0);
    let entries: Vec<(String, Vec<u8>)> = vec![
        (format!("{folder}/level.dat"), level),
        (format!("{folder}/region/r.0.0.mca"), b"region".to_vec()),
        (format!("{folder}/data/loadouts.dat"), loadout.to_vec()),
        (format!("{folder}/session.lock"), b"author's lock".to_vec()),
        ("README.txt".into(), b"read me".to_vec()),
    ];
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    make_zip(&path, &refs);
    path
}

fn sha512(bytes: &[u8]) -> String {
    hex::encode(Sha512::digest(bytes))
}

/// A catalogue entry for `zip`, with the zip already in the store (so no download happens).
fn entry_for(env: &Env, id: &str, version: &str, zip: &Path, url: &str) -> MapEntry {
    let bytes = std::fs::read(zip).unwrap();
    let hash = sha512(&bytes);
    let store = env.paths.store_sha512(&hash);
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    std::fs::write(&store, &bytes).unwrap();
    let mut m = Catalogue::builtin()
        .unwrap()
        .get("zero-practice-mescht")
        .unwrap()
        .clone();
    m.id = id.into();
    m.name = "Test Map".into();
    m.carry_over = vec![CarryOver {
        from: "data/loadouts.dat".into(),
        to: "data/loadouts.dat".into(),
        what: "loadouts".into(),
        then_run: Some("/trigger import".into()),
        same_version_only: false,
    }];
    let d = m.download.as_mut().unwrap();
    d.version = version.into();
    d.url = url.into();
    d.sha512 = hash;
    d.size = bytes.len() as u64;
    m
}

/// Opening a world in the game rewrites level.dat; this does the same.
fn play(world: &Path) {
    std::fs::write(world.join("level.dat"), level_dat(2567, "played", 12345)).unwrap();
    std::fs::write(world.join("region/r.0.0.mca"), b"changed by playing").unwrap();
    std::fs::write(world.join("data/loadouts.dat"), b"my loadouts").unwrap();
}

fn installed(s: MapStatus) -> InstalledMap {
    match s {
        MapStatus::Installed(i) => i,
        other => panic!("not installed: {other:?}"),
    }
}

#[tokio::test]
async fn install_reset_uninstall_never_touch_other_saves() {
    let env = Env::new("flow");
    let zip = map_zip(&env.root, "Test Map v1", "Test §7Map v1", b"default");
    let p = env.practice(vec![entry_for(
        &env,
        "test-map",
        "1",
        &zip,
        "http://127.0.0.1:9/x",
    )]);

    // The player's own worlds: one unrelated, one with the map's folder name.
    let saves = env.saves();
    std::fs::create_dir_all(saves.join("My World")).unwrap();
    std::fs::write(saves.join("My World/level.dat"), b"mine").unwrap();
    std::fs::create_dir_all(saves.join("Test Map v1")).unwrap();
    std::fs::write(saves.join("Test Map v1/level.dat"), b"also mine").unwrap();

    assert_eq!(p.status("test-map").await.unwrap(), MapStatus::NotInstalled);
    let out = p.install("test-map", None).await.unwrap();
    assert_eq!(out.worlds.len(), 1);
    let w = &out.worlds[0];
    assert_eq!(
        w.folder, "Test Map v1 (2)",
        "never lands on an existing folder"
    );
    assert_eq!(
        w.name, "Test Map v1",
        "the Singleplayer name, without colour codes"
    );
    assert_eq!(w.state, WorldState::Fresh);
    assert_eq!(w.data_version, Some(2567));
    assert!(matches!(&out.warnings[..], [Warning::Renamed { .. }]));
    let world = saves.join(&w.folder);
    assert!(world.join("region/r.0.0.mca").is_file());
    assert!(!world.join("session.lock").exists());
    assert!(!saves.join("README.txt").exists());
    assert!(matches!(
        p.install("test-map", None).await,
        Err(PracticeError::AlreadyInstalled(_))
    ));

    // Played, then reset with the loadouts kept.
    play(&world);
    std::fs::write(world.join("new-file"), b"x").unwrap();
    let st = installed(p.status("test-map").await.unwrap());
    assert!(st.any_played() && st.stored && st.update.is_none());
    let plan = p.plan_reset("test-map").await.unwrap();
    assert!(!plan.needs_download);
    assert_eq!(plan.worlds[0].state, WorldState::Played);
    let out = p.reset("test-map", true, None).await.unwrap();
    assert_eq!(out.worlds[0].folder, "Test Map v1 (2)");
    assert_eq!(
        std::fs::read(world.join("region/r.0.0.mca")).unwrap(),
        b"region"
    );
    assert!(!world.join("new-file").exists());
    assert_eq!(
        std::fs::read(world.join("data/loadouts.dat")).unwrap(),
        b"my loadouts"
    );
    assert_eq!(out.carried.len(), 1);
    // Carrying data over changes a file but not level.dat: still fresh.
    assert_eq!(out.worlds[0].state, WorldState::Fresh);

    // Reset without keeping data.
    play(&world);
    p.reset("test-map", false, None).await.unwrap();
    assert_eq!(
        std::fs::read(world.join("data/loadouts.dat")).unwrap(),
        b"default"
    );

    // The player deletes the world: Install puts it back.
    std::fs::remove_dir_all(&world).unwrap();
    assert!(installed(p.status("test-map").await.unwrap()).all_missing());
    p.install("test-map", None).await.unwrap();
    assert!(world.join("level.dat").is_file());

    let hash = p.store_files_in_use().await.unwrap();
    assert_eq!(hash.len(), 1);
    let gone = p.uninstall("test-map").await.unwrap();
    assert_eq!(gone.removed, vec!["Test Map v1 (2)".to_string()]);
    assert!(!world.exists());
    assert!(!env.paths.store_sha512(&hash[0]).exists());
    assert_eq!(p.status("test-map").await.unwrap(), MapStatus::NotInstalled);
    // The player's worlds are exactly as they were.
    assert_eq!(
        std::fs::read(saves.join("My World/level.dat")).unwrap(),
        b"mine"
    );
    assert_eq!(
        std::fs::read(saves.join("Test Map v1/level.dat")).unwrap(),
        b"also mine"
    );
    assert!(!env.paths.practice_instance().join(".srl-staging").exists());
}

#[tokio::test]
async fn reset_downloads_a_missing_store_copy_again() {
    let env = Env::new("redownload");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let (url, server) = serve(std::fs::read(&zip).unwrap(), 200).await;
    let p = env.practice(vec![entry_for(&env, "m", "1", &zip, &url)]);
    p.install("m", None).await.unwrap();
    let hash = p.store_files_in_use().await.unwrap().remove(0);
    std::fs::remove_file(env.paths.store_sha512(&hash)).unwrap();
    assert!(p.plan_reset("m").await.unwrap().needs_download);
    let steps = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s2 = steps.clone();
    let progress: ProgressFn = Arc::new(move |p| s2.lock().unwrap().push(p.step));
    p.reset("m", false, Some(progress)).await.unwrap();
    assert!(env.paths.store_sha512(&hash).is_file());
    let steps = steps.lock().unwrap();
    assert!(steps.contains(&Step::Downloading) && steps.contains(&Step::Extracting));
    server.abort();
}

fn v2(env: &Env) -> MapEntry {
    let zip2 = map_zip(&env.root, "Test Map v2", "Test Map v2", b"v2 default");
    entry_for(env, "test-map", "2", &zip2, "http://127.0.0.1:9/x")
}

#[tokio::test]
async fn update_replaces_unplayed_worlds_without_asking() {
    let env = Env::new("update-fresh");
    let zip = map_zip(&env.root, "Test Map v1", "Test Map v1", b"default");
    let v1 = entry_for(&env, "test-map", "1", &zip, "http://127.0.0.1:9/x");
    env.practice(vec![v1])
        .install("test-map", None)
        .await
        .unwrap();
    let p = env.practice(vec![v2(&env)]);
    assert_eq!(
        installed(p.status("test-map").await.unwrap())
            .update
            .as_deref(),
        Some("2")
    );
    assert_eq!(
        p.plan_update("test-map").await.unwrap(),
        UpdatePlan::Replace {
            from: "1".into(),
            to: "2".into()
        }
    );
    let out = p.update("test-map", None, None).await.unwrap();
    assert_eq!(out.worlds[0].folder, "Test Map v2");
    assert!(!env.saves().join("Test Map v1").exists());
    assert_eq!(
        p.plan_update("test-map").await.unwrap(),
        UpdatePlan::UpToDate
    );
    // Only the new pristine copy is left in the store.
    assert_eq!(p.store_files_in_use().await.unwrap().len(), 1);
}

#[tokio::test]
async fn update_of_a_played_world_asks_and_honours_each_answer() {
    for choice in [
        UpdateChoice::ReplaceWorlds,
        UpdateChoice::KeepBoth,
        UpdateChoice::StoreOnly,
    ] {
        let env = Env::new(&format!("update-{choice:?}"));
        let zip = map_zip(&env.root, "Test Map v1", "Test Map v1", b"default");
        let v1 = entry_for(&env, "test-map", "1", &zip, "http://127.0.0.1:9/x");
        env.practice(vec![v1])
            .install("test-map", None)
            .await
            .unwrap();
        let old = env.saves().join("Test Map v1");
        play(&old);
        let p = env.practice(vec![v2(&env)]);

        let plan = p.plan_update("test-map").await.unwrap();
        let UpdatePlan::Ask {
            played, carry_over, ..
        } = &plan
        else {
            panic!("{plan:?}");
        };
        assert_eq!(played[0].folder, "Test Map v1");
        assert_eq!(carry_over.len(), 1);
        // Without an answer nothing happens.
        match p.update("test-map", None, None).await {
            Err(PracticeError::NeedsDecision(p)) => assert_eq!(*p, plan),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            std::fs::read(old.join("region/r.0.0.mca")).unwrap(),
            b"changed by playing"
        );

        let out = p.update("test-map", Some(choice), None).await.unwrap();
        let new = env.saves().join("Test Map v2");
        let st = installed(p.status("test-map").await.unwrap());
        match choice {
            UpdateChoice::ReplaceWorlds => {
                assert!(!old.exists());
                assert_eq!(
                    std::fs::read(new.join("data/loadouts.dat")).unwrap(),
                    b"my loadouts"
                );
                assert_eq!(out.carried[0].then_run.as_deref(), Some("/trigger import"));
                assert_eq!(st.version, "2");
            }
            UpdateChoice::KeepBoth => {
                assert!(old.join("level.dat").is_file(), "the played world stays");
                assert!(new.join("level.dat").is_file());
                assert_eq!(
                    std::fs::read(new.join("data/loadouts.dat")).unwrap(),
                    b"my loadouts"
                );
                assert_eq!(out.handed_over, vec!["Test Map v1".to_string()]);
                // Uninstall leaves the handed-over world alone.
                let gone = p.uninstall("test-map").await.unwrap();
                assert_eq!(gone.kept, vec!["Test Map v1".to_string()]);
                assert!(old.join("level.dat").is_file());
                assert!(!new.exists());
            }
            UpdateChoice::StoreOnly => {
                assert!(out.worlds.is_empty());
                assert!(!new.exists());
                assert_eq!(
                    std::fs::read(old.join("region/r.0.0.mca")).unwrap(),
                    b"changed by playing"
                );
                assert_eq!(
                    (st.version.as_str(), st.stored_version.as_str()),
                    ("1", "2")
                );
                assert_eq!(st.update, None);
                // Reset later brings the new version, keeping loadouts.
                let out = p.reset("test-map", true, None).await.unwrap();
                assert_eq!(out.version, "2");
                assert!(!old.exists());
                assert_eq!(
                    std::fs::read(new.join("data/loadouts.dat")).unwrap(),
                    b"my loadouts"
                );
            }
        }
    }
}

#[test]
fn update_decisions() {
    let dl = |sha: &str| MapDownload {
        version: "2".into(),
        tag: "v2".into(),
        released: "x".into(),
        repo: "a/b".into(),
        asset_pattern: ".*".into(),
        url: "u".into(),
        sha512: sha.into(),
        size: 1,
        worlds: vec!["W".into()],
        data_version: Some(2567),
    };
    let rec = InstallRecord {
        id: "m".into(),
        pristine: Pristine {
            version: "1".into(),
            sha512: "aa".into(),
            size: 1,
            url: "u".into(),
        },
        worlds_version: "1".into(),
        worlds: Vec::new(),
        handed_over: Vec::new(),
        installed_at: 0,
    };
    let view = |state| WorldView {
        folder: "W".into(),
        name: "W".into(),
        state,
        data_version: None,
    };
    assert_eq!(
        decide_update(&rec, &dl("aa"), vec![view(WorldState::Played)], &[]),
        UpdatePlan::UpToDate
    );
    for states in [
        vec![],
        vec![view(WorldState::Fresh)],
        vec![view(WorldState::Missing), view(WorldState::Fresh)],
    ] {
        assert!(matches!(
            decide_update(&rec, &dl("bb"), states, &[]),
            UpdatePlan::Replace { .. }
        ));
    }
    assert!(matches!(
        decide_update(
            &rec,
            &dl("bb"),
            vec![view(WorldState::Fresh), view(WorldState::Played)],
            &[]
        ),
        UpdatePlan::Ask { played, .. } if played.len() == 1
    ));
}

#[tokio::test]
async fn bad_downloads_install_nothing() {
    let env = Env::new("bad-download");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let good = std::fs::read(&zip).unwrap();

    // The server sends other bytes than the pinned ones.
    let (url, server) = serve(b"not the map".to_vec(), 200).await;
    let mut m = entry_for(&env, "m", "1", &zip, &url);
    std::fs::remove_file(env.paths.store_sha512(&sha512(&good))).unwrap();
    m.download.as_mut().unwrap().size = 11;
    let p = env.practice(vec![m.clone()]);
    assert!(matches!(
        p.install("m", None).await,
        Err(PracticeError::HashMismatch { .. })
    ));
    server.abort();

    // The author removed the file.
    let (url, server) = serve(Vec::new(), 404).await;
    m.download.as_mut().unwrap().url = url;
    let p = env.practice(vec![m]);
    assert!(matches!(
        p.install("m", None).await,
        Err(PracticeError::SourceGone { .. })
    ));
    server.abort();

    assert!(!env.saves().exists() || std::fs::read_dir(env.saves()).unwrap().next().is_none());
    assert_eq!(p.status("m").await.unwrap(), MapStatus::NotInstalled);
    let store = env.paths.store_sha512(&sha512(&good));
    assert!(!store.exists());
    let mut part = store.into_os_string();
    part.push(".part");
    assert!(!PathBuf::from(part).exists(), "no partial file left");
}

/// A world the game has open is never changed. Another process holds `session.lock` the way
/// Java does (an fcntl lock; a lock from this process wouldn't count).
#[cfg(unix)]
#[tokio::test]
async fn an_open_world_is_left_alone() {
    use std::io::BufRead;
    let env = Env::new("in-use");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let p = env.practice(vec![entry_for(
        &env,
        "m",
        "1",
        &zip,
        "http://127.0.0.1:9/x",
    )]);
    p.install("m", None).await.unwrap();
    let world = env.saves().join("M");
    let lock = world.join("session.lock");
    std::fs::write(&lock, "\u{2603}").unwrap();
    assert!(!worlds::world_in_use(&world));
    let Ok(mut holder) = std::process::Command::new("python3")
        .args([
            "-I",
            "-c",
            "import fcntl,sys,time; f=open(sys.argv[1],'r+'); fcntl.lockf(f, fcntl.LOCK_EX); \
             print('locked', flush=True); time.sleep(60)",
        ])
        .arg(&lock)
        .stdout(std::process::Stdio::piped())
        .spawn()
    else {
        eprintln!("skipped: no python3 to hold the lock");
        return;
    };
    let mut line = String::new();
    std::io::BufReader::new(holder.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "locked");
    assert!(worlds::world_in_use(&world));
    play(&world);
    for result in [
        p.reset("m", false, None).await.map(|_| ()),
        p.uninstall("m").await.map(|_| ()),
    ] {
        assert!(
            matches!(&result, Err(PracticeError::WorldInUse { folder }) if folder == "M"),
            "{result:?}"
        );
    }
    assert_eq!(
        std::fs::read(world.join("region/r.0.0.mca")).unwrap(),
        b"changed by playing"
    );
    holder.kill().unwrap();
    holder.wait().unwrap();
    assert!(!worlds::world_in_use(&world));
    p.reset("m", false, None).await.unwrap();
    assert_eq!(
        std::fs::read(world.join("region/r.0.0.mca")).unwrap(),
        b"region"
    );
}

#[tokio::test]
async fn link_only_and_unknown_maps_are_refused() {
    let env = Env::new("refused");
    let p = Practice::new(env.paths.clone(), Downloader::new().unwrap()).unwrap();
    assert!(matches!(
        p.install("vine-ssg-practice-bdamja", None).await,
        Err(PracticeError::LinkOnly(_))
    ));
    assert!(matches!(
        p.install("nope", None).await,
        Err(PracticeError::UnknownMap(_))
    ));
    let all = p.statuses().await.unwrap();
    assert_eq!(all.len(), p.catalogue().maps.len());
    assert!(all.iter().any(|(_, s)| *s == MapStatus::LinkOnly));
    assert!(matches!(
        p.reset("zero-practice-mescht", true, None).await,
        Err(PracticeError::NotInstalled(_))
    ));
}

/// Serves `body` with `status` to every request on a local port, until aborted.
async fn serve(body: Vec<u8>, status: u16) -> (String, tokio::task::JoinHandle<()>) {
    serve_with(body, status, Served::default()).await
}

/// Knobs for [`serve_with`].
#[derive(Clone, Default)]
struct Served {
    /// Counts requests.
    hits: Arc<std::sync::atomic::AtomicUsize>,
    /// When set, each request waits for a permit before the body is sent.
    gate: Option<Arc<tokio::sync::Semaphore>>,
    /// Send only this many body bytes, then hang (a stalled download).
    hang_after: Option<usize>,
}

async fn serve_with(
    body: Vec<u8>,
    status: u16,
    opts: Served,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/map.zip", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let body = body.clone();
            let opts = opts.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                opts.hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if let Some(gate) = &opts.gate {
                    gate.acquire().await.unwrap().forget();
                }
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                match opts.hang_after {
                    Some(n) => {
                        let _ = sock.write_all(&body[..n.min(body.len())]).await;
                        let _ = sock.flush().await;
                        std::future::pending::<()>().await;
                    }
                    None => {
                        let _ = sock.write_all(&body).await;
                        let _ = sock.shutdown().await;
                    }
                }
            });
        }
    });
    (url, task)
}

// Regression tests for the review of 2026-10-26 (NOTES.md, "Review fixes").

/// A v2 zip the launcher refuses (a Windows device name inside the world).
fn bad_v2(env: &Env) -> MapEntry {
    let path = env.root.join("bad-v2.zip");
    let level = level_dat(2567, "Test Map v2", 0);
    make_zip(
        &path,
        &[
            ("Test Map v2/level.dat", level.as_slice()),
            ("Test Map v2/data/CON", b"x".as_slice()),
        ],
    );
    entry_for(env, "test-map", "2", &path, "http://127.0.0.1:9/x")
}

/// An update whose zip fails to extract must leave the played world, its loadouts and the
/// record exactly as they were.
#[tokio::test]
async fn a_failed_replace_keeps_the_played_world() {
    let env = Env::new("failed-replace");
    let zip = map_zip(&env.root, "Test Map v1", "Test Map v1", b"default");
    let v1 = entry_for(&env, "test-map", "1", &zip, "http://127.0.0.1:9/x");
    env.practice(vec![v1])
        .install("test-map", None)
        .await
        .unwrap();
    let old = env.saves().join("Test Map v1");
    play(&old);
    let p = env.practice(vec![bad_v2(&env)]);
    let r = p
        .update("test-map", Some(UpdateChoice::ReplaceWorlds), None)
        .await;
    assert!(matches!(r, Err(PracticeError::UnsafeArchive(_))), "{r:?}");
    assert_eq!(
        std::fs::read(old.join("data/loadouts.dat")).unwrap(),
        b"my loadouts"
    );
    let st = installed(p.status("test-map").await.unwrap());
    assert_eq!(st.version, "1");
    assert_eq!(st.worlds.len(), 1);
    assert_eq!(st.worlds[0].state, WorldState::Played);
    assert!(!env.saves().join("Test Map v2").exists());
    assert!(!env.paths.practice_instance().join(".srl-staging").exists());
}

/// A swap interrupted by a crash (the old world moved out, nothing committed) is undone by the
/// next call, so the played world comes back instead of being deleted with the staging folder.
#[tokio::test]
async fn an_interrupted_swap_is_rolled_back() {
    let env = Env::new("interrupted-swap");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let p = env.practice(vec![entry_for(
        &env,
        "m",
        "1",
        &zip,
        "http://127.0.0.1:9/x",
    )]);
    p.install("m", None).await.unwrap();
    let world = env.saves().join("M");
    play(&world);
    // What a crash in the middle of the swap leaves behind.
    let swap = env.paths.practice_instance().join(".srl-staging/swap-1-m");
    std::fs::create_dir_all(&swap).unwrap();
    std::fs::rename(&world, swap.join("M")).unwrap();
    // Any mutating call recovers first; this one then fails because the map is installed.
    assert!(matches!(
        p.install("m", None).await,
        Err(PracticeError::AlreadyInstalled(_))
    ));
    assert_eq!(
        std::fs::read(world.join("data/loadouts.dat")).unwrap(),
        b"my loadouts"
    );
    assert!(!env.paths.practice_instance().join(".srl-staging").exists());
}

/// Dropping a reset future (the UI's Cancel) must not let another call run while the reset's
/// file work is still going on.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_call_keeps_the_lock_until_its_file_work_ends() {
    let env = Env::new("cancel-lock");
    let path = env.root.join("big.zip");
    let level = level_dat(2567, "Big", 0);
    let mut entries: Vec<(String, Vec<u8>)> = vec![("Big/level.dat".into(), level)];
    for i in 0..300 {
        entries.push((format!("Big/data/f{i}"), vec![b'x'; 16]));
    }
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    make_zip(&path, &refs);
    let p = env.practice(vec![entry_for(
        &env,
        "m",
        "1",
        &path,
        "http://127.0.0.1:9/x",
    )]);
    p.install("m", None).await.unwrap();

    let extracting = Arc::new(tokio::sync::Notify::new());
    let e2 = extracting.clone();
    let slow: ProgressFn = Arc::new(move |pr| {
        if pr.step == Step::Extracting {
            e2.notify_one();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
    });
    let p2 = p.clone();
    let task = tokio::spawn(async move { p2.reset("m", false, Some(slow)).await });
    extracting.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());

    let gone = p.uninstall("m").await.unwrap();
    assert_eq!(gone.removed, vec!["Big".to_string()]);
    // Long enough for an orphaned reset (300 files x 3 ms) to have finished.
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert_eq!(p.status("m").await.unwrap(), MapStatus::NotInstalled);
    assert!(!env.saves().join("Big").exists());
}

/// `update(id, None)` decided "nothing played" before the download; if the player opened the
/// world meanwhile, it must ask instead of deleting it.
#[tokio::test]
async fn update_without_a_choice_asks_if_the_world_was_played_during_the_download() {
    let env = Env::new("update-race");
    let zip = map_zip(&env.root, "Test Map v1", "Test Map v1", b"default");
    let v1 = entry_for(&env, "test-map", "1", &zip, "http://127.0.0.1:9/x");
    env.practice(vec![v1])
        .install("test-map", None)
        .await
        .unwrap();
    let mut m2 = v2(&env);
    let new_zip = env
        .paths
        .store_sha512(&m2.download.as_ref().unwrap().sha512);
    let body = std::fs::read(&new_zip).unwrap();
    std::fs::remove_file(&new_zip).unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let opts = Served {
        gate: Some(gate.clone()),
        ..Served::default()
    };
    let (url, server) = serve_with(body, 200, opts.clone()).await;
    m2.download.as_mut().unwrap().url = url;
    let p = env.practice(vec![m2]);
    assert!(matches!(
        p.plan_update("test-map").await.unwrap(),
        UpdatePlan::Replace { .. }
    ));
    let p2 = p.clone();
    let task = tokio::spawn(async move { p2.update("test-map", None, None).await });
    while opts.hits.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let old = env.saves().join("Test Map v1");
    play(&old);
    gate.add_permits(10);
    let r = task.await.unwrap();
    assert!(matches!(r, Err(PracticeError::NeedsDecision(_))), "{r:?}");
    assert_eq!(
        std::fs::read(old.join("region/r.0.0.mca")).unwrap(),
        b"changed by playing"
    );
    server.abort();
}

/// A world folder whose level.dat is gone (the game was killed between the two renames of its
/// save; it then loads level.dat_old) is not "missing": it is the player's progress.
#[tokio::test]
async fn a_world_without_level_dat_counts_as_played() {
    let env = Env::new("no-level-dat");
    let zip = map_zip(&env.root, "Test Map v1", "Test Map v1", b"default");
    let v1 = entry_for(&env, "test-map", "1", &zip, "http://127.0.0.1:9/x");
    env.practice(vec![v1])
        .install("test-map", None)
        .await
        .unwrap();
    let old = env.saves().join("Test Map v1");
    play(&old);
    std::fs::rename(old.join("level.dat"), old.join("level.dat_old")).unwrap();
    let p = env.practice(vec![v2(&env)]);
    let st = installed(p.status("test-map").await.unwrap());
    assert_eq!(st.worlds[0].state, WorldState::Played);
    assert!(matches!(
        p.plan_update("test-map").await.unwrap(),
        UpdatePlan::Ask { .. }
    ));
    assert!(matches!(
        p.update("test-map", None, None).await,
        Err(PracticeError::NeedsDecision(_))
    ));
    assert!(old.join("level.dat_old").is_file());
    // A folder the player deleted is still Missing.
    std::fs::remove_dir_all(&old).unwrap();
    let st = installed(p.status("test-map").await.unwrap());
    assert_eq!(st.worlds[0].state, WorldState::Missing);
}

/// The right size with the wrong bytes (a moved tag) is not retried: three full downloads would
/// only waste the player's data.
#[tokio::test]
async fn a_hash_mismatch_is_downloaded_once() {
    let env = Env::new("mismatch-once");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let good = std::fs::read(&zip).unwrap();
    let mut m = entry_for(&env, "m", "1", &zip, "http://127.0.0.1:9/x");
    std::fs::remove_file(env.paths.store_sha512(&sha512(&good))).unwrap();
    let opts = Served::default();
    let (url, server) = serve_with(vec![b'z'; good.len()], 200, opts.clone()).await;
    m.download.as_mut().unwrap().url = url;
    let p = env.practice(vec![m]);
    assert!(matches!(
        p.install("m", None).await,
        Err(PracticeError::HashMismatch { .. })
    ));
    assert_eq!(opts.hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    server.abort();
}

fn part_of(env: &Env, sha: &str) -> PathBuf {
    let mut part = env.paths.store_sha512(sha).into_os_string();
    part.push(".part");
    PathBuf::from(part)
}

/// A cancelled download leaves no `.part` behind, and a stale one (the app was killed) is removed
/// by the next mutating call.
#[tokio::test]
async fn a_cancelled_download_leaves_no_part_file() {
    let env = Env::new("cancel-part");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let good = std::fs::read(&zip).unwrap();
    let sha = sha512(&good);
    let mut m = entry_for(&env, "m", "1", &zip, "http://127.0.0.1:9/x");
    std::fs::remove_file(env.paths.store_sha512(&sha)).unwrap();
    let opts = Served {
        hang_after: Some(good.len() / 2),
        ..Served::default()
    };
    let (url, server) = serve_with(good.clone(), 200, opts).await;
    m.download.as_mut().unwrap().url = url;
    let p = env.practice(vec![m]);
    let downloading = Arc::new(tokio::sync::Notify::new());
    let d2 = downloading.clone();
    let progress: ProgressFn = Arc::new(move |pr| {
        if pr.step == Step::Downloading && pr.done > 0 {
            d2.notify_one();
        }
    });
    let p2 = p.clone();
    let task = tokio::spawn(async move { p2.install("m", Some(progress)).await });
    downloading.notified().await;
    assert!(part_of(&env, &sha).exists());
    task.abort();
    let _ = task.await;
    assert!(!part_of(&env, &sha).exists(), "the .part file is removed");
    server.abort();

    std::fs::write(part_of(&env, &sha), b"stale").unwrap();
    assert!(matches!(
        p.reset("m", false, None).await,
        Err(PracticeError::NotInstalled(_))
    ));
    assert!(!part_of(&env, &sha).exists(), "a stale .part is cleaned up");
}

/// A hand-edited record can't turn its pristine hash into a path outside the store.
#[tokio::test]
async fn a_record_with_a_bad_hash_is_refused() {
    let env = Env::new("bad-record");
    let zip = map_zip(&env.root, "M", "M", b"d");
    let p = env.practice(vec![entry_for(
        &env,
        "m",
        "1",
        &zip,
        "http://127.0.0.1:9/x",
    )]);
    p.install("m", None).await.unwrap();
    let victim = env.root.join("victim");
    std::fs::write(&victim, b"keep me").unwrap();
    let rec_path = record::path(&env.paths, "m");
    let mut rec: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&rec_path).unwrap()).unwrap();
    rec["pristine"]["sha512"] = "../../../victim".into();
    std::fs::write(&rec_path, serde_json::to_vec(&rec).unwrap()).unwrap();
    assert!(p.uninstall("m").await.is_err());
    assert_eq!(std::fs::read(&victim).unwrap(), b"keep me");
}

/// A carry-over rule marked `same_version_only` (a file only the same version of the map can
/// read) is used by Reset but not by an update.
#[tokio::test]
async fn same_version_carry_over_is_not_used_across_versions() {
    let env = Env::new("same-version-carry");
    let zip = map_zip(&env.root, "Test Map v1", "Test Map v1", b"default");
    let mut v1 = entry_for(&env, "test-map", "1", &zip, "http://127.0.0.1:9/x");
    v1.carry_over[0].same_version_only = true;
    v1.carry_over[0].then_run = None;
    let p1 = env.practice(vec![v1]);
    p1.install("test-map", None).await.unwrap();
    let old = env.saves().join("Test Map v1");
    play(&old);
    assert_eq!(p1.plan_reset("test-map").await.unwrap().carry_over.len(), 1);
    let out = p1.reset("test-map", true, None).await.unwrap();
    assert_eq!(out.carried.len(), 1);
    assert_eq!(
        std::fs::read(old.join("data/loadouts.dat")).unwrap(),
        b"my loadouts"
    );

    play(&old);
    let mut m2 = v2(&env);
    m2.carry_over[0].same_version_only = true;
    let p = env.practice(vec![m2]);
    let UpdatePlan::Ask { carry_over, .. } = p.plan_update("test-map").await.unwrap() else {
        panic!("played")
    };
    assert!(carry_over.is_empty());
    let out = p
        .update("test-map", Some(UpdateChoice::ReplaceWorlds), None)
        .await
        .unwrap();
    assert!(out.carried.is_empty());
    assert_eq!(
        std::fs::read(env.saves().join("Test Map v2/data/loadouts.dat")).unwrap(),
        b"v2 default"
    );
}
