# srl-practice: notes (M3 stream "practice", 2026-10-26)

One click installs a practice map, with credit, verified, into a separate Practice instance; the
player finds it in Singleplayer. The API and the "For the UI" section are at the top of
`src/lib.rs`. This file has the decisions, the data and what was checked.

## What is in the crate

| File | What |
|---|---|
| `data/catalogue.json` | The install catalogue (embedded with `include_str!`). |
| `tools/refresh_catalogue.py` | Re-pins sha512, size, the world folders in each zip and their DataVersion; `--latest` reports newer GitHub releases. Run with `python3 -I`. |
| `src/catalogue.rs` | Types and validation (author's own GitHub release only, 128-hex sha512, 1.16.1 only, credit, category, carry-over paths inside the world). |
| `src/profile.rs` | `PRACTICE`: the practice instance profile (mods by mcsr-meta modid, with reasons). |
| `src/archive.rs` | Zip reading: the three layouts, the zip-slip guard, size and entry caps, safe folder names. |
| `src/level.rs` | level.dat (gzip NBT): DataVersion, Version.Name, LevelName. |
| `src/record.rs` | Install records, `meta/practice/<id>.json`. |
| `src/worlds.rs` | Placing, removing and fingerprinting worlds; the open-world check (`session.lock`). |
| `src/fetch.rs` | Streaming, verified download into the content store, with byte progress. |
| `src/lib.rs` | `Practice`, the facade the UI calls; the docs. |
| `examples/practice.rs` | CLI for live checks: list, install, status, check, plan-update, reset, uninstall. |
| `examples/practice_game.rs` | Stand-in for the post-M2 wiring: installs 1.16.1 Fabric (srl-game), the PRACTICE mods (srl-mods resolve), starts the game in `instances/practice`. |

## srl-core changes (additive)

- `Paths::practice_instance()` -> `<root>/instances/practice`, plus a line in the layout doc and a
  test assertion. Nothing else in srl-core changed. `practice` is not a `Mode` id.

Other files outside the crate: the `members` line in `launcher/Cargo.toml`, `Cargo.lock` (the new
package only), and a `THIRD_PARTY_NOTICES.md` entry for `level.rs` (adapted from Pandora's
`load_world_summary`).

## The catalogue

Downloads come only from the author's own GitHub release asset (research/04 section 3: no
licence means link only, never re-host). Each version is pinned by sha512 and size, computed on
2026-10-26 by `tools/refresh_catalogue.py`. The four assets that have a GitHub `digest` (MCSR
Practice Map, LBP, Zero Practice, Blaze Practice by Mescht) matched it in sha256 as well.

| id | version | size | world folder in zip | LevelName in Singleplayer | DataVersion |
|---|---|---|---|---|---|
| the-mcsr-practice-map-dibedy | 2.0.0 | 92.5 MB | (zip root) | MCSR Practice v2.0.0 by Dibedy | 2567 |
| llama-s-bastion-practice-llamapag | 3.15.0 | 26.3 MB | LBP 3.15.0 | Llama's Bastion Practice v3.15.0 | 2567 |
| end-practice-ryguy2k4 | 3.4.0 | 15.6 MB | _Ryguy2k4 End Practice v3.4.0-1.16.1 | End Practice | 2567 |
| zero-practice-mescht | 1.2.2 | 9.1 MB | Zero Practice v1.2.2 | Zero Practice v1.2.2 by Mescht | 2567 |
| portal-practice-v2-semperzz | 2.8 | 27.5 MB | Portal Practice v2 | Portal Practice v2 | 2567 |
| portal-practice-mescht | 1.3.1 | 3.6 MB | Portal-Practice v1.3.1 | Portal Practice v1.3.1 by Mescht | 2567 |
| ow-practice-7rowl | 2.0 | 21.3 MB | OW Practice V2 | OW Practice V2 | 2567 |
| crafting-practice-v2-semperzz | 2.1 | 7.0 MB | Crafting Practice v2 | Crafting Practice v2 | 2567 |
| bt-practice-mescht | 1.3 | 56 KB | BT Practice v1.3 | BT Practice v1.3 by Mescht | 2567 |
| blaze-practice-mescht | 1.1.0 | 3.7 MB | Blaze Practice v1.1.0 | Blaze Practice v1.1.0 by Mescht | 2567 |
| blaze-practice-semperzz | 1.3 | 9.2 MB | Blaze Practice | Blaze Practice | 2567 |
| end-portal-fill-v2-cylorun | 2.0 | 5.3 MB | End Portal Practice | End Portal Practice | 2567 |
| zero-sorting-practice-semperzz | 1.5 | 13.9 MB | Zero Sorting | Zero Inventory Sorting | 2567 |

