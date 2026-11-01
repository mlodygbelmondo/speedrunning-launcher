# srl-app

The GPUI desktop app: scene v3 from the prototype (`../../../bakeoff/REDESIGN.md`, code in
`../../../bakeoff/gpui/src/v3.rs`), with real state. The binary is `speedrunning-launcher`.

```sh
CARGO_TARGET_DIR=$HOME/.cache/srl-target/app srl-cpu cargo run -j 4 -p srl-app
```

With arguments the same binary is a headless command line for CI and support (`src/cli.rs`): no window, the same
pipeline as the Play button, exit code 0 on success, 1 on failure, 2 on a usage error.

```sh
speedrunning-launcher --install rsg                    # Java, Minecraft, Fabric Loader, the mods, the tools
speedrunning-launcher --install custom-2               # the same for a custom instance (--launch too)
speedrunning-launcher --launch draftout --until-ready 300 --ready-cmd 'import -window root shot.png'
speedrunning-launcher --launch rsg --tools none        # the game alone (default: the saved choice)
speedrunning-launcher --status                         # what is installed, from local files only
speedrunning-launcher --tools                          # each tool's state, from local files only
speedrunning-launcher --install-tool toolscreen        # prints every installed file with its hash
speedrunning-launcher --help
```

`--launch` prints the tools it started with and, after `--until-ready`, how the game stopped (`stopped in
0.2s: exit Some(0) (asked by Some(Window(1)), escalated: None)`). CI uses `--install-tool` on Windows and hashes the
files it prints again.

On Windows the release build is a GUI program; from a terminal it attaches to the terminal's console for its
output, and when CI pipes the output it writes there.

## Layout

