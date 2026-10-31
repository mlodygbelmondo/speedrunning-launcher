//! `FakeStats`: the stats side of `FakeBackend`, with fixed sample data for every state the Stats
//! page and Home can be in (`StatsPreset`, chosen by the `stats-*` screen ids and `SRL_DEMO`). The
//! data goes through the real srl-stats types (`RunHistory::new` computes the PBs, best segments
//! and sums of best), so the screenshots show what the real page shows for such records. All of
//! it is made up; nothing here reads a file or the network.
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use chrono::{Local, TimeZone};
use srl_core::Mode;
use srl_stats::{
    ApiError, Cached, DraftoutMatch, DraftoutProfile, DraftoutStats, LocalStats, MatchKind,
    Outcome, RankedMatch, RankedOverview, RankedPlayer, RankedProfile, RankedRecord, Record,
    Refresh, Run, RunHistory, RunType, Skipped, Split, SplitKind, StatsSettings, Tier, WeeklyRace,
    WeeklyRaceEntry, WeeklyRaceResult, cache::now_ms,
};

use super::{
    Event, Events,
    stats::{StatsEvent, StatsService},
};

/// The fake's stats state (README "Screen ids").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsPreset {
    /// Plain `SRL_FAKE`: no records, and no network for the APIs.
    Nothing,
    /// Runs in Ranked practice, RSG and FSG; Ranked and Draftout profiles, fresh (`stats`, the
    /// demo).
    Sample,
    /// The first scan never ends and the APIs never answer: the skeletons.
    Scanning,
    /// The scan found no runs, and no Ranked or Draftout name is set.
    Empty,
    /// Both APIs say there is no such player.
    Unknown,
    /// The runs, plus 3 unreadable record files and an unreadable folder; both APIs offline,
    /// with their data from the cache, 3 hours old.
    Offline,
    /// Both APIs offline with nothing cached; Draftout rate limited.
    Unreachable,
    /// A new player: resets only, no completion; Ranked placements not done; Draftout unranked.
    New,
}

/// The name the presets with API data use for both lookups.
pub const SAMPLE_NAME: &str = "VioletRunner";

impl StatsPreset {
    /// The names this preset needs in the settings (`Nothing` keeps the saved ones).
    pub fn apply_names(self, s: &mut StatsSettings) {
        match self {
            StatsPreset::Nothing => {}
            StatsPreset::Empty => *s = StatsSettings::default(),
            _ => {
                s.ranked_nickname = Some(SAMPLE_NAME.into());
                s.draftout_username = Some(SAMPLE_NAME.into());
            }
        }
    }
}

pub struct FakeStats {
    preset: StatsPreset,
    events: Events,
}

impl FakeStats {
    pub fn new(preset: StatsPreset, events: Events) -> Self {
        Self { preset, events }
    }

    fn send(&self, ev: StatsEvent) {
        let _ = self.events.unbounded_send(Event::Stats(ev));
    }
}

impl StatsService for FakeStats {
    fn scan(&self) {
        let local = match self.preset {
            StatsPreset::Scanning => return,
            StatsPreset::Nothing | StatsPreset::Empty => LocalStats::default(),
            StatsPreset::Offline => {
                let mut l = sample_local(false);
                l.skipped = Skipped {
                    count: 3,
                    examples: vec![],
                };
                l.unreadable = vec![(
                    PathBuf::from("/home/violet/speedrunigt/records"),
                    "Permission denied (os error 13)".into(),
                )];
                l
            }
            StatsPreset::New => sample_local(true),
            _ => sample_local(false),
        };
        self.send(StatsEvent::Local(Arc::new(local)));
    }

    fn ranked(&self, name: &str, refresh: Refresh) {
        let result = match api_answer(self.preset, refresh, false) {
            None => return,
            Some(Ok(age)) => Ok(sample_ranked(name, age, self.preset == StatsPreset::New)),
            Some(Err(e)) => Err(e),
        };
        self.send(StatsEvent::Ranked {
            name: name.into(),
            refresh,
            result: Box::new(result),
        });
    }

    fn draftout(&self, name: &str, refresh: Refresh) {
        let result = match api_answer(self.preset, refresh, true) {
            None => return,
            Some(Ok(age)) => Ok(sample_draftout(name, age, self.preset == StatsPreset::New)),
            Some(Err(e)) => Err(e),
        };
        self.send(StatsEvent::Draftout {
            name: name.into(),
            refresh,
            result: Box::new(result),
        });
    }
}

