# srl-stats: notes

Read-only stats for the Stats page and Home: SpeedRunIGT run records, the MCSR Ranked API and the
Draftout API. The API and the "For the UI" section are at the top of `src/lib.rs`. This file holds
what was found, how it was checked, and what is still open. Everything below was run and seen on
2026-10-26 unless it says otherwise.

## Changes outside this crate

- `launcher/Cargo.toml`: `crates/srl-stats` added to the workspace members.
- `launcher/Cargo.lock`: the `srl-stats` entry. No new third-party crates.
- `srl-core`: no changes. `srl-app`, `srl-game`, `srl-mods`, `srl-tools`: untouched.
- Pandora Launcher: nothing adapted (it has no SpeedRunIGT or stats code). An NBT reader based
  on its decoder was written and then removed, see "Seeds".

## SpeedRunIGT 16.0+1.16.1

Source read at tag `16.0` of github.com/RedLime/SpeedRunIGT (MIT; commit 183b89e6, the version
the legal 1.16.1 set ships and MCSR Ranked 6.0.4 bundles). The details are in the doc comments of
`src/igt/mod.rs` and `src/igt/record.rs`. In short:

- Global records: `<user.home>/speedrunigt/records/<timer uuid>.json`, one per run, written when
  the player leaves the world, when the run completes and when the next world starts. Per world:
  `saves/<world>/speedrunigt/record.json`, the same JSON, also written on every pause.
  `<user.home>/speedrunigt/latest_world.json`: one line, `world_path`, `mods`, `version`,
  `mod_version`, `category`.
- Unreleased later commit b5d7fe9 moves the global folder to `$XDG_DATA_HOME/speedrunigt`,
  `~/.local/share/speedrunigt` or `%APPDATA%\speedrunigt`. `IgtSources::for_user` reads both. Not
  tested with a real build after 16.0, because none is released.
- Record JSON fields: `category`, `run_type` (`random_seed`, `set_seed`, `old_world`),
  `is_completed`, `world_name`, `date` (the last write, epoch ms), `final_igt`, `retimed_igt`,
  `final_rta`, `timelines[{name, igt, rta}]`, plus some we ignore. There is no seed and no game
  directory.
- `retimed_igt` = `final_igt` plus pause time, for random-seed Any% under 13 minutes (the
  speedrun.com guideline). In the real session: 248 040 raw, 261 116 retimed.
- Split timeline names: `enter_nether`, `enter_bastion`, `enter_fortress`, `nether_travel` (blind:
  the first Nether to Overworld trip carrying eyes, or pearls with rods; also `nether_travel_home`
  or `nether_travel_blind`), `enter_stronghold`, `enter_end`, `kill_ender_dragon`. Each is recorded
  once per run. The finish is `is_completed` with the final time.
- MCSR Ranked 6.0.4: the strings in its `InGameTimerPrevent` mixin show that it cancels
  `writeRecordFile` during online matches, so Ranked matches leave no records. This is from the
  bytecode strings only; no online match was played, because an offline account can't play Ranked.

## Real files: what was run

On Linux, with `SRL_HOME=~/.cache/srl-dev/m3-stats` and no GPUI:

1. `srl-game install rsg` (9.1 s), then `examples/install_mods.rs rsg`: 22 mods, among them
   SpeedRunIGT 16.0, Atum 2.7.2, SeedQueue 1.8 and MCSR Fairplay 1.2.1.
2. `srl-game launch rsg --jvm-arg -Duser.home=$SRL_HOME/javahome`, inside `Xvfb :77` with
   `env -u WAYLAND_DISPLAY SDL_VIDEO_DRIVER=x11 DISPLAY=:77`. The `user.home` override kept
   SpeedRunIGT's global folder out of the real home.
3. With xdotool: Atum's button on the title screen, then F6 (Atum's reset key) several times. In
   world #4: Open to LAN with cheats, `/execute in` to the Nether, the End and back,
   `/advancement grant` for bastion, fortress and stronghold, `/kill` on the dragon, an
   `end_portal` block, then the credits. Then two more resets. Finally Esc (a pause, which writes
   the world record) and SIGTERM to the game.
