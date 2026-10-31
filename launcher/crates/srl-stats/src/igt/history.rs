//! A mode's runs turned into the numbers the Stats page and Home show: PB per category, recent
//! runs, splits, best segments, sum of best and the counts.
use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use srl_core::Mode;

use super::record::{Record, RunType, SplitKind};

/// Where a run's world is, when it still exists inside one of our instances.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRef {
    pub mode: Mode,
    /// Folder name under `instances/<mode>/saves/`.
    pub dir_name: String,
    pub path: PathBuf,
}

/// One run: a record, the mode it belongs to and what else we know about its world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// Stable id: `g:<timer uuid>` for a run with a global record, else `w:<mode>/<world dir>`.
    pub id: String,
    /// `None`: not from one of our instances (another launcher, or a world deleted before the
    /// launcher ever saw it). See the attribution rule in `igt/mod.rs`.
    pub mode: Option<Mode>,
    pub record: Record,
    pub world: Option<WorldRef>,
}

/// The points of the standard 1.16 Any% route, in order. Bastion and fortress are taken in either
/// order, so they become the first and second structure, the way MCSR Ranked and PaceMan count them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Segment {
    Nether,
    FirstStructure,
    SecondStructure,
    Blind,
    Stronghold,
    End,
    Finish,
}

impl Segment {
    pub const ALL: [Segment; 7] = [
        Segment::Nether,
        Segment::FirstStructure,
        Segment::SecondStructure,
        Segment::Blind,
        Segment::Stronghold,
        Segment::End,
        Segment::Finish,
    ];
}

/// Cumulative IGT at each route point a run reached (`None` where it did not). Uses the
/// tick-based IGT throughout, finish included (`igt_raw_ms`): the timeline times are tick-based,
/// and mixing in the retimed final time would make the last segment longer than it was played.
pub fn route_points(r: &Record) -> [Option<u64>; 7] {
    let bastion = r.split(SplitKind::Bastion);
    let fortress = r.split(SplitKind::Fortress);
    let (first, second) = match (bastion, fortress) {
        (Some(a), Some(b)) => (Some(a.min(b)), Some(a.max(b))),
        (a, b) => (a.or(b), None),
    };
    [
        r.split(SplitKind::Nether),
        first,
        second,
        r.split(SplitKind::Blind),
        r.split(SplitKind::Stronghold),
        r.split(SplitKind::End),
        r.completed.then_some(r.igt_raw_ms),
    ]
}

/// Length of each segment in this run: from the previous route point (0 for the first) to this
/// one. A segment whose start or end the run never reached has no length.
pub fn segment_times(r: &Record) -> [Option<u64>; 7] {
    let points = route_points(r);
    let mut out = [None; 7];
    for i in 0..7 {
        let start = if i == 0 { Some(0) } else { points[i - 1] };
        if let (Some(s), Some(e)) = (start, points[i])
            && e >= s
        {
            out[i] = Some(e - s);
        }
    }
    out
}

/// Runs are grouped by category and run type: a set-seed Any% must not become the PB of a
/// random-seed Any%.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CategoryKey {
    pub category: String,
    pub run_type: RunType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BestSegment {
    pub segment: Segment,
    pub ms: u64,
    /// `Run::id` of the run it came from.
    pub run_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryStats {
    pub key: CategoryKey,
    /// Every run, eligible or not.
    pub runs: u32,
    /// Every completed run, eligible or not.
    pub completions: u32,
    /// Runs left out of `pb`, `best_segments` and `reached` because they are not
    /// `Record::eligible` (cheats, opened to LAN, co-op); `excluded_completions` of them finished.
    pub excluded: u32,
    pub excluded_completions: u32,
    /// The fastest completed eligible run, by `Record::igt_ms`.
    pub pb: Option<Run>,
    /// Best length of each route segment over eligible runs, in route order; a segment no run has
    /// both ends of is missing.
    pub best_segments: Vec<BestSegment>,
    /// Sum of the best segments, only when every segment has one (else it would be meaningless).
    pub sum_of_best_ms: Option<u64>,
    /// How many eligible runs reached each route point (Finish = `completions -
    /// excluded_completions`).
    pub reached: BTreeMap<Segment, u32>,
}

/// Everything about one mode's (or the unattributed) runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunHistory {
    /// Newest first, by `Record::date_ms`.
    pub runs: Vec<Run>,
    /// Most played first.
    pub categories: Vec<CategoryStats>,
    pub run_count: u32,
    pub completion_count: u32,
}

