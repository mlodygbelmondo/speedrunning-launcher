# Speedrunning Launcher: the app

The bake-off is over: the app is GPUI, with our own Rust core that reuses MIT code from Pandora Launcher
(decisions in `../WORKLOG.md`, 2026-10-26). The prototype in `../bakeoff/` stays as the design reference:
`../bakeoff/REDESIGN.md` (scene v3) and `../bakeoff/gpui/` (its GPUI code, which this app ports).

## What the launcher does

1. You pick a mode: Ranked, RSG, FSG, SSG or Draftout. The four 1.16.1 modes share one game directory
   (`instances/mcsr`) and differ in the mods Play puts into it; Draftout has its own. Players can also create their
   own instances (below, "Custom instances").
2. You press Play. The launcher installs whatever is missing or outdated, then starts Minecraft itself, with no
   official launcher involved:
   - the Minecraft version, its libraries, natives and assets;
   - a Java runtime;
   - Fabric Loader;
   - the mode's legal mod set.
3. Everything else happens in the game: Ranked queues and private rooms (the MCSR Ranked mod), resets and seeds
   (Atum, SeedQueue, the FSG mod). The launcher never pretends to drive them.
4. While the game runs, the launcher steps aside (the Game screen) and comes back when the game exits.

Since milestone 2 the speedrun tools start with the game: Ninjabrain Bot, Toolscreen (Windows) and waywall (Linux).
Milestone 3 adds read-only stats (SpeedRunIGT records, the MCSR Ranked and Draftout APIs), practice maps and
Microsoft login. Until Mojang approves our client ID, login is off in the build and the launcher uses **offline
accounts**. They work for singleplayer (RSG, FSG, SSG). Ranked and Draftout need a Microsoft account to play online,
but the game must still start with their mod sets. With no Microsoft account active every mode starts offline, with
one active every mode uses it (`account::play_as` in `srl-app`, the one place that choice is made). A build with a
client ID opens offline play only after someone has signed in on that computer once with an account that owns the
game (Prism's rule, which Mojang's review expects); before that, Play asks for a sign-in.

## Milestone 1

Pick a mode, press Play, and the game starts with the right mods, on Linux and Windows, with an offline account.

On Linux (Xvfb) all five modes install from an empty data folder and reach the title screen with their mods. Ranked
then shows the MCSR Ranked mod's privacy-policy screen, which the player accepts in the game. Windows is built,
tested and runs a headless RSG install in CI; it has not been tried on a real Windows desktop yet. Screenshots of the
end-to-end run: `shots/m1/e2e-*.jpg`.

## Milestone 2

The tools and the rest of the pages: Tools, Learn, Stats and the full Settings.