4. Result: 8 worlds, 7 global records and `latest_world.json`. They were copied to
   `tests/fixtures/real/` (record JSON only, no world files; the `world_path` there has a
   placeholder in place of the real folder).

What the files showed:

- For every world that was left, the global and world records are byte-identical (same `date` and
  times). That is attribution rule 2.
- World #8, whose game was killed, has a world record but no global one. That is rule 1, and with
  an older global copy it would be rule 3 (tested with a modified copy).
- Run #4 completed: `is_completed: true`, timelines nether, bastion, fortress, stronghold and end.
  `/execute` teleports don't trigger `nether_travel`, which needs `changeDimension`, a real portal
  trip, and `/kill` on the dragon didn't record `kill_ender_dragon`. The blind split is therefore
  covered only by the hand-built fixture `tests/fixtures/records/completed.json`, which follows the
  source format and is labelled as such in the test.
- `latest_world.json` pointed to world #4, the last world that logged an event (resets without
  advancements log none).
- Seeds: every `level.dat` (8 worlds plus their `level.dat_old`) is not gzip. MCSR Fairplay
  encrypts it (the worlds have `mcsrfairplay/worldDataKey.txt` and `*.enc`). Fairplay is in every
  legal 1.16.1 set, so the launcher cannot read seeds, and it does not try to get around an
  anti-cheat measure. Runs have no seed field.
- `examples/show_stats.rs` on those files: `rsg: 8 runs, 1 completed, PB None`. The PB is `None`
  because the completed run had cheats on. With an empty index, the first scan parsed 15 files and
  the second parsed 0; both took under 1 ms (debug build).

## Attribution rule

The full rule is in `src/igt/mod.rs`. In short:

1. A world record belongs to its instance's mode.
2. A global record that is the same write as a world record belongs to that world's mode.
3. A global record that a unique same-name world record continues belongs to that mode (same
   category and run type, timeline prefix, no time going down), if there is evidence it is the
   same run (a shared non-empty timeline, or written after the world folder was created) and no
   other global qualifies for the same world.
4. Otherwise, the mode remembered from an earlier scan (`meta/stats/attribution.json`).
5. Otherwise, unattributed.

There is no guessing from run type, mods or world names. Tests cover these cases: a name clash with
another launcher (not attributed), deleted worlds (they keep their mode), a newer world copy
(it merges), foreign resets with our world's Atum name (not merged, counted once, not RSG after
the world is deleted), two candidates for one world (none merged), and remembered modes surviving
an index format change, a broken index and an unreadable records folder.

## Speed

`examples/bench_scan.rs`, release build, synthetic records with the real format (a global and a
world record per run, 2 % completed), files in the OS cache:

| runs | files | cold (parse all) | warm (index only) | after 10 new runs | index size |
|---|---|---|---|---|---|
| 1000 | 2000 | 17 ms | 10 ms | 11 ms | 1.1 MB |
| 5000 | 10000 | 91 ms | 58 ms | 80 ms | 5.6 MB |
| 5000, after the review fixes | 10000 | 96 ms | 61 ms | 69 ms | 5.6 MB |

The warm scan is mostly loading the JSON index. Not measured: a cold disk, and Windows (where a
`stat` costs more).

## MCSR Ranked API (checked live)

From docs.mcsrranked.com and its `openapi.yaml`, then verified live: `/leaderboard`, `/users/{name}`,
`/users/{name}/matches`, `/weekly-race` and `/live`.

- Limit: 500 requests per 10 minutes. The live `ratelimit` headers confirm it:
  `"500-in-10min"; r=497; t=598`. Our limiter allows 120 per 10 minutes and honours a 429's wait.
- An unknown user gets HTTP 400 `{"status":"error","data":{"error":"User is not exists."}}`,
  which becomes `NotFound`.
- `changes[].eloRate` is the rating before the match. The newest match had 2350 and -25, and the
  profile showed 2325. The live test also showed `elo_after` of the newest match equal to the
  profile's elo (2026).
