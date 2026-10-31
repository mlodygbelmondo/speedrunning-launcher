//! One SpeedRunIGT record file, parsed.
//!
//! The format is `InGameTimerUtils.convertTimelineJson` in SpeedRunIGT 16.0+1.16.1 (MIT,
//! github.com/RedLime/SpeedRunIGT, tag `16.0`), written pretty-printed by Gson with nulls:
//!
//! ```json
//! { "mc_version": "1.16.1", "speedrunigt_version": "16.0+1.16.1", "category": "ANY",
//!   "run_type": "random_seed", "is_completed": false, "is_coop": false, "is_hardcore": false,
//!   "world_name": "Random Speedrun #12", "is_cheat_allowed": false, "default_gamemode": 0,
//!   "date": 1761480000000, "retimed_igt": 81234, "final_igt": 80000, "stats_igt": 0,
//!   "final_rta": 95000, "open_lan": null,
//!   "timelines": [ { "name": "enter_nether", "igt": 61000, "rta": 70000 } ],
//!   "advancements": { ... }, "stats": { ... } }
//! ```
//!
//! What the fields mean, from the source:
//! - `date` is `System.currentTimeMillis()` when the record string was last rebuilt (on each pause,
//!   leave or completion), so for a reset run it is roughly when the player left the world, not
//!   when the run started.
//! - `final_igt` is the tick-based IGT. `retimed_igt` adds the paused time back for random-seed
//!   Any% runs under 13 minutes (`RunCategories.anyPercentRetime`), the speedrun.com guideline;
//!   otherwise it equals `final_igt`. Both are milliseconds.
//! - For a completed run every time is frozen at the completion; for a reset run it is the time
//!   when the player left.
//! - `run_type` is `random_seed`, `set_seed` (the seed field was filled in) or `old_world` (a world
//!   the timer had no data for).
//! - There is no seed and no game directory in the file. The world's `level.dat` would have the
//!   seed, but MCSR Fairplay (in every legal 1.16.1 set) encrypts it: all 8 worlds of the real
//!   session had non-gzip `level.dat` files. We do not try to get around an anti-cheat measure, so
//!   runs have no seed.
use serde::{Deserialize, Serialize};

/// How the world was made, from `run_type`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunType {
    RandomSeed,
    SetSeed,
    OldWorld,
    /// A value a later SpeedRunIGT may add; kept so it is not mixed into another type's PB.
    Other(String),
}

impl RunType {
    fn parse(s: &str) -> RunType {
        match s {
            "random_seed" => RunType::RandomSeed,
            "set_seed" => RunType::SetSeed,
            "old_world" => RunType::OldWorld,
            other => RunType::Other(other.to_string()),
        }
    }
}

/// The timeline events that make the standard 1.16 splits. Names are the strings SpeedRunIGT
/// passes to `tryInsertNewTimeline`; each is recorded once per run, the first time it happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitKind {
    /// `enter_nether`: the client joined the Nether.
    Nether,
    /// `enter_bastion`: advancement `nether/find_bastion`.
    Bastion,
    /// `enter_fortress`: advancement `nether/find_fortress`.
    Fortress,
    /// `nether_travel`: first Nether -> Overworld trip carrying eyes, or pearls with rods/powder
    /// (`isEnoughTravel`). Speedrunners call it the blind travel.
    Blind,
    /// `enter_stronghold`: advancement `story/follow_ender_eye`.
    Stronghold,
    /// `enter_end`: the client joined the End.
    End,
    /// `kill_ender_dragon`. The run's finish is `Run::completed` with its final time, which for
    /// Any% comes slightly later (the credits).
    Dragon,
}

impl SplitKind {
    pub fn from_timeline(name: &str) -> Option<SplitKind> {
        Some(match name {
            "enter_nether" => SplitKind::Nether,
            "enter_bastion" => SplitKind::Bastion,
            "enter_fortress" => SplitKind::Fortress,
            "nether_travel" => SplitKind::Blind,
            "enter_stronghold" => SplitKind::Stronghold,
            "enter_end" => SplitKind::End,
            "kill_ender_dragon" => SplitKind::Dragon,
            _ => return None,
        })
    }
}

