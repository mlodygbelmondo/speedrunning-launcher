//! MCSR Ranked public API (<https://docs.mcsrranked.com>, OpenAPI at `docs.mcsrranked.com/openapi.yaml`,
//! read and checked live on 2026-10-26).
//!
//! What we use, all public and without a key:
//! - `GET /users/{name}`: elo (`eloRate`), leaderboard rank (`eloRank`), `statistics.season` and
//!   `.total` (each `{ranked, casual}`), `seasonResult` (`last`, `highest`, `lowest`),
//!   `weeklyRaces` (`[{id, time, rank}]`, the player's own results), `timestamp.lastRanked`.
//!   An unknown name answers 400 `{"status":"error","data":{"error":"User is not exists."}}`;
//!   only that is `NotFound`. An unknown route answers 404, which stays `Http(404)`.
//! - `GET /users/{name}/matches?type=2&count=N&excludedecay=true`: ranked matches (type 2; casual
//!   and private rooms have no elo and, in rooms, often no winner), newest first. `result.uuid` is the
//!   winner (`null` = draw), `result.time` the winning time, or when the match ended for a
//!   forfeit; `changes[].eloRate` is the rating BEFORE the match (checked: 2350 - 25 = the
//!   profile's 2325 after the newest match).
//! - `GET /weekly-race`: the current race: `id`, `endsAt` (epoch s), `leaderboard` (the top only:
//!   30, 32 and 33 entries in three checks). The profile's `weeklyRaces` lists finished races
//!   only, so the player's result in the running race comes from this leaderboard, by UUID, and
//!   only when they are in its top.
//! - `GET /live`: `players` is the number of players online.
//!
//! Limits: 500 requests per 10 minutes per IP (the `ratelimit` header confirms it); answers are
//! cached 5 s on their side. One full refresh here is 4 requests, and TTLs keep a normal session to
//! a few a minute. Our own limiter allows 120 per 10 minutes and honours 429s.
use std::{path::PathBuf, time::Duration};

use futures::future::join4;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    Refresh,
    cache::{self, Cached},
    http::{self, ApiError, Limiter},
};

pub const RANKED_API: &str = "https://api.mcsrranked.com";

/// How long each part stays fresh. Elo and matches change after a match (a match is ~10-20 min),
/// the weekly race leaderboard moves slowly, the online count is a "live" number.
pub const PROFILE_TTL: Duration = Duration::from_secs(120);
pub const MATCHES_TTL: Duration = Duration::from_secs(120);
pub const WEEKLY_RACE_TTL: Duration = Duration::from_secs(600);
pub const LIVE_TTL: Duration = Duration::from_secs(60);
/// Recent matches fetched per refresh.
pub const MATCH_COUNT: u32 = 20;

/// The rank tiers by elo, as on mcsrranked.com: Coal from 0, Iron 600, Gold 900, Emerald 1200,
/// Diamond 1500, Netherite 2000.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Coal,
    Iron,
    Gold,
    Emerald,
    Diamond,
    Netherite,
}

impl Tier {
    pub const ALL: [Tier; 6] = [
        Tier::Coal,
        Tier::Iron,
        Tier::Gold,
        Tier::Emerald,
        Tier::Diamond,
        Tier::Netherite,
    ];

    pub fn min_elo(self) -> i32 {
        match self {
            Tier::Coal => 0,
            Tier::Iron => 600,
            Tier::Gold => 900,
            Tier::Emerald => 1200,
            Tier::Diamond => 1500,
            Tier::Netherite => 2000,
        }
    }

    pub fn from_elo(elo: i32) -> Tier {
        Tier::ALL
            .into_iter()
            .rev()
            .find(|t| elo >= t.min_elo())
            .unwrap_or(Tier::Coal)
    }

    /// The next tier and the elo still needed for it; `None` at Netherite.
    pub fn next(elo: i32) -> Option<(Tier, i32)> {
        let i = Tier::ALL.iter().position(|&t| t == Tier::from_elo(elo))?;
        let next = *Tier::ALL.get(i + 1)?;
        Some((next, next.min_elo() - elo))
    }

