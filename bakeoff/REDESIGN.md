# Scene v3: direction A ("Plansza")

Piotr's feedback on scene v2: "very basic, template layout and icons, looks vibecoded". He chose direction A from the
2026-10-25 proposals, with the split strip from direction B, and our own Blender key art for categories and modes.

This file overrides SCREENS.md where they differ. The order of work, agreed with Piotr:
1. **App shell:** the top nav replaces the rail on every page.
2. **Home**, with 5 mode states.
3. **Modes.**
4. **Practice.**

The other screens only get the new shell for now. Both builds (web for Tauri and Electron, and GPUI) must look and
behave the same, as before.

## What the launcher really does

Copy and buttons must stay truthful.

- **The launcher prepares and starts the game; play happens inside Minecraft.** "Play" starts the selected mode's
  instance plus its tools, then the launcher goes to the tray (the Game screen).
- **Queues, private rooms, Weekly Race, resets and seed picking are all in-game.** They belong to the MCSR Ranked mod,
  Atum/SeedQueue, the FSG Mod and the Draftout mod.
- **No "find match" or "private room" button anywhere.** The MCSR Ranked API is read-only, and Ranked hash-checks
  every mod, so the launcher can't drive it.
- **Data is read-only. The prototype shows sample data from these sources:**
  - SpeedRunIGT record files: PB, splits, sum of best, runs.
  - api.mcsrranked.com: elo, tier, season record, matches, Weekly Race, players online.
  - The Draftout stats API.

## Inputs

**Mockups** (open in Chrome; renders in `out/`):
- `/tmp/srl/redesign/a-home-ranked.html` and `a-home-rsg.html`, renders `out/a-home-ranked.png` and
  `out/a-home-rsg.png`;
- `a-modes.html`;
- `a-practice.html`;
- `a-shell.css`, which holds every size and colour below.

If anything here is unclear, the mockup wins.

**Copy:** new keys in `assets/copy.json` and `copy.pl.json`:
- `nav.*` and `tier.*`;
- `v3.*`;
- `rail.*` is no longer used.

Caps labels are written in normal case in copy. Code uppercases them with Unicode-aware upper (JS `toUpperCase`,
Rust `to_uppercase`).

**Data:** `screens.json` gets a new `v3` block, and `screens.pl.json` overlays its dates. Mode names, layers and
statuses still come from `modes[]` and `layers`.

**Art:** `assets/art/`. Masters are made outside the app by `tools/make-art.sh`:

| Path | Size | Use |
|---|---|---|
| `hero/<id>.jpg` | 1280×800 | full-window background, already cropped |
| `card/<id>.jpg` | 640×360 | tiles, rows, split segments; cover-fit and centred |
| `shade-hero.png` | 1280×800, RGBA | the hero shading, drawn over `hero/*` on Home in both builds, so gradients match exactly |

Art ids:
- modes: `ranked`, `rsg`, `fsg`, `ssg`, `draftout`;
- categories and splits: `overworld`, `nether-portal`, `bastion`, `fortress`, `stronghold`, `the-end`.

Today some mode art is a placeholder (a category render). New renders replace the files later; no code change is
needed.

**Fonts** (`assets/fonts/`, OFL):

| Font | Weights | Use |
|---|---|---|
| `Archivo SRL` (static Archivo at width 112) | SemiBold 600, Bold 700, ExtraBold 800 | display, caps labels |
| `IBM Plex Mono` | 400, 500, 600 | times, numbers, versions; it replaces JetBrains Mono in the v3 screens |
| `Inter` | as before | body |

## Tokens

| Token | Value | Use |
|---|---|---|
| bg | `#0A0810` | window |
| text | `#F1EEF8` | primary text, Play button fill |
| muted | text @ 64% | secondary text |
| faint | text @ 40% | caps labels, sources |
| line | text @ 9% | hairlines |
| accent | `#8B5CF6` | own layer fill (@ 22%) |
| accent-2 | `#B79CFF` | eyebrows, active nav underline, "download" badges |
| ok | `#4ADE80` | ready status |
| gold | `#D8AF1F` | LiveSplit best segment, sum of best, "update" status |
| ahead | `#29CC54` | won, elo up (LiveSplit) |
| behind | `#E0493B` | lost |
| tier colours | Coal `#AAAAAA`, Iron `#FFFFFF`, Gold `#EAB308`, Emerald `#21A83B`, Diamond `#2CE0D8`, Netherite `#7C5CBF` | MCSR Ranked tiers (thresholds 0 / 600 / 900 / 1200 / 1500 / 2000) |

**Corners:** none in v3 screens. Everything is square, so GPUI's missing rounded clipping doesn't matter.