/// How old the data is and why it couldn't be refreshed, or the error, as the real client would
/// answer in this preset. `None`: no answer at all (still loading).
fn api_answer(
    preset: StatsPreset,
    refresh: Refresh,
    draftout: bool,
) -> Option<Result<Age, ApiError>> {
    let offline = || ApiError::Offline("the fake backend has no network".into());
    Some(match (preset, refresh) {
        (StatsPreset::Scanning, _) => return None,
        // A cache-only call with nothing cached.
        (
            StatsPreset::Nothing
            | StatsPreset::Empty
            | StatsPreset::Unknown
            | StatsPreset::Unreachable,
            Refresh::Never,
        ) => Err(offline()),
        (StatsPreset::Nothing, _) => Err(offline()),
        (StatsPreset::Unknown, _) => Err(ApiError::NotFound),
        (StatsPreset::Unreachable, _) if draftout => {
            Err(ApiError::RateLimited { retry_after_s: 40 })
        }
        (StatsPreset::Unreachable, _) => Err(offline()),
        (StatsPreset::Offline, Refresh::Never) => Ok(Age::HoursAgo(3, ApiError::NotRefreshed)),
        (StatsPreset::Offline, _) => Ok(Age::HoursAgo(3, offline())),
        _ => Ok(Age::Fresh),
    })
}

enum Age {
    Fresh,
    /// Fetched this many hours ago; refreshing failed with this.
    HoursAgo(i64, ApiError),
}

fn cached<T>(value: T, age: &Age) -> Cached<T> {
    match age {
        Age::Fresh => Cached::new(value),
        Age::HoursAgo(h, e) => Cached {
            value,
            fetched_at_ms: now_ms() - h * 3_600_000,
            refresh_error: Some(e.clone()),
        },
    }
}

/// Epoch ms of a local date and time in October 2026, so the dates read the same in any time zone.
fn oct(day: u32, hour: u32, min: u32) -> i64 {
    Local
        .with_ymd_and_hms(2026, 10, day, hour, min, 0)
        .earliest()
        .map(|t| t.timestamp_millis())
        .unwrap_or_default()
}

const fn mmss(m: u64, s: u64) -> u64 {
    (m * 60 + s) * 1000
}

/// A record with these splits (in route order), completed when `dragon` is set.
fn record(date_ms: i64, run_type: RunType, splits: &[(SplitKind, u64)], end_ms: u64) -> Record {
    let completed = splits.iter().any(|(k, _)| *k == SplitKind::Dragon);
    Record {
        mc_version: "1.16.1".into(),
        igt_version: "16.0+1.16.1".into(),
        category: "ANY".into(),
        run_type,
        completed,
        coop: false,
        hardcore: false,
        cheats: false,
        open_lan_ms: None,
        world_name: "Random Speedrun #1".into(),
        date_ms,
        igt_ms: end_ms,
        igt_raw_ms: end_ms.saturating_sub(if completed { 31 } else { 0 }),
        rta_ms: end_ms + 4_000,
        splits: splits
            .iter()
            .map(|(kind, ms)| Split {
                name: timeline(*kind).into(),
                kind: Some(*kind),
                igt_ms: *ms,
                rta_ms: ms + 2_000,
            })
            .collect(),
    }
}

fn timeline(kind: SplitKind) -> &'static str {
    match kind {
        SplitKind::Nether => "enter_nether",
        SplitKind::Bastion => "enter_bastion",
        SplitKind::Fortress => "enter_fortress",
        SplitKind::Blind => "nether_travel",
        SplitKind::Stronghold => "enter_stronghold",
        SplitKind::End => "enter_end",
        SplitKind::Dragon => "kill_ender_dragon",
    }
}

/// A full route: nether, bastion, fortress, blind, stronghold, end, dragon (cumulative).
fn route(t: [u64; 7]) -> Vec<(SplitKind, u64)> {
    use SplitKind::*;
    [Nether, Bastion, Fortress, Blind, Stronghold, End, Dragon]
        .into_iter()
        .zip(t)
        .collect()
}