- **Tools:** Ninjabrain Bot, Toolscreen (Windows) and waywall (Linux), from `srl-tools`. The Tools page shows what is
  installed from the files on disk, installs and updates with every hash checked, and saves which modes start each
  tool. Play installs the tools a mode starts with, launches through their launch hook, starts them next to the game
  and closes them when it exits. Draftout bans Ninjabrain Bot; waywall is never installed by the launcher (it comes
  from the system's package manager).
- **Settings:** memory per mode, the optional mods (Extra Options sliders for accessibility; No Chat Restrictions and
  No Telemetry for Draftout), handed to `srl-mods`, and the credits.
- **Learn:** other runners' guides by topic, opened in the browser. **Stats:** what it will show and where the numbers
  will come from; no numbers yet.
- **Stop** asks the game to quit the way its window's X does, so the world saves; the log says how it stopped.

On Linux (Xvfb, an empty data folder) RSG installs and starts with Ninjabrain Bot; quitting the game closes Ninjabrain
Bot and the mode is Ready again; with Ninjabrain Bot off for RSG only the game starts; Draftout with its extras gets
both jars in `mods/`; Stop ends the game cleanly (`Stopping!` in its log). Windows runs a headless install of
Ninjabrain Bot and Toolscreen in CI; Toolscreen's injection into a real game still needs a test on a Windows PC
(`crates/srl-tools/README.md`, "What to check on Windows"). Screenshots: `shots/m2/` (`e2e-*.jpg` for the run above,
`en/` and `pl/` for every new page).

## Milestone 3

Stats, practice maps and Microsoft accounts, wired into the app from the `srl-stats`, `srl-practice` and `srl-auth`
crates (plan: `docs/m3-ui-plan.md`).

- **Stats:** PB and sum of best per mode, run counts and recent runs with splits against the PB, from the SpeedRunIGT
  records of this launcher's instances; for the names on Settings (else the active Microsoft account's name) the MCSR
  Ranked profile (elo, tier, season record, recent matches, the Weekly Race) and the Draftout elo, cached on disk with
  their age and "offline" when a refresh fails. Home shows the same numbers where the design has a place for them.
  The records are read again at start, when Stats opens and when a game (a mode's or the practice one) exits.
- **Practice:** the maps from srl-practice's catalogue install from their authors' releases (sha512-checked), with
  progress and Cancel while downloading, and Reset, Update and Remove behind a confirmation. Play installs the
  practice instance (`instances/practice`: Minecraft 1.16.1, Fabric and the practice mods through `srl-mods`' legal
  checks), starts it with RSG's tools and memory, and the map's row says where to find the world.
- **Microsoft accounts:** with a client ID in the build, Settings lists the accounts and logs in through the browser
  or a phone code (with a QR code); Play signs in before the game starts and hands the session to `srl-game`, with
  the token masked in every log. Without a client ID (the shipped build today) Settings shows one line and every
  mode plays offline.

On Linux (Xvfb, an empty data folder): RSG installs and starts with Ninjabrain Bot, a few resets later Stats lists
the runs; a live MCSR Ranked name shows its elo and matches; BT Practice installs, its game starts with the map in
the Singleplayer list, and Reset gives a fresh copy after the game closes; with a dummy client ID and srl-auth's
mock services the login works and RSG starts as the Microsoft account (`--userType msa`) to the title screen with no
token in any log; Stop ends the game cleanly. Screenshots: `shots/m3/e2e-*.jpg`, and every page in `shots/m3/en/`
and `shots/m3/pl/`.

## Crates

| Crate | Owns |
|---|---|
| `srl-core` | Shared contracts: `Paths` (disk layout, `$SRL_HOME`, `game_dir` for any instance), `Mode`, `Instance` (a mode, practice or `Custom(CustomId)`), `Progress`, `OfflineAccount`, `Downloader` (verified, retried, concurrent downloads). Change it only when a contract really needs to. |
| `srl-game` | Minecraft itself: Mojang version manifest and version JSON, OS rules, libraries, natives, client jar, asset index and objects, Java runtimes, the Fabric Loader profile, building the launch command and starting and watching the game process. |
| `srl-mods` | Legal mods: mcsr-meta schema 7 (`mods.json`, `extra.json`), `legal-builds.csv` and `conditional-mods.json`, the per-mode sets, the content store and materialising jars into the instance's `mods/`, the player's changes from the Mods page (`choice`), update checks and removing jars that are no longer legal. |
| `srl-tools` | Speedrun tools: Ninjabrain Bot, Toolscreen (Windows) and waywall (Linux). The pinned legal versions, verified downloads, `legal-dlls.csv`, per-mode defaults, and the launch hook that wraps the game command (waywall), injects Toolscreen and runs Ninjabrain Bot next to the game. See `crates/srl-tools/README.md`. |
| `srl-stats` | Read-only stats: SpeedRunIGT records (PB, splits, sum of best, run counts per mode), the MCSR Ranked and Draftout APIs, cached on disk for offline use. The app's Stats page and Home use it (`crates/srl-app/src/backend/stats.rs`); see the top of `src/lib.rs` and `NOTES.md`. |
| `srl-practice` | Practice maps: the catalogue (author download URLs, pinned sha512, credits), the practice instance, install, reset, update and uninstall. The app's Practice page uses it (`crates/srl-app/src/backend/practice.rs`). |
| `srl-auth` | Microsoft login (browser with PKCE racing the device code), the token chain to Minecraft, tokens in the OS keystore. Off until Mojang approves our client ID (`SRL_MSA_CLIENT_ID`); guide: `docs/microsoft-login.md`. The app's Settings, login dialog and Play use it (`crates/srl-app/src/backend/account.rs`); `examples/mock-services.rs` runs its test mock for the app. |
| `srl-import` | "Import my current settings": finds the player's instances in MultiMC, Prism Launcher (and PolyMC), MCSR Launcher and the official launcher (search roots are a parameter; `SRL_IMPORT_HOME` for tests), and copies the ticked parts (options.txt, mod settings, resource packs, worlds) into the launcher's instances: everything into the ones on the source's Minecraft version, only key bindings, mouse sensitivity and FOV into the others (Draftout). Backs up what it replaces (`import-backup-<unix time>/` in the instance), never writes to the source, never follows links. See the top of `src/lib.rs`. |
| `srl-update` | The launcher's own update: the latest GitHub release against the running version (semver), the zip checked against its `.sha256` asset, extracted with the zip-slip guard and size caps, and swapped in for the running exe with a rollback. The app's Settings → About and the gear's badge use it (`crates/srl-app/src/backend/update.rs`); see the top of `src/lib.rs`. |
| `srl-app` | The GPUI app: scene v3 ported from the prototype, wired to the core through a `Backend` facade. |

Async code runs on tokio. The app owns one tokio runtime on a background thread and talks to it through the
`Backend` facade; the UI thread never blocks on I/O.

### What Play does

`srl-app/src/backend/pipeline.rs` is the one place that strings the crates together; the window (`RealBackend`) and
the headless command line run the same code:

The practice instance goes through the same steps with `Instance::Practice` (its game and mods from `srl-practice`),
and so does a custom instance with `Instance::Custom` (below).

1. `srl-mods` loads the mod lists (fetched again when older than 10 minutes, else the cached copy).
2. `srl-game` installs the game: the version metadata (Mojang's and Fabric Loader's), then in parallel Java, the
   client jar with the libraries and natives, and the assets.
3. `srl-mods` resolves the mode's legal set for this OS, applies the player's changes from the Mods page, and installs it into the instance's `mods/` (Ranked, RSG, FSG and SSG share `instances/mcsr`, `srl_core::shared`; Draftout has `instances/draftout`): hard links from
   the content store (copies on Windows, where a running game would pin a shared link), never while a game of that
   mode runs. A jar that isn't in the set leaves `mods/`; if the store has no copy of it (the player put it there),
   it is moved to `mods-removed/<date>/` next to `mods/` rather than deleted.
4. `srl-tools` installs the tools the mode starts with (the Tools page's choice), at their pinned versions.
5. The account: offline with the saved name, or, when a Microsoft account is active, `srl-auth` signs it in
   ("Signing in", usually instant) and keeps it from being refreshed while the game runs. A sign-in that has to be
   done again stops Play and opens the login; one that only couldn't be renewed starts the game with the saved token
   and the game screen says until when it is valid.
6. `srl-game` builds the game command with that account and the settings (name, memory per mode);
   `srl-tools`' launch hook may wrap it (waywall), and `srl-game` starts it. Then `srl-tools` injects Toolscreen
   (Windows) and starts Ninjabrain Bot next to the game. A tool that can't run is left out with a note; it never
   blocks the game. The pipeline watches the log until the title screen loads, and reports a game that exits or that
   Fabric stops while starting.
7. When the game exits, the tools started for it close, and the mode is Ready again.

Every install writes a record (`meta/launcher/<mode>.json`). On start the launcher checks it against the files on
disk, so it knows which modes are ready without the network; the update check runs after that, in the background.
Details: `crates/srl-app/README.md`.

### Custom instances

Besides the built-in instances (the shared 1.16.1 directory, Draftout, practice) the player can create their own:
each is `instances/custom-<n>` with a definition in `settings.json` (`custom_instances`: name, base mode, memory, mod
changes, JVM arguments). The base mode gives it its Minecraft version, Java, Fabric Loader, legal mod set and tool
rules; Play installs and starts it through the same pipeline, keyed by its own id (`custom-<n>`: its install record,
log, pid file and `mods/`), and it never touches the shared directory's per-mode Atum settings. It can be created
empty or with another instance's options, `config/` and resource packs, duplicated (the whole directory but `mods/`,
logs and the launcher's notes; worlds if asked), brought over from another launcher (MultiMC, Prism, MCSR
Launcher, the official launcher: only its options, `config/`, resource packs and, if asked, worlds), renamed,
changed and deleted. Its runs count in Statistics for its base mode. Everything is on the Instances page
(`crates/srl-app/src/screens/instances/`). Details: `crates/srl-app/README.md`, "Custom instances".

**Deleting a custom instance deletes nothing.** Its directory moves to `<data>/trash/custom-<n>-<unix time>/` (with
the launcher's logs of it in `launcher-logs/`). To get it back, create a new instance with the same base mode and
move that folder's contents into the new one's directory (Open folder); to free the space, delete the folder by hand.
The launcher never empties `trash/` itself.

## Build and run

Linux needs the packages GPUI builds against (the binary links only libxcb and libxkbcommon; Wayland, Vulkan and
fontconfig are loaded at runtime):

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev libxcb1-dev \
  libwayland-dev libfontconfig-dev libfreetype-dev libvulkan1
export CARGO_TARGET_DIR=$HOME/.cache/srl-target/<stream>
srl-cpu cargo test -j 4 --workspace
srl-cpu cargo build -j 4 --release              # -> $CARGO_TARGET_DIR/release/speedrunning-launcher
SRL_HOME=$HOME/.cache/srl-dev/<stream> speedrunning-launcher                  # the launcher
SRL_HOME=$HOME/.cache/srl-dev/<stream> speedrunning-launcher --install rsg    # headless, exit code 1 on failure
```

Windows builds with the MSVC toolchain. Release builds need `fxc.exe` and `rc.exe` from the Windows SDK, which
GitHub's `windows-latest` has.

## CI and the Windows build

`.github/workflows/launcher.yml` runs on every push to the `launcher` branch that touches `launcher/`, on every tag
`v<version>` and by hand:

- **Linux:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, the release build.
- **Windows:** `cargo fmt --check`, `cargo test --workspace`, the release build (its version resource must name
  the product, `build.rs`), then a headless RSG install into an
  empty `SRL_HOME`, a headless install of Ninjabrain Bot and Toolscreen into another one (every file the launcher
  reports is hashed again with coreutils and must match a hash pinned in `srl-tools`), and the zip: the exe,
  `README.txt` (`tools/windows/README.txt`), the licences and `VERSION.txt`.
- **Signing:** the tested exe goes to SignPath, an approver confirms the request and the signed exe goes into the
  zip (`tools/windows/SIGNPATH.md`). A run started by hand with `sign` ticked always signs (and fails without the
  SignPath settings); a tag signs when the `SIGNPATH_API_TOKEN` secret and the `SIGNPATH_ORGANIZATION_ID` variable
  are set, and builds unsigned when they aren't; a push to `launcher` never signs.
- **Publish**, when both pass, with the zip's SHA-256 next to it (`speedrunning-launcher-windows.zip.sha256`, one
  `sha256sum` line):
  - a push to `launcher`: the assets of the `launcher-latest` prerelease, a test build:
    <https://github.com/mlodygbelmondo/speedrunning-launcher/releases/download/launcher-latest/speedrunning-launcher-windows.zip>;
  - a tag `vX.Y.Z`: the release `vX.Y.Z`, a normal release marked latest, so GitHub's `releases/latest` returns it
    and every installed launcher updates to it. Both jobs first check that the tag is `v` plus the
    `[workspace.package]` version and fail otherwise.

### Releasing a version

1. Bump `version` under `[workspace.package]` in `launcher/Cargo.toml` (semver: `0.2.0`), run `cargo check` so
   `Cargo.lock` follows, and add a line to `WORKLOG.md`.
2. Commit and push to `launcher` as usual.
3. Tag that commit and push the tag: `git tag v0.2.0 && git push origin v0.2.0`.

CI builds, tests and publishes the release `v0.2.0`. Never move or reuse a version tag: launchers that already
updated to it won't download it again.

### Self-update

`srl-update` and `crates/srl-app/src/backend/update.rs`. Only a Windows build from CI (`SRL_COMMIT` set) updates
itself; a local build and other systems say so in Settings → About and do nothing.

- **When:** once on start, in the background, unless Settings → About's "Check for updates automatically" is off,
  and when the player clicks "Check now". An automatic check that fails (offline, GitHub's rate limit, a bad
  answer) only writes a log line.
- **What:** `GET https://api.github.com/repos/mlodygbelmondo/speedrunning-launcher/releases/latest` (never a
  prerelease, so `launcher-latest` is never offered). A tag newer than the running version (semver precedence, no
  pre-releases) with both assets is an update. The checksum is downloaded first, then the zip into
  `<data>/updates/<version>/` with srl-core's `Downloader`, which checks the SHA-256 before the file is used. The
  zip is extracted into `staged/` with srl-practice's zip-slip guard, no symlinks or duplicate names, at most 1,000
  entries and 512 MB, and must have `speedrunning-launcher.exe` at its root.
- **The swap happens right after that**, not on Restart, so the update applies on any next start: the new exe is
  copied next to the running one as `.new`, the running exe is renamed to `speedrunning-launcher.old.exe`
  (Windows allows renaming a running exe), `.new` takes its name and the zip's other files are copied over; a
  failed rename puts the old exe back. The next start deletes the `.old.exe`. If the exe's folder can't be
  written (Program Files), nothing is downloaded and About says so, with the release's page.
- **Restart to update** (About, the primary button while an update is ready; the gear in the top nav carries an
  accent square then) starts the new exe and quits. It waits while a game the launcher started runs or anything
  installs, and says which.
- **Tests:** `SRL_UPDATE_URL=<release JSON URL>` points the check at another server and lets it run on Linux
  too; `crates/srl-update/tests/update.rs` runs the whole path against a local HTTP server.

## Sources

- **Research:** `../research/02-mcsr-ecosystem.md` (mods, modes, tools, Java, rules) and
  `../research/03-auth-and-launcher-core.md` (metadata, mod loaders, Java, launching, content store).
- **Pandora Launcher:** a read-only copy is in `~/.cache/srl-ref/PandoraLauncher` (MIT, commit 2d83d6f).
  - Useful parts: `crates/schema` (version JSON, Java runtimes, Fabric) and `crates/backend/src/launch`,
    `metadata`, `install_content.rs`.
  - Read it; never build or run anything from that directory.
  - When you adapt code from it, say so in a comment at the top of the file and add an entry to
    `THIRD_PARTY_NOTICES.md`.
- **Prism, theseus and MCSR Launcher** are GPL: use them only to understand behaviour. Never copy or translate their
  code.

## Rules for everyone working here (agents included)

- **CPU:** Piotr uses this laptop for other work at the same time. Run every heavy command through `srl-cpu`, which
  puts it in a systemd slice capped at 8 of the 16 threads and 12 GB of RAM, shared by all agents. Heavy commands
  include cargo builds and tests, the game, Xvfb, ffmpeg and pnpm. Examples: `srl-cpu cargo test -j 4 -p srl-mods`,
  `srl-cpu xvfb-run ...`.
  - Pass `-j 4` to cargo.
  - Use your own target directory: `CARGO_TARGET_DIR=$HOME/.cache/srl-target/<stream>`.
  - Never leave a game, Xvfb or watcher running when you finish.
- **Never on Piotr's desktop:** start every GUI or game test inside Xvfb with
  env -u WAYLAND_DISPLAY SDL_VIDEO_DRIVER=x11 DISPLAY=<the Xvfb display>. Minecraft 26.x (SDL3) and GPUI both pick
  Wayland whenever WAYLAND_DISPLAY is set, and then they open on his real screen. That happened during milestone 1.
- **Data:** tests and manual runs set `SRL_HOME=$HOME/.cache/srl-dev/<stream>`. Keep everything big out of `/tmp`:
  it is a small tmpfs.
- **No Mojang files in git.** The game, its assets and the mod jars are downloaded at runtime only.
- **Verify before you claim:** run the tests and the real thing (an install, a game start, screenshots of the app).
- **Formatting:** `cargo fmt` (default style). Keep `cargo clippy` free of new warnings.