| File | What |
|---|---|
| `src/main.rs` | Startup, the window, the dev switches and the `SRL_SCREEN` deep links. |
| `src/app.rs` | `Launcher`, the root view: pages, navigation, the page transition, backend events. |
| `src/shell.rs` | The top nav and the account block. |
| `src/screens/*.rs` | One file per screen: `home`, `modes`, `practice`, `learn`, `tools`, `stats`, `settings` (with the credits view), `game` (Minecraft is running), `name` (first-run prompt), `import` (the settings import); `play` (Play button, progress, errors) and `field` (text input) are shared. |
| `src/v3.rs` | Tokens and primitives ported from the prototype (letter-spacing, rings, pixel corners, gradients, the fade veil), and the UI system every screen uses (below, "Buttons and labels"). |
| `src/backend.rs` | The facade and the settings file. |
| `src/backend/pipeline.rs` | What Play does, over `srl-game`, `srl-mods` and `srl-tools`: `Core::prepare_instance` (metadata, game, mods, the tools; a mode, practice or a custom instance), `Core::launch` (through the tools' launch hook, then the tools start), `Game::startup` (title screen or startup failure), `Game::wait` (the game and its tools), `Core::install_tool`, `Core::local_state` (the start-up check). Shared by the window and the CLI. |
| `src/backend/real.rs` | `RealBackend`: the pipeline on one tokio runtime on a background thread, the per-mode state machine, the update check, the launcher log. |
| `src/backend/fake.rs` | `FakeBackend`: simulated installs and game, for screenshots (`SRL_FAKE=1`). |
| `src/backend/stats.rs` | The stats extension of the facade: `StatsService`, `StatsEvent`, `StatsView` (what the pages show), `StatsSettings` (the Ranked and Draftout names) and `RealStats` over `srl-stats`. |
| `src/backend/stats_sample.rs` | `StatsPreset` and `FakeStats`: sample records and API answers for every Stats state. |
| `src/backend/tools.rs` | The tools extension of the facade: `Tools`, `ToolState`, `ToolSnapshot`, `RealTools` (srl-tools: the state from disk, installs and updates) and `FakeTools`. |
| `src/backend/account.rs` | The Microsoft account extension (srl-auth): `Accounts`, `AccountSnapshot`, the login dialog's `LoginView`, `play_as` (who a Play starts the game as), `launch_account` (the sign-in before a launch), `RealAccounts` and `FakeAccounts`. |
| `src/screens/account.rs` | The account section on Settings and the login dialog. |
| `src/backend/practice.rs` | The practice extension of the facade: `PracticeMaps`, `PracticeSnapshot`, `PracticeEvent`, `RealPractice` (srl-practice built once, and the practice game through the pipeline) and `FakePractice`. Decides what the practice instance runs (`target`, `mod_set`, `TOOLS_LIKE`). |
| `src/backend/instances.rs` | Custom instances on disk: `NewInstance`, `Created`, `CopyProgress`, `CopyPlan` (counting, then copying a game directory with progress; `NOT_COPIED`), `fix_per_mode_files` (a copy of the shared directory gets the source mode's Atum settings), `reserve` (a new number and its directory), `move_to_trash`. |
| `src/backend/importer.rs` | The settings import's extension of the facade: `Importer`, `ImportEvent`, `ImportResult`, `targets` (every mode and the practice instance, Draftout with its seeded line), `effective` (the ticks a source can bring), `RealImport` (srl-import on the blocking pool, instances whose game runs or installs left out) and `FakeImport` (`ImportPreset`). |
| `src/backend/update.rs` | The launcher's own update (srl-update): `Updates`, `UpdateState`, `RealUpdates` (when a build checks, the check on start and by hand, the swap right after the verified download, Restart) and `FakeUpdates` (`UpdatePreset`). Settings → About shows it; the gear in the top nav carries an accent square while an update is ready. |
| `src/screens/import.rs` | The Import page (`ImportView`): the instances found, "Choose folder…", the four ticks, Import, the result; the first start's offer. |
| `src/cli.rs` | The headless command line. |
| `src/catalog.rs` | Static data: modes, their layers, the practice maps (srl-practice's catalogue, the one source), Practice's categories and tools (`assets/practice.json`), Learn's topics and guides (`assets/learn.json`). |
| `src/copy.rs` | UI strings from `assets/copy.json` and `assets/copy.pl.json`, and the current language. |
| `src/demo.rs` | The prototype's sample data (`assets/demo/`), only with `SRL_DEMO=1`. |
| `src/assets.rs`, `build.rs` | Fonts, copy and art embedded in the binary. |

## Buttons and labels

Every screen takes its buttons and labels from `src/v3.rs`, never its own:

- **Buttons.** One primary per view: `play_button` (the notched Play, also Stop, Import, Continue
  and a dialog's confirmation) or its small form `primary` (a section's or a dialog's main action:
  Sign in with Microsoft, Done, How to fix it). `secondary` is outlined (Mods, Import…, Open
  folder, Add your own .jar…, Install and Play on a practice row, Install and Update on a tool
  card); `secondary_off` is one that has to wait. `tertiary` is text over a hairline (Cancel, Back,
  Sign out, Use, Reset map, the author's links); `tertiary_off` waits. `chip` is the Mods page's
  mode tab and, with a tick, the Tools page's "start with the game" toggle; `chip_off` is a mode
  whose rules ban the tool. `disclosure` opens and closes a block ("About this map").
- **Labels.** Tracked caps (`caps`) only in the top nav, on the primary buttons and in one
  `eyebrow` per hero (Home, the game screen, the name prompt, the data-folder problem). Other
  pages put the same line in sentence case (`kicker`: Modes, Mods, Learn). Section heads are
  `section_head`, a label over a value or a column is `label`, a value with its label is `fact`,
  a status word is `status_text` (`status_dot` with its square). `upper` never upper-cases a
  version ("v1.2.2").
- **Fonts.** Prose in Inter (`inter`); IBM Plex Mono (`plex`) only for times, numbers, versions,
  sizes and paths; Archivo SRL for titles and caps.
- **Scrolling lists** (Practice, Mods, Learn) fade into the page at an edge where rows are hidden
  (`fade_above`, `fade_below`); Settings fades at the window's bottom edge.

## The Backend facade

`src/backend.rs` is the only path from the UI to `srl-core`, `srl-game`, `srl-mods` and `srl-tools`. The UI reads a `Snapshot`
once at start, then applies `Event`s; it asks for work only through the trait:

```rust
pub trait Backend {
    fn snapshot(&self) -> Snapshot;          // modes[5] (state + info) and settings, no I/O
    fn play(&self, mode: Mode);              // install or update if needed, then launch
    fn cancel(&self, mode: Mode);            // stop an install (state goes back) or the game
    fn set_settings(&self, settings: Settings); // everything on Settings; persisted
    fn open_data_folder(&self);              // the data folder in the system file manager
    fn tools(&self) -> &dyn Tools;           // Ninjabrain Bot, Toolscreen, waywall
    fn stats(&self) -> &dyn StatsService;    // SpeedRunIGT records, MCSR Ranked, Draftout
    fn account(&self) -> &dyn Accounts;      // Microsoft accounts (srl-auth)
    fn import(&self) -> &dyn Importer;       // the settings import (srl-import)
    // Custom instances (below): Play and Stop, and their lifecycle; results come as events.
    fn play_custom(&self, id: CustomId);
    fn cancel_custom(&self, id: CustomId);
    fn create_instance(&self, spec: NewInstance);          // Event::InstanceCreated
    fn duplicate_instance(&self, from: Instance, name: String, with_worlds: bool); // InstanceCopy, then InstanceCreated
    fn delete_instance(&self, id: CustomId);               // Event::InstanceDeleted
    fn add_instance_jar(&self, id: CustomId, path: PathBuf); // Event::InstanceJarAdded
    fn open_game_folder(&self, instance: Instance);
}

pub trait Importer {
    fn snapshot(&self) -> ImportSnapshot;    // the instances found so far; no I/O
    fn scan(&self);                          // look again: ImportEvent::Sources
    fn inspect(&self, dir: PathBuf);         // a picked folder: ImportEvent::Chosen
    fn import(&self, source: Source, choice: Choice); // ImportEvent::Progress, then Done
}

pub trait StatsService {
    fn scan(&self);                          // scan the records; the answer comes as Event::Stats
    fn ranked(&self, name: &str, refresh: Refresh);   // Never: the cache only; IfStale; Force
    fn draftout(&self, name: &str, refresh: Refresh);
}

pub trait Tools {
    fn snapshot(&self) -> Vec<ToolSnapshot>; // every tool, in ToolId::ALL order, no I/O
    fn install(&self, tool: ToolId);         // install or update; progress comes as Event::Tool
    fn cancel(&self, tool: ToolId);          // stop an install; the state goes back
}

pub trait Updates {
    fn snapshot(&self) -> UpdateState;       // Unavailable, Idle, Checking, UpToDate, Downloading, Ready, Manual, Failed
    fn check(&self);                         // "Check now": check, download, verify, swap; Event::Update
    fn restart(&self) -> bool;               // start the new exe; true: quit now
}

pub trait Accounts {
    fn snapshot(&self) -> AccountSnapshot;   // accounts, the active one, the login dialog; no I/O
    fn login(&self);                         // opens the dialog; browser and phone code at once
    fn cancel_login(&self);                  // Cancel: the listener closes, the polling stops
    fn close_login(&self);                   // Done / Close after the login ended
    fn logout(&self, uuid: Uuid);
    fn set_active(&self, uuid: Uuid);
    fn refresh_in_background(&self);         // once, after the first frame
}
```

- **Per-mode state** (`ModeState`): `NotInstalled`, `Installing(Progress)`, `Ready`, `UpdateAvailable`, `Running`,
  `Failed(Failure)`. `srl_core::Progress` is the stage, done, total and a `Unit` (`Bytes` or `Files`) that says what
  the numbers count. A `Failure` has a kind (`Offline`, `Install` or `Launch`) that the UI turns into one translated
  line, a `FailDetail` (the core's error, the game's exit code and its last log message, or Fabric's reason) that
  the UI words in the player's language around the quoted text, whether the files are still installed, and the game
  log. The last message comes from the log4j events in the game log (`pipeline::last_message`), never their XML.
  `ModeInfo` carries optional facts (mod count, size on disk, download size, what an update updates, the name and
  memory a running game was started with, the Java version once installed); `None` hides them in the UI.
  `mode_needs(mode)` gives a mode's Java major and its memory floor.
- **Tools** (`ToolState`): `Unsupported` (not on this OS), `NotInstalled`, `Installing(Progress)`, `Installed`,
  `UpdateAvailable`, `Failed(ToolFailure)`. A `ToolSnapshot` adds the installed and latest versions and the download
  size, and for waywall whether the system has it and where; `ToolId::info()` (srl-tools) has the name, authors,
  licence, homepage and the systems it runs on. `RealBackend` serves `RealTools`: on start each tool's state comes from its files on disk (no network),
  the latest version and its download size from the pins in `srl-tools`; Install and Update go through
  `Core::install_tool`, which checks every hash and runs one tool install at a time, shared with Play. waywall is
  never installed by the launcher (the page says to use the system's package manager); Toolscreen is Windows-only.
  `mode_rule(tool, mode)` says why a mode can't start a tool (Draftout bans Ninjabrain Bot; Toolscreen isn't set up
  for Draftout yet), and the page shows that mode's chip struck through.
- **Stats** (`StatsService`, `src/backend/stats.rs`): one `srl_stats::Stats` built in `RealBackend`. `scan` reads
  the SpeedRunIGT records (this launcher's instances and `~/speedrunigt`) on the runtime, coalesced to one scan
  running and at most one queued, and answers with `StatsEvent::Local`. `ranked` and `draftout` answer with the
  cached profile (`Refresh::Never`) or the API's (`IfStale`, `Force`); a repeat request for a name while one is out
  is dropped. The app keeps the answers in `StatsView`, whose `apply` never lets a cache-only answer replace newer
  data. The app scans at start, when Stats opens and when a game exits; it reads the API cache at start and asks
  `IfStale` when Stats opens, so Home and Stats show cached numbers without the network.
- **Lookup names** (`StatsSettings`, on Settings under Statistics): the Ranked and the Draftout name. An empty one
  falls back to the active Microsoft account's name (`stats::active_microsoft_name`, over
  `AccountSnapshot::active_name()`); with no name Stats asks for one and links to Settings. Names that fail
  `srl_stats::is_lookup_name` are dropped on load.
- **Microsoft accounts** (`Accounts`, `backend/account.rs`): `RealBackend` builds srl-auth's `Auth` once and serves
  `RealAccounts`. Without a client ID in the build (`SRL_MSA_CLIENT_ID` at build or run time) login is off:
  `AccountSnapshot::available` is false, Settings shows only srl-auth's "not available in this build yet" line, and
  every mode plays offline. With one, Settings lists the accounts (a face cut from the skin, which comes only from
  textures.minecraft.net and is cached in `meta/skins/`), marks the active one and the ones that need a new login,
  and opens the login dialog. The dialog shows the browser path and the phone code with its QR side by side, then
  the chain steps (Xbox Live, Minecraft, the profile), the result or srl-auth's error (`copy_key()`, with its
  `help_url()` as "How to fix it"). Errors and srl-auth's copy keys are merged into `assets/copy*.json`.
- **Who plays** is decided in one place, `account::play_as`: no Microsoft account active (or login off), every mode
  plays offline with the Settings name, as before (Ranked and Draftout keep their "needs Microsoft login" note); an
  active one plays every mode. With login on, offline opens only after someone has signed in here once
  (`AccountList::signed_in_once`, kept after a logout); before that Play stops with `SIGN_IN_FIRST` and its main
  button, "Sign in", opens the login dialog (`Fix::SignInFirst`). Play then signs in (`Stage::SigningIn`, after the install, before the game starts:
  `Auth::prepare_launch`) and holds `mark_in_use` while the game runs. A sign-in that couldn't be renewed but is
  still valid starts the game anyway, and the game screen says until when; a revoked one stops Play (`FailKind::SignIn`)
  and opens the login dialog; an unreadable keyring says to unlock it. The game gets `--userType msa`, the profile
  id and the token through srl-game's `GameAccount::Microsoft`; every logged and shown command has the token
  replaced (`<access token>`). Home and the game screen name the account; `AccountSnapshot::active_name()` is
  the Microsoft name for other screens (the default Ranked and Draftout names on Stats).
- **Import my current settings** (`Importer`, `backend/importer.rs`, over srl-import): the Import page is offered once
  on the first start, after the name prompt (only without a deep link, and only when the scan finds something: with
  nothing found it goes on to Home by itself), and Settings opens it from General ("Import…"). It lists the instances
  srl-import finds (best match selected: 1.16.1, then the most recently played), "Choose folder…" (the system's folder
  picker; an instance folder, its game directory or a launcher's folder), four ticks (game settings, mod settings and
  resource packs on, worlds off; one the instance lacks, or that no mode's Minecraft version can take, is off) and
  Import. The import goes into every mode and the practice instance: the ones on the source's Minecraft version get
  everything ticked, the others (Draftout, from a 1.16.1 instance) only key bindings, mouse sensitivity and FOV, with
  `onboardAccessibility:false` kept (`pipeline::DRAFTOUT_SEED`). Import waits while a game runs or installs (the
  game writes its options.txt when it quits); `RealImport` also leaves such an instance out. The result names what
  went where, where the replaced files are (`instances/<id>/import-backup-<unix time>/`), worlds that were already
  there and files left out. Skip, Back and Continue set `import_offered` in the settings, so the offer never comes
  back.
- **Custom instances** (`Instance::Custom(CustomId)`, `instances/custom-<n>`): the definitions are
  `Settings::custom_instances` (`CustomInstance`: `id`, `name`, `base`, `memory_mb` (`None`: the base mode's),
  `mods` (a `ModChoice` like `mod_choices`), `jvm_args`, `created_ms`); their state is `Snapshot::customs`, changed
  by `Event::CustomState` and `Event::CustomInfo` as a mode's by `State` and `Info` (the catalogue and concerns
  against the base mode's set). The base mode gives the game (`srl_game::profile(base)`), the legal set
  (`srl_mods::resolve` for the base, then the instance's `ModChoice`), the tools and their rules
  (`Settings::tool_selection_of`) and the memory fallback (`Settings::memory_mb_of`); the instance's JVM arguments
  (`validate_jvm_args`: no `-Xmx`/`-Xms`, no `-cp`/`-jar`, no Java agents, each one option) go right before the
  main class. Play is the modes' pipeline with the instance's own record, stamp, log, pid file and `mods/`, one Play
  per game directory, and Draftout's `options.txt` seed for a Draftout base; it never touches the shared directory's
  Atum settings. Startup reads each one's state from disk, and the update check fills in their Mods pages.
  - **The backend owns which exist.** `create_instance` (a name `Settings::validate_instance_name` takes, a base,
    optionally another instance's option files, `config/` and resource packs), `duplicate_instance` (the whole game
    directory but `instances::NOT_COPIED`, `saves/` only `with_worlds`; base, memory, mods and JVM arguments from the
    source: a mode's own, a custom instance's, practice as RSG with no mod changes) and `delete_instance` add and
    remove definitions, save the settings and send `Event::CustomInstances` (the list the UI puts into its settings
    copy) before their result. `set_settings` takes the UI's changes to existing ones (name, memory, mods, JVM
    arguments; an invalid one keeps the old value) but never adds, removes or rebases one, and sends
    `CustomInstances` when it had to keep something. Rename, memory, JVM arguments and mods are plain settings
    changes: `Settings::rename_custom`, `custom_mut`, `set_custom_mod`, `add_custom_jar_to`.
  - **A copy of the shared directory** carries the source mode's own Atum settings (`shared::per_mode_file`), so an
    SSG seed never reaches an RSG-based copy. A duplicate copies on the blocking pool, counting the files first, with
    `Event::InstanceCopy` at most every 50 ms; a file it can't read is left out and named in `Created::skipped`,
    links are never followed. A failed copy leaves nothing behind.
  - **Delete** is refused while the instance installs, runs or is being copied from; otherwise its directory moves
    to `<data>/trash/custom-<n>-<unix time>/` (a rename) and its record, stamp and pid file go. New instances take
    the next number whose directory doesn't exist (`Settings::next_custom_id`, `instances::reserve`).
  - **Not yet:** Stats don't count a custom instance's runs (srl-stats attributes records to the modes' own
    directories), and the import doesn't offer custom instances as targets.
- **Events** (`Event::State`, `Event::Info`, `Event::Tool`, `Event::Stats`, `Event::Account`, `Event::Import`, and the
  custom instances' `CustomState`, `CustomInfo`, `CustomInstances`, `InstanceCreated`, `InstanceCopy`,
  `InstanceDeleted`, `InstanceJarAdded`) go over a `futures`
  unbounded channel given at creation, from any thread. The app drains it on the GPUI executor and calls `notify` once per batch.
- **Rules:** methods return at once and never block the UI thread; idle sends nothing, so an idle app renders no
  frames; `play` does nothing while the mode is already installing or running.
- **Settings** live in `<Paths root>/settings.json` (`$SRL_HOME`), written atomically: player name, language,
  memory (`max_memory_mb`, plus `mode_memory_mb` per mode), the last selected mode (the launcher opens on it), the
  first start's import offer being done (`import_offered`), the optional mods (`accessibility_mods`,
  `draftout_extras`, both off by default; `Settings::resolve_options()` hands them to `srl-mods`) and which modes
  start each tool (`tool_autostart`; by default srl-tools' choice: Ninjabrain
  Bot and Toolscreen in every mode that allows them, waywall in none; `Settings::starts_with(tool, mode)`) and the
  custom instances (`custom_instances`, left out of the file while empty). Every new field has a default, so older
  files still load.

### RealBackend

The app runs `RealBackend` (`src/backend/real.rs`). It owns one tokio runtime on a background thread; the UI thread
only reads the snapshot and hands work to the runtime.

- **On start**, each mode's state comes from `Core::local_state`: the install record that the last successful
  install wrote (`meta/launcher/<mode>.json`) and a stat of every file it names. No network and no hashing, so an
  installed mode shows Ready at once, offline too. A mod jar that is missing or changed shows UpdateAvailable.
- **The update check** then runs in the background. It fetches the mod lists, resolves every mode's set, fills in
  the mod counts and compares installed modes with their `mods/`; a difference turns Ready into UpdateAvailable,
  with the mods named.
- **Play** runs the pipeline in a task:
  1. Installing: metadata, then Java, Minecraft and the assets in parallel (one bar, their bytes added up), then the
     mods, then the tools the mode starts with (`Stage::Tools`: what is missing or out of date). Installs run one at
     a time, because modes share files; a Play queued behind another shows "waiting for the other download".
  2. Running, once the game process exists. It starts through the tools' launch hook (waywall may wrap the command),
     then Toolscreen is injected (Windows) and Ninjabrain Bot starts next to it, unless the player already runs one.
     The game screen names the tools it started with; a tool that couldn't start is left out with a note there and
     never blocks the game.
  3. Ready when the game exits and the tools started for it have closed (Ninjabrain Bot gets its window closed, then
     a kill after 5 s).

  If the game dies before its title screen, the mode shows Failed with the exit code and the log path. If Fabric
  stops the start with its error window (a mod refused to load), the launcher ends the game and shows Fabric's
  reason. The mod lists are fetched again when they are older than 10 minutes, the game's lists after an hour, so
  a repeat Play needs no network: about 0.04 s from Play to the game process. A list with a cached copy gets one
  try of at most 4 s, so a network that drops packets delays Play by seconds, not minutes.
- **Memory:** set per mode on Settings (`Settings::memory_mb(mode)`: the mode's own value, else the shared one),
  raised to the mode's minimum (Draftout: 3 GB). A saved value Settings doesn't offer snaps to the nearest choice.
- **Optional mods:** a change re-runs the update check, so a mode installed without them shows UpdateAvailable.
- **A data folder that can't be written** (a read-only drive, a bad `SRL_HOME`) is found on start; the window then
  shows only that, with the folder and the error.
- **Cancel** drops an install where it is (downloads go to `.part` files) or asks the game to quit so it saves (its
  window gets `WM_DELETE_WINDOW` on X11, `taskkill` without `/F` on Windows; SIGTERM without a window), and kills
  it after 10 s (srl-game `GameProcess::stop_detailed`). The log says how it stopped: how long it took, the exit
  code, what asked it to quit and whether it had to be killed.
- **A game from an earlier launcher session** is found through `logs/<mode>.pid`; while it runs, Play refuses to
  touch that mode's mods. A Ninjabrain Bot it left running (`logs/tools/ninjabrain-bot.pid`) is taken over on start
  and stopped once that game is gone.
- **The log** goes to stderr and `logs/launcher.log` (the previous run's is `launcher-previous.log`).
- **The account** every game starts with (modes and practice) is chosen in one place, `account::play_as` (below):
  offline with the saved player name unless a Microsoft account is active. Play never waits for an account.

### Practice

`Backend::practice()` (`src/backend/practice.rs`) is the Practice page's side: the maps and the practice game.
`RealBackend` builds srl-practice's `Practice` once, with the core's downloader, and reads every map's state in the
background on start and whenever the page opens (`PracticeSnapshot::loaded`).

- **Maps**: Install (the size from the catalogue), Update, Reset and Remove run one at a time per map, with
  srl-practice's progress (`Verifying`, `Downloading` in bytes, `Extracting` in files) on the row. Cancel exists
  only while downloading; once extracting starts, srl-practice finishes on its own. Reset and Remove always ask
  first (`Question::Reset` with `plan_reset`, `Question::Remove`); Update asks only when a world was played
  (`UpdatePlan::Ask`, three choices). A failure is one line per kind (`MapFailKind`), worded as srl-practice says.
- **The practice game**: Play installs the practice instance (`instances/practice`: srl-practice's `PRACTICE`
  game and mods through `Core::prepare_instance(Instance::Practice)`, with srl-mods' legal checks) and starts it.
  It has its own log (`logs/practice-latest.log`), pid file (`logs/practice.pid`) and install record
  (`meta/launcher/practice.json`). Tools, their rules and memory are RSG's (`TOOLS_LIKE`: the same 1.16.1 game), the
  account and its sign-in are the modes' (`account::launch_account`). It runs next to a mode's game the way two modes do; it doesn't open the game
  screen: the map's row says "In the game: Singleplayer, then <world name>" (1.16.1 can't open a world from the
  command line). Reset, Update and Remove are greyed out while it starts or runs.

`FakeBackend` (`src/backend/fake.rs`) simulates installs and a running game on its own thread, for modes and custom
instances alike (a custom instance's create is instant, a duplicate "copies" for 2.5 s with progress, a delete only
logs where the folder would go): a full install in
about 9 s, an update, a running game until `cancel`, and, with `SRL_FAKE_FAIL=1`, a failure at 40% of the assets
stage on the first `play`. It persists settings like the real one. `SRL_FAKE=1` selects it; `SRL_DEMO` and the screen
ids with a state (`installing`, `error`, `crash`, `stopped`, `running`, `tools-*`, `account*`, `login*`, `msa-*`,
`practice-<state>`) imply it, since those states exist only in the fake. Its tools are `FakeTools`: Ninjabrain Bot
installed, Toolscreen not on Linux, waywall not installed, and a 3 s install. Its accounts are `FakeAccounts`
(`AccountsPreset`): login off unless `SRL_MSA_CLIENT_ID` is set, as in a real build; a login takes 4 s, then a second
per step. With a Microsoft account active its Play has a signing-in step. Its practice is `FakePractice`: nothing
installed (with `SRL_DEMO` the prototype's states), a 4 s map install, a practice game that starts in 2 s and runs
until Stop. Its stats are `FakeStats` with a `StatsPreset` (`src/backend/stats_sample.rs`): fixed sample records
(RSG and FSG PBs with splits, Ranked practice runs, resets) and API answers, answered at once; the preset picks the
state (sample, first scan, no records, unknown name, cached and offline, API unreachable, a new player). The
`stats-*`, `settings-stats` and `home-*-stats` screen ids pick a preset; `SRL_DEMO` uses the sample.
`open_data_folder` only logs the folder (`fake: would open ...`).

## Real data only

Without `SRL_DEMO` the app shows only what it knows. Stats shows the player's SpeedRunIGT records (PB and sum of
best per mode, recent runs with splits against the PB, notes on files it skipped or could not read) and, for the
names on Settings (else the active Microsoft account's name), the MCSR Ranked profile (elo, tier, season record,
recent matches, the Weekly Race) and the Draftout elo, with the age of cached data and "offline" when a refresh
failed. Home shows the same cached numbers where the design has a place for them (Ranked: elo, record, best time,
recent matches; RSG, FSG and SSG: PB, sum of best, runs this month, the PB's splits); with no data those places stay
empty. Players online is not shown. `SRL_DEMO` shows the design with the prototype's sample data, and its Stats
uses the fake's sample. Practice installs the maps (srl-practice) and the practice game; the practice tools are
listed only, their installs come later. Without an active Microsoft account Ranked and Draftout say that online
play needs a Microsoft login.

Learn is real: other runners' guides in `assets/learn.json`, each with its author, opened in the browser
(`open_url`), never embedded. Every link was checked live; `tools/check-learn-links.sh` checks them again.

## Dev switches

| Variable | Effect |
|---|---|
| `SRL_FAKE=1` | Uses `FakeBackend`: simulated installs, no game, no network. The screenshot tools set it. |
| `SRL_SCREEN=<id>` | Opens on a screen (below). |
| `SRL_LANG=en\|pl` | The UI language for this run, without changing the saved setting. |
| `SRL_DEMO=1` | Fills in the prototype's sample data (account, ranked, records, statuses), for design comparison only. Saves no settings. |
| `SRL_FAKE_FAIL=1` | With the fake: the first install fails, to see the error and the retry. |
| `SRL_TRACE_RENDERS=1` | Logs every rendered frame and every backend event to stderr (used by `tools/idle-check.sh`). |
| `SRL_MSA_CLIENT_ID=<guid>` | Turns Microsoft login on (srl-auth; normally set at build time). Never borrow another launcher's ID. |
| `SRL_MSA_BASE_URL=<url>` | Sends every Microsoft, Xbox and Minecraft request to test services instead (loopback `http` only, else ignored): srl-auth's mock, `cargo run -p srl-auth --example mock-services`, whose client ID is `11111111-2222-3333-4444-555555555555`. The "browser" then only fetches the sign-in page; no browser opens. Skins aren't fetched. |
| `SRL_MSA_NO_BROWSER=1` | With `SRL_MSA_BASE_URL`: the browser can't be opened (the dialog shows the link to copy). |
| `SRL_KEYSTORE=file` | srl-auth keeps the tokens in its encrypted file instead of the system keyring (tests). |
| `SRL_IMPORT_HOME=<dir>` | The settings import searches this folder as if it were the home folder, and nothing else (tests with fake MultiMC, Prism, MCSR Launcher and `.minecraft` trees). |
| `SRL_IMPORT_PICK=<dir>` | "Choose folder…" takes this folder instead of opening the system's folder picker (tests: the picker is a desktop portal on Linux, which would open on the real desktop). |
| `SRL_UPDATE_URL=<url>` | The launcher's update check asks this URL for the latest release's JSON instead of GitHub's API, and runs on Linux too (still only in a CI-like build with `SRL_COMMIT`: `GITHUB_SHA=<sha> cargo build`). For tests with a local server; the swap then replaces the running binary. |
| `SRL_DEV_GAME_HOME=<dir>` | Every game this launcher starts (modes and practice) gets `-Duser.home=<dir>`, so a test game keeps what mods write to the home folder (SpeedRunIGT's records) out of the real one. For tests; the app has no JVM arguments setting. |

## Screen ids

`<mode>` is `ranked`, `rsg`, `fsg`, `ssg` or `draftout` (default `ranked`).

| Id | Shows |
|---|---|
| `home`, `home-<mode>` | Home with that mode selected. |
| `modes`, `modes-<mode>` | Modes with that mode selected. |
| `practice`, `practice-<category>`, `practice-tools` | Practice; categories `all-in-one`, `overworld`, `bastion`, `blaze`, `stronghold`, `end`. Nothing installed. |
| `practice-installed` | Overworld with maps installed (BT Practice fresh and open, Portal Practice v2 played; Zero Practice with an update, The MCSR Practice Map and End Portal Fill elsewhere). |
| `practice-about` | The same with BT Practice's "About this map" open. |
| `practice-installing` | All-in-one with The MCSR Practice Map downloading (40 %, Cancel). |
| `practice-update` | End with the update question for Zero Practice (played; its loadouts carried over). |
| `practice-reset` | End with the reset confirmation for Zero Practice (keep the loadouts). |
| `practice-remove` | Overworld with the remove confirmation for BT Practice. |
| `practice-starting` | Overworld with the practice game installing for BT Practice (mods). |
| `practice-running` | Overworld with the practice game running: "In the game: Singleplayer, then ...", map changes greyed out. |
| `practice-error` | Overworld with failed changes: Crafting Practice v2 offline, Portal Practice v2 open in the game. |
| `practice-link` | Stronghold with Llama's Practice Map open: only on the author's page, and why. |
| `learn`, `learn-<topic>` | Learn with that topic selected: `setup`, `resets`, `bastions`, `nether`, `eyes`, `stronghold`, `end`, `ranked`, `draftout`. |
| `stats` | Stats. With the real backend the player's records and API data; with the fake (or `SRL_DEMO`) the sample. |
| `stats-scanning` | The first scan of the records, still running (the skeleton). |
| `stats-empty` | No records and no names: the empty state and the name prompt. |
| `stats-unknown` | The Ranked and Draftout names are not found. |
| `stats-offline` | Cached API data 3 h old with "offline", 3 skipped files and a folder that can't be read. |
| `stats-unreachable` | No cache and the API can't be reached (Ranked offline, Draftout rate-limited). |
| `stats-new` | A new player: a few resets, nothing finished, Ranked placements not done. |
| `settings-stats` | Settings with the Statistics names filled in. |
| `home-stats`, `home-<mode>-stats` | Home with the sample stats (every mode installed). |
| `tools` | Tools: Ninjabrain Bot installed, Toolscreen not on this system, waywall not installed. |
| `tools-installing`, `tools-update`, `tools-failed` | Tools with waywall installing, Ninjabrain Bot's update out, waywall's install failed offline. |
| `settings` | Settings. |
| `settings-credits` | The credits view of Settings (`assets/credits.md` or `credits.pl.md`, and the licences of adapted code from THIRD_PARTY_NOTICES.md). |
| `name` | The first-run name prompt (also shown on its own while no player name is saved). |
| `import` | The Import page as the first start offers it (Skip): MultiMC's RSG selected, Prism's Ranked, MCSR Launcher's MCSR Ranked and the official `.minecraft` (1.21.4). |
| `import-scanning`, `import-none`, `import-other` | The same while the scan runs; with nothing found; with only instances on other versions (only keys, mouse and FOV can come over). |
| `import-importing`, `import-done` | An import 412 of 1038 files through; the result (everything into the 1.16.1 modes, keys into Draftout, RSG's backup). |
| `mods`, `mods-<mode>` | The Mods page of the mode (Ranked by default): the mode's set first, then every other mod the lists know for its version; `SHOOT_SETTINGS='"mod_choices":{...}'` shows changes and their concerns. |
| `installing`, `installing-<mode>` | Home with the mode installing (assets, 148 of 352 MB). |
| `error`, `error-<mode>` | Home with the mode's last install failed. |
| `crash`, `crash-<mode>` | Home with the game closed while starting (exit code 1 and its last log message). |
| `stopped`, `stopped-<mode>` | Home with Fabric's stop reason (Draftout's disallowed mods). |
| `running`, `running-<mode>` | The game screen: player, account, Minecraft version, memory and the tools it started with. |
| `account`, `account-list` | Settings with Microsoft login on: no account yet; two accounts (one active, one that needs a new login) with the file-keystore note. |
| `login`, `login-nobrowser` | The login dialog waiting: browser path and phone code with QR; the same when no browser opened (the link to copy). |
| `login-steps`, `login-done`, `login-error` | The dialog during the chain (Xbox Live done, Minecraft running), after it (logged in), and failed at XSTS (a child account, with "How to fix it"). |
| `msa-home`, `msa-signing` | Ranked on Home with a Microsoft account active; the same while Play signs in. |
| `msa-running`, `msa-warning` | The game screen with a Microsoft account; the same when the sign-in couldn't be renewed ("valid until"). |
| `msa-relogin`, `msa-keyring` | Play stopped by the account: the sign-in was revoked (Log in again), the keyring is locked. |
| `msa-sign-in-first` | Play stopped before anyone has signed in, in a build that can log in: offline opens after one sign-in (Sign in). |
| `update-ready` | Home while a new version of the launcher is ready: the accent square on the gear. |
| `update-about`, `update-blocked` | Settings → About with 0.2.0 ready (Restart to update); the same while RSG installs (Restart waits, and says why). |
| `update-checking`, `update-current`, `update-downloading` | About while the check runs, up to date, 0.2.0 downloading (26 MB). |
| `update-manual`, `update-error`, `update-local` | About when the launcher's folder can't be written (the releases page), after a check by hand failed offline, in a local build (no update). |

## Tools

- `../../tools/shoot-app.sh <outDir> [ids...]`: screenshots at 1280×800, scale 1, in Xvfb (`SRL_LANG`, `SRL_DEMO`,
  `SHOOT_FORMAT=png` for pixel diffs, `SHOOT_SETTINGS` for extra settings fields). A black capture is retaken.
- `../../tools/compare-shots.sh <protoDir> <appDir> <diffDir> <ids...>`: pixel diff of `SRL_DEMO` shots against the
  prototype's GPUI shots.
- `../../tools/idle-check.sh`: frames and CPU time while idle, installing and running.
- `../../tools/idle-pages.sh <ids...>`: frames drawn while idle on each screen id (must be 0).
- `../../tools/check-learn-links.sh`: the HTTP status of every Learn link, and YouTube's oEmbed status for videos (a
  removed video still answers its watch page with 200). No `srl-cpu` needed.

Run them through `srl-cpu`. Screenshots of milestone 1 are in `../../shots/m1/`, of milestone 2 (Settings, Tools,
Learn, Stats, the startup failures and the end-to-end run with the tools) in `../../shots/m2/`, of the Microsoft
account (every screen id above in English and Polish, `*-pl.jpg`, and `e2e-*` from a run against srl-auth's mock
services up to the game's title screen) in `../../shots/m3/account/`, of Stats and Practice by their streams in
`../../shots/m3/stats/` and `../../shots/m3/practice/`, and of milestone 3 as merged: every screen id in
`../../shots/m3/en/` and `../../shots/m3/pl/`, the end-to-end run in `../../shots/m3/e2e-*.jpg`.
