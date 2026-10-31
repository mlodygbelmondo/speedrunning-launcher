//! Read-only stats: the player's own SpeedRunIGT run records, the MCSR Ranked API and the
//! Draftout API. Nothing here writes to the game, the instances or the APIs; the only files it
//! writes are its caches under `Paths::meta()/stats`, `/ranked` and `/draftout`.
//!
//! # API
//!
//! ```ignore
//! use srl_stats::{Refresh, Stats};
//!
//! // Once, next to the other backend services. Cheap; holds no open files.
//! let stats = Stats::new(paths.clone(), &downloader);
//!
//! // Local runs, per mode. Blocking I/O runs on spawn_blocking; scans are serialised, and a scan
//! // that finds nothing new reads no record file, only its index (NOTES.md has timings).
//! let local = stats.local().await;
//! let rsg = local.mode(Mode::Rsg);               // &RunHistory (empty if no runs), no copy
//! rsg.headline_pb();                             // Option<&Run>: random-seed Any% PB
//! rsg.recent(10);                                // newest first, each with its splits
//! rsg.category("ANY", &RunType::RandomSeed);     // PB, best segments, sum of best, counts
//!
//! // MCSR Ranked by nickname (a separate setting from the offline name: people play Ranked
//! // under their Microsoft name). Cache first for an instant paint, then the network.
//! let quick = stats.ranked_cached("Name").await;           // Option<RankedOverview>, no network
//! let fresh = stats.ranked("Name", Refresh::IfStale).await; // Result<RankedOverview, ApiError>
//!
//! // Draftout by Minecraft name, the same way.
//! let d = stats.draftout("Name", Refresh::IfStale).await;  // Result<Cached<DraftoutStats>, _>
//! ```
//!
//! Every method is async and returns quickly when the cache is fresh; none of them blocks the
//! calling thread, so the app calls them on its background tokio runtime and sends the result to
//! the UI. Errors are values (`ApiError`), never panics, and a missing network only marks data as
//! stale.
//!
//! # For the UI
//!
//! **Stats page, "My runs" (per mode tab: RSG, FSG, SSG, Ranked practice):**
//! - Headline: PB (`headline_pb`, IGT; show RTA small), runs and completions (`run_count`,
//!   `completion_count`), sum of best when there is one (`CategoryStats::sum_of_best_ms`).
//! - Splits table for a run: `Run::record.splits` (the standard ones have a `kind`), or the route
//!   view: `route_points` / `segment_times` against `CategoryStats::best_segments`.
//! - Recent runs: `recent(n)`, newest first: world name, IGT, completed or reset, date, the
//!   furthest split reached.
//! - Several categories: `categories` (most played first). Most players only have `ANY`.
//! - "Today": `runs_since(local midnight in epoch ms)`.
//!
//! **Home:** the mode tile can show the mode's PB and "N runs today". Ranked's tile can show elo,
//! tier and rank from `ranked_cached` (no network on start); when its `profile.refresh_error` is
//! set, label the numbers with their age.
//!
//! **Loading:** `local()` took 58 ms for 5000 runs from the index and 91 ms parsing all 10 000
//! files (release build, files in the OS cache; a cold disk and Windows are slower). Keep the last
//! result in app state and show it, or a skeleton, while a scan runs; never block. For Ranked and
//! Draftout, paint `*_cached` at once, then replace it with the network result.
//!
//! **Empty:**
//! - No runs for a mode: "No runs yet. SpeedRunIGT records every run you play in this mode." A
//!   mode tab exists even if empty. Draftout never has local runs (its mod set has no
//!   SpeedRunIGT), so it shows only the Draftout API.
//! - No Ranked nickname set: ask for it ("Your Minecraft name on MCSR Ranked"), with the note that
//!   it is not the offline player name.
//! - `ApiError::NotFound`: "No MCSR Ranked player called X." (the same for Draftout).
//! - Ranked `elo == None`: the player has not finished placements; show "Placements" instead of a
//!   tier.
//!
//! **Errors:**
//! - `Cached::refresh_error` set: the value is older than its TTL. Show it with "Updated 3 h ago"
//!   from `Cached::age()`, plus "(offline)" for `Offline`; `NotRefreshed` (a `*_cached` call, the
//!   Home tile) only means nobody asked the network yet, so show the age without "offline".
//!   `Err(Offline)` with nothing cached: "Can't reach MCSR Ranked. Showing nothing until you're
//!   online." `RateLimited`: "Try again in N s." `Http(code)` with a cache: the data, aged, and
//!   "MCSR Ranked answered with an error". Never a modal.
//! - `LocalStats::skipped.count > 0`: a small note "N record files could not be read", never an
//!   error state. `LocalStats::unreadable` not empty: "Can't open <folder>: some runs are
//!   missing" (permissions, usually); the runs come back once it is readable.
//!
//! **What the copy may honestly claim:**
//! - "Your runs from SpeedRunIGT records in this launcher's instances." Not "all your runs":
//!   runs from other launchers are `unattributed`, and runs whose worlds were deleted before the
//!   launcher ever scanned them can't be tied to a mode (attribution rule in `igt`).
//! - IGT is SpeedRunIGT's: for random-seed Any% under 13 minutes it is the retimed IGT the
//!   speedrun.com guideline asks for (`Record::igt_ms`; the raw one is `igt_raw_ms`). Call it
//!   "IGT", not "verified time": nothing here is verified by a leaderboard.
//! - A run with cheats, opened to LAN (with or without cheats) or in co-op counts as a run and a
//!   completion, but never as a PB, a best segment or in "reached" (`Record::eligible`;
//!   `CategoryStats::excluded` and `excluded_completions` count them, so the page can say "1
//!   completion with cheats or LAN, not counted"). SpeedRunIGT's cheat flag alone misses LAN
//!   cheats after a rejoin, which is why opening to LAN excludes a run by itself.
//! - "Reset" means the run was not completed; SpeedRunIGT does not record why it ended.
//! - Ranked numbers are the season's ranked matches as the MCSR Ranked API reports them, as of
//!   the time shown; Ranked matches themselves leave no local records (the Ranked mod stops
//!   SpeedRunIGT from writing during online matches).
//! - Weekly race: the leaderboard is the API's top entries only (30 to 33 seen). The player's
//!   place in the running race (`my_weekly_race`) is found on that board, so `None` means "not in
//!   the top N" (N = `leaderboard.len()`), never "you have not run it": say "Not in the top N
//!   yet". Their result in the last finished race (`my_last_weekly_race`) is from their profile.
//! - "Players online" is the API's count of players online in MCSR Ranked, not on this computer.
//! - Draftout data comes from Draftout's public stats endpoint, which has no official docs; say
//!   "from draftoutmc.com".
pub mod cache;
pub mod draftout;
pub mod http;
pub mod igt;
pub mod ranked;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use srl_core::{Downloader, Paths};