**Caps label:** Archivo SRL SemiBold 11–12 px, tracking 0.16–0.2 em, uppercase, `faint` or `muted`.

**GPUI has no letter-spacing.** Approximate the tracking by putting U+200A (hair space) between letters of caps
labels. Use one helper and keep it consistent.

## Shell: top nav (all pages except Onboarding and Game)

- **Bar:** 68 px high, over the page, no background. On pages without hero art the bar sits on `bg`.
- **Left:**
  - Padding 0 48. Gap 44 between groups.
  - `logo.svg` at 22 px in `accent-2`, then "SPEEDRUNLUNCHER" in Archivo SRL Bold 13, tracking 0.16 em.
- **Nav:** `nav.play`, `nav.modes`, `nav.practice`, `nav.learn`, `nav.tools`, `nav.stats`.
  - Archivo SRL SemiBold 12, uppercase, tracking 0.16 em, 28 apart. Inactive in `muted`, active in `text`.
  - The active item has a 2 px `accent-2` underline, 18 px above the bar's bottom edge.
  - Stats stays active on Library and Submit.
- **Right:**
  - The settings gear: 18 px, `muted`; hover `text`. It opens Settings.
  - Then the account block, right-aligned. Line 1: the name in Inter SemiBold 13. Line 2: a 7×7 square in the tier
    colour, then `nav.account_line` ("1642 · Diamond") in Plex Mono 11, `muted`.
  - Then the 32×32 avatar, nearest-neighbour. The account block opens Accounts.
- **Content:** pages keep their own layout. Other pages put their content 104 px from the top, with side padding 64
  left and 48 right.
- **Transitions:** SPEC's page transition still applies between pages.

## Home

There is one layout per selected mode. The default is `v3.default_mode` (ranked). Every number below is a position in
the 1280×800 window.

**Background and shading:**
- `hero/<art>.jpg` fills the window.
- `shade-hero.png` goes over it.

**Hero** (left 64, top 116, width 560):
- **Eyebrow:** `v3.eyebrow.<mode>`, Archivo SRL SemiBold 12, tracking 0.2 em, `accent-2`.
- **Title:** `v3.modes[mode].short`, Archivo SRL ExtraBold 112, line height 0.86, tracking −0.03 em, 16 below the
  eyebrow.
