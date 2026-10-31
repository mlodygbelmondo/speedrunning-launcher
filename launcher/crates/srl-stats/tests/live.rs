//! The real APIs. The live tests are `#[ignore]`d (network, someone else's servers); run them with
//! `cargo test -p srl-stats --test live -- --ignored --nocapture`. The offline test runs always.
use std::time::Duration;

use srl_core::{Downloader, Paths};
use srl_stats::{
    ApiError, DraftoutClient, Outcome, RankedClient, Refresh, Stats, Tier, cache::Cached,
};

fn scratch(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("srl-stats-live-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// Picks a player from the live leaderboard, then fetches their profile, matches, the weekly race
/// and the online count through the real client, then once more to check the cache answers.
#[tokio::test]
#[ignore = "network: api.mcsrranked.com"]
async fn ranked_live() {
    let dl = Downloader::new().unwrap();
    let board: serde_json::Value = dl
        .fetch_json("https://api.mcsrranked.com/leaderboard")
        .await
        .unwrap();
    let users = board["data"]["users"].as_array().unwrap();
    let pick = &users[users.len() / 2];
    let name = pick["nickname"].as_str().unwrap();
    let dir = scratch("ranked");
    let client = RankedClient::new(dl.client().clone(), dir.clone());
    let o = client.overview(name, Refresh::IfStale).await.unwrap();
    let p = &o.profile.value;
    println!(
        "{} elo {:?} rank {:?} tier {:?}; season {}W/{}L/{} played, best {:?} ms",
        p.nickname,
        p.elo,
        p.rank,
        p.tier,
        p.season.wins,
        p.season.losses,
        p.season.played,
        p.season.best_time_ms
    );
    assert!(p.nickname.eq_ignore_ascii_case(name));
    assert_eq!(p.elo, pick["eloRate"].as_i64().map(|e| e as i32));
    assert_eq!(p.tier, p.elo.map(Tier::from_elo));
    assert!(p.season.played > 0);
    let matches = o.matches.as_ref().expect("matches");
    assert!(!matches.value.is_empty());
    for m in matches.value.iter().take(5) {
        println!(
            "  #{} {:?} vs {:?}: {:?}{} {} ms, elo {:?} -> {:?}",
            m.id,
            m.kind,
            m.opponent.as_ref().map(|o| o.nickname.as_str()),
            m.outcome,
            if m.forfeited { " (forfeit)" } else { "" },
            m.time_ms,
            m.elo_change,
            m.elo_after
        );
    }
    assert!(
        matches
            .value
            .windows(2)
            .all(|w| w[0].date_ms >= w[1].date_ms)
    );
    assert!(matches.value.iter().any(|m| m.outcome != Outcome::Draw));
    let race = o.weekly_race.as_ref().expect("weekly race");
    println!(
        "  weekly race #{} ends {} ms, {} on the board, mine {:?}, last {:?}; online {:?}",
        race.value.id,
        race.value.ends_at_ms,
        race.value.leaderboard.len(),
        o.my_weekly_race,
        o.my_last_weekly_race,
        o.players_online.as_ref().map(|c| c.value)
    );
    assert!(o.players_online.is_some());

    // The race's #1 gets their place in the running race (the profile lists finished races only).
    let top = &race.value.leaderboard[0];
    let leader = client
        .overview(&top.player.nickname, Refresh::IfStale)
        .await
        .unwrap();
    println!(
        "  race leader {}: running {:?}, last {:?}",
        top.player.nickname, leader.my_weekly_race, leader.my_last_weekly_race
    );
    let mine = leader.my_weekly_race.expect("the leader's running race");
    assert_eq!(
        (mine.race_id, mine.rank, mine.time_ms),
        (race.value.id, top.rank, top.time_ms)
    );
    assert!(
        leader
            .my_last_weekly_race
            .is_none_or(|w| w.race_id < race.value.id)
    );

    // Within the TTL everything comes from the cache, and the cache alone gives the same answer.
    let again = client.overview(name, Refresh::IfStale).await.unwrap();
    assert_eq!(again.profile.fetched_at_ms, o.profile.fetched_at_ms);
    let cached = client.cached_overview(name).await.unwrap();
    assert_eq!(cached.profile.value, o.profile.value);

    assert_eq!(
        client
            .overview("zz_nobody_zz9", Refresh::IfStale)
            .await
            .unwrap_err(),
        ApiError::NotFound
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
#[ignore = "network: draftoutmc.com"]
async fn draftout_live() {
    let dl = Downloader::new().unwrap();
    let board: serde_json::Value = dl
        .fetch_json("https://draftoutmc.com/api/stats?limit=10")
        .await
        .unwrap();
    let name = board["rows"][3]["username"].as_str().unwrap().to_string();
    let dir = scratch("draftout");
    let client = DraftoutClient::new(dl.client().clone(), dir.clone());
    let s = client.stats(&name, Refresh::IfStale).await.unwrap();
    let p = &s.value.profile;
    println!(
        "{} elo {:?} ({}) rank {:?}: {}W {}L {}D, {} recent",
        p.username,
        p.elo,
        p.rank_name,
        p.rank,
        p.wins,
        p.losses,
        p.draws,
        s.value.matches.len()
    );
    assert!(p.username.eq_ignore_ascii_case(&name));
    assert!(p.matches > 0 && !s.value.matches.is_empty());
    assert_eq!(
        client
            .stats("zz_nobody_zz9", Refresh::IfStale)
            .await
            .unwrap_err(),
        ApiError::NotFound
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// No network (a port nothing listens on): no cache means an `Offline` error, a stale cache comes
/// back marked with its age and the error.
#[tokio::test]
async fn offline_returns_the_cache_with_its_age() {
    let dl = Downloader::new().unwrap();
    let dir = scratch("offline");
    let client = RankedClient::with_base(dl.client().clone(), dir.clone(), "http://127.0.0.1:9");
    let err = client
        .overview("SampleRunner", Refresh::Now)
        .await
        .unwrap_err();
    assert!(matches!(err, ApiError::Offline(_)), "{err:?}");

    // A profile fetched two hours ago.
    let mut old = Cached::new(serde_json::json!({
        "uuid": "00000000000000000000000000000000", "nickname": "SampleRunner", "country": null,
        "elo": 1530, "rank": 900, "tier": "diamond",
        "season": {"played": 10, "wins": 6, "losses": 4, "forfeits": 1, "completions": 5,
                   "best_time_ms": 700000, "current_streak": 2, "best_streak": 3, "playtime_ms": 1},
        "all_time": {"played": 10, "wins": 6, "losses": 4, "forfeits": 1, "completions": 5,
                   "best_time_ms": 700000, "current_streak": 2, "best_streak": 3, "playtime_ms": 1},
        "season_highest_elo": 1600, "season_lowest_elo": 1200, "last_ranked_ms": null,
        "weekly_races": []
    }));
    old.fetched_at_ms -= 2 * 3600 * 1000;
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("user-samplerunner.json"),
        serde_json::to_vec(&old).unwrap(),
    )
    .unwrap();

    let o = client
        .overview("SampleRunner", Refresh::IfStale)
        .await
        .unwrap();
    assert_eq!(o.profile.value.elo, Some(1530));
    assert_eq!(o.profile.value.tier, Some(Tier::Diamond));
    assert!(o.profile.age() >= Duration::from_secs(7200));
    assert!(matches!(
        o.profile.refresh_error,
        Some(ApiError::Offline(_))
    ));
    assert!(o.matches.is_none() && o.weekly_race.is_none() && o.players_online.is_none());
    assert_eq!(
        client.overview("bad name", Refresh::Now).await.unwrap_err(),
        ApiError::InvalidName
    );
    std::fs::remove_dir_all(&dir).unwrap();

    // The facade wires the same caches under Paths::meta().
    let root = scratch("facade");
    let stats = Stats::new(Paths::new(&root), &dl);
    assert!(stats.ranked_cached("SampleRunner").await.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

/// A local server answering each connection with the next canned response, then stopping.
fn serve(responses: Vec<(u16, &'static str, Vec<u8>)>) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        for (code, content_type, body) in responses {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            let head = format!(
                "HTTP/1.1 {code} X\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(head.as_bytes());
            let _ = sock.write_all(&body);
        }
    });
    (base, handle)
}

/// A route that moved (404 with an HTML page) keeps the cached stats with the error, and only
/// Draftout's own "player_not_found" 404 means the player does not exist.
#[tokio::test]
async fn a_moved_route_is_not_a_missing_player() {
    let dl = Downloader::new().unwrap();
    let dir = scratch("draftout-404");
    let fixture = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/api/draftout-player.json"),
    )
    .unwrap();
    let (base, server) = serve(vec![
        (200, "application/json", fixture),
        (
            404,
            "text/html",
            b"<!DOCTYPE html><title>404</title>".to_vec(),
        ),
        (
            404,
            "application/json",
            br#"{"error":"player_not_found"}"#.to_vec(),
        ),
    ]);
    let client = DraftoutClient::with_base(dl.client().clone(), dir.clone(), &base);
    let first = client.stats("SampleRunner", Refresh::Now).await.unwrap();
    assert_eq!(first.value.profile.elo, Some(1915));

    // Age the cache past MIN_REFRESH so the next call fetches.
    let file = dir.join("player-samplerunner.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    v["fetched_at_ms"] = serde_json::json!(v["fetched_at_ms"].as_i64().unwrap() - 600_000);
    std::fs::write(&file, serde_json::to_vec(&v).unwrap()).unwrap();

    let moved = client.stats("SampleRunner", Refresh::Now).await.unwrap();
    assert_eq!(moved.value.profile.elo, Some(1915));
    assert_eq!(moved.refresh_error, Some(ApiError::Http(404)));
    assert_eq!(
        client
            .stats("SampleRunner", Refresh::Now)
            .await
            .unwrap_err(),
        ApiError::NotFound
    );
    server.join().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