impl RunHistory {
    pub fn new(mut runs: Vec<Run>) -> Self {
        runs.sort_by(|a, b| {
            b.record
                .date_ms
                .cmp(&a.record.date_ms)
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut groups: BTreeMap<CategoryKey, Vec<&Run>> = BTreeMap::new();
        for run in &runs {
            let key = CategoryKey {
                category: run.record.category.clone(),
                run_type: run.record.run_type.clone(),
            };
            groups.entry(key).or_default().push(run);
        }
        let mut categories: Vec<CategoryStats> = groups
            .into_iter()
            .map(|(key, runs)| category_stats(key, &runs))
            .collect();
        categories.sort_by(|a, b| b.runs.cmp(&a.runs).then_with(|| a.key.cmp(&b.key)));
        let completion_count = runs.iter().filter(|r| r.record.completed).count() as u32;
        RunHistory {
            run_count: runs.len() as u32,
            completion_count,
            runs,
            categories,
        }
    }

    /// The `n` newest runs.
    pub fn recent(&self, n: usize) -> &[Run] {
        &self.runs[..n.min(self.runs.len())]
    }

    pub fn category(&self, category: &str, run_type: &RunType) -> Option<&CategoryStats> {
        self.categories
            .iter()
            .find(|c| c.key.category == category && &c.key.run_type == run_type)
    }

    /// The headline PB for a mode tile: random-seed Any% if there is one, else the fastest
    /// completed Any% of any run type.
    pub fn headline_pb(&self) -> Option<&Run> {
        self.category("ANY", &RunType::RandomSeed)
            .and_then(|c| c.pb.as_ref())
            .or_else(|| {
                self.categories
                    .iter()
                    .filter(|c| c.key.category == "ANY")
                    .filter_map(|c| c.pb.as_ref())
                    .min_by_key(|r| r.record.igt_ms)
            })
    }

    /// Runs whose last write is at or after `since_ms` (epoch ms), e.g. local midnight for
    /// "today". Runs are sorted newest first, so this stops at the first older one.
    pub fn runs_since(&self, since_ms: i64) -> impl Iterator<Item = &Run> {
        self.runs
            .iter()
            .take_while(move |r| r.record.date_ms >= since_ms)
    }
}

fn category_stats(key: CategoryKey, runs: &[&Run]) -> CategoryStats {
    let completions = runs.iter().filter(|r| r.record.completed).count() as u32;
    let (eligible, excluded): (Vec<&Run>, Vec<&Run>) =
        runs.iter().copied().partition(|r| r.record.eligible());
    let excluded_completions = excluded.iter().filter(|r| r.record.completed).count() as u32;
    let pb = eligible
        .iter()
        .filter(|r| r.record.completed)
        .min_by_key(|r| (r.record.igt_ms, r.record.date_ms))
        .map(|r| (*r).clone());
    // (length, date, run id): on a tie the earlier run keeps the best, as a split timer would.
    let mut best: [Option<(u64, i64, &str)>; 7] = [None; 7];
    let mut reached = BTreeMap::new();
    for run in &eligible {
        for (i, seg) in segment_times(&run.record).into_iter().enumerate() {
            let date = run.record.date_ms;
            if let Some(ms) = seg
                && best[i].is_none_or(|(b, d, _)| (ms, date) < (b, d))
            {
                best[i] = Some((ms, date, &run.id));
            }
        }
        for (i, point) in route_points(&run.record).into_iter().enumerate() {
            if point.is_some() {
                *reached.entry(Segment::ALL[i]).or_insert(0) += 1;
            }
        }
    }
    let sum_of_best_ms = best
        .iter()
        .try_fold(0u64, |acc, b| b.map(|(ms, _, _)| acc + ms));
    let best_segments = best
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            b.map(|(ms, _, id)| BestSegment {
                segment: Segment::ALL[i],
                ms,
                run_id: id.to_string(),
            })
        })
        .collect();
    CategoryStats {
        key,
        runs: runs.len() as u32,
        completions,
        excluded: excluded.len() as u32,
        excluded_completions,
        pb,
        best_segments,
        sum_of_best_ms,
        reached,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::igt::record::Split;

    fn rec(date: i64, completed: bool, igt: u64, splits: &[(&str, u64)]) -> Record {
        Record {
            mc_version: "1.16.1".into(),
            igt_version: "16.0+1.16.1".into(),
            category: "ANY".into(),
            run_type: RunType::RandomSeed,
            completed,
            coop: false,
            hardcore: false,
            cheats: false,
            open_lan_ms: None,
            world_name: format!("Random Speedrun #{date}"),
            date_ms: date,
            igt_ms: igt,
            igt_raw_ms: igt,
            rta_ms: igt + 1000,
            splits: splits
                .iter()
                .map(|(n, t)| Split {
                    name: n.to_string(),
                    kind: SplitKind::from_timeline(n),
                    igt_ms: *t,
                    rta_ms: *t,
                })
                .collect(),
        }
    }

    fn run(id: &str, r: Record) -> Run {
        Run {
            id: id.into(),
            mode: Some(Mode::Rsg),
            record: r,
            world: None,
        }
    }

    const FULL: &[(&str, u64)] = &[
        ("enter_nether", 100),
        ("enter_bastion", 150),
        ("enter_fortress", 250),
        ("nether_travel", 300),
        ("enter_stronghold", 400),
        ("enter_end", 450),
    ];

    #[test]
    fn structures_in_either_order() {
        let a = rec(
            1,
            false,
            0,
            &[
                ("enter_nether", 10),
                ("enter_fortress", 20),
                ("enter_bastion", 50),
            ],
        );
        assert_eq!(&route_points(&a)[..3], &[Some(10), Some(20), Some(50)]);
        let one = rec(
            1,
            false,
            0,
            &[
                ("enter_nether", 10),
                ("enter_fortress", 20),
                ("nether_travel", 40),
            ],
        );
        assert_eq!(
            &route_points(&one)[..4],
            &[Some(10), Some(20), None, Some(40)]
        );
        // The blind segment needs the second structure as its start, so it has no length here.
        assert_eq!(segment_times(&one)[3], None);
    }

    #[test]
    fn pb_best_segments_and_sum_of_best() {
        let runs = vec![
            run("a", rec(1000, true, 600, FULL)),
            // Faster Nether and End segments, but a slower finish.
            run(
                "b",
                rec(
                    2000,
                    true,
                    700,
                    &[
                        ("enter_nether", 90),
                        ("enter_bastion", 150),
                        ("enter_fortress", 260),
                        ("nether_travel", 320),
                        ("enter_stronghold", 425),
                        ("enter_end", 460),
                    ],
                ),
            ),
            // A reset after the Nether with the best first structure.
            run(
                "c",
                rec(
                    3000,
                    false,
                    200,
                    &[("enter_nether", 120), ("enter_bastion", 140)],
                ),
            ),
        ];
        let h = RunHistory::new(runs);
        assert_eq!(h.run_count, 3);
        assert_eq!(h.completion_count, 2);
        assert_eq!(h.runs[0].id, "c", "newest first");
        let any = h.category("ANY", &RunType::RandomSeed).unwrap();
        assert_eq!(any.pb.as_ref().unwrap().id, "a");
        let segs: Vec<(Segment, u64, &str)> = any
            .best_segments
            .iter()
            .map(|b| (b.segment, b.ms, b.run_id.as_str()))
            .collect();
        assert_eq!(
            segs,
            vec![
                (Segment::Nether, 90, "b"),
                (Segment::FirstStructure, 20, "c"),
                (Segment::SecondStructure, 100, "a"),
                (Segment::Blind, 50, "a"),
                (Segment::Stronghold, 100, "a"),
                (Segment::End, 35, "b"),
                (Segment::Finish, 150, "a"),
            ]
        );
        assert_eq!(
            any.sum_of_best_ms,
            Some(90 + 20 + 100 + 50 + 100 + 35 + 150)
        );
        assert_eq!(any.reached[&Segment::Nether], 3);
        assert_eq!(any.reached[&Segment::Finish], 2);
        assert_eq!(h.headline_pb().unwrap().id, "a");
        assert_eq!(h.runs_since(2000).count(), 2);
        assert_eq!(h.recent(1).len(), 1);
    }

    #[test]
    fn no_sum_of_best_without_every_segment_and_no_pb_with_cheats() {
        let mut cheated = rec(1, true, 100, FULL);
        cheated.cheats = true;
        let h = RunHistory::new(vec![
            run("x", cheated),
            run("y", rec(2, false, 50, &[("enter_nether", 40)])),
        ]);
        let any = &h.categories[0];
        assert!(any.pb.is_none());
        assert_eq!(any.completions, 1);
        assert_eq!((any.excluded, any.excluded_completions), (1, 1));
        assert_eq!(any.best_segments.len(), 1);
        assert_eq!(any.sum_of_best_ms, None);
        assert_eq!(any.reached.get(&Segment::Finish), None);
        assert!(h.headline_pb().is_none());
    }

    #[test]
    fn lan_and_coop_runs_are_never_a_pb_or_a_best_segment() {
        // LAN cheats with a rejoin: the cheat flag is false again, `open_lan` stays.
        let mut lan = rec(1, true, 95, FULL);
        lan.open_lan_ms = Some(20_383);
        let mut coop = rec(2, true, 90, FULL);
        coop.coop = true;
        let h = RunHistory::new(vec![
            run("lan", lan),
            run("coop", coop),
            run("clean", rec(3, true, 900, FULL)),
        ]);
        let any = h.category("ANY", &RunType::RandomSeed).unwrap();
        assert_eq!(any.pb.as_ref().unwrap().id, "clean");
        assert!(any.best_segments.iter().all(|b| b.run_id == "clean"));
        assert_eq!((any.completions, any.excluded_completions), (3, 2));
        assert_eq!(
            any.reached[&Segment::Finish],
            any.completions - any.excluded_completions
        );
    }

    #[test]
    fn set_seed_runs_do_not_mix_with_random_seed() {
        let mut ss = rec(1, true, 100, FULL);
        ss.run_type = RunType::SetSeed;
        let h = RunHistory::new(vec![run("s", ss), run("r", rec(2, true, 900, FULL))]);
        assert_eq!(h.categories.len(), 2);
        assert_eq!(h.headline_pb().unwrap().id, "r", "random seed first");
        assert_eq!(
            h.category("ANY", &RunType::SetSeed)
                .unwrap()
                .pb
                .as_ref()
                .unwrap()
                .id,
            "s"
        );
    }
}
