//! The stats extension of the facade: the player's SpeedRunIGT records (srl-stats' scan), the MCSR
//! Ranked API and the Draftout API, for the Stats page and the Home numbers.
//!
//! Like `Tools`, it is a small trait the UI asks through (`StatsService`), with the answers coming
//! back as `Event::Stats` on the backend's channel. The UI keeps what it has been told in a
//! `StatsView` and decides when to ask:
//! - **local records** at app start, when the Stats page opens and after a game exits (the run
//!   just played shows up, and its world is seen while it exists, which keeps it attributed to
//!   its mode after the world is deleted: srl-stats' rule 4);
//! - **Ranked and Draftout** from the cache only at start (`Refresh::Never`, no network, for Home),
//!   then `Refresh::IfStale` when the Stats page opens.
//!
//! Two implementations: `RealStats` (srl-stats on the backend's tokio runtime) and `FakeStats`
//! (fixed sample data per `StatsPreset`, sent at once, for screenshots and UI work).
use std::{
    collections::HashSet,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use srl_stats::{ApiError, Cached, DraftoutStats, LocalStats, RankedOverview, Stats};
pub use srl_stats::{Refresh, StatsSettings};

use super::{AccountSnapshot, Event, Events};

/// What a stats request brought back. The API answers are boxed: an overview is over 500 bytes,
/// and every `Event` would otherwise be that big.
#[derive(Clone, PartialEq)]
pub enum StatsEvent {
    /// A finished scan of the local records.
    Local(Arc<LocalStats>),
    /// The answer for `name` to a `StatsService::ranked` call made with `refresh`.
    Ranked {
        name: String,
        refresh: Refresh,
        result: Box<Result<RankedOverview, ApiError>>,
    },
    /// The same for `StatsService::draftout`.
    Draftout {
        name: String,
        refresh: Refresh,
        result: Box<Result<Cached<DraftoutStats>, ApiError>>,
    },
}

/// A summary, not the data: `SRL_TRACE_RENDERS` logs every event, and a scan can hold tens of
/// thousands of runs.
impl fmt::Debug for StatsEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsEvent::Local(l) => write!(
                f,
                "Local({} runs in {} modes, {} files, {} parsed, {} ms)",
                l.modes.values().map(|h| h.run_count).sum::<u32>(),
                l.modes.len(),
                l.files,
                l.parsed,
                l.scan_ms
            ),
            StatsEvent::Ranked {
                name,
                refresh,
                result,
            } => write!(
                f,
                "Ranked({name}, {refresh:?}, {})",
                match &**result {
                    Ok(o) => format!("ok, error {:?}", o.profile.refresh_error),
                    Err(e) => format!("{e:?}"),
                }
            ),
            StatsEvent::Draftout {
                name,
                refresh,
                result,
            } => write!(
                f,
                "Draftout({name}, {refresh:?}, {})",
                match &**result {
                    Ok(c) => format!("ok, error {:?}", c.refresh_error),
                    Err(e) => format!("{e:?}"),
                }
            ),
        }
    }
}

/// The stats side of the facade. Like `Backend`: every method returns at once, the answer comes
/// as `Event::Stats`, and nothing is sent unless asked.
pub trait StatsService {
    /// Scans the SpeedRunIGT records again (cheap when nothing is new). A request while a scan
    /// runs queues one more scan after it, never more than one.
    fn scan(&self);
    /// The MCSR Ranked overview of `name` (profile, matches, weekly race).
    fn ranked(&self, name: &str, refresh: Refresh);
    /// The Draftout stats of `name`.
    fn draftout(&self, name: &str, refresh: Refresh);
}

/// The name of the Microsoft account Play uses (`AccountSnapshot::active_name`): the Ranked and
/// Draftout names default to it when the player has not typed one in Settings (plan section 2),
/// because that is the name those servers know the player by. `None` without an active account
/// or in a build without Microsoft login; then only the typed names count. Every caller goes
/// through `lookup_name`. Takes the app's last snapshot, so a render never asks the backend.
pub fn active_microsoft_name(account: &AccountSnapshot) -> Option<String> {
    account.active_name().map(str::to_string)
}

