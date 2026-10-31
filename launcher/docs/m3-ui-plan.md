# M3: wiring srl-stats, srl-practice and srl-auth into the app

Branch `m3-core` = `main` (6f27470) + `m3-stats` + `m3-practice` + `m3-auth`. The three crates are
libraries only: nothing in `srl-app`, `srl-game` or `srl-mods` calls them yet. This plan is for the
work after M2 (branch `launcher`) is merged. Each crate's `lib.rs` has a "For the UI" section with
the exact copy, states and claims; this file says where the calls go and in what order.

Merged commits: m3-stats 73ae9b6, m3-practice e692904, m3-auth f23ab69. Conflicts resolved: the
workspace members, `Cargo.lock` (two new `[[package]]` entries side by side) and
`THIRD_PARTY_NOTICES.md` (both Pandora entries kept). Checked on the merge with
`CARGO_TARGET_DIR=~/.cache/srl-target/m3-merge`: `cargo fmt --all --check`, `cargo clippy
--workspace --all-targets --locked -D warnings`, `cargo test --workspace --locked` (all pass, the
network ones stay ignored), and `cargo clippy --target x86_64-pc-windows-gnu -p srl-stats -p
srl-practice -p srl-auth --all-targets -D warnings`. No integration fix was needed.

## 0. Merge m3-core into the M2 branch

A trial `git merge-tree` of `origin/launcher` (af06a7e) and `m3-core` conflicts in four files,
all "both sides added lines next to each other", keep both:

