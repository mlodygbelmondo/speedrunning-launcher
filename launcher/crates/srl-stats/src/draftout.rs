//! Draftout's public stats (`https://draftoutmc.com/api/stats`). There are no official docs; the
//! community OpenAPI description (memerson12.github.io/draftout-api-spec, v0.2.0, "inferred from
//! live responses on 2026-09-14") matches what the API answered on 2026-10-26:
//!
//! - `GET /api/stats/{username}` (competitive queue by default): `player` (`elo`, leaderboard
//!   `rank`, `rankName` like "Guardian II", `rankColor`), `record` (matches, wins, losses, draws,
//!   win rate, average finish time), `aggregate` (peak elo, best streak, fastest win, forfeits) and
//!   the first page of `matches` (20 per page, newest first; each with both participants' score,
//!   `won`, `eloBefore`, `eloChange`, `eloAfter`).
//! - An unknown player answers 404 `{"error":"player_not_found"}` (the spec says 200 with
//!   `player: null`; both are handled). Any other 404 (an unknown route answers 404 with an HTML
//!   page) is `Http(404)`, not `NotFound`, so a moved route keeps the cached stats on screen.
//!
//! No rate limit is documented and the answers are `Cache-Control: no-store`, so this is gentle:
//! one request per refresh, a 5-minute TTL, at most 30 requests per 10 minutes.
use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    Refresh,
    cache::{self, Cached},
    http::{self, ApiError, Limiter},
};