- Recent matches use `type=2` (ranked only) and `excludedecay=true`. The first live player's last
  20 matches were all private rooms, with no elo and mostly no winner.
- `/live` `players`: 903, then 916 and 917 later. `/weekly-race`: race #109, a leaderboard of 30
  entries, later 32 and 33 (the top only). The profile's `weeklyRaces` lists finished races only
  (fainrada, #1 of race 109, had `[83, 84, 108]`), so `my_weekly_race` comes from the race board by
  UUID (`None` = not in the top N) and `my_last_weekly_race` from the profile.
- An unknown route answers 404 `{"status":"error","data":{"error":"not found"}}`; it is
  `Http(404)`, not `NotFound`. Only the 400 "User is not exists." is `NotFound`.
- TTLs: profile and matches 2 min, weekly race 10 min, online count 1 min. `Refresh::Now` still
  reuses anything younger than 10 s.
- Cache: `meta/ranked/{user,matches}-<lowercase name>.json`, `weekly-race.json`, `live.json`.
  When offline, the cached value comes back with `refresh_error` and its age. Tested against a
  closed port with a cache 2 h old.
- Tier thresholds: 0, 600, 900, 1200, 1500 and 2000 for Coal, Iron, Gold, Emerald, Diamond and
  Netherite. `elo == None` means placements.
- The live test (`cargo test -p srl-stats --test live -- --ignored --nocapture`) passed. It took a
  player from the middle of the leaderboard (cmfn1: elo 2026, rank 76, Netherite, 116 W and 89 L
  of 213) and got 20 ranked matches with opponents, results, times and elo changes, the weekly
  race and the online count. A second call came from the cache, and an unknown name gave
  `NotFound`.

## Draftout

`GET draftoutmc.com/api/stats/{username}` has per-player data (community spec v0.2.0, checked
live): elo, leaderboard rank, rank band name and colour, the W/L/D record, peak elo, best streak,
fastest win, forfeits and 20 recent matches with both scores and elo changes. So the basics are
in (`src/draftout.rs`).

- An unknown player gets 404 `{"error":"player_not_found"}`. The spec says 200 with
  `player: null`; both cases are handled. An unknown route gets 404 `text/html`, which is
  `Http(404)` and keeps the cached stats (offline test `a_moved_route_is_not_a_missing_player`).
- No rate limit is documented and the answers are `no-store`, so the client makes one request per
  refresh, keeps a 5 min TTL and allows at most 30 requests per 10 minutes.
- The live test passed: c_nut, elo 1749, Phantom II, rank 4, 155 W, 85 L and 2 D, 20 recent
  matches.

## Review fixes (branch m3-stats, 2026-10-26)

Each finding of the review was checked before fixing; all 11 were real.

| Finding | Checked by | Fix |
|---|---|---|
| `my_weekly_race` always `None` for the running race (medium) | live: `/weekly-race` #109 has fainrada #1, `/users/fainrada` `weeklyRaces` = 83, 84, 108 | found on the race board by UUID; `my_last_weekly_race` from the profile; copy: "not in the top N" |
| LAN cheats + rejoin can be a PB (medium) | SpeedRunIGT 16.0 source: `InGameTimer.java:739` overwrites the flag on unpause; `open_lan` is written to the record | `Record::open_lan_ms`, `Record::eligible()` (no cheats, no LAN, not co-op) for PB, best segments and reached; index format 2 |
| Rule 3 not one-to-one (medium) | code: rule-3 matches were not counted per world | evidence (shared timeline or date after the world folder's creation) and one global per world; the reviewer's probe is a test |
| Remembered modes only in the index (medium) | code: version/parse failure discarded them, an unlistable folder pruned them | `meta/stats/attribution.json` with its own version; pruned only after a clean listing; `LocalStats::unreadable` |
| Any 404 = `NotFound` (medium) | live curl: Draftout unknown route 404 HTML, Ranked unknown route 404 JSON "not found" | `NotFound` only for Draftout `player_not_found` and Ranked 400 "not exist" |
| `Refresh::Never` returns old data unmarked (low) | code | `refresh_error = Some(NotRefreshed)` past the TTL |
| `reached` vs `completions` (low) | real fixture: completions 1, reached empty | `excluded`, `excluded_completions`; docs say reached is eligible runs |
| Cancelled `local()` frees the scan lock (low) | test: with the guard released early, the next scan parsed 3000 of 3000 | owned guard moved into the blocking task; unique temp names in `write_atomic` |
| Future `fetched_at` counts as fresh (low) | code | more than 5 s ahead = stale |
| Huge `Retry-After` panics and poisons (low) | test against a local 429 server | clamped to 1 h, `checked_add`, poisoned lock recovered |
| `mode()` clones (low) | code | returns `&RunHistory` |

API changes from the fixes (no users yet): `LocalStats::mode` returns `&RunHistory`;
new `RankedOverview::my_last_weekly_race`, `ApiError::NotRefreshed`, `Record::open_lan_ms`,
`Record::eligible`, `CategoryStats::{excluded, excluded_completions}`, `LocalStats::unreadable`,
`http::MAX_RETRY_AFTER`, `cache::CLOCK_SLACK`. A v1 index (dev machines only) is re-parsed and its
remembered modes are not carried over; a world still on disk is attributed again.

Not done: rule 3 still accepts a lone same-name reset from another launcher written after our
world folder was created (two launchers used at the same time); the LAN-rejoin case was not
played in game (a record built from the source format is the test).

## Tests and checks run

- After the review fixes: `cargo test -p srl-stats --all-targets`: 39 unit tests and 2 offline
  integration tests pass; the 2 live tests pass with `--ignored` (the Ranked one now also checks
  that the race's #1 gets `my_weekly_race` = race 109, rank 1, 385 600 ms, and
  `my_last_weekly_race` = race 108). `cargo clippy --all-targets`, Linux and
  `x86_64-pc-windows-gnu`: no warnings. `cargo fmt --check`: clean.
- Under wine64 (in Xvfb, no desktop window): 38 of 38 Windows unit tests (the chmod test is
  Unix-only) and both offline integration tests pass. Wine refused concurrent renames onto one
  file ("Access denied"); callers ignore a failed cache write, and the test allows it on Windows.
- `examples/show_stats` on the real dev install (`~/.cache/srl-dev/m3-stats`): rsg 8 runs,
  1 completed, excluded 1/1, PB none; second scan parsed 0; `attribution.json` written (338 B);
  `--ranked fainrada` gave the running race #109 rank 1 and last race #108 rank 17.
- Before the fixes: `cargo test -p srl-stats --all-targets`: 25 unit tests and 1 integration
  test (offline) passed.
- With `--target x86_64-pc-windows-gnu`: `cargo clippy --all-targets` is clean and the tests and
  examples build. Under wine64 (no display), the Windows unit tests passed (25 of 25) and so did
  the offline integration test.
- Fixtures: `tests/fixtures/real/` holds the real session's files. `tests/fixtures/api/` holds
  live answers with every name, UUID, country, VOD and stream URL replaced. The script that
  replaced them is not in git.

## Open issues

- The app has to keep `StatsSettings` (`ranked_nickname`, `draftout_username`) in its settings
  and ask for them; that is the Settings page's work (M2 owns `srl-app`).
- The app should call `Stats::local()` when a game exits. Rule 4 (deleted worlds keep their mode)
  only knows runs that some scan saw.
- The app should show `LocalStats::unreadable` and `CategoryStats::excluded_completions` as small
  notes (wording in the "For the UI" section of `src/lib.rs`).
- Not run for real: the blind (`nether_travel`) and dragon splits, FSG/SSG/Ranked-practice
  instances (same mod, same files, same attribution code path), a cold-disk scan, Windows paths on
  a real Windows machine (the unit tests ran under wine).
- The index stores every parsed record (about 1.1 KB per file). For tens of thousands of resets
  it could store only the fields the histories use.
- Draftout's API is unofficial and may change without notice.
- Records from other launchers are counted as `unattributed`. The page could offer to show them
  ("other SpeedRunIGT runs on this computer"), but they are not mode stats.