- **Description:** `v3.desc.<mode>`, Inter 16/1.5, `muted`, max width 440, 20 below the title.
- **Status row** (Plex Mono 12, `faint`, items 18 apart, 16 below):
  - The status in its colour, after a 6×6 square: ready → `ok`, update → `gold`, missing → `accent-2`.
  - Then `v3.mods_checked`.
  - Then a mode extra: Ranked shows `v3.online`; run modes show their key tools ("SeedQueue 1.8 · Ninjabrain ·
    PaceMan"); missing modes show `v3.download_hint`.
- **Actions** (30 below):
  - **Play button:** 288×64, fill `text`, text `#120D1C`. Label `v3.play` (or `v3.download` when the mode is
    missing) in Archivo SRL ExtraBold 19, tracking 0.2 em.
  - **Pixel corners:** draw them as two overlapping rects, one inset 4 px horizontally and one inset 4 px vertically.
  - **Under the label:** the hint in Plex Mono 10, `#120D1C @ 55%`.
    - Ranked: `v3.play_hint.ranked`.
    - run modes: `v3.play_hint.run`.
    - a mode with status update: `v3.play_hint.update` with `update_mod`.
    - a missing mode: `v3.download_hint`.
  - **Then** `v3.verify` as a ghost caps label (Archivo SRL SemiBold 12, `muted`), 28 to the right. A missing mode has
    nothing to verify, so it shows no ghost actions (here and on Modes).
  - **Click behaviour:** Play opens the existing pre-launch check modal (`check`). Download runs SCREENS.md's fake
    install (progress under the button, 1200 ms), after which the mode turns ready.

**Right column** (right 48, top 116, right-aligned, items 22 apart):
- Each item is a value in Plex Mono SemiBold 26, with a caps label 7 below it (Archivo SRL 11, `muted`).
- **Ranked:**
  - elo plus `elo_delta` in `ahead` 13 px, labelled `v3.r.elo` with a tier-colour square;
  - "18 – 11", labelled `v3.r.season`;
  - `best`, labelled `v3.r.best`.
- **RSG, FSG:**
  - `pb`, labelled `v3.pb`;
  - `sob` in `gold`, labelled `v3.sob`;
  - `runs_month`, labelled `v3.runs_month`.
- **Missing modes (SSG, Draftout):** no right column.

**Middle band** (left 64, right 48, top 494):
- **Ranked: recent matches.**
  - Header row: `v3.matches` on the left and `v3.matches_src` on the right, both caps labels, 12 above the row.
  - 4 equal cells, 52 high, 14 apart, bg text @ 4%. Each has a 3 px left border: `ahead` for won, `behind` for lost.
  - Each match cell: a 26 px avatar, then the opponent in Inter SemiBold 13. Under it `v3.match_won` or
    `v3.match_lost` in Plex Mono 11, `muted`. On the right, the elo change in Plex Mono 13, coloured.
  - The 4th cell is the Weekly Race: `v3.weekly`, then `v3.weekly_place` and the time on the right, with no border
    colour.
- **RSG and FSG: the split strip** (from direction B).
  - Header: `v3.strip` and `v3.strip_src`.
  - 6 segments, 66 high, 3 apart. Each is `flex: secs` wide, filled with `card/<art>.jpg` under a bottom gradient
    (bg @ 92% at the bottom to bg @ 25% at 80%).
  - Inside, 10 from the left and 9 from the bottom: the split name (Archivo SRL SemiBold 11, uppercase, tracking
    0.14 em), then the time in Plex Mono 13 with the best segment after it in `gold` 11.
- **SSG and Draftout: what gets installed.**
  - Header: `v3.install_title`.
  - One row of chips, 40 high, bg text @ 5%: each layer name from `install.new` (tag `v3.install_new` in
    `accent-2`), then `install.have` (tag `v3.install_have` in `faint`).

**Mode strip** (left 64, right 48, bottom 36):
- Header: `v3.modes` on the left and `v3.modes_shared` with `disk_saved` on the right.
- 5 tiles, 116 high, 14 apart, using `card/<art>.jpg`.
- Inactive tiles get a bg @ 50% overlay. Every tile gets a bottom gradient.
- The name sits 14 from the left and 12 from the bottom (Archivo SRL Bold 17), with the version under it (Plex Mono
  11, `muted`).
- Missing modes get the `v3.badge_download` badge at the top right: Archivo 10, `accent-2` on bg @ 75%.
- The selected tile has a 2 px `text` outline, offset 3. Clicking a tile switches Home to that mode instantly: no page
  transition and no animation, so idle stays idle.

**Pre-launch check:** the modal stays as in SCREENS.md §4, over the new Home.

## Modes (`nav.modes`)

See `a-modes.html`.

**Background:** `hero/<selected>.jpg` at 16% opacity, under a gradient: bg on the left, bg @ 70% in the middle,
bg @ 85% on the right, and bg at the bottom.

- **GPUI:** do the opacity by drawing a bg @ 84% rect over the image.
- **Web:** do it the same way, for parity.

**Left list** (left 64, top 104, width 432): 5 rows, 104 high, 10 apart.
- Each row uses `card/<art>.jpg` with a left-to-right darkening gradient. Inactive rows are darker.
- Selected row: 2 px `text` outline, offset 3.
- Left side: the name in Archivo SRL ExtraBold 30, with `modes[].full` under it in Plex Mono 11.
- Top right:
  - the status in caps (`v3.status.*` colours: ready `ok`, update `gold`, missing `accent-2`);
  - the MC version under it in Plex Mono 11, `faint`.

**Detail** (left 560, right 48, top 104):
- Eyebrow, then the title in Archivo SRL ExtraBold 72, then `v3.desc.<mode>`.
- **Facts row**, items 36 apart, each a caps label over a Plex Mono 15 value:
  - `v3.f_mc`;
  - `v3.f_mods` (`mods_total`);
  - `v3.f_disk`;
  - `v3.f_board` (`leaderboard`), shown only if the mode has one.
- **Layer stack:**
  - Header: `v3.layers` on the left and `v3.layers_saved` on the right.
  - One row per layer of the mode, top to bottom in `modes[].layers` order. Rows are 40 high, 3 apart, bg text @ 5%.
  - Each row: the layer name (Inter SemiBold 14, 210 wide), then the detail or mod count in Plex Mono 12, `muted`.
    On the right, `v3.only_this` or `v3.shared_with` with the other modes that use that layer, in Plex Mono 11,
    `faint`.
  - The mode's own layer (used by no other mode) gets accent @ 22% and a 3 px `accent-2` left edge, with the tag in
    `accent-2`.
- **Actions:**
  - Play is the same button as Home, at 280×58 (Download when missing).
  - Then `v3.verify` and `v3.open_folder` as ghost caps labels.
- **Note:** `v3.note.<mode>`, Inter 12, `faint`.
- **Clicks:** a row selects that mode. Play opens the check modal; Download runs the fake install.

## Practice (`nav.practice`)

See `a-practice.html`.

**Background:** `hero/<art of selected category>.jpg` at 22% opacity, pre-blurred, over the top 360 px, fading into
bg.

GPUI has no blur. Use the card image upscaled (it is soft already) under a bg @ 78% rect, and do the same on web for
parity.

**Header:** "Practice" in Archivo SRL ExtraBold 56 at left 64, top 104. `practice.subtitle` is right-aligned, Inter 14,
`muted`, max width 420.

**Category grid** (30 below):
- 4 columns: 1.5fr, 1fr, 1fr, 1fr; 2 rows of 128; gaps 12.
- **The first category** spans both rows: `practice_categories[0]`, the big tile, whose subtitle is the map name plus
  its description.
- **The other 5 categories** fill the remaining cells.
- **The last cell is "Tools":** `v3.practice_tools_tile`, listing the practice tools' names, with no image (bg #17131F
  and a 1 px `line` border).