pub use cache::Cached;
pub use draftout::{DraftoutClient, DraftoutMatch, DraftoutProfile, DraftoutStats};
pub use http::{ApiError, is_lookup_name};
pub use igt::{
    BestSegment, CategoryKey, CategoryStats, IgtSources, LatestWorld, LocalStats, Record, Run,
    RunHistory, RunType, Segment, Skipped, Split, SplitKind, route_points, segment_times,
};
pub use ranked::{
    MatchKind, Outcome, RankedClient, RankedMatch, RankedOverview, RankedPlayer, RankedProfile,
    RankedRecord, Tier, WeeklyRace, WeeklyRaceEntry, WeeklyRaceResult,
};

/// How hard to try for fresh data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    /// Use the cache while it is within the TTL (opening a page, switching tabs).
    IfStale,
    /// The player asked for it (a refresh button): fetch unless the cache is a few seconds old
    /// (`cache::MIN_REFRESH`).
    Now,
    /// Cache only, never the network. A value past its TTL comes back with
    /// `refresh_error: Some(ApiError::NotRefreshed)`.
    Never,
}

/// The two names the stats need, for the app's settings. Neither is the offline player name:
/// people play Ranked and Draftout under their Microsoft account's name. `None` means "not set";
/// the page then asks for it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsSettings {
    #[serde(default)]
    pub ranked_nickname: Option<String>,
    #[serde(default)]
    pub draftout_username: Option<String>,
}