/// The first `n` points of a route.
fn reached(t: &[u64]) -> Vec<(SplitKind, u64)> {
    use SplitKind::*;
    [Nether, Bastion, Fortress, Blind, Stronghold, End]
        .into_iter()
        .zip(t.iter().copied())
        .collect()
}

/// The sample runs: Ranked practice, RSG and FSG with splits, PBs and resets; SSG and Draftout
/// without runs. `new_player`: only resets, nothing finished.
pub fn sample_local(new_player: bool) -> LocalStats {
    let mut runs: BTreeMap<Mode, Vec<Run>> = BTreeMap::new();
    let mut n = 0;
    let mut add = |mode: Mode, mut rec: Record| {
        n += 1;
        rec.world_name = format!("Random Speedrun #{n}");
        runs.entry(mode).or_default().push(Run {
            id: format!("g:fake-{n}"),
            mode: Some(mode),
            record: rec,
            world: None,
        });
    };
    let rs = RunType::RandomSeed;
    if !new_player {
        // Ranked practice.
        add(
            Mode::Ranked,
            record(
                oct(25, 21, 40),
                rs.clone(),
                &route([
                    mmss(2, 39),
                    mmss(4, 30),
                    mmss(7, 2),
                    mmss(8, 35),
                    mmss(11, 40),
                    mmss(14, 21),
                    mmss(15, 46),
                ]),
                mmss(15, 48) + 210,
            ),
        );
        add(
            Mode::Ranked,
            record(
                oct(23, 20, 12),
                rs.clone(),
                &route([
                    mmss(2, 50),
                    mmss(4, 41),
                    mmss(6, 58),
                    mmss(8, 20),
                    mmss(11, 2),
                    mmss(14, 40),
                    mmss(16, 18),
                ]),
                mmss(16, 20) + 4,
            ),
        );
        // RSG: the PB, a slower finish (fortress first) and a finish opened to LAN.
        add(
            Mode::Rsg,
            record(
                oct(24, 22, 5),
                rs.clone(),
                &route([
                    mmss(2, 31),
                    mmss(4, 12),
                    mmss(6, 40),
                    mmss(7, 58),
                    mmss(10, 51),
                    mmss(13, 2),
                    mmss(14, 30),
                ]),
                mmss(14, 32) + 481,
            ),
        );
        let mut slower = record(
            oct(22, 19, 30),
            rs.clone(),
            &route([
                mmss(2, 40),
                mmss(4, 45),
                mmss(6, 5),
                mmss(7, 12),
                mmss(9, 58),
                mmss(12, 30),
                mmss(14, 51),
            ]),
            mmss(14, 53) + 87,
        );
        // Fortress before bastion: the structures swap places in the route.
        slower.splits.swap(1, 2);
        slower.splits[1].kind = Some(SplitKind::Fortress);
        slower.splits[1].name = timeline(SplitKind::Fortress).into();
        slower.splits[2].kind = Some(SplitKind::Bastion);
        slower.splits[2].name = timeline(SplitKind::Bastion).into();
        add(Mode::Rsg, slower);
        let mut lan = record(
            oct(20, 23, 2),
            rs.clone(),
            &route([
                mmss(1, 50),
                mmss(3, 1),
                mmss(4, 40),
                mmss(5, 30),
                mmss(7, 45),
                mmss(9, 0),
                mmss(10, 2),
            ]),
            mmss(10, 4) + 300,
        );
        lan.open_lan_ms = Some(mmss(3, 0));
        add(Mode::Rsg, lan);
        // FSG: one finish (a set seed, as the FSG mod makes them).
        add(
            Mode::Fsg,
            record(
                oct(22, 17, 48),
                RunType::SetSeed,
                &route([
                    mmss(1, 12),
                    mmss(2, 40),
                    mmss(4, 31),
                    mmss(5, 20),
                    mmss(7, 12),
                    mmss(8, 30),
                    mmss(9, 49),
                ]),
                mmss(9, 51) + 337,
            ),
        );
        add(
            Mode::Fsg,
            record(
                oct(21, 16, 10),
                RunType::SetSeed,
                &reached(&[mmss(1, 20), mmss(2, 51)]),
                mmss(3, 30),
            ),
        );
    }
    // Resets that reached a split, and resets in the overworld.
    let resets: [(Mode, i64, &[u64]); 6] = [
        (Mode::Rsg, oct(25, 22, 10), &[mmss(2, 22), mmss(4, 51)]),
        (
            Mode::Rsg,
            oct(24, 21, 15),
            &[mmss(2, 44), mmss(4, 5), mmss(7, 30)],
        ),
        (
            Mode::Rsg,
            oct(23, 22, 31),
            &[mmss(2, 18), mmss(3, 57), mmss(6, 31), mmss(7, 49)],
        ),
        (Mode::Ranked, oct(24, 18, 2), &[mmss(3, 5)]),
        (Mode::Rsg, oct(21, 20, 44), &[mmss(1, 58)]),
        (
            Mode::Rsg,
            oct(19, 21, 0),
            &[
                mmss(2, 10),
                mmss(3, 40),
                mmss(5, 2),
                mmss(6, 1),
                mmss(8, 40),
            ],
        ),
    ];
    for (mode, date, points) in resets {
        add(
            mode,
            record(
                date,
                rs.clone(),
                &reached(points),
                points.last().unwrap() + 20_000,
            ),
        );
    }
    for i in 0..37u32 {
        let mode = if i % 5 == 0 { Mode::Ranked } else { Mode::Rsg };
        let date = oct(18 + i % 8, 17 + i % 5, i % 60);
        add(
            mode,
            record(date, rs.clone(), &[], 15_000 + u64::from(i) * 1_700),
        );
    }
    let modes: BTreeMap<Mode, RunHistory> = runs
        .into_iter()
        .map(|(m, r)| (m, RunHistory::new(r)))
        .collect();
    let files = modes.values().map(|h| h.run_count).sum();
    LocalStats {
        modes,
        files,
        scan_ms: 14,
        ..LocalStats::default()
    }
}