- `launcher/Cargo.toml`: members = core, game, mods, tools, app, stats, practice, auth (and keep
  M2's `srl-tools` / `x11rb` workspace deps).
- `launcher/Cargo.lock`: take one side, then `cargo metadata` (no `--locked`) adds the rest; check
  that no existing version moved.
- `launcher/THIRD_PARTY_NOTICES.md`: keep M2's and M3's Pandora entries.
- `launcher/crates/srl-core/src/paths.rs`: M2 adds `Paths::tools()`, M3 adds
  `Paths::practice_instance()` (the only srl-core change in M3, additive), both with a line in the
  layout comment and the test.

Then fmt, clippy (Linux and `x86_64-pc-windows-gnu`), the workspace tests.

## 1. Backend services

`RealBackend` gets the three services, built once on its tokio runtime next to the installer:

- `Stats::new(paths.clone(), &downloader)`
- `Practice::new(paths.clone(), downloader.clone())?`
- `Auth::new(&paths, AuthConfig::detect())`

Each is cheap, `Send + Sync`, and all their I/O is async or on `spawn_blocking`, so the UI thread
only ever sends a request and receives an `Event`. New `Event` variants, one per page:
`Stats(StatsEvent)`, `Practice(PracticeEvent)`, `Account(AccountEvent)`. The fake backend
(`backend/fake.rs`, `SRL_DEMO`) gets fixed data for each, so screenshots keep working.

## 2. Stats with real data

- On app start: `stats.local()` (index only, ~60 ms for 5000 runs) and, if a Ranked nickname is
  set, `ranked_cached(name)` (no network) for the Home tiles.
- On opening Stats: paint the last `LocalStats` at once; re-run `local()` (cheap when nothing is
  new) and `ranked(name, Refresh::IfStale)` / `draftout(name, Refresh::IfStale)`; replace the
  painted values when they arrive. After a game exits (`ModeState::Ready` after `Running`), re-run
  `local()` so the new run shows.
- Settings gains one field: "Your Minecraft name on MCSR Ranked" (not the offline name). Once a
  Microsoft account is active, default it to that account's name.
- The page: the existing v3 layout from `SRL_DEMO`, with `headline_pb`, `recent(n)`,
  `category(...)`, the Ranked column and the weekly race from the API types. The non-demo
  placeholder text in `screens/stats.rs` goes away.
- States: skeleton while the first scan runs; per-mode empty text; the `Cached::age()` label and
  "(offline)"; `skipped` / `unreadable` as a quiet note. Never a modal.

## 3. Practice installs

Two pieces: the map files (srl-practice, done) and the practice instance's game (not done).

1. **The instance.** srl-game and srl-mods are keyed by `Mode`, and `practice` is not a mode (and
   should not become one: `Mode` is matched exhaustively and sized into `[ModeSnapshot; 5]`). Add
   to srl-game an install target that is not a mode: game dir `Paths::practice_instance()`,
   Minecraft `PRACTICE.minecraft_version`, Fabric, Java `PRACTICE.java_major`; and to srl-mods a
   resolve from an explicit id list (`PRACTICE.mod_ids(accessibility, macos)`) instead of a mode.
   Both additive; the mode paths stay as they are.
2. **The page.** `practice.catalogue()` (no I/O) for the cards that exist; `practice.statuses()`
   on open and after every action. Buttons per `MapStatus`: Install (with size), Play, Reset map
   (or "Reset to vX"), Remove, Update to vX, Open the author's page for `LinkOnly`.
3. **Progress.** `install` / `update` take a progress callback; forward `Step::Verifying`,
   `Downloading` (bytes) and `Extracting` (files) as events into the row's bar. Cancel only while
   `Downloading` (drop the task); after that the button is disabled until the call ends.
4. **Confirmations.** `plan_reset` before Reset (show `carry_over` and that other maps' records
   go), a plain "remove this map's world?" before Remove, the choice dialog for `UpdatePlan::Ask`.
5. **Play.** Installs the practice instance if needed (step 1), then starts it. The row then says
   "In the game: Singleplayer, then <WorldView::name>". Reset, Update and Remove are greyed out
   while the practice game runs; `WorldInUse` is the fallback message.
6. **Errors:** one line per `PracticeError` variant, copy as in srl-practice's `lib.rs`.

## 4. Microsoft account in Settings

- If `!auth.is_available()` (no client ID in this build, today's state): Settings shows the one
  line from `auth.error.not_available` and no login button. Everything below stays hidden.
- Otherwise an "Account" section: `auth.accounts()` (names, faces from `skin.url`, the active
  check, "Log in again" badge for `needs_login`, the file-keystore note). "Log in with
  Microsoft" starts `auth.login(events)` as a task; the dialog shows the browser and phone paths
  side by side from `LoginEvent`s (QR from `qr_matrix`), then the Xbox/Minecraft/Profile steps,
  and Cancel aborts the task. "Log out" = `remove(uuid)`; picking an account = `set_active`.
- Merge `crates/srl-auth/copy/{en,pl}.json` into the app's copy files; show errors through
  `AuthError::copy_key()` and `help_url()`.
- On start, after the first paint: `auth.refresh_in_background()` once.
- The offline name stays: it is the account for RSG/FSG/SSG when no Microsoft account is active.
  The first-run name prompt (`screens/name.rs`) keeps working as is.

## 5. Microsoft account in Play

- srl-game: `LaunchOptions::account` becomes an enum `Account::Offline(OfflineAccount)` /
  `Account::Microsoft(MinecraftSession)`-like data (srl-game must not depend on srl-auth: pass
  the placeholder list from `MinecraftSession::placeholders()` instead of the offline values).
  Logs and any shown command go through `MinecraftSession::redact`.
- In `play(mode)`: if a Microsoft account is active, call `auth.prepare_launch(uuid)` before the
  game starts (as an install stage, "Signing in", usually instant). Hold `auth.mark_in_use(uuid)`
  for as long as the game runs. On `warning: Some(_)` start anyway and show "valid until HH:MM";
  on `needs_relogin()` stop and open the login; on `Keystore` say "unlock your keyring".
- Ranked and Draftout tiles: with no Microsoft account, Play explains that these modes need one
  (and, while `!is_available()`, that it isn't in this build yet). Never fall back to offline for
  them silently.

## 6. Order and checks

1. Merge (section 0). 2. Stats (no game changes, smallest risk). 3. Account in Settings (works
behind `is_available()`, testable with `SRL_MSA_CLIENT_ID` and the mock). 4. Account in Play
(srl-game change). 5. Practice instance + page (srl-game and srl-mods changes).

Every step: unit tests for the new backend glue, screenshots in Xvfb (`tools/shoot-app.sh`), a real
install with `SRL_HOME` under `~/.cache/srl-dev`, and `SRL_KEYSTORE=file` for anything that
touches srl-auth, so no test uses the desktop keyring. A live Microsoft login waits for Mojang's
approval of the client ID.