/// The name to look a player up by: the one set in Settings, else the active Microsoft account's.
/// Never the offline player name: people play Ranked and Draftout under their Microsoft account's
/// name, and an offline name that happens to belong to someone else would show their stats.
pub fn lookup_name(set: Option<&str>, microsoft: Option<&str>) -> Option<String> {
    set.or(microsoft)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------------------------
// What the UI keeps
// ---------------------------------------------------------------------------------------------

/// One API lookup as the page holds it: the name it is for, the last value and the last error.
#[derive(Clone, Debug, PartialEq)]
pub struct Lookup<T> {
    pub name: String,
    /// The newest value for `name`, from the cache or the network. Its `Cached::refresh_error`
    /// says whether it is current.
    pub value: Option<T>,
    /// Why the last network request returned no value (`NotFound`, offline with nothing
    /// cached, rate limited). Cleared by the next value.
    pub error: Option<ApiError>,
    /// A network request for `name` is under way.
    pub loading: bool,
}

impl<T> Lookup<T> {
    fn new(name: &str) -> Self {
        Lookup {
            name: name.to_string(),
            value: None,
            error: None,
            loading: false,
        }
    }

    /// Applies an answer for `name`. A cache-only answer (`Refresh::Never`) never replaces a
    /// newer value from the network and never sets an error: "nothing cached yet" is not news.
    fn apply(
        &mut self,
        name: &str,
        refresh: Refresh,
        result: Result<T, ApiError>,
        fetched: impl Fn(&T) -> i64,
    ) -> bool {
        if !self.name.eq_ignore_ascii_case(name) {
            return false;
        }
        if refresh == Refresh::Never {
            return match result {
                Ok(v)
                    if self
                        .value
                        .as_ref()
                        .is_none_or(|old| fetched(old) < fetched(&v)) =>
                {
                    self.value = Some(v);
                    true
                }
                _ => false,
            };
        }
        self.loading = false;
        match result {
            Ok(v) => {
                self.value = Some(v);
                self.error = None;
            }
            // The name is wrong (or was changed): the old numbers are not this player's.
            Err(e @ (ApiError::NotFound | ApiError::InvalidName)) => {
                self.value = None;
                self.error = Some(e);
            }
            Err(e) => self.error = Some(e),
        }
        true
    }
}

/// Everything the Stats page and Home know about stats, as the events told it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatsView {
    /// The last scan; `None` until the first one ends (the page shows a skeleton).
    pub local: Option<Arc<LocalStats>>,
    /// `None` while no Ranked name is known (the page asks for one).
    pub ranked: Option<Lookup<RankedOverview>>,
    pub draftout: Option<Lookup<Cached<DraftoutStats>>>,
}

impl StatsView {
    /// Records a Ranked request for `name` (`None`: no name, the lookup is dropped). Returns
    /// whether a request should go out.
    pub fn want_ranked(&mut self, name: Option<&str>, refresh: Refresh) -> bool {
        want(&mut self.ranked, name, refresh)
    }

    pub fn want_draftout(&mut self, name: Option<&str>, refresh: Refresh) -> bool {
        want(&mut self.draftout, name, refresh)
    }

    /// Applies an event; `true` when something the UI shows changed.
    pub fn apply(&mut self, ev: StatsEvent) -> bool {
        match ev {
            StatsEvent::Local(l) => {
                let changed = self.local.as_ref() != Some(&l);
                self.local = Some(l);
                changed
            }
            StatsEvent::Ranked {
                name,
                refresh,
                result,
            } => self
                .ranked
                .as_mut()
                .is_some_and(|l| l.apply(&name, refresh, *result, |o| o.profile.fetched_at_ms)),
            StatsEvent::Draftout {
                name,
                refresh,
                result,
            } => self
                .draftout
                .as_mut()
                .is_some_and(|l| l.apply(&name, refresh, *result, |c| c.fetched_at_ms)),
        }
    }
}