All 13 worlds were last saved by 1.16.1 (DataVersion 2567); no mismatch. These are the 13 cards
the app shows; since M3 the app reads its cards from this catalogue (one source), and
`assets/practice.json` keeps only the categories' art (a test checks the categories agree).

Open-the-page entries (`link_only`, with the reason):

- `vine-ssg-practice-bdamja` (research M14): made for 1.16.5; the practice instance runs 1.16.1.
- `llama-s-practice-map-llama` (research A2): only on Mediafire, which needs a browser; no stable
  URL to pin.

The app shows them too, with their own `map.<id>` copy, as "Open the author's page".

Per-map extras from the authors' READMEs, release notes and the maps' own files: requirements
(Portal Practice v2: render distance 16 if the inventory doesn't load; Blaze Practice by Mescht:
the spawning-potential view can lag; Zero Sorting: `playerattribute`, without which the hunger and
health reset does nothing (all 36 of its command blocks run the mod's `setplayer`); LBP and The
MCSR Practice Map (which bundles LBP3): Carpet for saturation resetting and the tick warp; OW
Practice v2.0: AutoCraftable (release notes); LBP: the LBP Extensions data pack. All optional and
not provided: none of these mods is in mcsr-meta) and carry-over files (Zero Practice and Blaze
Practice by Mescht: their loadout `.dat`, on reset and update; Portal Practice by Mescht:
`command_storage_portal.dat` to the same name, `same_version_only`, so only on a Reset of the same
version). BT Practice keeps its records and settings in the scoreboard, which Reset clears; the
note says so and names the author's fix (delete `region/`).

Portal Practice's import (`command_storage_portal_import.dat` + `/trigger import`) exists only on
the repo's main branch (README and `functions/importer/` added 2026-05-22); the pinned v1.3.1 has
no importer (no `import` anywhere in the zip). When a release with the importer is pinned, add a
carry-over rule `command_storage_portal.dat` -> `command_storage_portal_import.dat` with
`then_run: "/trigger import"` for updates.

Notes:
- Dibedy publishes under a moving tag, `latest`. When v2.1 comes out, the v2.0.0 URL may stop
  working: install then fails with `SourceGone` until the catalogue is refreshed. The script's
  `--latest` compares asset URLs, not tags, so it catches this.
- Every map has "No licence" status. We download from the public release the way Map-Check does;
  we have no written permission (research/04 section 4.2 suggests asking). Don't write copy that
  implies endorsement.

## The practice instance profile (`PRACTICE`)

Minecraft 1.16.1, Fabric (Loader 0.16.0 or newer), Java 21: the same game files as the 1.16.1
modes.

- In: sodium (+ speedrunapi, its dependency), lithium, starlight, lazydfu, voyager, antigone,
  krypton, speedrunigt; extra-options only with the accessibility setting. On macOS
  `PRACTICE.mod_ids(_, macos = true)` gives `sodiummac` instead of `sodium` (`MAC_REPLACES`,
  the same pair srl-mods' resolve swaps), so an id lookup in a macOS-resolved set finds it.
- Out, with the reason in `profile.rs`: atum, seedqueue, worldpreview, fast_reset (skips saving),
  standardsettings, antiresourcereload, sleepbackground, lazystronghold, setspawnmod,
  mcsrfairplay, hermes, hermes-core, state-output, chunkumulator, forceport.
- What the maps expect: none of the 13 needs a mod (the READMEs and research 1.1). They use data
  packs, functions and command blocks, so nothing that skips or caches data pack reloads.
- All ids exist as 1.16.1 mods in mcsr-meta (a test reads srl-mods' fixture, and the live run
  resolved all nine from the current mods.json).

## Installing, resetting, updating, removing

- **Store:** the verified zip is the pristine copy, in `store/sha512/<ab>/<hex>`. Reset extracts it
  again (verifying it first, and downloading it again if it is missing or damaged).
- **Zip layouts:** level.dat at the root (MCSR Practice Map; the folder is named
  `<name> v<version>`), one folder (the other 12), several worlds (each folder with a level.dat is a
  world; a level.dat inside a world belongs to it). Files outside every world are left out;
  `session.lock` is skipped (7 maps ship the author's).
- **Zip-slip guard:** `\` counts as a separator; absolute paths, drive prefixes, `..`, `:`, NUL,
  Windows device names and names ending in a dot or space are refused; symlink entries are
  refused; total size capped at 4 GiB and 200,000 entries, counted while writing too. Every name is
  checked before anything is written, so a bad zip leaves nothing.
- **Never touching other saves:** a world is changed only if a record names it; record folder
  names must be one plain component; a new world never lands on an existing folder (it gets
  ` (2)`); worlds move into `saves/` whole from `instances/practice/.srl-staging`, and leave it the
  same way before deletion; symlinks are not followed.
- **Fresh or played:** the record keeps the sha512 of each level.dat as extracted. Opening a world
  rewrites level.dat at once (seen live: Zero Practice was `Played` seconds after opening). Only a
  folder that is gone is `Missing`; a real folder whose level.dat is gone or unreadable is
  `Played` (a crash mid-save leaves only level.dat_old, which the game loads).
- **Update decision:** `plan_update` returns `UpToDate`, `Replace` (no world played: `update(id,
  None)` goes ahead) or `Ask` (played worlds and what would carry over). `update` without a choice
  on `Ask` returns `NeedsDecision`, and so does `update(id, None)` if a world was played during
  the download (the plan is checked again after extraction, right before the swap). Choices:
  `ReplaceWorlds`, `KeepBoth` (the played world becomes the player's, listed as handed over,
  never touched again), `StoreOnly` (only the stored copy; Reset installs the new version later).
  The stored copy is replaced in every case.
- **Replacing worlds safely** (`worlds::place_worlds`): extract into
  `.srl-staging/new-<id>`, write the carried files into the staged world, run the last checks
  (re-plan, `session.lock`), then move the old worlds into `.srl-staging/swap-<time>-<id>`, the
  new ones into `saves/`, and save the record. Any failure moves the new worlds out and the old
  ones back. Only after the record is saved is the swap folder marked `.committed` and deleted. If
  the process dies mid-swap, the next mutating call (`recover_staging`) puts an uncommitted swap's
  worlds back into `saves/` (under ` (2)` if the name is taken) before deleting other leftovers.
- **Open world:** reset, update (ReplaceWorlds) and uninstall check `session.lock`, which the game
  locks while a world is loaded (fcntl F_GETLK on Unix; on Windows a read of the locked file
  fails), and return `WorldInUse` before changing anything.
- **Concurrency:** mutating calls take one async mutex (`lock_owned`). The guard is shared (an
  `Arc`) with every blocking task the call starts, so dropping the call's future (Cancel) frees
  the lock only after its running file work ends. A dropped download deletes its `.part` at once
  (a drop guard); stale `.part` files of map zips are removed at the start of every mutating call.
- **Downloads:** network errors and an early end are retried (3 tries); a 404/410 and a full-size
  body with the wrong hash are not (the same wrong file would come again).
- **Records:** `load` refuses a pristine sha512 that isn't 128 lower-case hex (it becomes a store
  path that uninstall deletes). Reset downloads from the catalogue's URL when the catalogue's
  sha512 is the record's, else from the record's URL (the file is sha512-checked either way).
- **Catalogue URLs:** parsed, not prefix-matched: `https://github.com/<owner>/<name>/releases/
  download/<tag>/<asset>` exactly, with owner/name = `repo`, the segment tag = `tag`, no port,
  user, query or fragment, no `%2e`/`%2f`/`%5c`/`\`, and already in normal form (parsing changes
  nothing, so dot segments can't resolve to another repo). The source page must be the repo page
  or under it (`<repo>-mirror` no longer passes).

## The honest in-game flow

Minecraft 1.16.1 has no Quick Play (it arrived in 1.20), so the launcher can't open a world from the
command line. The flow: Play starts the practice instance, the player clicks Singleplayer and picks
the world. The list shows the LevelName (with the author's colours) and, under it, the folder and
last-played date; it is sorted by last played, and its search box filters by name.
`WorldView::name` gives the name to show in the launcher.

## What was run (2026-10-26)

- `cargo test -p srl-practice -p srl-core` on Linux: srl-practice 21 tests pass (catalogue validation
  and agreement with practice.json, the three zip layouts, the zip-slip guard, level.dat, the
  profile, install/reset/uninstall that leave other saves alone, update decisions and each update
  choice, bad downloads from a local HTTP server (hash mismatch, 404) leaving nothing, the
  open-world lock held by another process); srl-core's tests pass.
- `cargo clippy -p srl-practice -p srl-core --all-targets -D warnings`: clean, on Linux and for
  `x86_64-pc-windows-gnu`. `cargo check --target x86_64-pc-windows-gnu`: clean. The Windows lib
  tests (20; the lock test is Unix-only) also pass under Wine 64.
- `refresh_catalogue.py --write --latest`: downloaded all 13, pinned them; no newer releases.
- Live install into an empty `SRL_HOME` (`examples/practice install all`): all 13 installed,
  0.3 s to 3.4 s each (MCSR Practice Map: 3.4 s). `practice check`: every store file matches the
  catalogue sha512 and size; every world's level.dat reads DataVersion 2567, saved by 1.16.1.
- The game (`examples/practice_game` in Xvfb :57): 1.16.1 + Fabric Loader 0.20.1, Java 21.0.7, the
  nine PRACTICE mods from the live mcsr-meta; title screen in 15.9 s. Clicked Singleplayer with
  xdotool: all 13 maps listed (`shots/2-singleplayer-list.jpg`). Opened Zero Practice v1.2.2: the
  lobby loaded with the SpeedRunIGT timer (`shots/3-zero-practice-open.jpg`). With the world open,
  `practice reset` refused: "the game has saves/Zero Practice v1.2.2 open". After Save and Quit
  to Title, reset (keeping loadouts) worked; the world list showed the fresh copy again (original
  date) and it opened (`shots/4-after-reset-search.jpg`). The game was then stopped; no Java,
  Xvfb or Wine process of this stream is left.

## Review fixes (stream "fix", 2026-10-26)

Each finding of the review was checked first. The code findings were reproduced by new tests that
failed on a7ba34d (9 failed, then all pass after the fixes); the content findings were checked in
the pinned zips and on GitHub.

| Finding | Confirmed by | Fix |
|---|---|---|
| Portal Practice import doesn't exist in v1.3.1 (high) | no `import` in the v1.3.1 zip; `functions/importer/` only on main (README commits 2026-05-22) | carry `command_storage_portal.dat` to itself, `same_version_only`, no `then_run`; lib.rs claims corrected |
| Replace deletes old worlds before extracting (medium) | test `a_failed_replace_keeps_the_played_world` | staged swap with rollback and crash recovery (`an_interrupted_swap_is_rolled_back`) |
| Lock released when the future is dropped (medium) | test `a_cancelled_call_keeps_the_lock_until_its_file_work_ends` | `lock_owned`, guard shared with the blocking tasks |
| `update(None)` doesn't re-check played (medium) | test `update_without_a_choice_asks_if_the_world_was_played_during_the_download` | re-plan after extraction, `NeedsDecision` |
| Zero Sorting hunger note (medium) | all 36 hunger/health command blocks run `setplayer`; no vanilla fallback in the world or data packs | note says the reset does nothing without the mod |
| Missing optional mods (medium) | LBP3 `check_for_carpet` and "requires Carpet Mod" (also in Dibedy's zip); OW v2.0 release notes name AutoCraftable | `carpet` (LBP, Dibedy) and `autocraftable` (OW) as optional, not provided; profile doc; a test that they are still not in mcsr-meta |
| Folder without level.dat counts as Missing (low) | test `a_world_without_level_dat_counts_as_played` | Missing only when the folder is gone |
| Mismatch retried 3x; `.part` left on cancel (low) | tests `a_hash_mismatch_is_downloaded_once`, `a_cancelled_download_leaves_no_part_file` | no retry of a full-size mismatch; drop guard; stale-part clean-up |
| URL prefix rule (low) | new validation cases (dot segments, `%2e%2e`, port, user, query...) | parsed URL check |
| Record sha512 used as a path (low) | test `a_record_with_a_bad_hash_is_refused` | hex check in `load`; reset prefers the catalogue URL |
| Llama credit (low) | Mediafire og:title "Lama's Practice Map"; GitHub `addlama` owns `preemptivenav` | credit "By Lama (addlama)"; the card's `authors` stays "Llama" (the name the community knows; the app shows the catalogue's `authors`) |
| macOS Sodium (low) | srl-mods `MAC_REPLACES` | `mod_ids(accessibility, macos)`, test |
| BT Reset clears records (low) | BT v1.3 data pack adds `records` and `settings` scoreboard objectives | note says Reset clears them and names the author's fix |

Run for the fixes:
- `cargo test -p srl-practice -p srl-core`: srl-practice 31 tests + 1 doctest, srl-core 5, pass
  (three runs, no flakes). `cargo clippy --all-targets -D warnings` clean on Linux and for
  `x86_64-pc-windows-gnu`; the 30 Windows lib tests pass under Wine 64.
- Live, in Xvfb :63 with the existing dev home (`m3-practice/home`, old records read fine): opened
  Portal Practice v1.3.1, ran `/data modify storage portal:levels srl_check set value 42b`;
  `practice reset portal-practice-mescht --keep` with the world open refused ("the game has
  saves/Portal-Practice v1.3.1 open"); after Save and Quit it worked (carried "loadouts, PBs and
  level settings", no command); reopening the fresh world (original date in the list),
  `/data get storage portal:levels srl_check` printed `42b`
  (`shots/5-portal-storage-kept-after-reset.jpg`). The map was then reset without `--keep`; the
  game, Xvfb and Wine were stopped.
- Seen on the way: the world list search finds "Portal" but not "Portal Practice v1.3" for
  "Portal Practice v1.3.1 by Mescht" (the LevelName likely has colour codes between words). The
  launcher's in-game hint should name one distinctive word.

## Open issues and notes for the integration after M2

- **Wiring:** srl-game and srl-mods key instances by `Mode`. The practice instance needs: a game
  install with `game_dir = Paths::practice_instance()` (the example sets `Installed::game_dir`),
  mods resolved from `PRACTICE.mod_ids(accessibility, macos)` into `instances/practice/mods` (the example resolves the
  RSG set and filters it; srl-mods' `install` only writes to a mode's folder), its own game log
  (the example's game logs to `logs/rsg-latest.log`, shared with RSG) and an `InstanceGuard`-like
  token so Play and map changes don't overlap.
- **Clean stop:** stopping the game with SIGTERM (exit 143) doesn't save the world; M2's clean
  stop applies here too.
- **SpeedRunIGT timer:** after a reset, reopening the world showed the timer continuing (IGT about
  2:54) rather than from zero. Don't claim the timer measures map attempts.
- **Windows:** the open-world check (read fails on a locked `session.lock`) is reasoned from how
  Java locks files on Windows, not tested on Windows. As a second line, moving a world out of
  `saves/` before deleting it should fail on Windows while the game holds its files (also not
  tested there).
- **Content-store clean-up** (none exists yet) must keep `Practice::store_files_in_use()`.
- **Copy keys** needed: `map.<id>` for the two link-only entries, and optionally
  `practice.req.<id>` for requirement notes (English text is in the catalogue).
- Permission from map authors, a hosted signed catalogue and practice mods (peepoPractice,
  StrongholdTrainer, MPK's hotbar) are later work (research/04 sections 1.3 and 4).