pub const DRAFTOUT_API: &str = "https://draftoutmc.com";
pub const TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftoutProfile {
    pub uuid: String,
    pub username: String,
    /// `None` before the player's first rated match: the API answers `null`, not a start value.
    pub elo: Option<i32>,
    /// Leaderboard place.
    pub rank: Option<u32>,
    /// Draftout's own rank band ("Silverfish I" to "Warden") and its colour (`#RRGGBB`); empty
    /// without an elo.
    pub rank_name: String,
    pub rank_color: String,
    /// False while the player is not on the ranked leaderboard yet.
    pub ranked: bool,
    pub matches: u32,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub win_rate: f64,
    pub average_finish_ms: Option<u64>,
    pub peak_elo: Option<i32>,
    pub best_streak: u32,
    pub fastest_win_ms: Option<u64>,
    pub forfeits: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftoutMatch {
    pub id: u64,
    /// `competitive`, `quick_play` or `lobby`.
    pub queue: String,
    /// `finished`, `forfeited`, `draw_by_vote`, `draw` or `cancelled`.
    pub outcome: String,
    pub completed_at_ms: i64,
    pub duration_ms: u64,
    pub won: bool,
    pub score: u32,
    /// The other player of a 1v1, with their score.
    pub opponent: Option<(String, u32)>,
    pub elo_change: Option<i32>,
    pub elo_after: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DraftoutStats {
    pub profile: DraftoutProfile,
    /// Newest first, competitive queue.
    pub matches: Vec<DraftoutMatch>,
}

pub struct DraftoutClient {
    client: reqwest::Client,
    base: String,
    dir: PathBuf,
    limiter: Limiter,
}

impl DraftoutClient {
    /// `dir` is the cache folder (`Paths::meta()/draftout`).
    pub fn new(client: reqwest::Client, dir: PathBuf) -> Self {
        Self::with_base(client, dir, DRAFTOUT_API)
    }

    pub fn with_base(client: reqwest::Client, dir: PathBuf, base: &str) -> Self {
        DraftoutClient {
            client,
            base: base.trim_end_matches('/').to_string(),
            dir,
            limiter: Limiter::new(30, Duration::from_secs(600)),
        }
    }

    /// The player's stats and recent competitive matches, cached like `RankedClient::overview`.
    pub async fn stats(
        &self,
        username: &str,
        refresh: Refresh,
    ) -> Result<Cached<DraftoutStats>, ApiError> {
        if !http::is_lookup_name(username) {
            return Err(ApiError::InvalidName);
        }
        let file = self
            .dir
            .join(format!("player-{}.json", username.to_ascii_lowercase()));
        let url = format!("{}/api/stats/{username}", self.base);
        cache::cached_fetch(&file, TTL, refresh, async {
            match http::get(&self.client, &self.limiter, &url).await? {
                (200, body) => parse(&body),
                (404, body) => Err(not_found_or(&body)),
                (code, _) => Err(ApiError::Http(code)),
            }
        })
        .await
    }

    /// From the cache only, for the first paint.
    pub async fn cached_stats(&self, username: &str) -> Option<Cached<DraftoutStats>> {
        self.stats(username, Refresh::Never).await.ok()
    }
}

/// A 404 is "no such player" only when the body says so (`{"error":"player_not_found"}`, seen
/// live). Any other 404 (a moved route answers with an HTML page) is `Http(404)`, so the page
/// keeps showing the cached stats instead of claiming the player does not exist.
fn not_found_or(body: &[u8]) -> ApiError {
    #[derive(Deserialize)]
    struct Error {
        error: String,
    }
    match serde_json::from_slice::<Error>(body) {
        Ok(e) if e.error == "player_not_found" => ApiError::NotFound,
        _ => ApiError::Http(404),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    player: Option<RawPlayer>,
    record: RawRecord,
    aggregate: RawAggregate,
    #[serde(default)]
    matches: Vec<RawMatch>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPlayer {
    uuid: String,
    username: String,
    // All null for a player without a rated match (seen live on 2026-10-26).
    elo: Option<i32>,
    rank: Option<u32>,
    #[serde(default)]
    rank_name: Option<String>,
    #[serde(default)]
    rank_color: Option<String>,
    #[serde(default)]
    ranked: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRecord {
    matches: u32,
    wins: u32,
    losses: u32,
    draws: u32,
    win_rate: f64,
    average_finish_time: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAggregate {
    peak_elo: Option<i32>,
    best_streak: u32,
    fastest_win_ms: Option<u64>,
    forfeit_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMatch {
    id: u64,
    match_type: String,
    outcome: String,
    completed_at: i64,
    duration_ms: u64,
    participants: Vec<RawParticipant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawParticipant {
    uuid: String,
    username: String,
    won: bool,
    score: u32,
    elo_change: Option<i32>,
    elo_after: Option<i32>,
}

fn parse(body: &[u8]) -> Result<DraftoutStats, ApiError> {
    let raw: Raw =
        serde_json::from_slice(body).map_err(|e| ApiError::BadResponse(e.to_string()))?;
    let p = raw.player.ok_or(ApiError::NotFound)?;
    let me = p.uuid.clone();
    let matches = raw
        .matches
        .into_iter()
        .filter_map(|m| {
            let mine = m.participants.iter().find(|x| x.uuid == me)?;
            let opponent = (m.participants.len() == 2)
                .then(|| m.participants.iter().find(|x| x.uuid != me))
                .flatten()
                .map(|o| (o.username.clone(), o.score));
            Some(DraftoutMatch {
                id: m.id,
                queue: m.match_type,
                outcome: m.outcome,
                completed_at_ms: m.completed_at,
                duration_ms: m.duration_ms,
                won: mine.won,
                score: mine.score,
                elo_change: mine.elo_change,
                elo_after: mine.elo_after,
                opponent,
            })
        })
        .collect();
    Ok(DraftoutStats {
        profile: DraftoutProfile {
            uuid: p.uuid,
            username: p.username,
            elo: p.elo,
            rank: p.rank,
            rank_name: p.rank_name.unwrap_or_default(),
            rank_color: p.rank_color.unwrap_or_default(),
            ranked: p.ranked,
            matches: raw.record.matches,
            wins: raw.record.wins,
            losses: raw.record.losses,
            draws: raw.record.draws,
            win_rate: raw.record.win_rate,
            average_finish_ms: raw.record.average_finish_time.map(|t| t.max(0.0) as u64),
            peak_elo: raw.aggregate.peak_elo,
            best_streak: raw.aggregate.best_streak,
            fastest_win_ms: raw.aggregate.fastest_win_ms,
            forfeits: raw.aggregate.forfeit_count,
        },
        matches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_from_a_live_answer() {
        // A live answer of 2026-10-26 with names and UUIDs replaced.
        let s = parse(include_bytes!("../tests/fixtures/api/draftout-player.json")).unwrap();
        let p = &s.profile;
        assert_eq!(p.username, "SampleRunner");
        assert_eq!((p.elo, p.rank), (Some(1915), Some(1)));
        assert_eq!(p.rank_name, "Guardian II");
        assert_eq!((p.matches, p.wins, p.losses, p.draws), (183, 160, 22, 1));
        assert_eq!(p.peak_elo, Some(1998));
        assert_eq!(s.matches.len(), 4);
        let m = &s.matches[0];
        assert!(m.won);
        assert_eq!(
            (m.score, m.elo_change, m.elo_after),
            (13, Some(7), Some(1915))
        );
        assert_eq!(m.opponent.as_ref().unwrap().1, 7);
        assert_eq!(m.queue, "competitive");
    }

    #[test]
    fn a_player_without_rated_matches_has_no_elo() {
        // A live answer of 2026-10-26 (name and UUID replaced): elo and rank band are null.
        let body = br#"{"player":{"uuid":"00000000-0000-0000-0000-000000000000",
            "username":"NewRunner","elo":null,"rd":null,"ranked":false,"rank":null,
            "rankName":null,"rankColor":null,"eraId":2},
            "record":{"matches":0,"completedMatches":0,"wins":0,"losses":0,"draws":0,
            "winRate":0,"averageFinishTime":null,"averageGoals":null},
            "aggregate":{"peakElo":null,"bestStreak":0,"fastestWinMs":null,"forfeitCount":0},
            "matches":[],"page":1,"totalPages":1,"filter":"competitive","eraId":2,"eras":[]}"#;
        let p = parse(body).unwrap().profile;
        assert_eq!((p.elo, p.rank, p.ranked), (None, None, false));
        assert_eq!((p.rank_name.as_str(), p.matches), ("", 0));
    }

    #[test]
    fn player_null_is_not_found() {
        let body =
            br#"{"player":null,"record":{"matches":0,"completedMatches":0,"wins":0,"losses":0,
            "draws":0,"winRate":0,"averageFinishTime":null,"averageGoals":null},
            "aggregate":{"peakElo":null,"bestStreak":0,"fastestWinMs":null,"forfeitCount":0},
            "matches":[],"page":1,"totalPages":1,"filter":"competitive","eraId":2,"eras":[]}"#;
        assert_eq!(parse(body).unwrap_err(), ApiError::NotFound);
        assert!(matches!(
            parse(br#"{"error":"player_not_found"}"#),
            Err(ApiError::BadResponse(_))
        ));
    }

    #[test]
    fn only_a_player_not_found_404_is_not_found() {
        // Both bodies as seen live on 2026-10-26: an unknown player, and an unknown route.
        assert_eq!(
            not_found_or(br#"{"error":"player_not_found"}"#),
            ApiError::NotFound
        );
        assert_eq!(
            not_found_or(b"<!DOCTYPE html><html><body>404</body></html>"),
            ApiError::Http(404)
        );
        assert_eq!(
            not_found_or(br#"{"error":"route_not_found"}"#),
            ApiError::Http(404)
        );
    }
}