fn want<T>(slot: &mut Option<Lookup<T>>, name: Option<&str>, refresh: Refresh) -> bool {
    let Some(name) = name else {
        *slot = None;
        return false;
    };
    if slot
        .as_ref()
        .is_none_or(|l| !l.name.eq_ignore_ascii_case(name))
    {
        *slot = Some(Lookup::new(name));
    }
    if refresh != Refresh::Never
        && let Some(l) = slot.as_mut()
    {
        l.loading = true;
    }
    true
}

// ---------------------------------------------------------------------------------------------
// RealStats
// ---------------------------------------------------------------------------------------------

/// srl-stats on the backend's runtime. `Stats` is built once (it holds the API clients, their
/// rate limiters and the scan lock); every call is a task whose answer goes out as an event.
pub struct RealStats {
    stats: Stats,
    events: Events,
    rt: tokio::runtime::Handle,
    /// A scan is waiting to start: further requests add nothing (it will see their files too).
    scan_queued: Arc<AtomicBool>,
    /// Held by the scan that runs, so the queued one knows when it is its turn.
    scan_running: Arc<tokio::sync::Mutex<()>>,
    /// API requests under way, by kind and lower-case name, so opening the page twice while
    /// offline doesn't start a second 8 s wait for the same answer.
    in_flight: Arc<Mutex<HashSet<(bool, String)>>>,
    /// The player's own instances' game directories, with the mode their runs count for.
    instance_dirs: Arc<Mutex<Vec<(std::path::PathBuf, srl_core::Mode)>>>,
}

impl RealStats {
    /// The custom instances changed: the next scans read these game directories too.
    pub fn set_instance_dirs(&self, dirs: Vec<(std::path::PathBuf, srl_core::Mode)>) {
        *self.instance_dirs.lock().unwrap() = dirs;
    }

    pub fn new(stats: Stats, events: Events, rt: tokio::runtime::Handle) -> Self {
        Self {
            stats,
            events,
            rt,
            scan_queued: Arc::default(),
            scan_running: Arc::default(),
            in_flight: Arc::default(),
            instance_dirs: Arc::default(),
        }
    }

    /// Runs `fetch` unless the same lookup is already under way; `ranked` tells the kinds apart.
    fn lookup<F>(&self, ranked: bool, name: &str, refresh: Refresh, fetch: F)
    where
        F: Future<Output = StatsEvent> + Send + 'static,
    {
        let key = (ranked, name.to_ascii_lowercase());
        // A cache read is instant and never deduplicated: it must not swallow the network
        // request that follows it.
        if refresh != Refresh::Never && !self.in_flight.lock().unwrap().insert(key.clone()) {
            return;
        }
        let events = self.events.clone();
        let in_flight = self.in_flight.clone();
        self.rt.spawn(async move {
            let ev = fetch.await;
            if refresh != Refresh::Never {
                in_flight.lock().unwrap().remove(&key);
            }
            super::real::log(format_args!("stats: {ev:?}"));
            let _ = events.unbounded_send(Event::Stats(ev));
        });
    }
}

impl StatsService for RealStats {
    fn scan(&self) {
        if self.scan_queued.swap(true, Ordering::SeqCst) {
            return;
        }
        let (stats, events, queued, running, dirs) = (
            self.stats.clone(),
            self.events.clone(),
            self.scan_queued.clone(),
            self.scan_running.clone(),
            self.instance_dirs.clone(),
        );
        self.rt.spawn(async move {
            // Once this scan is the one that runs, a new request queues another: files written
            // from now on may be missed by this one.
            let _running = running.lock().await;
            queued.store(false, Ordering::SeqCst);
            let extra = dirs.lock().unwrap().clone();
            let local = stats.local_with(extra).await;
            super::real::log(format_args!(
                "stats: scan of {} record files ({} parsed) in {} ms; {} skipped, {} unreadable \
                 folders",
                local.files,
                local.parsed,
                local.scan_ms,
                local.skipped.count,
                local.unreadable.len()
            ));
            let _ = events.unbounded_send(Event::Stats(StatsEvent::Local(Arc::new(local))));
        });
    }