/// A timeline entry. `name` is kept raw (SpeedRunIGT also records `nether_travel_home`,
/// `nether_travel_blind`, `portal_no_N`, custom category goals...), `kind` when it is a standard
/// split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Split {
    pub name: String,
    pub kind: Option<SplitKind>,
    pub igt_ms: u64,
    pub rta_ms: u64,
}

/// A parsed record, before it is attributed to a mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub mc_version: String,
    pub igt_version: String,
    /// SpeedRunIGT category id: `ANY`, `HIGH`, `ALL_ADVANCEMENTS`, `CUSTOM`, ...
    pub category: String,
    pub run_type: RunType,
    pub completed: bool,
    pub coop: bool,
    pub hardcore: bool,
    /// `is_cheat_allowed`: cheats allowed when the record was written. Not enough on its own, see
    /// `eligible`.
    pub cheats: bool,
    /// `open_lan`: the RTA (ms) at which the world was opened to LAN, if it was.
    pub open_lan_ms: Option<u64>,
    pub world_name: String,
    /// Epoch ms of the last write (see the module docs).
    pub date_ms: i64,
    /// `retimed_igt`: the IGT to compare and show (equal to `final_igt` unless retimed).
    pub igt_ms: u64,
    /// `final_igt`, tick-based, without the retime.
    pub igt_raw_ms: u64,
    pub rta_ms: u64,
    /// In the order SpeedRunIGT recorded them (chronological).
    pub splits: Vec<Split>,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    mc_version: Option<String>,
    #[serde(default)]
    speedrunigt_version: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    run_type: Option<String>,
    is_completed: bool,
    #[serde(default)]
    is_coop: Option<bool>,
    #[serde(default)]
    is_hardcore: Option<bool>,
    #[serde(default)]
    is_cheat_allowed: Option<bool>,
    #[serde(default)]
    open_lan: Option<i64>,
    #[serde(default)]
    world_name: Option<String>,
    date: i64,
    #[serde(default)]
    retimed_igt: Option<i64>,
    final_igt: i64,
    final_rta: i64,
    #[serde(default)]
    timelines: Vec<RawTimeline>,
}

#[derive(Deserialize)]
struct RawTimeline {
    name: String,
    igt: i64,
    rta: i64,
}

/// Parses a record. `Err` carries a short reason for the skipped-files count; the caller never
/// fails a scan because of one file.
pub fn parse(bytes: &[u8]) -> Result<Record, String> {
    let raw: Raw = serde_json::from_slice(bytes).map_err(|e| format!("not a record: {e}"))?;
    let ms = |v: i64| v.max(0) as u64;
    let splits = raw
        .timelines
        .into_iter()
        .map(|t| Split {
            kind: SplitKind::from_timeline(&t.name),
            name: t.name,
            igt_ms: ms(t.igt),
            rta_ms: ms(t.rta),
        })
        .collect();
    Ok(Record {
        mc_version: raw.mc_version.unwrap_or_default(),
        igt_version: raw.speedrunigt_version.unwrap_or_default(),
        category: raw.category.unwrap_or_else(|| "unknown".into()),
        run_type: RunType::parse(raw.run_type.as_deref().unwrap_or("")),
        completed: raw.is_completed,
        coop: raw.is_coop.unwrap_or(false),
        hardcore: raw.is_hardcore.unwrap_or(false),
        cheats: raw.is_cheat_allowed.unwrap_or(false),
        open_lan_ms: raw.open_lan.map(ms),
        world_name: raw.world_name.unwrap_or_default(),
        date_ms: raw.date,
        igt_ms: ms(raw.retimed_igt.unwrap_or(raw.final_igt)),
        igt_raw_ms: ms(raw.final_igt),
        rta_ms: ms(raw.final_rta),
        splits,
    })
}