- **Each tile:** `card/<practice_art[cat]>.jpg`. Inactive tiles get a bg @ 34% overlay; the selected one gets the
  2 px outline.
- **Tile text:** the name in Archivo SRL Bold 20 (30 on the big tile), with the count under it: `v3.practice_count`,
  or `v3.practice_count_one` for one map.

**Map list** (30 below the grid):
- **Header:** `v3.practice_list` on the left, with the category name and count, and `v3.practice_sort` on the right.
- **Rows** are 58 high with a 1 px `line` top border:
  - an 84×46 thumbnail (`card/<cat art>.jpg`);
  - the name (Inter SemiBold 15) with `authors` under it (12, `muted`), 250 wide;
  - the description (13, `muted`, flex);
  - `version · downloads` in Plex Mono 12, `faint`, 130 wide, right-aligned;
  - the action, 150 wide, right-aligned, caps label: installed in `ok`, available as "Install" in `accent-2`, update
    as "Update" in `text`.
- **Install** uses the fake install, with progress under the row.
- **Overflow:** when the list doesn't fit (Overworld has 5 maps), it scrolls inside its own container.
- **Tools:** clicking the Tools tile lists the 3 practice tools in the same row format.
- `practice.ranked_tip` moves under the list as a single Inter 12 `faint` line.

## Screen ids (additions)

| id | State |
|---|---|
| `home` | Home, Ranked (default) |
| `home-rsg`, `home-fsg`, `home-ssg`, `home-draftout` | Home with that mode selected |
| `modes` | Modes, Ranked selected |
| `modes-ssg` | Modes, SSG selected (missing state) |
| `practice` | Practice, first category selected |
| `practice-end` | Practice, End selected (the mockup's state) |

All the old ids still work. There are 26 ids in total now. The `shoot-*` tools must know them.

## Scenario and measurement

- The scenario is unchanged: idle → Home↔Library transitions → Library scroll → minimized → destroyed. It runs on
  Home (Ranked) and Library (Stats → Leaderboard).
- **Home now has a full-window image.** That is intended: it is what the real launcher will look like.
- **Results from v3 are not comparable with v2 or v1.** Note the scene version in the report.
- **Idle must still request no frames.**
- **Image handling:** decode each image once and cache it.
  - Web: plain `<img>` or CSS background with the same file.
  - GPUI: `img()` with an embedded asset.

## Parity rules for GPUI

- **No filters.** Dim or darken with overlay rects, never image filters, in both builds.
- **Gradients:** GPUI's linear gradients have 2 stops. Stack 2-stop gradient layers where a mockup has more. The hero
  uses `shade-hero.png`, so it needs none.
- **Outline with offset:** a 2 px border on a wrapper 3 px larger than the tile.
- **Object fit:** cover, centred. The hero files are pre-cropped, so they are drawn 1:1.
- **Text:** caps tracking via hair spaces as above. Report any text that clips.

## Disclaimer and credits

Mojang's usage guidelines (minecraft.net/usage-guidelines) require the disclaimer on anything Minecraft-related that
is not theirs, and they count free apps as well. The key art is our own Blender scenes with our own 16×16 textures:
no file from the game jar, no Mojang logo, no Minecraft font.

- **Settings** gets a last section, `settings.section_about`, with two lines in Inter 12, `faint`:
  `settings.disclaimer` (the exact English text in both languages, as Mojang words it) and `settings.credits`.
- **Onboarding welcome** shows `settings.disclaimer` at the bottom of the window, Inter 11, `faint`, centred.
- `assets/art/CREDITS.md` lists how each render was made and the font licences.