    fn ranked(&self, name: &str, refresh: Refresh) {
        let (stats, owned) = (self.stats.clone(), name.to_string());
        self.lookup(true, name, refresh, async move {
            let result = Box::new(stats.ranked(&owned, refresh).await);
            StatsEvent::Ranked {
                name: owned,
                refresh,
                result,
            }
        });
    }

    fn draftout(&self, name: &str, refresh: Refresh) {
        let (stats, owned) = (self.stats.clone(), name.to_string());
        self.lookup(false, name, refresh, async move {
            let result = Box::new(stats.draftout(&owned, refresh).await);
            StatsEvent::Draftout {
                name: owned,
                refresh,
                result,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use srl_stats::{RankedProfile, RankedRecord};

    use super::*;

    fn overview(name: &str, fetched_at_ms: i64) -> RankedOverview {
        RankedOverview {
            profile: Cached {
                value: RankedProfile {
                    uuid: "u".into(),
                    nickname: name.into(),
                    country: None,
                    elo: Some(1500),
                    rank: None,
                    tier: None,
                    season: RankedRecord::default(),
                    all_time: RankedRecord::default(),
                    season_highest_elo: None,
                    season_lowest_elo: None,
                    last_ranked_ms: None,
                    weekly_races: vec![],
                },
                fetched_at_ms,
                refresh_error: None,
            },
            matches: None,
            weekly_race: None,
            my_weekly_race: None,
            my_last_weekly_race: None,
            players_online: None,
        }
    }

    fn ranked(
        name: &str,
        refresh: Refresh,
        result: Result<RankedOverview, ApiError>,
    ) -> StatsEvent {
        StatsEvent::Ranked {
            name: name.into(),
            refresh,
            result: Box::new(result),
        }
    }

    #[test]
    fn the_typed_name_wins_over_the_account() {
        assert_eq!(lookup_name(Some("Typed"), Some("Ms")), Some("Typed".into()));
        assert_eq!(lookup_name(None, Some("Ms")), Some("Ms".into()));
        assert_eq!(lookup_name(Some("  "), None), None);
        assert_eq!(lookup_name(None, None), None);
    }

    /// Start (cache), open the page (network): the network answer replaces the cached one, a
    /// late cache answer doesn't undo it, and an answer for an old name is dropped.
    #[test]
    fn cache_then_network_then_a_new_name() {
        let mut v = StatsView::default();
        assert!(!v.want_ranked(None, Refresh::Never));
        assert!(v.ranked.is_none());
        assert!(v.want_ranked(Some("Violet"), Refresh::Never));
        assert!(!v.ranked.as_ref().unwrap().loading);
        assert!(v.apply(ranked(
            "violet",
            Refresh::Never,
            Ok(overview("Violet", 100))
        )));
        assert!(v.want_ranked(Some("Violet"), Refresh::IfStale));
        let l = v.ranked.as_ref().unwrap();
        assert!(
            l.loading && l.value.is_some(),
            "the cached value stays while it loads"
        );
        assert!(v.apply(ranked(
            "Violet",
            Refresh::IfStale,
            Ok(overview("Violet", 200))
        )));
        assert!(!v.apply(ranked(
            "Violet",
            Refresh::Never,
            Ok(overview("Violet", 100))
        )));
        let l = v.ranked.as_ref().unwrap();
        assert_eq!(l.value.as_ref().unwrap().profile.fetched_at_ms, 200);
        assert!(!l.loading);
        // Nothing cached is not an error.
        assert!(!v.apply(ranked(
            "Violet",
            Refresh::Never,
            Err(ApiError::Offline("not fetched yet".into()))
        )));
        assert_eq!(v.ranked.as_ref().unwrap().error, None);

        v.want_ranked(Some("Other"), Refresh::IfStale);
        assert!(!v.apply(ranked(
            "Violet",
            Refresh::IfStale,
            Ok(overview("Violet", 300))
        )));
        let l = v.ranked.as_ref().unwrap();
        assert_eq!(
            (l.name.as_str(), l.value.is_none(), l.loading),
            ("Other", true, true)
        );
    }

    #[test]
    fn not_found_drops_the_old_numbers_but_offline_keeps_them() {
        let mut v = StatsView::default();
        v.want_ranked(Some("Violet"), Refresh::IfStale);
        v.apply(ranked(
            "Violet",
            Refresh::IfStale,
            Ok(overview("Violet", 1)),
        ));
        v.want_ranked(Some("Violet"), Refresh::IfStale);
        v.apply(ranked(
            "Violet",
            Refresh::IfStale,
            Err(ApiError::RateLimited { retry_after_s: 30 }),
        ));
        let l = v.ranked.as_ref().unwrap();
        assert!(l.value.is_some());
        assert_eq!(l.error, Some(ApiError::RateLimited { retry_after_s: 30 }));
        v.want_ranked(Some("Violet"), Refresh::IfStale);
        v.apply(ranked("Violet", Refresh::IfStale, Err(ApiError::NotFound)));
        let l = v.ranked.as_ref().unwrap();
        assert!(l.value.is_none());
        assert_eq!(l.error, Some(ApiError::NotFound));
    }

    #[test]
    fn a_new_scan_replaces_the_last() {
        let mut v = StatsView::default();
        let a = Arc::new(LocalStats::default());
        assert!(v.apply(StatsEvent::Local(a.clone())));
        // The same result again changes nothing on screen.
        assert!(!v.apply(StatsEvent::Local(Arc::new(LocalStats::default()))));
        let b = LocalStats {
            files: 3,
            ..LocalStats::default()
        };
        assert!(v.apply(StatsEvent::Local(Arc::new(b))));
        assert_eq!(v.local.as_ref().unwrap().files, 3);
    }

    /// The real service against a records folder: a scan request answers with the runs, and a
    /// burst of requests while one runs ends in at most two scans.
    #[test]
    fn real_scans_send_the_runs_and_coalesce() {
        use futures::StreamExt;
        let root = std::env::temp_dir().join(format!("srl-app-stats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = srl_core::Paths::new(root.join("srl"));
        let world = paths
            .instance(srl_core::Mode::Rsg)
            .join("saves/Random Speedrun #1");
        std::fs::create_dir_all(world.join("speedrunigt")).unwrap();
        std::fs::write(
            world.join("speedrunigt/record.json"),
            r#"{"is_completed":true,"date":1000,"final_igt":600000,"final_rta":610000,
               "retimed_igt":600000,"category":"ANY","run_type":"random_seed",
               "world_name":"Random Speedrun #1",
               "timelines":[{"name":"enter_nether","igt":120000,"rta":121000}]}"#,
        )
        .unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let dl = srl_core::Downloader::new().unwrap();
        let stats = Stats::with_sources(
            paths,
            &dl,
            srl_stats::IgtSources::with_global_dirs([root.join("home/speedrunigt")]),
        );
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let s = RealStats::new(stats, tx, rt.handle().clone());
        for _ in 0..5 {
            s.scan();
        }
        let first = futures::executor::block_on(rx.next()).unwrap();
        let Event::Stats(StatsEvent::Local(l)) = first else {
            panic!("{first:?}")
        };
        let rsg = l.mode(srl_core::Mode::Rsg);
        assert_eq!(rsg.run_count, 1);
        assert_eq!(rsg.headline_pb().unwrap().record.igt_ms, 600_000);
        // At most one more scan was queued behind the first.
        rt.shutdown_timeout(std::time::Duration::from_secs(5));
        let mut more = 0;
        while rx.try_recv().is_ok() {
            more += 1;
        }
        assert!(more <= 1, "{more} extra scans");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
