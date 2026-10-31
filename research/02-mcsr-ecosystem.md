# 02 — MCSR ecosystem research (state as of 2026-10-23)

Scope: what a new-player-focused Minecraft speedrun launcher ("Speedrunluncher") needs to know about modes, legal mods,
external tools, layering, competition, Java, rules and gaps. Practice maps / tutorials are covered elsewhere.

Method: primary sources only (speedrun.com API, the `Minecraft-Java-Edition-Speedrunning` GitHub org, tildejustin's
`mcsr-meta`, Modrinth API, GitHub API for repos/licences/releases, MCSR Ranked wiki + jar inspection, draftoutmc.com,
MCSR Launcher source + meta). Every jar targeting 1.16.1 listed in `mcsr-meta` was downloaded and hash-checked
(all sha1/sha512/size matched). Anything not confirmed from a primary source is marked **(unverified)**.
Note: the web-search quota ran out near the end of the research, so a few "community folklore" items (GraalVM
performance claims, exact pain-point frequency) could not be cross-checked and are flagged.

---

## 0. TL;DR — the facts that drive the design

1. **The single source of truth for legal mods is machine-readable.** The legal authority is
   [`legal-builds.csv`](https://github.com/Minecraft-Java-Edition-Speedrunning/legal-mods/blob/main/legal-builds.csv)
   (sha512 per legal jar, rule A.8.5). The consumable manifest is tildejustin's **mcsr-meta schema 7**:
   `https://meta.mc.sr/schema-7/mods.json` (CDN, byte-identical to
   `https://raw.githubusercontent.com/tildejustin/mcsr-meta/schema-7/mods.json`). It has per-version MC targets,
   download URL, sha1, sha512, size, dependencies, incompatibilities, traits (`rsg-only`, `ssg-only`, `mac-only`,
   `accessibility`) and recommended/obsolete flags. mc.sr/mods, ModCheck, MCSR Launcher, Jingle **and the MCSR Ranked
   mod's own anti-cheat** all read it.
2. **Launchers are whitelisted.** speedrun.com rule A.1.1 (rules v8, 2026-10-17) lists the only allowed launchers:
   official, MultiMC, Prism, Modrinth App, ATLauncher, MCSR Launcher, Pandora, Nixcraft. A new launcher must be
   legalized via a thread ("only allowed if they do not significantly modify the game"). **Until then, runs made
   with Speedrunluncher are not leaderboard-legal.** ([rules.typ](https://github.com/Minecraft-Java-Edition-Speedrunning/rules/blob/main/rules.typ))
3. **Draftout** = a closed-source Fabric **mod** (by 7rowl and Mar1n, Modrinth `draftout`) providing a ranked 1v1
   Lockout/Bingo game with a draft phase, on **Minecraft 26.3, Java 25**. It is not a 1.16.1 mode and shares almost
   nothing with the RSG stack beyond "speedrun-legal 26.3 mods". ([draftoutmc.com](https://draftoutmc.com/), [Modrinth](https://modrinth.com/mod/draftout))
4. **Toolscreen** = jojoe77777's MIT C++ **DLL injected into the Minecraft JVM** (Windows 10/11 x64/ARM64 only) for
   instant resizing (Thin/Wide/EyeZoom), in-game overlays/mirrors (e.g. Ninjabrain), key rebinds — "all without OBS
   projectors or leaving fullscreen". First release 2026-02-14; it replaced Jingle's resizing script/EyeSee in
   Jingle 2.0. Its DLLs are whitelisted in [`legal-dlls.csv`](https://github.com/Minecraft-Java-Edition-Speedrunning/legal-builds/blob/main/legal-dlls.csv). ([repo](https://github.com/jojoe77777/Toolscreen))
5. **Ranked delta vs RSG = one jar (`mcsrranked-*.jar`)**, verified by unpacking the official Ranked "RSG" mrpack:
   it is exactly the mc.sr 1.16.1 RSG set + `mcsrranked`. The Ranked jar bundles (jar-in-jar) SpeedRunIGT, Fabric API
   modules, Lithium, etc. The "2 mods" claim could not be confirmed (see §1.4). Ranked is **illegal on speedrun.com**,
   so RSG and Ranked must be separate instances / mod sets.
6. **The closest competitor already exists:** RedLime's **MCSR Launcher** (Kotlin, GPL-3.0, beta 0.8.10, on the A.1.1
   whitelist) with Ranked and Draftout instance types, a legal-mod browser, practice-map browser, tool list, built-in
   Toolscreen, and Java downloads (Adoptium/Azul/GraalVM/Mojang). ([repo](https://github.com/MCSRLauncher/Launcher), [meta](https://github.com/MCSRLauncher/meta))

---

## 1. Modes / categories

### 1.1 speedrun.com categories (game `mc`, id `j1npme6p`)

Run counts pulled from the speedrun.com v1 API leaderboards on 2026-10-23
([categories](https://www.speedrun.com/api/v1/games/j1npme6p?embed=categories.variables)).

| Category | Subcategories (current) | Board size (runs) | Matters for new players? |
|---|---|---|---|
| **Any% Glitchless, Random Seed (RSG)** | Pre 1.8 / 1.8 / 1.9–1.12 / 1.13–1.15 / **1.16–1.19** / 1.20+ | **1.16–1.19: 6,751** (WR 6:39.8); 1.13–1.15: 765; Pre 1.8: 348; 1.8: 78; 1.9–1.12: 50 | **Yes — the core category.** 6,491 of the 6,751 runs (96%) are on **1.16.1**. |
| Any% Glitchless, Set Seed (SSG) | same version splits | 1.16–1.19: **1,568**; 1.8: 191; 1.13–1.15: 173; Pre 1.8: 81; 1.20+: 75; 1.9–1.12: 55 | Yes — second most popular, good for learning routes. |
| All Advancements (AA) | RSG/SSG/RS/SS × versions from 1.1–1.6 up to 1.21.11–26.1, 26.2–26.3 | RSG 1.16: 125; newer versions single digits | Niche, long runs. Later. |
| Any% Glitchless Co-op | RS/SS × Pre1.9/1.9–1.15/1.16+ × 2/3/4/5–9/10+ players | RSG 1.16+ 2p: 427; 3p: 153 | Moderate (friends). |
| Any% Glitchless (Peaceful) | RS/SS × Pre1.9/1.9+ | SS 1.9+: 113 | Niche. |
| Any% (glitched) | RS/SS × Internal/External × Pre1.9/1.9+/1.14+ | 5–81 per board | Niche. |
| AA Co-op, Demo, Time Travel, per-level | misc | small | No. |

Notes:
- **RSG 1.16+ is effectively one board.** The RSG "1.20+" board holds a placeholder run whose comment says all 1.16+
  RSG runs (including 1.20+) go to 1.16–1.19 ([run yd47e0jy](https://www.speedrun.com/mc/runs/yd47e0jy)); the
  rules section is titled "Random Seed, 1.16+" ([rules.typ](https://github.com/Minecraft-Java-Edition-Speedrunning/rules/blob/main/rules.typ)). SSG remains split 1.16–1.19 / 1.20+.
- Minecraft versions are now numbered **26.x** (AA ranges include "26.2–26.3"; rules mention "Minecraft 26.1 to 26.2").
- Category Extensions live at [speedrun.com/mcce](https://www.speedrun.com/mcce) (Kill Bosses, HDWGH, Obtain Item,
  All Portals, etc.). FSG is **not** there; it has its own game (§1.3).
- Rules versions: v5 2025-06-26, v6 2025-09-13, v7 2026-02-15, **v8 2026-10-17** (added Pandora/Nixcraft launchers,
  banned Java agents/JVM debugging, LWJGL/GLFW rules, banned timed auto-resets, wall-macro rules, pinned
  NinjabrainBot v1.5.2, banned chunkmap freezing) ([rules repo](https://github.com/Minecraft-Java-Edition-Speedrunning/rules), [v8 commit](https://github.com/Minecraft-Java-Edition-Speedrunning/rules/commit/6cff8ae)).

**Priority for a new-player launcher:** (1) RSG 1.16.1, (2) MCSR Ranked (1.16.1), (3) SSG 1.16.1,
(4) Draftout (26.3), (5) FSG (1.16.1), (6) AA 1.16.1, (7) co-op. Older-version categories (1.8, pre-1.9 with Legacy
Fabric/Ornithe) are out of scope for v1.

### 1.2 Draftout — what it is (high confidence)

| Fact | Source |
|---|---|
| A **Fabric mod** implementing a **ranked 1v1 Lockout/Bingo game with a draft phase**: players alternately pick one of two goals to build a 5×5 board, then race to claim 13 goals. Each player plays in their own singleplayer world on the same seed, connected to an external matchmaking server. "Basically just combines MCSR Ranked with Lockout." | [Modrinth `draftout`](https://modrinth.com/mod/draftout), [draftoutmc.com](https://draftoutmc.com/), [frontcage post 2026-04-21](https://frontcage.com/t/draftout-a-ranked-1v1-lockout-mode-in-singleplayer/94) |
| Authors: **7rowl and Mar1n** (Modrinth owner `mar1n`). Published 2026-04-19. **All-Rights-Reserved, closed source.** | Modrinth API, frontcage |
| **Minecraft 26.3**, Fabric Loader ≥ 0.19.5, **Java 25+**, ≥ 3 GB RAM. No Fabric API dependency. Latest 1.18.0 (2026-10-20). Up to v1.16.x it ran on 26.1.1 → it **moves MC versions** and needs a new instance when it does. | [download page](https://draftoutmc.com/download), [wiki getting-started](https://github.com/draftout/draftout-wiki/blob/main/content/docs/gameplay/getting-started.md) |
| Recommended JVM args: `-XX:StackShadowPages=32 -XX:CompileCommand=exclude,io/netty/util/internal/ReferenceCountUpdater,retryRelease0` (Java 25 crash workaround). MCSR Launcher applies these automatically for Draftout instances. | download page; [MCSR Launcher `CreateInstanceGui.kt`/`InstanceProcess.kt`](https://github.com/MCSRLauncher/Launcher) |
| Modes: Quick Play (4×4, 30 min, unrated), Competitive (5×5, 75+15 min, Glicko-2, ranks Silverfish→Warden), Lobbies (Draftout 2p, Lockout 2–4, Blackout 1–4). Filtered seed pool (11 seed types); standardized RNG (drops, barters, golems). | [wiki ranks](https://github.com/draftout/draftout-wiki/blob/main/content/docs/gameplay/ranks.md), [seeds](https://github.com/draftout/draftout-wiki/blob/main/content/docs/gameplay/seeds.md), [standardization](https://github.com/draftout/draftout-wiki/blob/main/content/docs/gameplay/standardization.md) |
| Mod **self-updates from Modrinth** when prompted in-game. Official modpack "coming soon". Community pack: [Draftout Essentials](https://modrinth.com/modpack/draftout-essentials). | Modrinth body, wiki |
| Allowed mods: speedrun-legal 26.3 mods ([mc.sr/mods?version=26.3](https://mc.sr/mods/?version=26.3)) + No Chat Restrictions, No Telemetry, CommandQMod, retiNO (mac). "The launcher blocks disallowed mods, and this must not be bypassed" (interpreted as the mod's own check — **unverified**). | [draftoutmc.com/rules](https://draftoutmc.com/rules) |
| Allowed tools: Toolscreen, waywall, SlackowWall, Jingle, Julti. **Ninjabrain Bot is banned** (and any tool reading board/game state). | download page, rules |
| Recommended launcher Prism; MultiMC works; Lunar/Feather/cracked unsupported. | download page |
| Scale: ~183k Modrinth downloads; >514k matches; public stats API `https://draftoutmc.com/api/stats` ([community spec](https://memerson12.github.io/draftout-api-spec/)). | Modrinth API |

Implication: Draftout is a **separate stack** (26.3 + Java 25 + its own mod allowlist + Ninjabrain disabled).

### 1.3 FSG (Filtered Seed Glitchless)

- Own leaderboard: [speedrun.com/fsg](https://www.speedrun.com/fsg) (id `3698293d`), one category "Any% (1.16)",
  split by filter (ZigSeedGlitchless, Seedbank, Beach) × player count. Solo Seedbank: 780 runs (latest 2026-09-21);
  ZSG: 108; Beach: 3. Active but small.
- Delivery: the **[FSG Mod](https://modrinth.com/mod/fsg-mod)** by DuncanRuns (MIT,
  [source](https://github.com/DuncanRuns/FSG-Mod)), 1.16.1 and 1.15.2, latest **5.3.0** (2026-01-22). Requires Atum,
  works with SeedQueue, no Fabric API. Seeds come from the online database **[filteredseed.com](https://filteredseed.com/)**
  (also practice seeds + token checker) or a locally installed filter. After a run, `/fsgtoken` produces a
  verification token pasted into the submission. FSG board rules require the FSG Mod for 1.16.1 filters.
- **FSG Mod is not legal for /mc** (listed under "Other Mods… not legal for main board submissions" on
  [mc.sr/mods](https://mc.sr/mods/), and in `extra.json`). So an FSG instance must be separate from the RSG instance.
- Ranked uses its **own** server-side seed filter (overworld/nether filtered independently; seed types depend on Elo)
  ([Ranked wiki: seed](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/gameplay/seed.md)); Ranked FAQ points to FSG mod for filtered-seed practice.
- "Practice FSG" runs cannot be verified (search snippet of speedrun.com/fsg news; page returned 403 — **unverified**).

### 1.4 MCSR Ranked

| Aspect | Finding | Source |
|---|---|---|
| Mod | [Modrinth `mcsr-ranked`](https://modrinth.com/mod/mcsr-ranked) (id `I9W1u5Ac`), **6.0.4 (2026-10-17)**, **1.16.1 only**, Fabric. ~2.29M downloads. **All-Rights-Reserved, closed source.** Authors RedLime, OliverMCSR, Void_X_Walker, DylanDC14, Quesia. File `mcsrranked-<v>.jar`. | Modrinth API, jar `fabric.mod.json` |
| Requirements | MC 1.16.1, Fabric Loader 0.16+ (jar declares `>=0.14`). **Java 17+ "highly recommended"** (replay downloads and Discord RPC need it). | Modrinth, [wiki FAQ](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/install/faq.md) |
| Bundled (jar-in-jar, 6.0.4) | SpeedRunIGT 16.0, Lithium backport 0.6.7, SpeedrunAPI 2.2, antigone, extra-options-ranked, Fabric-ASM, mixinsquared, QueUI, ~9 Fabric API modules, mobspawnstandardization, woodlightingstandards, xray-fix. Declares `breaks`: practiceseedmod, glacier, atum <1.1.3, speedrunigt <15.1 (and others). | jar inspection |
| Auth | **Genuine Microsoft account required** — mod calls `api.minecraftservices.com/player/certificates`; "Cracked launchers/accounts are not supported and will not work." Launcher must do a real MSA login and pass a valid access token. | jar strings, [wiki install](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/install/index.md) |
| Anti-cheat | On start it downloads `raw.githubusercontent.com/tildejustin/mcsr-meta/refs/heads/schema-7/mods.json` and **hash-checks every mod file** ("Incompatible mod set found!", "not whitelisted", "whitelisted but different version", "files do not match"). Resource packs are hash-checked too (`allowed-resourcepacks`). Replays recorded. | jar strings (`ModVerifier`, `FileVerifier`, `ResourcePackVerifier`) |
| Official install | No standalone Ranked launcher. Official `.mrpack`s per OS: `https://redlime.github.io/MCSRMods/modpacks/v4/MCSRRanked-<Windows|macOS|Linux>-1.16.1-{Basic,Basic-w-SS,RSG}.mrpack` (pack `v4+26.07.02`, Fabric 0.19.3). Guides for MCSR Launcher (beta), Prism/MultiMC, Modrinth App, official launcher. | [mcsrranked.com/download](https://mcsrranked.com/download) (JS bundle), [wiki download](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/install/download.md) |
| Updating | Mod self-updates; **only the latest version can connect.** | Modrinth body |
| Allowed launchers | Prism (recommended), MultiMC, MCSR Launcher (beta), Modrinth App, ATLauncher, official (discouraged). | [wiki rules](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/gameplay/rules.md) |
| Rules | [Guidelines](https://mcsrranked.com/guidelines) apply speedrun.com glitchless cheating rules; ban alts/smurfing/match fixing/cheat injection; macros/rebinds follow srcom (resize + one ≤16384 px out-of-bounds resolution, toggle sprint). Ninjabrain Bot, Toolscreen, Jingle, SlackowWall, resetti, waywall legal. | guidelines, wiki rules |
| Modes | Ranked, Casual, Private Rooms (solo rooms need paid tier), Weekly Race (set seed + practice/ghosts), replays, events. Season 12: 2026-10-23 → 2026-12-30. | [private_rooms](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/gameplay/private_rooms.md), [weekly_race](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/gameplay/weekly_race.md), `api.mcsrranked.com/leaderboard` |
| API | [docs.mcsrranked.com](https://docs.mcsrranked.com), base `https://api.mcsrranked.com/`, 500 req / 10 min, optional `Private-Key` header (generated in-game, Profile → Settings). | docs |

**Exact delta (verified by unpacking the Windows mrpacks on 2026-10-23):**

| Ranked pack | Contents |
|---|---|
| `Basic` | antigone, antiresourcereload, fast-reset, krypton, lazydfu, lazystronghold, lithium, sodium, speedrunapi, starlight, state-output, voyager **+ mcsrranked** |
| `Basic-w-SS` | Basic + standardsettings |
| `RSG` | antigone, antiresourcereload, atum, chunkumulator, fast-reset, forceport, hermes (0.12.5), hermes-core, krypton, lazydfu, lazystronghold, lithium, mcsrfairplay, seedqueue (1.7.1), sleepbackground, sodium, speedrunapi, SpeedRunIGT, standardsettings, starlight, state-output, voyager, worldpreview **+ mcsrranked (5.8.1 pinned; self-updates)** |

So **Ranked = RSG set + 1 jar**. The packs pin older builds than meta (SeedQueue 1.7.1 vs 1.8; Hermes 0.12.5 vs
0.16.0) — resolve from `meta.mc.sr` instead. Whether Ranked rejects older-but-still-whitelisted builds is
**unverified** (its message "whitelisted but different version" suggests it may warn). The owner's "2 mods" might come
from an older setup (e.g. Ranked + a companion) — **could not be confirmed from any current source.**

---

## 2. Legal mods

### 2.1 Source of truth and manifests

| Layer | What | URL |
|---|---|---|
| Legal authority | `legal-builds.csv` — `modid,filename,timestamp,date,sha512`, 1,118 rows. "Use of a Fabric mod jar not listed in that file at the time of the run is disallowed" (A.8.5). | https://github.com/Minecraft-Java-Edition-Speedrunning/legal-mods/blob/main/legal-builds.csv |
| Jar storage | `legal-mods` repo ("the true source of legal mods"), jars under `legal-mods/<modid>/<mc-range>/`; third-party mods as `.json` link files `{link, hash}` pointing at Modrinth CDN. | https://github.com/Minecraft-Java-Edition-Speedrunning/legal-mods |
| Conditional | `{"ssg-only": ["chunkcacher","setspawnmod"]}` | https://raw.githubusercontent.com/Minecraft-Java-Edition-Speedrunning/legal-mods/main/conditional-mods.json |
| **Manifest (consume this)** | mcsr-meta schema 7, generated by a Kotlin tool that clones legal-mods and hashes every jar. | **https://meta.mc.sr/schema-7/mods.json** (CDN) / https://raw.githubusercontent.com/tildejustin/mcsr-meta/schema-7/mods.json |
| Non-legal extras | `extra.json`: practice & modded-category mods (MCSR Ranked, FSG mod, peepoPractice, Replay Mod…) "not legal for main board submissions". | https://meta.mc.sr/schema-7/extra.json |
| Generator config | `data.jsonc` (homepages, not-recommended/obsolete lists, incompatibilities, traits). | https://raw.githubusercontent.com/tildejustin/mcsr-meta/schema-7/data.jsonc |
| Legal DLLs | `legal-dlls.csv` — `filename,version,sha512` for Toolscreen.dll, liblogger_*, patched glfw/libglfw. | https://github.com/Minecraft-Java-Edition-Speedrunning/legal-builds/blob/main/legal-dlls.csv |
| Front-end | [mc.sr/mods](https://mc.sr/mods/) — "All mods not explicitly allowed are banned… including Fabric API and MCSR Ranked"; "use the most up-to-date version of mods"; exports `.mrpack`. | — |
| Mirrors of the same | MCSR Launcher meta `https://mcsrlauncher.github.io/meta/org.mcsr.mods/verified.json` (same commit-pinned URLs); ModCheck [tildejustin/modcheck](https://github.com/tildejustin/modcheck) 3.1.1 (GUI reads schema-7; CLI still reads schema-6; RedLime/ModCheck archived). | — |

Schema 7 (from [`Meta.kt`](https://github.com/tildejustin/mcsr-meta)):

```jsonc
{ "schemaVersion": 7,
  "mods": [{
    "modid": "seedqueue", "name": "SeedQueue", "description": "...", "homepage": "https://github.com/KingContaria/seedqueue",
    "traits": ["rsg-only"|"ssg-only"|"mac-only"|"accessibility"],   // default []
    "incompatibilities": ["modid"],                                  // default []
    "recommended": true, "obsolete": false,                          // defaults
    "versions": [{
      "target_version": ["1.16.1"],
      "version": "1.8+1.16.1",
      "url": "https://github.com/Minecraft-Java-Edition-Speedrunning/legal-mods/raw/<commit>/legal-mods/seedqueue/1.16.1/seedqueue-1.8+1.16.1.jar",
      "sha1": "…", "sha512": "…", "size": 12345,
      "dependencies": ["atum","speedrunapi"],
      "recommended": true, "obsolete": false,
      "intermediary": ["net.fabricmc:intermediary"]
    }]
  }]}
```

Current file: 47 mods, 499 versions, MC 1.0 → 26.3. Download hosts: commit-pinned `legal-mods` raw URLs
(immutable), `cdn.modrinth.com` (LazyDFU, retiNO), optifine.net.

**Selection algorithm** (same as mc.sr "Select recommended" and ModCheck CLI): keep a mod if `mod.recommended &&
version.recommended && !obsolete`, filter traits by category (`rsg-only`/`ssg-only`), OS (`mac-only`; use `sodiummac`
instead of `sodium` on macOS), and only include `accessibility` on opt-in.

**Gotchas for an updater:**
- 77 manifest versions are **not** in `legal-builds.csv` (mostly April Fools/OptiFine/pre-release, but also
  `lazystronghold 1.1.4`). Always cross-check sha512 against the CSV (**this one is worth raising with tildejustin**:
  lazystronghold 1.1.4 is recommended in meta and shipped in Ranked's pack, yet missing from the CSV on 2026-10-23).
- Builds get **illegalized** (removed from CSV), e.g. "illegalize 1.16.1-specific chunkcacher" (#197, 2025-10-26),
  "Illegalize Starlight 1.2.0" (#139). Grace period is typically a month for parity updates, shorter for bug fixes
  (A.8.5 note). Re-fetch the CSV on every launch and block/upgrade illegal jars.
- **Do not resolve mods from upstream GitHub Releases or Modrinth slugs**: legal builds are often only tags
  (SeedQueue 1.8, Atum 2.7.2), and several Modrinth slugs are a different or stale project (`sodium` = upstream
  CaffeineMC, `voyager`, `hermes`, `forceport`, `atum`, `worldpreview`).
- A.8.5 note: jars in legal-mods "are not intended for users to download directly" and recommends mc.sr/mods,
  ModCheck and MCSR Launcher — i.e. tools downloading them programmatically is the expected path.

### 2.2 Standard 1.16.1 RSG set (mods.json snapshot 2026-10-10)

"Rec" = passes the selection algorithm. Licence from jar `fabric.mod.json` / GitHub repo.

| Mod (modid) | Purpose | Modes | Source | Licence | Latest legal 1.16.1 | Update channel |
|---|---|---|---|---|---|---|
| SpeedRunIGT (`speedrunigt`) | In-game RTA/IGT timer, record files | RSG, SSG, FSG, AA, Ranked (bundled) | [RedLime/SpeedRunIGT](https://github.com/RedLime/SpeedRunIGT) | MIT | 16.0+1.16.1 | mcsr-meta (Modrinth slug stops at 15.1 for 1.16.1) |
| Atum (`atum`) | Auto world reset (RSG/SSG), reset key | RSG, SSG, FSG (required by FSG mod) | [KingContaria/atum](https://github.com/KingContaria/atum) | MIT | 2.7.2+1.16-1.16.1 | mcsr-meta (git tag only; Modrinth `atum` is stale ARR) |
| SeedQueue (`seedqueue`) | Background world generation + in-game "wall" (replaces multi-instance) | RSG, FSG | [contariaa/seedqueue](https://github.com/contariaa/seedqueue) | MIT | **1.8+1.16.1** (CSV 2026-10-09) | mcsr-meta (tag v1.8-1.16.1; not on Modrinth) |
| SpeedrunAPI (`speedrunapi`) | Config screens, Fabric API subset, MixinExtras (library) | all 1.16.1 | [KingContaria/SpeedrunAPI](https://github.com/KingContaria/SpeedrunAPI) | MIT | 2.2+1.16-1.16.1 | mcsr-meta |
| StandardSettings (`standardsettings`) | Resets options to a standard profile at world start | all | [KingContaria/StandardSettings](https://github.com/KingContaria/StandardSettings) | MIT | 2.3+1.16-1.16.1 | mcsr-meta |
| WorldPreview (`worldpreview`) | Shows world while generating | RSG (`rsg-only` meta trait), FSG | [KingContaria/worldpreview](https://github.com/KingContaria/worldpreview) | MIT | 6.3.1+1.16-1.16.1 | mcsr-meta |
| FastReset (`fast_reset`) | Skips world save on reset | all | [KingContaria/FastReset](https://github.com/KingContaria/FastReset) | MIT | 2.4.1+1.16.x | mcsr-meta |
| Sodium (speedrun fork) (`sodium`) | Rendering performance | all (non-mac) | [MJES/sodium](https://github.com/Minecraft-Java-Edition-Speedrunning/sodium) branch `1.16.1` | LGPL-3.0 | 2.5.1+1.16.1 | mcsr-meta only |
| Sodium Mac (`sodiummac`) | macOS backport | all (mac-only) | same repo, `mac/1.16.1` | LGPL-3.0 | 3.5.1+1.16.1 | mcsr-meta |
| Lithium (`lithium`) | General optimization (backport) | all | [KingContaria/lithium-mcsr](https://github.com/KingContaria/lithium-mcsr) | LGPL-3.0 | 1.0+backport-0.6.7+1.16.1 | mcsr-meta |
| Starlight (`starlight`) | Light engine rewrite | all | [MJES/starlight](https://github.com/Minecraft-Java-Edition-Speedrunning/starlight) | LGPL-3.0 | 1.3.0+1.16.x-backport-1.1.3 | mcsr-meta |
| Voyager (`voyager`) | Fixes MC-149777 crash on modern Java | all | [tildejustin/voyager](https://github.com/tildejustin/voyager) | WTFPL | 1.0.1 | mcsr-meta (Modrinth `voyager` is different) |
| AntiResourceReload (`antiresourcereload`) | Skips resource reload on world creation | all | [MJES/antiresourcereload](https://github.com/Minecraft-Java-Edition-Speedrunning/antiresourcereload) | MIT (jar) | 5.2.0+1.16.1 | mcsr-meta |
| State Output (`state-output`) | Writes `wpstateout.txt` for tools | all (legacy) | [tildejustin/state-output](https://github.com/tildejustin/state-output) — **archived**, Hermes named successor | MIT | 1.2.3+1.16-1.18.1 | mcsr-meta (still Rec; waywall still uses it) |
| Hermes (`hermes`) + HermesCore (`hermes-core`) | Structured state/world logs for tools (Jingle 2, PaceMan) | all | [DuncanRuns/Hermes](https://github.com/DuncanRuns/Hermes) | CC0-1.0 | 0.16.0+MC1.16.1 / core 0.3.2 (core not Rec standalone; dependency) | mcsr-meta |
| SleepBackground (`sleepbackground`) | Throttles unfocused instances | all (re-recommended 2026-10-10) | [RedLime/SleepBackground](https://github.com/RedLime/SleepBackground) | MIT | 4.1+1.16.1 (source location of 4.x **unverified**) | mcsr-meta |
| LazyDFU (`lazydfu`) | Lazy DataFixerUpper init | all | [astei/lazydfu](https://github.com/astei/lazydfu) | MIT | 0.1.2 | mcsr-meta → Modrinth CDN |
| Krypton (`krypton`) | Networking optimization (backport) | all | [astei/krypton](https://github.com/astei/krypton) | MIT (jar) / LGPL-3.0 upstream now | 0.1.3-SNAPSHOT+2021-02-20 | mcsr-meta |
| LazyStronghold (`lazystronghold`) | Lazy/multithreaded stronghold gen | RSG (`rsg-only`) | [Gregor0410/LazyStronghold](https://github.com/Gregor0410/LazyStronghold) | MIT | 1.1.4+1.16.1 (**not in CSV**, see §2.1) | mcsr-meta |
| Antigone (`antigone`) | Fixes 1.16.1 server deadlock | all | [MJES/antigone](https://github.com/Minecraft-Java-Edition-Speedrunning/antigone) | MIT | 2.0.0 | mcsr-meta |
| Chunkumulator (`chunkumulator`) | Batches chunk sending | all | [DuncanRuns/Chunkumulator](https://github.com/DuncanRuns/Chunkumulator) | MIT | 1.1.1+MC1.16.1 | mcsr-meta |
| ForcePort (`forceport`) | Fixed LAN port | all | [DuncanRuns/Force-Port-Mod](https://github.com/DuncanRuns/Force-Port-Mod) | MIT | 1.3.0 | mcsr-meta |
| MCSR Fairplay (`mcsrfairplay`) | Verification/anti-cheat archives for submissions (legal since 2025-10-25) | RSG etc. | [ExeRSolver/mcsr-fairplay-public](https://github.com/ExeRSolver/mcsr-fairplay-public) | jar: none; public repo MIT (jar-from-repo **unverified**) | 1.2.1+1.16.1 | mcsr-meta |
| Extra Options (`extra-options`) | FOV/distortion sliders backport | opt-in (`accessibility`; abuse banned A.8.12) | [tildejustin/extra-options](https://github.com/tildejustin/extra-options) | MIT | 2.2.1+1.16.1 | mcsr-meta |
| Glacier (`glacier`) | Worldgen optimization | not Rec; **broken by Ranked** | [contariaa/glacier](https://github.com/contariaa/glacier) | MIT | 1.0.1 (needs loader ≥0.18.4) | mcsr-meta |
| SetSpawn (`setspawnmod`) | SSG spawn point | **SSG only** | MJES/set-spawn | LGPL-3.0 | 4.2.1+1.14-1.18.2 | mcsr-meta |
| retiNO, BoundlessWindow | Retina half-res; resizable past screen (eye zoom on mac) | mac-only | [juliand665/retiNO](https://github.com/juliand665/retiNO), [Slackow/BoundlessWindow](https://github.com/Slackow/BoundlessWindow) | MIT | 1.0.1 / 1.2.0 | mcsr-meta |
| Dynamic Menu FPS, Phosphor | — | **obsolete** | — | — | — | — |

Not legal for 1.16.1 per manifest: Planifolia (1.17+), BiomeThreadLocalFix (1.16.2+), NoPaus (1.3.x), OptiFabric/OptiFine
(≤1.14.4), chunkcacher 1.16.1 build (illegalized 2025-10-26), ClearScreen (absent), **Fabric API** (banned, A.8.2.c).

**Default 1.16.1 RSG set (Win/Linux, 22 jars + hermes-core dependency):** antigone, antiresourcereload, atum,
chunkumulator, fast_reset, forceport, hermes (+hermes-core), krypton, lazydfu, lazystronghold, lithium,
mcsrfairplay, seedqueue, sleepbackground, sodium, speedrunapi, speedrunigt, standardsettings, starlight,
state-output, voyager, worldpreview. macOS: `sodiummac` + retino + boundlesswindow.

**Fabric Loader:** "All versions of Fabric Loader are allowed" (A.8.2), guidance to use 0.16.0+ (A.8.2.b); 1.16.1 jars
declare `fabricloader >=0.16.0`; current stable/Prism-recommended loader is **0.20.1**
([meta.fabricmc.net](https://meta.fabricmc.net/v2/versions/loader), [Prism meta](https://meta.prismlauncher.org/v1/net.fabricmc.fabric-loader/)).

### 2.3 Deltas per mode

| Mode | Delta vs RSG 1.16.1 set | Notes |
|---|---|---|
| **Ranked** | **+ `mcsrranked`** (Modrinth `mcsr-ranked`, self-updating) | Must not contain glacier, practiceseedmod, or any non-whitelisted jar. Fabric API comes bundled inside the Ranked jar — never add it separately. Illegal on speedrun.com. |
| **FSG** | **+ `fsg-mod`** (Modrinth `fsg-mod`, MIT) | Requires atum; works with SeedQueue. Not legal on /mc. Submission token via `/fsgtoken`. |
| **SSG** | **+ setspawnmod; − worldpreview, − lazystronghold** (meta traits; SeedQueue optional) | Only SSG may use setspawnmod/chunkcacher. |
| **AA (1.16.1)** | Same legal set (no Ranked) + external AA tracker (AATool / Advancely / PaceMan AA). | — |
| **Draftout** | **Entirely different base:** MC 26.3, Java 25, legal-26.3 mods (recommended set for 26.3 ≈ forceport, hermes, krypton, lithium, planifolia, sodium, speedrunigt) + `draftout` + allowed extras (No Chat Restrictions, No Telemetry, CommandQMod, retiNO). | No Ninjabrain. |
| **Practice** | Practice maps / practice mods from `extra.json` (e.g. peepoPractice) — not legal for runs, conflict with Ranked whitelist. | Covered by another agent. |

---

## 3. External tools

Data from `gh api` (licence, latest release, assets) on 2026-10-23 unless noted.

| Tool | Purpose | Platforms | Runtime | Licence | Distribution + latest | Asset naming | Auto-update | Game integration |
|---|---|---|---|---|---|---|---|---|
| [Ninjabrain Bot](https://github.com/Ninjabrain1/Ninjabrain-Bot) | Stronghold triangulation; boat-eye, blind, divine, AA modes | Win/mac/Linux | Java (built for 8) | GPL-3.0 | GH Releases **1.5.2** (2026-03-24); rules v8 pins v1.5.2 | `Ninjabrain-Bot-<ver>.jar` (tag w/o `v`) | Easy (`releases/latest`); also has own update checker. **Pin to rule-approved version.** | Reads **F3+C clipboard**; HTTP API **localhost:52533** `/api/v1/{stronghold,blind,divine,boat,all-advancements,…}` + `/events` SSE ([wiki/API](https://github.com/Ninjabrain1/Ninjabrain-Bot/wiki/API)); OBS overlay PNG in `%TEMP%` |
| [Jingle](https://github.com/DuncanRuns/Jingle) | Single-instance companion: hotkeys, Lua scripts, plugins, borderless, submission-file packaging | **Windows only** | Java 8 | MIT | **v2.0.0+pre6** (2026-09-19, flagged non-prerelease); last 1.x v1.3.0 | `Jingle-<ver>.jar` | Easy — use [meta_v2.json](https://raw.githubusercontent.com/DuncanRuns/Jingle/refs/heads/meta/meta_v2.json) (Jingle's own updater) | 2.x reads **Hermes**; sends keys to Toolscreen; plugins in `~/.config/Jingle/plugins/*.jar`. Default plugins: OBS Link, PaceMan Tracker, Standard Switcher, Program Launching, Easy Co-op, World Bopper, Pace Status |
| [Jingle Launcher](https://github.com/DuncanRuns/Jingle-Launcher) | Exe that runs Jingle via Toolscreen's `prelaunch.txt` | Windows | C++ | CC0 | v1 (2026-08-09) | `JingleLauncher.exe` | Static | `prelaunch.txt` |
| [**Toolscreen**](https://github.com/jojoe77777/Toolscreen) | Instant resizing (Thin/Wide/**EyeZoom**), overlays/mirrors (e.g. Ninjabrain inside fullscreen), rebinds, sensitivity hotkeys; config Ctrl+I | **Windows 10/11 x64/ARM64** (unofficial Linux ports exist — unverified) | C++ DLL (OpenGL; Vulkan for 26.3 in 1.5.0) + Java/exe installer | MIT | stable **v1.4.7** (2026-09-07); v1.5.0 prerelease (2026-10-10) | `Toolscreen-<ver>-double-click-me.(jar\|exe)` | Has own updater (EasyInject). MCSR Launcher meta lists versions with sha256 at `https://mcsrlauncher.github.io/meta/org.mcsr.tools/toolscreen.json`. **Check DLL against legal-dlls.csv** (see §7). | Installer adds `PreLaunchCommand=` to Prism/MultiMC `instance.cfg` (also ATLauncher/Modrinth App); a watcher injects `Toolscreen.dll` + `liblogger_<arch>.dll` into the JVM; runs per-instance `prelaunch.txt`. Asks for Defender exclusions; SignPath-signed. |
| [Julti](https://github.com/DuncanRuns/Julti) | Multi-instance manager / wall | Windows (Win32) | Java 8 | MIT | v1.6.1 (2025-01-01) | `Julti-<ver>.jar` | Easy, but **legacy** — SeedQueue's in-game wall replaces multi-instancing | wpstateout, Julti state file read by OBS Lua script |
| [waywall](https://github.com/tesselslate/waywall) | Nested Wayland compositor: rebinds, resolution switching, mirrors/eye zoom, Ninjabrain floating window, Lua config | Linux (Wayland) | C + LuaJIT | GPL-3.0 | All releases are prereleases (`releases/latest` 404); latest `0.2026.10.12b` | `waywall_0.5-1_amd64.deb`, `…fc42.x86_64.rpm`, `…pkg.tar.zst` | `releases?per_page=1`; distro packages need sudo | `waywall wrap --` as Prism wrapper command + patched GLFW (legal per A.8.4.e.ii); `-DMC_DEBUG_ENABLED -DMC_DEBUG_PREFER_WAYLAND` on 26.1+; reads wpstateout. Doesn't work with Flatpak launchers ([setup](https://github.com/tesselslate/waywall/blob/main/doc/00_setup.md)) |
| [resetti](https://github.com/tesselslate/resetti) | Old Linux reset macro/wall | Linux X11 | Go | GPL-3.0 | v0.7.0 (2024-09) | — | Superseded by waywall | — |
| [SlackowWall](https://github.com/Slackow/SlackowWall) | macOS wall + **BoatEye** eye projector, sensitivity scaling | macOS | Swift | GPL-3.0 | v1.4.9 (2026-06-20) | `SlackowWall.dmg` + Sparkle `appcast.xml`; Homebrew `slackow/apps/slackowwall` | Self-updates | Window capture |
| [PaceMan Tracker](https://github.com/PaceMan-MCSR/PaceMan-Tracker) | Uploads live RSG pace to [paceman.gg](https://paceman.gg) | Any (standalone jar) + Jingle/Julti plugin | Java 8 | MIT | v0.7.3 (2026-08-08) | `paceman-tracker-<v>.jar`, `-jingle-plugin-<v>.jar`, `-julti-plugin-<v>.jar` | Easy | Reads `~/speedrunigt/latest_world.json`, world `speedrunigt/events.log` & `record.json`; posts to `paceman.gg/api/sendevent`. Needs access key (paceman.gg: MC auth token via `auth.aristois.net` + Twitch OAuth). Explicitly allowed by A.8.15.a. |
| [PaceMan AA Tracker](https://github.com/PaceMan-MCSR/PaceMan-AA-Tracker) | Same for AA | Any | Java | MIT | v0.4.0 (2025-06) | same pattern | Easy | SpeedRunIGT files |
| [LiveSplit](https://github.com/LiveSplit/LiveSplit) | Split timer | Windows | C# | MIT | 1.8.37 (2026-02) | `LiveSplit_<ver>.zip` | Easy | Minecraft autosplitter [LiveSplit.Minecraft](https://github.com/LiveSplit/LiveSplit.Minecraft) unmaintained since 2021; runners rely on SpeedRunIGT's in-game timer (share using LiveSplit **unverified**). Low priority. |
| OBS + [Jingle OBS Link](https://github.com/DuncanRuns/Jingle-OBS-Link) / [Julti OBS Link](https://github.com/DuncanRuns/Julti-OBS-Link) | Scene switching / overlays | Win (Jingle) | Lua script in OBS | MIT | — | — | — | **File-based**: plugin writes `~/.config/Jingle/obs-link-state`, OBS Lua script polls it (not obs-websocket). |
| [MCSR Launcher](https://github.com/MCSRLauncher/Launcher) | Speedrun launcher (competitor) | Win installer; mac/Linux jar; AUR | Kotlin, JVM 17 | GPL-3.0 | 0.8.10-beta (2026-10-04) | `MCSRLauncher-windows-installer.exe`, `MCSRLauncher.jar` | — | Built-in Toolscreen, Ranked/Draftout instance types |
| [ModCheck](https://github.com/tildejustin/modcheck) | Legal-mod installer/updater (GUI + CLI) | Any | Kotlin/JVM | none listed | 3.1.1 (2026-01-28) | `modcheck-<v>.jar` | Easy | Prism prelaunch `java -jar modcheck.jar --path $INST_DIR update`. Does **not** verify hashes on download. |
| [CTM AATool](https://github.com/DarwinBaker/AATool) / [Advancely](https://github.com/LNXSeus/Advancely) / [SwiftAA](https://github.com/Kihron/SwiftAA) | AA trackers | Win / Any / macOS | C# / ? / Swift | GPL-3.0 / ? / GPL-3.0 | v1.8.0.1 / v1.8.0 / v1.1.3 | zip+Setup.exe / zip / dmg | Medium | Reads advancement/stat files (Advancely allowed to read Hermes, A.8.15.b) |
| [NinjaLink](https://github.com/DuncanRuns/NinjaLink) | Co-op: share Ninjabrain results | Any | Java | MIT | v1.1.0 (2025-07) | `NinjaLink-<v>.jar` | Easy | Ninjabrain API |
| [Calc Overlay plugin](https://github.com/marin774/Jingle-CalcOverlay-Plugin), [NBTrackr](https://github.com/qMaxXen/NBTrackr) (Linux) | Nicer Ninjabrain overlays | Win / Linux | Java / Python | CC0 / MIT | v2.4.1 / v2.7.0 | — | Easy | Ninjabrain API |
| [Jarfix](https://github.com/qMaxXen/Jarfix) | Fixes `.jar` file association on Windows | Windows | — | — | v1.0.5 | exe | — | — |
| MCSR Ranked desktop app | **None found.** Ranked is the mod + mrpacks; MCSR Launcher is "Made for MCSR Community and MCSR Ranked". | — | — | — | — | — | — | — |

**Eye measuring today.** Boat eye: entering a boat snaps the view angle to a known value so Ninjabrain can recover extra
precision from F3+C; requires a **tall resolution (height ≈16384)**, "Tall resolution" + "Enable boat measurements" in
Ninjabrain, and a matching sensitivity in `options.txt` (wiki lists values, e.g. 0.004676729)
([Ninjabrain wiki: Boat measurements](https://github.com/Ninjabrain1/Ninjabrain-Bot/wiki/Boat-measurements)). Rules allow
exactly **one** out-of-monitor resolution up to 16384 px (A.10.4). Provided by: Windows → Toolscreen EyeZoom; Linux →
waywall mirrors; macOS → SlackowWall Eye Projector + BoundlessWindow. Jingle 1.x resizing/EyeSee/OBS projector are
removed in Jingle 2.0. Exact standard width (e.g. 384×16384) **unverified**.

**Aggregated tool manifest to reuse:** MCSR Launcher publishes `https://mcsrlauncher.github.io/meta/org.mcsr.programs/verified.json`
(ninjabrain-bot, jingle, julti, toolscreen, waywall, aatool, advancely, jarfix, paceman-tracker — with pinned
download pages and OS rules) and `org.mcsr.tools/toolscreen.json` (versions + sha256 + prerelease flag). Licence of
that meta repo: none declared — treat as a reference, not a dependency, or ask RedLime.

**Recommended stack for a new player (synthesis):**
- Windows: launcher + legal mods; Toolscreen; Ninjabrain Bot 1.5.2; Jingle 2.x (+ PaceMan plugin, OBS Link) optional; OBS.
- macOS: legal mods incl. sodiummac/retino/boundlesswindow; SlackowWall; Ninjabrain; standalone PaceMan Tracker.
- Linux: native (non-Flatpak) launch; waywall (+ patched GLFW); Ninjabrain launched via `waywall.exec`; PaceMan Tracker.

---

## 4. Shared packages / layering model

### 4.1 Proposed layers

```
L0  Runtime          JDK (per MC version: 1.16.1 → Java 17/21/25 LTS; 26.3 → Java 25)
L1  Game             Minecraft version + Fabric Loader (latest stable, ≥0.16) + intermediary + LWJGL (unmodified)
L2  Core-perf        sodium|sodiummac, lithium, starlight, voyager, lazydfu, krypton, antigone, chunkumulator,
                     antiresourcereload, speedrunapi, forceport, sleepbackground          (all 1.16.1 modes)
L3  Run-infra        speedrunigt, standardsettings, fast_reset, hermes(+core), state-output, mcsrfairplay
L4a RSG              atum, seedqueue, worldpreview, lazystronghold                        ← "RSG" mode = L0..L3+L4a
L4b SSG              atum, setspawnmod, (seedqueue optional)                              ← "SSG" mode
L5  Ranked overlay   + mcsrranked   (on top of L4a)                                      ← "Ranked" mode
L5  FSG overlay      + fsg-mod      (on top of L4a)                                      ← "FSG" mode
L6  Practice         practice map worlds / practice mods (extra.json)                    ← "Practice" mode, never mixed into run instances
D   Draftout stack   separate: 26.3 + Java 25 + legal-26.3 recommended set + draftout + allowed extras
Tools (per OS, per mode): Toolscreen/waywall/SlackowWall, Ninjabrain (off for Draftout), Jingle, PaceMan, OBS
```

All layers are resolved from `meta.mc.sr/schema-7/mods.json` (+ `extra.json` for mcsrranked/fsg-mod) except
`mcsrranked`, `fsg-mod`, `draftout` (Modrinth API) — and each is hash-verified against `legal-builds.csv` where applicable.

### 4.2 Conflicts and per-mode differences

| Item | RSG | SSG | Ranked | FSG | Draftout | Practice |
|---|---|---|---|---|---|---|
| mcsrranked | **forbidden** (srcom) | forbidden | required | forbidden | n/a | forbidden for runs |
| fsg-mod | forbidden | forbidden | not whitelisted → Ranked will reject | required | n/a | ok |
| setspawnmod / chunkcacher | forbidden | allowed | not Rec | forbidden | n/a | — |
| worldpreview / lazystronghold | yes | meta says rsg-only | yes | yes | n/a | — |
| glacier | legal, not Rec | legal | **broken by Ranked** | ? | n/a | — |
| Fabric API | **banned** | banned | bundled inside Ranked jar only | not needed | not needed | some practice mods may need it |
| practice mods (peepoPractice etc.) | forbidden | forbidden | Ranked `breaks` practiceseedmod | — | — | yes |
| Ninjabrain Bot | yes | yes | yes | yes | **banned** | yes |
| Java | 17+ | 17+ | 17+ (replays/RPC) | 17+ | **25** | per map |

### 4.3 What to share vs. separate

| Asset | Recommendation | Why |
|---|---|---|
| Mod jars | **Content-addressed store** (by sha512) → materialize into each instance's `mods/` as a **copy or hardlink** on the same volume. | Rules require each simultaneously-open instance to have its own `.minecraft` (A.9.1) but say nothing about link mechanics. Ranked hash-checks file *contents*, which hardlinks preserve. Symlinks are likely fine too but **unverified** for Ranked's FileVerifier and for Windows (needs dev mode/admin) — default to hardlink, fall back to copy. |
| `.minecraft` / instance dir | **Separate per mode.** | A.9.1; worlds per mode; Ranked writes its own data; FSG/RSG worlds are submission evidence. |
| `saves/` | **Separate**, plus auto-cleanup of reset worlds (SeedQueue/Atum create thousands) but keep last N + any run worlds (A.7.8: 5 previous worlds needed for sub-10 runs). | Verification. |
| `options.txt` (keybinds, sensitivity, FOV) | **Shared "player profile"** synced into each instance at create time (keybinds/sensitivity are personal and identical across modes). Must respect A.1.4 bounds (values reachable in-game; gamma ≤5, maxFps 10–260). | Avoid re-binding per mode; sensitivity is tied to boat-eye. |
| StandardSettings config | Shared base profile with per-mode overrides (e.g. render distance / `f3PauseOnWorldLoad`; note that setting marks the run as F3, A.6.2.d). Exact file path (likely `config/mcsr/standardsettings.json`) **unverified**. | Same reason. |
| Atum / SeedQueue configs | Per mode (RSG vs FSG queue sizes; SSG seed). Paths under `config/mcsr/` **unverified**. | — |
| SpeedRunIGT config/records | Note SpeedRunIGT already writes a **global** `~/speedrunigt/` dir (PaceMan reads `latest_world.json` there) → shared by design. | — |
| Resource packs | Shared, but **only hash-allowed packs in Ranked** (`allowed-resourcepacks`). SeedQueue wall layout/lock/sound packs are legal (A.4.6). | — |
| Screenshots/logs | Per instance; logs are submission evidence for sub-13 runs. | — |

---

## 5. Existing solutions and competition

| Solution | What it does | Notes |
|---|---|---|
| **MCSR Launcher** ([repo](https://github.com/MCSRLauncher/Launcher), [site](https://mcsrlauncher.github.io/)) | Full launcher: Ranked instance types (Basic / StandardSettings / All / ModOnly), Draftout, legal-mod browser, practice maps (13 in meta), tool list, built-in Toolscreen, Java manager (Adoptium/Azul/GraalVM/Mojang meta), MultiMC import. Defaults: Xmx 4096 if >15 GB RAM else 2048; Windows Intel-driver heapdump trick; Draftout JVM flags auto-applied. | **Direct competitor**, by RedLime (author of SpeedRunIGT and Ranked), already whitelisted in A.1.1. Kotlin/Swing UI, beta. GPL-3.0 — any code reuse forces GPL. |
| [mc.sr/mods](https://mc.sr/mods/) | Pick version/category/OS, download zip or `.mrpack` for Prism/Modrinth App | Official, most common path for srcom. |
| Prism/MultiMC + guides | Most players: Prism + mc.sr mrpack or instance zip + YouTube/Discord guides. | Prism recommended by Ranked and Draftout. |
| Ranked mrpacks | Basic / Basic-w-SS / RSG per OS. | Pinned, older builds. |
| [ModCheck](https://github.com/tildejustin/modcheck) | Install/update legal mods into an existing instance (GUI/CLI, prelaunch hook). | — |
| Draftout Essentials (Modrinth modpack), Jingle Launcher, Toolscreen installer | Partial automation per tool. | — |
| Legacy: Specnr's AHK [MultiResetWall](https://github.com/Specnr/MultiResetWall) (archived), Julti | Multi-instance walls. | Replaced by SeedQueue. |
| "MCSR auto setup" tools | None current found under that name (**unverified**; search budget ran out). | — |

**Known pain points** (sources: Ranked wiki FAQ/troubleshooting, Toolscreen README, waywall setup docs; frequency
ranking is judgement):
1. **Java**: Java 8 bundled with 1.16.1 → "UnsupportedClassVersionError", replay/Discord RPC failures; 32-bit Java
   heap errors; Prism's JavaCheck errors ([Ranked FAQ](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/install/faq.md)).
2. **RAM**: too little (SeedQueue ~250 MB per queued seed — [SeedQueue wiki Config](https://github.com/contariaa/seedqueue/wiki/Config)) or too much (lag spikes).
3. **iGPU vs dGPU** on laptops (Windows "High performance" for javaw.exe — Ranked FAQ).
4. **`.jar` won't open on Windows** (→ Jarfix) for Ninjabrain/Jingle/Toolscreen installer.
5. **Defender/AV flags** on DLL injection (Toolscreen asks for exclusions) and historically AHK scripts.
6. **Mod legality drift**: illegalized builds, wrong Modrinth slug (upstream Sodium), Fabric API accidentally installed; Ranked rejecting the mod set.
7. **Eye-measuring setup**: tall resolution, sensitivity numbers, boat-eye config, overlay placement; differs per OS.
8. **OBS**: game capture vs window capture, scenes for wall/eye-zoom, Toolscreen making OBS projectors unnecessary but new guides lag behind.
9. **Linux**: Flatpak launchers don't work with waywall; patched GLFW; NVIDIA env var.
10. **macOS**: sodiummac/retiNO, SlackowWall, `-XstartOnFirstThread`.
11. **Accounts**: Ranked/Draftout need genuine MSA; PaceMan needs Twitch.
12. **Version split**: Draftout needs 26.3 + Java 25 + special JVM flags, separate from everything else.

**What a one-click launcher must solve:** pick mode → auto-install correct JDK, MC, loader, legal mod set (hash-verified,
auto-updated, illegal builds auto-replaced), OS-specific variants, tools (Toolscreen/waywall/SlackowWall + Ninjabrain
pinned to legal version), sensible RAM/GC, shared keybind/sensitivity profile, StandardSettings preset, OBS scene setup,
Defender guidance, MSA login — and stay on the A.1.1 whitelist.

---

## 6. Java and JVM

| Topic | Recommendation | Source |
|---|---|---|
| Java version, 1.16.1 | **Java 17+** (Ranked FAQ "highly recommended"; many current speedrun mods/tools assume it). Java 21 or 25 LTS are fine; Voyager exists precisely to fix the MC-149777 crash on modern Java. | [Ranked FAQ](https://github.com/MCSR-Ranked/Wiki/blob/main/docs/install/faq.md), Voyager meta description |
| Java version, Draftout (26.3) | **Java 25+**, plus `-XX:StackShadowPages=32 -XX:CompileCommand=exclude,io/netty/util/internal/ReferenceCountUpdater,retryRelease0`. | [draftoutmc.com/download](https://draftoutmc.com/download) |
| Distribution | Any 64-bit JDK. MCSR Launcher offers **Adoptium, Azul (Zulu), GraalVM, Mojang** runtimes via its meta (`com.azul.java`, `net.adoptium.java`, `org.graalvm.java` — GraalVM java25 marked recommended *within that vendor*). Community claim that GraalVM is faster for MCSR: **unverified** (no primary benchmark found). Default suggestion: Adoptium Temurin 21/25 (stable, permissive licence), GraalVM as an option. | [MCSR Launcher meta index](https://mcsrlauncher.github.io/meta/index.json) |
| RAM | Ranked FAQ: "around 3000" MB; MCSR Launcher default 4096 MB if >15 GB system RAM else 2048; SeedQueue: +~250 MB per queued seed. Don't over-allocate (lag spikes). | Ranked FAQ, MCSR Launcher `LauncherOptions.kt`, SeedQueue wiki |
| GC | SeedQueue wiki: with large allocations use **`-XX:+UseZGC`**; Ranked FAQ mentions Shenandoah in troubleshooting (per research agent). | [SeedQueue wiki Config](https://github.com/contariaa/seedqueue/wiki/Config) |
| Windows trick | `-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump` (vanilla launcher's Intel driver workaround; MCSR Launcher adds it). | MCSR Launcher `InstanceProcess.kt` |
| macOS | `-XstartOnFirstThread` for LWJGL3 versions. | same |
| **Rules** | JVM tuning flags are allowed. **Forbidden**: Java agents (except Ornithe Flap), attached debuggers (A.8.19), game debug properties except `ENABLED`/`PREFER_WAYLAND` (A.8.18), interfering with launcher-provided libraries (A.8.17), non-legal LWJGL/GLFW (A.8.4). Only `-Dorg.lwjgl.glfw.libname=` pointing at a **legal-dlls.csv** GLFW build is acceptable. | [rules.typ](https://github.com/Minecraft-Java-Edition-Speedrunning/rules/blob/main/rules.typ) |

---

## 7. Rules and risks for a third-party launcher

| Risk | Severity | Detail / mitigation |
|---|---|---|
| **Launcher not on A.1.1 whitelist** | **Critical** | Runs from an unlisted launcher can be rejected. Plan: build to be clearly "does not significantly modify the game", then open a legalization thread in the MCSR Discord early (Pandora and Nixcraft were added in v8 this way). Ranked's wiki also has its own launcher list. Draftout says Lunar/Feather unsupported — confirm with Draftout devs. |
| Unlisted/modified mod jars | Critical | Only sha512s in `legal-builds.csv` are legal (A.8.5); Ranked/Draftout hash-check too. Never repack, re-sign, strip, or patch jars; download exact bytes; verify sha512 vs CSV before launch; re-check on every launch (illegalizations). |
| Fabric API / extra jars leaking into run instances | High | Banned on srcom; Ranked rejects non-whitelisted jars. Mode isolation required. |
| Java agents / launch hooks | High | A.8.19 bans agents → don't implement features as a `-javaagent`. Don't inject anything into the game except whitelisted DLLs (A.8.16: Toolscreen + LibLogger). Don't touch libraries (A.8.17). |
| Toolscreen version vs legal-dlls | Medium | `legal-dlls.csv` lists Toolscreen.dll 1.0.0–1.4.4 and 1.5.0/1.5.1 (x64/arm64) but **not 1.4.5–1.4.7** on 2026-10-23, while GitHub "latest" is v1.4.7. Possibly unchanged DLLs or lag — **unverified**. The launcher should hash the injected DLL against legal-dlls.csv before enabling it, and only auto-update to legal builds. |
| Ninjabrain version pin | Medium | Rules v8 pinned NinjabrainBot to v1.5.2 (per rules changelog). Auto-update must respect the pinned version. |
| Settings reset | Medium | StandardSettings is a "resetting mod" — must not be interacted with after gameplay start (A.8.13). Launcher must not rewrite `options.txt` while a run is in progress; values must be in-game-reachable (A.1.4). `f3PauseOnWorldLoad=true` marks the run as F3. |
| Macros | Medium | Only resize macros (in-bounds unlimited + one out-of-bounds ≤16384 px), toggle sprint, player-triggered resets; no timed auto-reset, no screen reading (A.10.4–A.10.6). Any launcher hotkey feature must stay in these bounds; better to delegate to Toolscreen/waywall/Jingle. |
| Reading mod output files | Low–Med | Hermes/state-output/SpeedRunIGT files may only be read for permitted purposes after gameplay start (A.8.14, A.8.15). A stats dashboard is fine if purely aesthetic/after the run; a live in-run helper that uses state for advantage is not. |
| Multi-instance | Low | Separate `.minecraft` per open instance (A.9.1); only one SeedQueue instance (A.9.4). |
| Symlinks/hardlinks to shared mod store | Low (unverified) | No rule mentions links. Ranked hashes contents. Prefer hardlinks/copies; avoid symlinks on Windows. Ask Ranked devs to be sure. |
| **Redistribution** | Medium | Default-set licences (MIT/LGPL/WTFPL/CC0) allow redistribution of unmodified jars with notices; **mcsrfairplay jar has no licence**, **legal-mods repo has no licence**, **MCSR Ranked and Draftout are ARR/closed**. Do **not** mirror or bundle; download from manifest URLs (legal-mods commit-pinned GitHub raw, Modrinth CDN) and Modrinth API for Ranked/Draftout/FSG. A cache on the user's machine is fine. Self-hosted mirror would need permission (no community statement found either way). |
| Modrinth API etiquette | Low | Use a unique User-Agent; respect rate limits. GitHub API unauthenticated limit 60/h → prefer `meta.mc.sr`, Jingle `meta_v2.json`, Sparkle appcasts. |
| MSA auth | Medium | Ranked/Draftout need a genuine MSA token (no offline mode). Implement official MSA device-code/OAuth flow (needs an Azure app registration approved for Minecraft API access — **verify current Mojang approval process**). |
| MCSR Launcher GPL | Low | Don't copy code unless Speedrunluncher is GPL-3.0; reading its meta formats is fine, but its meta repo has no licence. |

---

## 8. Gaps — things the owner didn't mention (ranked by value for a new player)

1. **Legal-mod verification + "Run readiness" check** (hash vs CSV, no Fabric API, correct OS variant, Ranked whitelist
   match, legal Toolscreen DLL, Ninjabrain pinned version) — prevents rejected runs. Highest value, low cost given the manifest.
2. **Eye-measuring setup wizard** (per OS: Toolscreen EyeZoom / waywall / SlackowWall; tall resolution; boat-eye
   sensitivity written to options.txt; Ninjabrain configured with boat measurements + overlay). This is the hardest
   part of onboarding.
3. **Java + RAM auto-config** (download proper JDK per mode, RAM by SeedQueue queue size, ZGC when large, Draftout flags).
4. **Shared keybind/sensitivity profile + StandardSettings presets** (a "beginner standard" preset; render distance,
   FOV, GUI scale, F3 settings) synced to all modes.
5. **OBS auto-setup** (generate scene collection: game capture, Ninjabrain overlay, eye-zoom scene, mic/game audio
   tracks — sub-10 runs need audio; Jingle OBS Link script install).
6. **Run-submission helper**: package world files, logs, 5 previous worlds, MCSR Fairplay archive unmodified
   (A.7.4/A.7.8), FSG token; link to speedrun.com submit page; explain proof thresholds (sub-13 / sub-10 / WR).
   Jingle already packages submission files on Windows — integrate rather than rebuild.
7. **Pace/stats dashboard**: PaceMan account linking (Twitch + token), MCSR Ranked API stats (Elo, matches),
   Draftout stats API, local SpeedRunIGT records (reset count, split history). Strictly post-run/aesthetic (A.8.14).
8. **SeedQueue wall onboarding** (start with queue disabled → enable wall with sane defaults; wall resource packs).
   Multi-instance (Julti) is legacy — don't build it.
9. **Defender/AV + Jarfix guidance** on Windows; non-Flatpak note + GLFW on Linux; mac variants.
10. **Account flows**: MSA (required for Ranked/Draftout), PaceMan token, Ranked private key.
11. **Disk hygiene**: auto-delete old reset worlds (SeedQueue creates thousands) while keeping the last N.
12. **Rules digest / "what's legal" UI** sourced from rules.typ with version (v8) and change notifications
    (illegalizations, grace periods).
13. **Community links**: MCSR Discord, Ranked Discord/wiki, Draftout wiki, speedrun.com/mc resources, PaceMan.
14. **Co-op**: NinjaLink, Jingle Easy Co-op, LAN via ForcePort.
15. **AA mode** with AATool/Advancely/PaceMan-AA later.

---

## 9. Open questions / unverified

- Owner's "Ranked = 2 mods on top of RSG": current packs show **1** (`mcsrranked`).
- lazystronghold 1.1.4 recommended in meta but absent from `legal-builds.csv` (2026-10-23).
- Toolscreen 1.4.5–1.4.7 DLLs absent from `legal-dlls.csv`.
- Whether Ranked accepts older-but-legal builds (e.g. SeedQueue 1.7.1) or demands latest meta versions.
- Symlink behaviour with Ranked FileVerifier.
- StandardSettings / Atum / SeedQueue exact config paths.
- GraalVM performance claims; how many runners still use LiveSplit.
- Whether "MCSR auto setup" tools exist beyond MCSR Launcher / mc.sr / ModCheck (search budget exhausted).
- Draftout's "launcher blocks disallowed mods" wording (mod-side check assumed).