fn player(uuid: &str, nickname: &str, elo: i32) -> RankedPlayer {
    RankedPlayer {
        uuid: uuid.into(),
        nickname: nickname.into(),
        elo: Some(elo),
        rank: None,
        country: None,
    }
}

/// The sample Ranked profile: Diamond, 18 - 11 this season, three recent matches, not in the
/// running weekly race's top 33, 214th in the last one. `new_player`: placements not done yet.
fn sample_ranked(name: &str, age: Age, new_player: bool) -> RankedOverview {
    let season = if new_player {
        RankedRecord {
            played: 3,
            wins: 1,
            losses: 2,
            ..RankedRecord::default()
        }
    } else {
        RankedRecord {
            played: 31,
            wins: 18,
            losses: 11,
            forfeits: 2,
            completions: 14,
            best_time_ms: Some(mmss(12, 48) + 302),
            current_streak: 2,
            best_streak: 5,
            playtime_ms: 9 * 3_600_000,
        }
    };
    let profile = RankedProfile {
        uuid: "0f3b7c1e9a8d4e2b8c6a5d4e3f2a1b0c".into(),
        nickname: name.into(),
        country: None,
        elo: (!new_player).then_some(1642),
        rank: (!new_player).then_some(1873),
        tier: (!new_player).then_some(Tier::Diamond),
        season: season.clone(),
        all_time: season,
        season_highest_elo: (!new_player).then_some(1660),
        season_lowest_elo: (!new_player).then_some(1490),
        last_ranked_ms: Some(oct(25, 22, 50)),
        weekly_races: if new_player {
            vec![]
        } else {
            vec![
                WeeklyRaceResult {
                    race_id: 39,
                    time_ms: mmss(15, 2) + 11,
                    rank: 388,
                },
                WeeklyRaceResult {
                    race_id: 41,
                    time_ms: mmss(13, 51) + 90,
                    rank: 214,
                },
            ]
        },
    };
    let m =
        |id: u64, opp: RankedPlayer, won: bool, time: u64, change: i32, after: i32| RankedMatch {
            id,
            kind: MatchKind::Ranked,
            season: Some(12),
            date_ms: oct(25, 22, 50) - id as i64 * 1_800_000,
            category: Some("ANY".into()),
            opponent: Some(opp),
            players: 2,
            outcome: if won { Outcome::Won } else { Outcome::Lost },
            time_ms: time,
            forfeited: false,
            elo_change: (!new_player).then_some(change),
            elo_after: (!new_player).then_some(after),
        };
    let matches = vec![
        m(
            1,
            player("002", "Dracored", 1610),
            true,
            mmss(12, 48) + 302,
            14,
            1642,
        ),
        m(
            2,
            player("146", "Ninjatow91", 1655),
            false,
            mmss(13, 20) + 551,
            -11,
            1628,
        ),
        m(
            3,
            player("011", "Etherdoogcour", 1590),
            true,
            mmss(15, 2) + 117,
            12,
            1639,
        ),
    ];
    let leaderboard = (1..=33)
        .map(|r| WeeklyRaceEntry {
            rank: r,
            player: player(&format!("racer{r}"), &format!("Racer{r:02}"), 2000),
            time_ms: mmss(9, 0) + u64::from(r) * 4_000,
        })
        .collect();
    RankedOverview {
        profile: cached(profile, &age),
        matches: Some(cached(if new_player { vec![] } else { matches }, &age)),
        weekly_race: Some(cached(
            WeeklyRace {
                id: 42,
                ends_at_ms: now_ms() + 3 * 86_400_000,
                leaderboard,
            },
            &age,
        )),
        my_weekly_race: None,
        my_last_weekly_race: (!new_player).then(|| WeeklyRaceResult {
            race_id: 41,
            time_ms: mmss(13, 51) + 90,
            rank: 214,
        }),
        players_online: Some(cached(1243, &age)),
    }
}