    pub fn name(self) -> &'static str {
        match self {
            Tier::Coal => "Coal",
            Tier::Iron => "Iron",
            Tier::Gold => "Gold",
            Tier::Emerald => "Emerald",
            Tier::Diamond => "Diamond",
            Tier::Netherite => "Netherite",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedRecord {
    pub played: u32,
    pub wins: u32,
    pub losses: u32,
    pub forfeits: u32,
    pub completions: u32,
    /// Fastest completion, ms.
    pub best_time_ms: Option<u64>,
    pub current_streak: u32,
    pub best_streak: u32,
    pub playtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyRaceResult {
    pub race_id: u32,
    pub time_ms: u64,
    pub rank: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedProfile {
    /// Without dashes, as the API gives it.
    pub uuid: String,
    pub nickname: String,
    pub country: Option<String>,
    /// `None` until the player finished the placement matches.
    pub elo: Option<i32>,
    pub rank: Option<u32>,
    pub tier: Option<Tier>,
    /// This season, ranked matches only (casual numbers are left out: Ranked is what the page is
    /// about).
    pub season: RankedRecord,
    pub all_time: RankedRecord,
    pub season_highest_elo: Option<i32>,
    pub season_lowest_elo: Option<i32>,
    pub last_ranked_ms: Option<i64>,
    /// The player's own results in finished weekly races, newest race last (as the API orders
    /// them). The running race is not in it; see `RankedOverview::my_weekly_race`.
    pub weekly_races: Vec<WeeklyRaceResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedPlayer {
    pub uuid: String,
    pub nickname: String,
    pub elo: Option<i32>,
    pub rank: Option<u32>,
    pub country: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    Casual,
    Ranked,
    PrivateRoom,
    Event,
    Other(i32),
}

impl MatchKind {
    fn from_api(n: i32) -> Self {
        match n {
            1 => MatchKind::Casual,
            2 => MatchKind::Ranked,
            3 => MatchKind::PrivateRoom,
            4 => MatchKind::Event,
            n => MatchKind::Other(n),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Won,
    Lost,
    /// No winner: a draw, or a private room nobody finished.
    Draw,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedMatch {
    pub id: u64,
    pub kind: MatchKind,
    pub season: Option<u32>,
    pub date_ms: i64,
    pub category: Option<String>,
    /// The other player of a 1v1; `None` in rooms with more or fewer players.
    pub opponent: Option<RankedPlayer>,
    pub players: u32,
    pub outcome: Outcome,
    /// The winner's time, or when the match ended if `forfeited`.
    pub time_ms: u64,
    /// The match ended without a completion (someone forfeited).
    pub forfeited: bool,
    /// Our elo change; `None` for placements, casual and private matches.
    pub elo_change: Option<i32>,
    /// Our elo after the match (`changes[].eloRate` is before it).
    pub elo_after: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyRaceEntry {
    pub rank: u32,
    pub player: RankedPlayer,
    pub time_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeeklyRace {
    pub id: u32,
    pub ends_at_ms: i64,
    /// The leaderboard as the API gives it: the top entries only (30 and 32 in two checks).
    pub leaderboard: Vec<WeeklyRaceEntry>,
}

/// Everything the Ranked part of the Stats page shows.
#[derive(Clone, Debug, PartialEq)]
pub struct RankedOverview {
    pub profile: Cached<RankedProfile>,
    /// Ranked matches (type 2) only, newest first, decay matches left out (they are not games).
    pub matches: Option<Cached<Vec<RankedMatch>>>,
    pub weekly_race: Option<Cached<WeeklyRace>>,
    /// The player's place and time in the running `weekly_race`, found on its leaderboard. That
    /// board is the top only, so `None` means "not in the top `leaderboard.len()`" (or not run):
    /// the API has no way to tell which.
    pub my_weekly_race: Option<WeeklyRaceResult>,
    /// The player's result in the last finished race they ran (from their profile, which lists
    /// finished races only).
    pub my_last_weekly_race: Option<WeeklyRaceResult>,
    pub players_online: Option<Cached<u32>>,
}

/// The API client. Cheap to share behind an `Arc`.
pub struct RankedClient {
    client: reqwest::Client,
    base: String,
    dir: PathBuf,
    limiter: Limiter,
}

impl RankedClient {
    /// `dir` is the cache folder (`Paths::meta()/ranked`).
    pub fn new(client: reqwest::Client, dir: PathBuf) -> Self {
        Self::with_base(client, dir, RANKED_API)
    }

    /// Another server: the alternate `https://mcsrranked.com/api`, or a test server.
    pub fn with_base(client: reqwest::Client, dir: PathBuf, base: &str) -> Self {
        RankedClient {
            client,
            base: base.trim_end_matches('/').to_string(),
            dir,
            limiter: Limiter::new(120, Duration::from_secs(600)),
        }
    }

    /// Profile, recent matches, the weekly race and the online count, each from the cache while
    /// fresh, else from the API. Only the profile is required: `Err` when it is not in the cache
    /// and cannot be fetched, or the player does not exist. Every other failure leaves that part
    /// `None` or stale with `refresh_error` set.
    pub async fn overview(
        &self,
        nickname: &str,
        refresh: Refresh,
    ) -> Result<RankedOverview, ApiError> {
        if !http::is_lookup_name(nickname) {
            return Err(ApiError::InvalidName);
        }
        let key = nickname.to_ascii_lowercase();
        let profile = self.part(
            format!("user-{key}.json"),
            format!("/users/{nickname}"),
            PROFILE_TTL,
            refresh,
            parse_profile,
        );
        let matches = self.part(
            format!("matches-{key}.json"),
            format!("/users/{nickname}/matches?type=2&count={MATCH_COUNT}&excludedecay=true"),
            MATCHES_TTL,
            refresh,
            envelope::<Vec<ApiMatch>>,
        );
        let race = self.part(
            "weekly-race.json".into(),
            "/weekly-race".into(),
            WEEKLY_RACE_TTL,
            refresh,
            parse_weekly_race,
        );
        let live = self.part("live.json".into(), "/live".into(), LIVE_TTL, refresh, |b| {
            envelope::<ApiLive>(b).map(|l| l.players)
        });
        let (profile, matches, race, live) = join4(profile, matches, race, live).await;
        let profile = profile?;
        let me = profile.value.uuid.clone();
        let matches = matches
            .ok()
            .map(|m| m.map(|list| list.into_iter().map(|x| x.into_match(&me)).collect()));
        let race = race.ok();
        let (my_weekly_race, my_last_weekly_race) =
            my_weekly_results(&profile.value, race.as_ref().map(|r| &r.value));
        Ok(RankedOverview {
            profile,
            matches,
            weekly_race: race,
            my_weekly_race,
            my_last_weekly_race,
            players_online: live.ok(),
        })
    }

    /// The overview from the cache only, without touching the network: for the first paint of
    /// the page, before `overview` returns. `None` if the profile was never fetched.
    pub async fn cached_overview(&self, nickname: &str) -> Option<RankedOverview> {
        self.overview(nickname, Refresh::Never).await.ok()
    }

    /// One cached part (see `cache::cached_fetch`).
    async fn part<T, F>(
        &self,
        file: String,
        path: String,
        ttl: Duration,
        refresh: Refresh,
        parse: F,
    ) -> Result<Cached<T>, ApiError>
    where
        T: Serialize + DeserializeOwned + Sync,
        F: FnOnce(&[u8]) -> Result<T, ApiError>,
    {
        let url = format!("{}{path}", self.base);
        cache::cached_fetch(&self.dir.join(file), ttl, refresh, async {
            match http::get(&self.client, &self.limiter, &url).await? {
                (200, body) => parse(&body),
                (400, body) => Err(not_found_or(&body)),
                (code, _) => Err(ApiError::Http(code)),
            }
        })
        .await
    }
}

/// The player's result in the running race and in the last finished one.
///
/// The profile's `weeklyRaces` lists finished races only (checked live on 2026-10-26: the #1 of
/// race 109 had only 83, 84 and 108 there), so the running race's result comes from the race's
/// leaderboard, by UUID. That board is the top only (30 to 33 entries seen), so `None` there means
/// "not in the top N", not "has not run it". The profile is still checked for the running race's
/// id, in case the API starts listing it. The last finished race is the profile's highest id below
/// the running one (the highest id at all when the running race is unknown).
fn my_weekly_results(
    profile: &RankedProfile,
    race: Option<&WeeklyRace>,
) -> (Option<WeeklyRaceResult>, Option<WeeklyRaceResult>) {
    let same = |a: &str| {
        a.replace('-', "")
            .eq_ignore_ascii_case(&profile.uuid.replace('-', ""))
    };
    let current = race.and_then(|r| {
        r.leaderboard
            .iter()
            .find(|e| same(&e.player.uuid))
            .map(|e| WeeklyRaceResult {
                race_id: r.id,
                time_ms: e.time_ms,
                rank: e.rank,
            })
            .or_else(|| {
                profile
                    .weekly_races
                    .iter()
                    .find(|w| w.race_id == r.id)
                    .cloned()
            })
    });
    let last = profile
        .weekly_races
        .iter()
        .filter(|w| race.is_none_or(|r| w.race_id < r.id))
        .max_by_key(|w| w.race_id)
        .cloned();
    (current, last)
}

/// A 400 is "no such player" only when the API says the user does not exist (`"User is not
/// exists."`, seen live); any other 400 is a parameter problem. A 404 is never `NotFound`: the API
/// answers an unknown route with 404 `{"status":"error","data":{"error":"not found"}}` (seen live),
/// and a moved route must keep the cached stats on screen, not claim the player is gone.
fn not_found_or(body: &[u8]) -> ApiError {
    #[derive(Deserialize)]
    struct Error {
        error: String,
    }
    match serde_json::from_slice::<ApiEnvelope<Error>>(body) {
        Ok(ApiEnvelope {
            status,
            data: Some(Error { error }),
        }) if status == "error" && error.to_ascii_lowercase().contains("not exist") => {
            ApiError::NotFound
        }
        _ => ApiError::Http(400),
    }
}

#[derive(Deserialize)]
struct ApiEnvelope<T> {
    status: String,
    data: Option<T>,
}

fn envelope<T: DeserializeOwned>(body: &[u8]) -> Result<T, ApiError> {
    let e: ApiEnvelope<T> =
        serde_json::from_slice(body).map_err(|e| ApiError::BadResponse(e.to_string()))?;
    if e.status != "success" {
        return Err(ApiError::BadResponse(format!("status {}", e.status)));
    }
    e.data
        .ok_or_else(|| ApiError::BadResponse("no data".into()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiUser {
    uuid: String,
    nickname: String,
    #[serde(default)]
    country: Option<String>,
    elo_rate: Option<i32>,
    elo_rank: Option<u32>,
    #[serde(default)]
    statistics: Option<ApiStatistics>,
    #[serde(default)]
    season_result: Option<ApiSeasonResult>,
    #[serde(default)]
    weekly_races: Vec<ApiWeeklyResult>,
    #[serde(default)]
    timestamp: Option<ApiTimestamp>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiPlayer {
    uuid: String,
    nickname: String,
    #[serde(default)]
    elo_rate: Option<i32>,
    #[serde(default)]
    elo_rank: Option<u32>,
    #[serde(default)]
    country: Option<String>,
}

impl From<ApiPlayer> for RankedPlayer {
    fn from(p: ApiPlayer) -> Self {
        RankedPlayer {
            uuid: p.uuid,
            nickname: p.nickname,
            elo: p.elo_rate,
            rank: p.elo_rank,
            country: p.country,
        }
    }
}

#[derive(Deserialize, Default)]
struct ApiStatistics {
    #[serde(default)]
    season: ApiStatMap,
    #[serde(default)]
    total: ApiStatMap,
}

/// `{"wins": {"ranked": 5, "casual": 0}, ...}`
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ApiStatMap {
    best_time: ApiPair,
    highest_win_streak: ApiPair,
    current_win_streak: ApiPair,
    played_matches: ApiPair,
    playtime: ApiPair,
    forfeits: ApiPair,
    completions: ApiPair,
    wins: ApiPair,
    loses: ApiPair,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApiPair {
    ranked: Option<i64>,
}

impl ApiStatMap {
    fn record(&self) -> RankedRecord {
        let n = |p: &ApiPair| p.ranked.unwrap_or(0).max(0) as u32;
        RankedRecord {
            played: n(&self.played_matches),
            wins: n(&self.wins),
            losses: n(&self.loses),
            forfeits: n(&self.forfeits),
            completions: n(&self.completions),
            best_time_ms: self.best_time.ranked.filter(|&t| t > 0).map(|t| t as u64),
            current_streak: n(&self.current_win_streak),
            best_streak: n(&self.highest_win_streak),
            playtime_ms: self.playtime.ranked.unwrap_or(0).max(0) as u64,
        }
    }
}

#[derive(Deserialize)]
struct ApiSeasonResult {
    highest: Option<i32>,
    lowest: Option<i32>,
}

#[derive(Deserialize)]
struct ApiWeeklyResult {
    id: u32,
    time: u64,
    rank: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiTimestamp {
    last_ranked: Option<i64>,
}

fn parse_profile(body: &[u8]) -> Result<RankedProfile, ApiError> {
    let u: ApiUser = envelope(body)?;
    let stats = u.statistics.unwrap_or_default();
    Ok(RankedProfile {
        tier: u.elo_rate.map(Tier::from_elo),
        uuid: u.uuid,
        nickname: u.nickname,
        country: u.country,
        elo: u.elo_rate,
        rank: u.elo_rank,
        season: stats.season.record(),
        all_time: stats.total.record(),
        season_highest_elo: u.season_result.as_ref().and_then(|s| s.highest),
        season_lowest_elo: u.season_result.as_ref().and_then(|s| s.lowest),
        last_ranked_ms: u
            .timestamp
            .and_then(|t| t.last_ranked)
            .filter(|&s| s > 0)
            .map(|s| s * 1000),
        weekly_races: u
            .weekly_races
            .into_iter()
            .map(|w| WeeklyRaceResult {
                race_id: w.id,
                time_ms: w.time,
                rank: w.rank,
            })
            .collect(),
    })
}

/// A match as the API gives it; cached in this form so the outcome is worked out against the
/// profile's UUID when read.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMatch {
    id: u64,
    #[serde(rename = "type")]
    kind: i32,
    #[serde(default)]
    season: Option<u32>,
    date: i64,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    players: Vec<ApiMatchPlayer>,
    result: ApiResult,
    #[serde(default)]
    forfeited: bool,
    #[serde(default)]
    changes: Vec<ApiChange>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMatchPlayer {
    uuid: String,
    nickname: String,
    #[serde(default)]
    elo_rate: Option<i32>,
    #[serde(default)]
    elo_rank: Option<u32>,
    #[serde(default)]
    country: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ApiResult {
    uuid: Option<String>,
    #[serde(default)]
    time: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiChange {
    uuid: String,
    change: Option<i32>,
    elo_rate: Option<i32>,
}

impl ApiMatch {
    fn into_match(self, me: &str) -> RankedMatch {
        let same = |a: &str| {
            a.replace('-', "")
                .eq_ignore_ascii_case(&me.replace('-', ""))
        };
        let outcome = match &self.result.uuid {
            None => Outcome::Draw,
            Some(w) if same(w) => Outcome::Won,
            Some(_) => Outcome::Lost,
        };
        let mine = self.changes.iter().find(|c| same(&c.uuid));
        let elo_change = mine.and_then(|c| c.change);
        let elo_after = mine.and_then(|c| Some(c.elo_rate? + c.change?));
        let players = self.players.len() as u32;
        let opponent = if players == 2 {
            self.players
                .into_iter()
                .find(|p| !same(&p.uuid))
                .map(|p| RankedPlayer {
                    uuid: p.uuid,
                    nickname: p.nickname,
                    elo: p.elo_rate,
                    rank: p.elo_rank,
                    country: p.country,
                })
        } else {
            None
        };
        RankedMatch {
            id: self.id,
            kind: MatchKind::from_api(self.kind),
            season: self.season,
            date_ms: self.date * 1000,
            category: self.category,
            opponent,
            players,
            outcome,
            time_ms: self.result.time,
            forfeited: self.forfeited,
            elo_change,
            elo_after,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiWeeklyRace {
    id: u32,
    ends_at: i64,
    #[serde(default)]
    leaderboard: Vec<ApiWeeklyEntry>,
}

#[derive(Deserialize)]
struct ApiWeeklyEntry {
    rank: u32,
    player: ApiPlayer,
    time: u64,
}

fn parse_weekly_race(body: &[u8]) -> Result<WeeklyRace, ApiError> {
    let r: ApiWeeklyRace = envelope(body)?;
    Ok(WeeklyRace {
        id: r.id,
        ends_at_ms: r.ends_at * 1000,
        leaderboard: r
            .leaderboard
            .into_iter()
            .map(|e| WeeklyRaceEntry {
                rank: e.rank,
                player: e.player.into(),
                time_ms: e.time,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct ApiLive {
    players: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &[u8] = include_bytes!("../tests/fixtures/api/ranked-user.json");
    const MATCHES: &[u8] = include_bytes!("../tests/fixtures/api/ranked-matches.json");
    const RACE: &[u8] = include_bytes!("../tests/fixtures/api/ranked-weekly-race.json");
    const LIVE: &[u8] = include_bytes!("../tests/fixtures/api/ranked-live.json");

    #[test]
    fn tiers() {
        assert_eq!(Tier::from_elo(0), Tier::Coal);
        assert_eq!(Tier::from_elo(-5), Tier::Coal);
        assert_eq!(Tier::from_elo(599), Tier::Coal);
        assert_eq!(Tier::from_elo(600), Tier::Iron);
        assert_eq!(Tier::from_elo(1199), Tier::Gold);
        assert_eq!(Tier::from_elo(1200), Tier::Emerald);
        assert_eq!(Tier::from_elo(1500), Tier::Diamond);
        assert_eq!(Tier::from_elo(2476), Tier::Netherite);
        assert_eq!(Tier::next(1450), Some((Tier::Diamond, 50)));
        assert_eq!(Tier::next(2000), None);
    }

    #[test]
    fn profile_from_a_live_answer() {
        // tests/fixtures/api: live answers of 2026-10-26, names and UUIDs replaced.
        let p = parse_profile(USER).unwrap();
        assert_eq!(p.nickname, "SampleRunner");
        assert_eq!(
            (p.elo, p.rank, p.tier),
            (Some(2325), Some(11), Some(Tier::Netherite))
        );
        assert_eq!(p.season.played, 954);
        assert_eq!((p.season.wins, p.season.losses), (546, 398));
        assert_eq!(p.season.best_time_ms, Some(388_989));
        assert_eq!(
            (p.season_highest_elo, p.season_lowest_elo),
            (Some(2437), Some(1673))
        );
        assert_eq!(p.weekly_races.len(), 5);
        assert!(p.last_ranked_ms.unwrap() > 1_700_000_000_000);
    }

    #[test]
    fn matches_from_a_live_answer() {
        let me = parse_profile(USER).unwrap().uuid;
        let list: Vec<ApiMatch> = envelope(MATCHES).unwrap();
        let m: Vec<RankedMatch> = list.into_iter().map(|x| x.into_match(&me)).collect();
        assert_eq!(m.len(), 5);
        // A ranked loss by forfeit: -25, from 2350 to 2325 (the profile's elo now).
        assert_eq!(m[0].outcome, Outcome::Lost);
        assert!(m[0].forfeited);
        assert_eq!((m[0].elo_change, m[0].elo_after), (Some(-25), Some(2325)));
        assert_eq!(m[0].opponent.as_ref().unwrap().nickname, "Player1");
        assert_eq!(m[0].kind, MatchKind::Ranked);
        assert_eq!(m[1].outcome, Outcome::Won);
        assert_eq!((m[1].time_ms, m[1].elo_change), (919_156, Some(12)));
        // A private room without a winner and without elo.
        assert_eq!(m[3].kind, MatchKind::PrivateRoom);
        assert_eq!((m[3].outcome, m[3].elo_change), (Outcome::Draw, None));
        assert!(m[0].date_ms > m[1].date_ms, "newest first");
    }

    #[test]
    fn weekly_race_and_live() {
        let r = parse_weekly_race(RACE).unwrap();
        assert_eq!(r.id, 109);
        assert_eq!(r.leaderboard[0].rank, 1);
        assert_eq!(r.leaderboard[0].time_ms, 385_600);
        assert!(r.ends_at_ms > 1_700_000_000_000);
        assert_eq!(envelope::<ApiLive>(LIVE).unwrap().players, 903);
    }

    #[test]
    fn weekly_results_come_from_the_board_for_the_running_race() {
        // As seen live: the race's #1 has no entry for the running race in their profile.
        let race = parse_weekly_race(RACE).unwrap();
        let mut p = parse_profile(USER).unwrap();
        p.uuid = race.leaderboard[0].player.uuid.clone();
        assert!(p.weekly_races.iter().all(|w| w.race_id != race.id));
        let (now, last) = my_weekly_results(&p, Some(&race));
        assert_eq!(
            now,
            Some(WeeklyRaceResult {
                race_id: 109,
                time_ms: 385_600,
                rank: 1
            })
        );
        assert_eq!(
            last.map(|w| w.race_id),
            Some(28),
            "the newest finished race"
        );

        // Not on the board (the board is the top only): no current result, last still there.
        let p = parse_profile(USER).unwrap();
        let (now, last) = my_weekly_results(&p, Some(&race));
        assert_eq!((now, last.map(|w| w.rank)), (None, Some(7)));

        // A profile that lists the running race (in case the API changes) is used too, and the
        // last finished race is then the one before it. UUIDs match with or without dashes.
        let mut p = parse_profile(USER).unwrap();
        p.weekly_races.push(WeeklyRaceResult {
            race_id: 109,
            time_ms: 500_000,
            rank: 40,
        });
        let (now, last) = my_weekly_results(&p, Some(&race));
        assert_eq!(now.map(|w| w.rank), Some(40));
        assert_eq!(last.map(|w| w.race_id), Some(28));
        let mut p = parse_profile(USER).unwrap();
        p.uuid = "00000004-0000-0000-0000-000000000000".into();
        assert_eq!(
            my_weekly_results(&p, Some(&race)).0.map(|w| w.rank),
            Some(1)
        );
        // Without the race (not fetched), only the last finished one.
        let (now, last) = my_weekly_results(&p, None);
        assert_eq!((now, last.map(|w| w.race_id)), (None, Some(28)));
    }

    #[test]
    fn errors() {
        assert_eq!(
            not_found_or(br#"{"status":"error","data":{"error":"User is not exists."}}"#),
            ApiError::NotFound
        );
        assert_eq!(not_found_or(b"<html>"), ApiError::Http(400));
        assert_eq!(
            not_found_or(br#"{"status":"error","data":{"error":"Invalid parameter"}}"#),
            ApiError::Http(400)
        );
        assert!(matches!(
            envelope::<ApiLive>(b"{}"),
            Err(ApiError::BadResponse(_))
        ));
    }
}