/// Everything in one handle for the app. Clone it freely.
#[derive(Clone)]
pub struct Stats {
    inner: Arc<Inner>,
}

struct Inner {
    paths: Paths,
    sources: IgtSources,
    ranked: RankedClient,
    draftout: DraftoutClient,
    /// One scan at a time: two would parse the same files and race on the index. The guard is
    /// moved into the blocking task, so a `local()` future dropped mid-scan (a GPUI task
    /// cancelled, a timeout) keeps the lock until its scan really ends.
    scan_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Stats {
    /// Reuses the launcher's HTTP client (its User-Agent is what the MCSR API asks for) and reads
    /// SpeedRunIGT's folders in the user's home.
    pub fn new(paths: Paths, downloader: &Downloader) -> Self {
        Self::with_sources(paths, downloader, IgtSources::for_user())
    }

    pub fn with_sources(paths: Paths, downloader: &Downloader, sources: IgtSources) -> Self {
        let client = downloader.client().clone();
        Stats {
            inner: Arc::new(Inner {
                ranked: RankedClient::new(client.clone(), paths.meta().join("ranked")),
                draftout: DraftoutClient::new(client, paths.meta().join("draftout")),
                paths,
                sources,
                scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            }),
        }
    }

    /// Scans SpeedRunIGT's records (incrementally) and returns the runs per mode. Call it when the
    /// Stats page opens and after a game exits.
    pub async fn local(&self) -> LocalStats {
        let guard = self.inner.scan_lock.clone().lock_owned().await;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            igt::scan(&inner.paths, &inner.sources)
        })
        .await
        .unwrap_or_default()
    }

    pub async fn ranked(
        &self,
        nickname: &str,
        refresh: Refresh,
    ) -> Result<RankedOverview, ApiError> {
        self.inner.ranked.overview(nickname, refresh).await
    }

    pub async fn ranked_cached(&self, nickname: &str) -> Option<RankedOverview> {
        self.inner.ranked.cached_overview(nickname).await
    }

    pub async fn draftout(
        &self,
        username: &str,
        refresh: Refresh,
    ) -> Result<Cached<DraftoutStats>, ApiError> {
        self.inner.draftout.stats(username, refresh).await
    }

    pub async fn draftout_cached(&self, username: &str) -> Option<Cached<DraftoutStats>> {
        self.inner.draftout.cached_stats(username).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `local()` that is cancelled mid-scan keeps the scan lock until its scan ends, so the next
    /// call waits for it and then finds everything in the index instead of scanning in parallel.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_cancelled_scan_still_serialises_the_next_one() {
        let root = std::env::temp_dir().join(format!("srl-stats-cancel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let records = root.join("home/speedrunigt/records");
        std::fs::create_dir_all(&records).unwrap();
        const N: u32 = 3000;
        for i in 0..N {
            std::fs::write(
                records.join(format!("{i:08}.json")),
                format!(
                    r#"{{"is_completed":false,"date":{i},"final_igt":{i},"final_rta":{i},
                    "category":"ANY","run_type":"random_seed","world_name":"Other #{i}",
                    "timelines":[{{"name":"enter_nether","igt":1,"rta":1}}]}}"#
                ),
            )
            .unwrap();
        }
        let dl = Downloader::new().unwrap();
        let stats = Stats::with_sources(
            Paths::new(root.join("srl")),
            &dl,
            IgtSources::with_global_dirs([root.join("home/speedrunigt")]),
        );
        let cancelled =
            tokio::time::timeout(std::time::Duration::from_millis(1), stats.local()).await;
        assert!(
            cancelled.is_err(),
            "the first scan should take longer than 1 ms"
        );
        let next = stats.local().await;
        assert_eq!((next.files, next.parsed), (N, 0));
        assert_eq!(next.unattributed.run_count, N);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