fn sample_draftout(name: &str, age: Age, new_player: bool) -> Cached<DraftoutStats> {
    let mut profile = DraftoutProfile {
        uuid: "0f3b7c1e9a8d4e2b8c6a5d4e3f2a1b0c".into(),
        username: name.into(),
        elo: Some(1187),
        rank: Some(412),
        rank_name: "Guardian II".into(),
        rank_color: "#5BC0BE".into(),
        ranked: true,
        matches: 46,
        wins: 27,
        losses: 17,
        draws: 2,
        win_rate: 0.587,
        average_finish_ms: Some(mmss(11, 40)),
        peak_elo: Some(1240),
        best_streak: 6,
        fastest_win_ms: Some(mmss(7, 12)),
        forfeits: 3,
    };
    if new_player {
        // What the API answers before a first rated match (seen live): no elo, no rank band.
        profile = DraftoutProfile {
            elo: None,
            rank: None,
            rank_name: String::new(),
            rank_color: String::new(),
            ranked: false,
            matches: 0,
            wins: 0,
            losses: 0,
            draws: 0,
            win_rate: 0.0,
            average_finish_ms: None,
            peak_elo: None,
            best_streak: 0,
            fastest_win_ms: None,
            forfeits: 0,
            ..profile
        };
    }
    let matches = vec![DraftoutMatch {
        id: 1,
        queue: "competitive".into(),
        outcome: "finished".into(),
        completed_at_ms: oct(25, 20, 0),
        duration_ms: mmss(10, 21),
        won: true,
        score: 3,
        opponent: Some(("Dracored".into(), 1)),
        elo_change: Some(16),
        elo_after: Some(1187),
    }];
    let matches = if new_player { vec![] } else { matches };
    cached(DraftoutStats { profile, matches }, &age)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sample makes the numbers the screenshots show: PBs, the excluded LAN finish, sums of
    /// best and the resets before the Nether.
    #[test]
    fn the_sample_has_what_the_page_shows() {
        let l = sample_local(false);
        let rsg = l.mode(Mode::Rsg);
        assert_eq!(rsg.headline_pb().unwrap().record.igt_ms, mmss(14, 32) + 481);
        let any = rsg.category("ANY", &RunType::RandomSeed).unwrap();
        assert_eq!(any.excluded_completions, 1);
        assert!(any.sum_of_best_ms.unwrap() < mmss(14, 30));
        // FSG's PB is a set-seed run: the headline falls back to it.
        assert_eq!(
            l.mode(Mode::Fsg).headline_pb().unwrap().record.run_type,
            RunType::SetSeed
        );
        assert_eq!(l.mode(Mode::Ssg).run_count, 0);
        let new = sample_local(true);
        assert!(new.modes.values().all(|h| h.headline_pb().is_none()));
        assert!(new.mode(Mode::Rsg).run_count > 0);
    }
}