impl Record {
    /// True when the run may count for a PB, a best segment and the "reached" counts: no cheats,
    /// never opened to LAN, not co-op.
    ///
    /// `cheats` alone is not enough. SpeedRunIGT 16.0 overwrites the flag with the world's current
    /// state on every unpause (`InGameTimer.java:739`, `setCheatAvailable(areCheatsAllowed())`),
    /// and LAN cheats are not saved in `level.dat`, so a player who opens to LAN with cheats, quits
    /// and rejoins has `is_cheat_allowed: false` again. `open_lan` is a saved timer field and
    /// stays set. Opening to LAN without cheats is excluded too: it is not a normal solo run, and
    /// the record can't tell whether cheats were on at the time. Co-op runs are their own
    /// leaderboard, not a solo PB.
    pub fn eligible(&self) -> bool {
        !self.cheats && self.open_lan_ms.is_none() && !self.coop
    }

    /// IGT of the first split of `kind`, if the run reached it.
    pub fn split(&self, kind: SplitKind) -> Option<u64> {
        self.splits
            .iter()
            .find(|s| s.kind == Some(kind))
            .map(|s| s.igt_ms)
    }

    /// True when two files hold the same write of the same run: SpeedRunIGT writes the global
    /// record and the world's `record.json` from one string, so equal date, world and times
    /// mean one run.
    pub fn same_write(&self, other: &Record) -> bool {
        self.date_ms == other.date_ms
            && self.world_name == other.world_name
            && self.rta_ms == other.rta_ms
            && self.igt_raw_ms == other.igt_raw_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_completed_record() {
        let r = parse(include_bytes!(
            "../../tests/fixtures/records/completed.json"
        ))
        .unwrap();
        assert!(r.completed);
        assert_eq!(r.category, "ANY");
        assert_eq!(r.run_type, RunType::RandomSeed);
        assert_eq!(r.igt_ms, 612_345);
        assert_eq!(r.igt_raw_ms, 610_000);
        assert_eq!(r.split(SplitKind::Nether), Some(140_000));
        assert_eq!(r.split(SplitKind::Blind), Some(330_000));
        assert_eq!(r.split(SplitKind::Dragon), Some(600_000));
        // Raw names that are not standard splits stay, without a kind.
        assert!(
            r.splits
                .iter()
                .any(|s| s.name == "nether_travel_blind" && s.kind.is_none())
        );
    }

    #[test]
    fn missing_optional_fields_are_fine_but_times_are_required() {
        let ok = br#"{"is_completed":false,"date":5,"final_igt":10,"final_rta":20}"#;
        let r = parse(ok).unwrap();
        assert_eq!((r.igt_ms, r.category.as_str()), (10, "unknown"));
        assert_eq!(r.run_type, RunType::Other(String::new()));
        assert!(parse(br#"{"is_completed":false,"date":5}"#).is_err());
        assert!(parse(b"{not json").is_err());
        assert!(parse(b"[]").is_err());
    }

    #[test]
    fn lan_cheats_after_a_rejoin_are_not_eligible() {
        // LAN with cheats, then quit and rejoin: SpeedRunIGT reset the cheat flag and `open_lan`
        // stayed. Built from SpeedRunIGT's source, not played (the real session's LAN run never
        // rejoined, so its flag was still set).
        let r = parse(
            br#"{"is_completed":true,"date":5,"final_igt":95000,"final_rta":99000,
                "is_cheat_allowed":false,"open_lan":20383,"is_coop":false}"#,
        )
        .unwrap();
        assert_eq!((r.cheats, r.open_lan_ms), (false, Some(20_383)));
        assert!(!r.eligible());
        let coop = parse(
            br#"{"is_completed":true,"date":5,"final_igt":1,"final_rta":1,"is_coop":true,
                "open_lan":null}"#,
        )
        .unwrap();
        assert!(!coop.eligible());
        let clean = parse(
            br#"{"is_completed":true,"date":5,"final_igt":1,"final_rta":1,"open_lan":null,
                "is_cheat_allowed":false,"is_coop":false}"#,
        )
        .unwrap();
        assert!(clean.eligible());
    }

    #[test]
    fn negative_times_clamp_to_zero() {
        let r = parse(
            br#"{"is_completed":true,"date":5,"final_igt":-3,"final_rta":-1,"retimed_igt":-2}"#,
        )
        .unwrap();
        assert_eq!((r.igt_ms, r.igt_raw_ms, r.rta_ms), (0, 0, 0));
    }
}
