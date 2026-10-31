# Prototype screens (scene v2)

> **Scene v3:** REDESIGN.md replaces the shell, Home, Modes and Practice. It wins wherever it differs from this file.

SPEC.md describes the measured scene: Home, Library, the scenario and the report protocol. This file adds every other
launcher screen as a **clickable prototype with sample data**, built twice: once in the web scene (shared by Tauri and
Electron) and once in GPUI. Both builds must look the same and behave the same. Nothing here does real work: no
network, no files, no auth. Buttons change local state only.

Everything in SPEC.md still holds unless this file changes it. In particular:
- The scenario (idle → transitions Home↔Library → scroll Library → minimized → destroyed) is untouched.
- Home and Library keep their layout, sizes and data.
- When nothing animates, the app requests no frames. Every new animation below is short and stops by itself, so
  idle stays idle.

## Shared inputs

| File | What it holds |
|---|---|
| `assets/copy.json` | **Every UI string**, as a flat map `key → text`. `{name}` placeholders are filled by code. No UI text may be hard-coded in either build: Piotr edits copy in one place. Ignore keys starting with `_`. |
| `assets/screens.json` | Sample content: accounts, modes and their layers, pre-launch checks, practice maps, tutorials, tools, stats, run submission, settings, game session, onboarding. Names, versions and credits of real community maps/tools/videos are accurate. Do not change them. |
| `assets/data.json` | Unchanged (Home tiles, pace, 500 leaderboard rows). |
| `assets/icons/*.svg` | Lucide icons (ISC licence, `LICENSE-lucide.txt`): 24×24, stroke 2, `currentColor`. Plus `logo.svg` (pixel "S", fill). Always draw them monochrome, tinted with the current text colour. |

Placeholder fill: replace `{key}` with the value. Unknown placeholders stay as they are.

Lookup helper names (suggested): web `t(key, args?)`, GPUI `t(key)` / `tf(key, &[(k, v)])`.

## New colour token

| Token | Hex | Use |
|---|---|---|
| warn | `#FBBF24` | warnings (Toolscreen legal-list check, expired sign-in, launcher-legality note) |

Translucent fills below are written as `token @ NN%` (alpha).

## Shared components

Sizes are logical px. Fonts: Inter (UI), JetBrains Mono (times, versions, numbers, codes).

- **Card:** bg `surface`, 1 px `border`, radius 14, padding 18. A card title is Inter SemiBold 15, `text`, 12 px
  above the content, with an optional **pixel bullet** in front: a 6×6 square in `accent`, no rounding, 8 px gap.
- **Buttons** (height 40, radius 10, padding 0 18, Inter SemiBold 14, optional 16 px icon with 8 px gap):
  - primary: bg `accent`, text `text`; hover `accent-hover`.
  - secondary: bg `raised`, 1 px `border`, text `text`; hover border `accent`.
  - ghost: transparent, text `muted`; hover text `text`.
  - danger: like secondary with text `behind`.
  - small: height 32, radius 8, padding 0 12, Inter SemiBold 13.
  - disabled: bg `raised`, text `muted`, no hover, not clickable. Don't use element opacity for this; GPUI has no group
    opacity.
- **Chip** (filters): height 30, radius 15, padding 0 12, Inter Medium 13. Off: bg `surface`, 1 px `border`, text
  `muted`. On: bg `accent @ 18%`, 1 px `accent`, text `text`.
- **Badge:** height 22, radius 6, padding 0 8, Inter SemiBold 11. Variants:
  - ok: `ahead` text on `ahead @ 12%`;
  - warn: `warn` on `warn @ 12%`;
  - muted: `muted` on `raised`;
  - pb: `pb` on `pb @ 14%`;
  - accent: `text` on `accent @ 30%`.
- **Toggle:** 36×20 track, radius 10; 16 px round knob, inset 2 px, colour `text`. On: track `accent`, knob on the
  right. Off: track `border`, knob on the left. The knob slides in 120 ms ease-out.
- **Segmented control:** container bg `raised`, radius 10, padding 3, items height 30, radius 8, padding 0 12,
  Inter Medium 13. The selected item has bg `accent` and text `text`; the others are `muted`.
- **Checkbox:** 18×18, radius 5. Off: 1.5 px `border` outline. On: bg `accent` plus a 12 px `check` icon in `text`.
- **Progress bar:** 4 px tall, radius 2, track `border`, fill `accent`.
- **Status icon** (20 px): `circle-check` in `ahead`, `triangle-alert` in `warn`, `info` in `muted`, `circle-x` in
  `behind`.
- **Toast:**
  - Pill at the bottom centre of the window, 24 px from the bottom; bg `raised`, 1 px `border`, radius 12,
    padding 10 16, Inter Medium 13.
  - Fades in over 150 ms, stays 1800 ms, fades out over 150 ms.
  - Used for "Opens in your browser" and "Details copied".
- **Spinner:** the `refresh-cw` icon rotating once per 900 ms. It runs only while something is "working".
- **Fake install:**
  - Clicking Install or Update shows the button as `common.installing` with a progress bar under the card that
    fills over 1200 ms (linear).
  - The item then becomes Installed: an ok badge with a check icon, and no button.
  - Frames are requested only while the bar fills.

## Rail v2 (replaces SPEC's 64 px letter rail)

- 72 px wide, bg `bg`, 1 px right border `border`.
- Top: `logo.svg` at 26×26 in `accent`, centred, 18 px from the top, with 18 px below it.
- **Nav items:**
  - 56×52 buttons, radius 12, 4 px apart.
  - Each has a 20 px icon centred 9 px from the top, and a label 4 px under it (Inter Medium 10.5 px, centred).
  - Inactive: icon and label in `muted`. Hover: bg `raised @ 60%`, icon and label in `text`. Active: bg `raised`,
    icon in `accent`, label in `text`, plus the same 3×20 accent bar as before on the rail's left edge, vertically
    centred on the item.
- Items, top to bottom:

  | Key | Icon | Opens |
  |---|---|---|
  | `rail.home` | house | Home |
  | `rail.modes` | layers | Modes |
  | `rail.practice` | target | Practice |
  | `rail.learn` | graduation-cap | Learn |
  | `rail.tools` | wrench | Tools |
  | `rail.stats` | chart-no-axes-column | Stats (Overview tab); also active while Library or Submit is shown |

- Bottom, 16 px from the window bottom:
  - `rail.settings` (settings icon), the same nav item;
  - below it, 10 px apart, the **account avatar**: 36×36, radius 10, 1 px `border`, showing the active account's
    avatar (nearest-neighbour). It opens Accounts. While Accounts is shown, it gets a 2 px `accent` ring instead
    of the border.
  - No label under the avatar. `rail.account` is its tooltip/accessible name.

Every navigation between rail pages uses SPEC's page transition (out 150 ms, then in 220 ms). Onboarding and Game are
full-window screens without the rail; switching to or from them uses the same transition on the whole window
content.

## Page header v2

- Same as SPEC (title Bold 26 plus muted subtitle 13).
- It can now carry **actions on the right**, vertically centred on the title row: a segmented control (Stats) or
  buttons (Tools).
- Home's title, subtitle and texts now come from copy.json (same strings as before).
- Library's header changes to `library.title` / `library.subtitle` and shows the Stats tabs on the right (see Stats).

## Screen ids and deep link

Every screen state has an id. The app opens straight on that state when **`BAKEOFF_SCREEN=<id>`** is set and scenario
mode is off. With `BAKEOFF_OUT` set, `BAKEOFF_SCREEN` is ignored and the app starts on Home as always.

- **Web:** the host passes `screen` in `config()` (Tauri command and Electron preload). The scene also accepts
  `#screen=<id>` in the URL, for screenshots in a plain browser.
- **GPUI:** read the env var.

| id | State |
|---|---|
| `home` | Home (default) |
| `library` | Leaderboard (SPEC's Library page) |
| `onboarding-welcome`, `onboarding-signin`, `onboarding-signin-code`, `onboarding-mode`, `onboarding-java`, `onboarding-eye`, `onboarding-obs`, `onboarding-done` | Onboarding steps (`-signin-code` = Sign in with the code panel open) |
| `accounts` | Accounts |
| `modes` | Modes, MCSR Ranked selected |
| `check` | Home with the pre-launch check modal open |
| `practice` | Practice, filter All |
| `learn` | Learn, all levels |
| `tools` | Tools |
| `stats` | Stats, Overview tab |
| `submit` | Submit run for the PB run |
| `settings` | Settings |
| `game` | Game running, with the tray menu mock |

## Layout budget

- Window 1280×800, rail 72, so the content area is 1208×800.
- Page padding is 28, the header is ~54 px tall and the header-to-body gap is 20, so the body is ~1152×670.
- Every screen must fit without a window scrollbar. Long lists scroll inside their own container:
  - web: `overflow: auto`, thin dark scrollbar;
  - GPUI: `overflow_y_scroll` with an id, or `uniform_list`.

---

## 1. Onboarding (full window, no rail)

- Window bg `bg`, with `glow.png` static (opacity 0.8) centred on the window's top-right corner.
- **Left panel:** 340 px wide, full height, bg `surface`, 1 px right `border`, padding 32.
  - Top: logo 28 px in `accent`, plus the word "Speedrunluncher" in Inter Bold 18, 10 px gap. This product name is
    not copy.
  - 40 px below: the step list, one row per step (`onboarding.steps.*`). Rows are 40 px tall with a 12 px gap
    between marker and label.
    - Each row has a 10×10 square marker (pixel style, no radius) and the label Inter Medium 14.
    - Done steps: marker `accent`, label `muted`, plus a 14 px `check` in `ahead` at the right end.
    - Current step: marker `accent` with a 2 px `accent @ 30%` ring, label `text`, row bg `raised`, radius 10,
      padding 0 12.
    - Future steps: marker `border`, label `muted`.
  - Bottom: `onboarding.step_of` in muted 12.
- **Right side:**
  - Content column 600 px wide, centred horizontally, top padding 72.
  - Step title Inter Bold 28 (line height 36), then 10 px gap, then body 15 px `muted` (line height 22, max width
    560), then a 28 px gap, then the step content.
  - **Footer** (pinned 40 px from the bottom, same 600 px column): Back (ghost, with `chevron-left`) on the left, and
    the primary action on the right. Next uses `chevron-right` after the label. Welcome has no Back.

Steps (Next goes forward, Back goes back; every step's state survives Back/Next):

1. **welcome**
   - `hero.jpg` banner: 600×180, radius 14, 1 px `border`, cover. It sits above the title (so the title comes 24 px
     below the banner).
   - Primary `onboarding.welcome.cta` replaces Next.
   - Under the footer row on the left, a ghost button with `onboarding.welcome.have_setup` (it does nothing).
2. **signin**
   - Two option cards stacked, 12 px apart, each 600×76, padding 18:
     - a 40×40 icon tile (bg `raised`, radius 10, 20 px icon);
     - title Inter SemiBold 15 and hint muted 13;
     - a `chevron-right` on the right.
   - Card 1: `globe` icon, `onboarding.signin.browser`, `onboarding.signin.browser_hint`, with a
     `common.recommended` accent badge after the title. Card 2: `qr-code` icon, `onboarding.signin.code`,
     `onboarding.signin.code_hint`.
   - Clicking card 1:
     - the card shows a spinner plus `onboarding.signin.waiting` for 1500 ms;
     - then both cards are replaced by a success row (card, 64 px tall): the avatar of `accounts.list[0]` at 36×36
       radius 8, `onboarding.signin.done` with `name`, and a `circle-check` in `ahead`.
   - Clicking card 2 (or deep link `onboarding-signin-code`) shows the **code panel** in place of the cards. It is a
     card with two columns:
     - Left: `onboarding.signin.code_label` with `url` = `accounts.device_url` (muted 13); then the code
       `accounts.device_code` in Mono Bold 32, `text`, letter-spacing 3 px in web (GPUI can't do letter-spacing,
       so leave it out); then the spinner plus `onboarding.signin.waiting`.
     - Right: the QR code from `accounts.device_qr` (rows of 0/1). Draw it 4.5 px per module (33 modules = 148.5;
       rounding to 148 or 149 is fine), black modules `#0A090E` on a white `#FFFFFF` quiet zone of 4 modules, in
       a radius-10 white tile.
     - Below the panel, a ghost button `common.back` returns to the two cards.
   - Next is disabled until signed in, except on the deep-link states. For screenshots, the `onboarding-signin`
     deep link shows the two cards.
3. **mode**
   - A list card containing 5 rows (from `modes`), each 64 px tall, separated by 1 px `border` lines:
     - a 36×36 icon tile with the mode `icon`;
     - name Inter SemiBold 15, then `full` in muted 12 to its right, 8 px apart;
     - `desc` muted 13 below;
     - right side: a `mc` badge (muted, Mono 11 text), then a radio (an 18 px circle with a 1.5 px border, or when
       selected, an `accent` fill with an 8 px `text` dot).
   - The `recommended` mode gets a `common.recommended` accent badge after its name and is selected by default.
   - Clicking a row selects it. Selected row bg: `accent @ 10%`.
4. **java**
   - A card with two setting rows (see Settings for the row style):
     - `onboarding.java.java_label` with value `onboarding.java.java_value` (muted 13) and a `circle-check` on the
       right;
     - `onboarding.java.memory_label` with hint `onboarding.java.memory_hint`, and a segmented control from
       `settings.memory` (default index `settings.memory_default`).
5. **eye**
   - A card with three checkbox rows, 56 px each: checkbox, label 14 (the copy text), and a platform badge on the
     right.
   - Rows: `onboarding.eye.windows` (badge `tools.windows_only`), `onboarding.eye.linux` (badge `tools.linux_only`),
     `onboarding.eye.ninjabrain` (no badge).
   - The row for the OS you're running on is checked; the other OS row is unchecked and drawn in `muted`.
     Ninjabrain is checked.
   - Footer: ghost `onboarding.eye.later` to the left of Next.
6. **obs**
   - A card with:
     - a status row: `circle-check` plus `onboarding.obs.found` with `version` = `onboarding.obs_version`;
     - a setting row: `onboarding.obs.add` with a toggle (on).
   - Under the card, a ghost button `onboarding.obs.no_obs` (it sets the toggle off).
7. **done**
   - A 64×64 round tile (bg `accent @ 18%`) with a 32 px `check` in `accent`, above the title.
   - The body uses `mode` = the selected mode's name.
   - A summary card with three 40 px rows, each a muted label and a value on the right:
     - `rail.modes` → the mode name;
     - `onboarding.java.java_label` → "Temurin 21 · <memory>";
     - `rail.tools` → "Toolscreen, Ninjabrain Bot" (or "waywall, Ninjabrain Bot" on Linux; these are tool names
       from data).
   - Footer: secondary `onboarding.done.learn` (it opens Learn) and primary `onboarding.done.launch` (it goes to
     Home). No Back on this step.

## 2. Accounts

Header: `accounts.title` / `accounts.subtitle`. The body is two columns with a 20 px gap.

- **Left (flex):**
  - A card with one row per account in `accounts.list`, 72 px each, separated by 1 px lines:
    - 44×44 avatar, radius 10, nearest-neighbour;
    - name Inter SemiBold 15, with the status under it in 12 px: `accounts.status_ok` with `ago` in `muted`, or
      `accounts.status_expired` in `warn`;
    - right side: an `accounts.active` ok badge for the active account, or a secondary small button
      `accounts.set_active` (`accounts.sign_in_again` if expired); then a ghost small button `accounts.sign_out`.
  - Clicking "Use this account" makes that account active (and updates the rail avatar).
  - Under the card: a secondary button with `plus` and `accounts.add`, 16 px below.
  - At the bottom of the column: a `key-round` icon (16 px, muted) plus `accounts.storage_note` with `store` =
    `accounts.store[<current OS>]`, muted 12, max width 520.
- **Right (fixed 440 px):**
  - A card titled `accounts.add_title`, containing the same two sign-in option cards and the same code panel as the
    onboarding signin step, scaled to the column (the QR may shrink to 4 px per module).
  - "Add account" scrolls to or highlights this card: give it a 1 px `accent` border for 600 ms.

## 3. Modes

Header: `modes.title` / `modes.subtitle`. The body is two columns with a 20 px gap.

- **Left (flex): mode cards** in a 2-column grid, gap 16. Cards are 132 px tall with padding 18. Each card has:
  - row 1: a 32×32 icon tile with the mode `icon` and the name in Inter Bold 17;
  - the status badge sits at the right end of row 2 (row 1 is too narrow for name + Default + status):
    - `modes.status_ready` (ok);
    - `modes.status_update` (accent);
    - `modes.status_missing` (muted);
  - row 2: `full` in muted 12 plus `mc` in Mono 12 muted, joined with " · ";
  - row 3: `desc` in 13 px `text @ 85%`, up to 2 lines;
  - for the default mode, a `modes.default` badge (pb) after the name.

  The selected card has bg `raised` and a 1 px `accent` border. Click to select; MCSR Ranked starts selected.
- **Right (fixed 460 px, full body height): detail card.**
  - Top: the mode name in Inter Bold 20, then `modes.leaderboard` with `name` = `leaderboard` as a muted badge. If the
    mode has a `note`, it follows as a `warn` line with `triangle-alert` 14, in 13 px.
  - Then a pixel-bullet title `modes.layers`.
  - Then the layer list, in the order the mode lists them (top layer first, Java last). Each layer row is 44 px:
    - a 10×10 square: `accent` if no other mode uses this layer, `border` otherwise;
    - the layer name Inter SemiBold 14;
    - right side: `modes.mods_count` (`n` = number of mods) in muted 12 if it has mods, otherwise the layer
      `detail` in muted 12;
    - under the name, in muted 12: `modes.shared_with` with `modes` = the names of the other modes that list this
      layer, comma-separated. If no other mode lists it, show `modes.only_here` as an accent badge instead.
  - Clicking a layer that has mods expands it. Under the row it shows its mods as 28 px lines: a 14 px
    `circle-check` in `ahead`, the mod name 13, and the version in Mono 12 muted on the right. Clicking it again
    collapses it.
  - Expansion is instant: no layout animation.
  - The mode's own top layer starts expanded.
  - At the bottom of the card:
    - a `modes.disk_saved` line (`size` = `disk_saved`, muted 12, `hard-drive` icon 14);
    - a button row: primary `modes.play` (`mode` = name), secondary `modes.verify`, and a ghost `modes.set_default`.
      The ghost button is hidden if the mode is already the default; clicking it moves the default badge.
  - For `missing` modes, the primary button is `modes.install` and runs the fake install. When it finishes, the
    status becomes ready.

## 4. Pre-launch check (modal over Home)

- Clicking LAUNCH on Home opens it; the `check` deep link opens it directly.
- **Overlay:** covers the whole window, rail included, with `#050408 @ 70%`. Clicking the overlay closes the modal.
- **Modal:**
  - A card 600 px wide, centred, padding 24, radius 16, bg `surface`, border `border`, with a drop shadow if the
    stack has one.
  - It opens with a 150 ms fade of the overlay and a fade of the modal; the modal also moves from +8 px to 0 over
    the same 150 ms. Closing is 120 ms.
  - GPUI note: there's no group opacity, so animate the overlay alpha and the modal's offset only, and fade the
    modal by interpolating its bg/border/text alpha if that's simple; otherwise offset only.
- **Content:**
  - Title `check.title` in Inter Bold 20, then `check.subtitle` (`mode` = the default mode's name) in muted 13.
  - Then one row per `checks` item, 52 px each, separated by 1 px lines:
    - the status icon;
    - label `check.<id>` in Inter SemiBold 14;
    - detail `check.<id>_detail` filled with `args`, in muted 13, on the same line after a 12 px gap. It may wrap
      under the label if it doesn't fit; keep it on one line where possible.
    - Right side: for `warn` rows with a `check.<id>_action` key, a secondary small button with the action text
      (filled with `args`). Clicking it turns the row ok, and the detail becomes `check.<id>_fixed`.
  - Footer: `check.footer` in muted 12 on the left. On the right, a ghost `common.cancel` and a primary button:
    `check.fix_and_launch` while any warn is left, otherwise `check.launch`.
  - "Fix and launch" first fixes every warn row (instantly), waits 400 ms, then continues like Launch. Launch goes to
    the **Game** screen.

## 5. Practice

Header: `practice.title` / `practice.subtitle`. Body, top to bottom:

- **Chip row:** `practice.filter_all`, then one chip per `practice_categories` entry (its `name`), gap 8. Clicking
  a chip filters the maps; All shows every map.
- A **scroll container** fills the rest of the body (no card around it) and holds:
  - A pixel-bullet section title `practice.maps`, 12 px above a 3-column grid, gap 16.
  - **Map cards**, height 168, padding 16:
    - row 1: a 40×40 tile (bg `raised`, radius 10) with the category icon in `accent`; then the name in Inter
      SemiBold 15 (1 line, ellipsis) with the version in Mono 12 muted under it;
    - row 2: credits in muted 12: `practice.by` with `authors`, plus " · " and `practice.includes` with `includes`
      if `includes` isn't empty;
    - row 3: `desc` in 13 px, up to 2 lines;
    - bottom row: the category name as a muted badge; `practice.downloads` with `n` = `downloads` in Mono 12 muted
      if `downloads` isn't empty; and on the right the install state:
      - `installed`: ok badge with `common.installed`;
      - `update`: secondary small button `common.update`;
      - `available`: primary small button `common.install`.
    - The fake install progress bar sits along the card's bottom edge, inset 16.
  - A pixel-bullet title `practice.tools`, 28 px above a 3-column grid of the `practice_tools` cards (same card,
    with the tool `icon`; no category badge).
  - A tip row: `info` 16 muted plus `practice.ranked_tip` in muted 13, 20 px below the grid.

## 6. Learn (tutorials)

Header: `learn.title` / `learn.subtitle`.

- **Chip row:** `learn.filter_all`, `learn.level_beginner`, `learn.level_intermediate`, `learn.level_advanced`.
  Filtering hides non-matching cards, and hides groups that end up empty.
- **Scroll container** with one section per `tutorial_groups` entry, 28 px apart:
  - The section title is the group `icon` at 16 in `accent`, plus the `title` in Inter SemiBold 16.
  - Below it, a 4-column grid, gap 16, of **tutorial cards** (card style, padding 0, overflow hidden):
    - **Thumbnail:** 16:9, full card width (~276×155).
      - bg: a vertical gradient from `raised` to `#120F1A`, with the group icon at 56 px in `accent @ 35%`,
        centred.
      - On top, at the centre, a 44 px round button (bg `#000 @ 55%`) with a 20 px `play` icon in `text`. Use
        `book-open` instead of `play` when `kind` is `guide`.
      - A badge bottom-right, 8 px inset, with `length`, or with the `kind` if `length` is empty: bg
        `#000 @ 65%`, text in Mono 11.
    - **Text area** (padding 12 14 14):
      - `title` in Inter SemiBold 14, up to 2 lines (height reserved for 2 lines, so the cards line up);
      - `creator` in muted 12;
      - a row with the level badge (beginner → ok, intermediate → accent, advanced → pb, using the `learn.level_*`
        texts) and the `date` in muted 12 on the right. If `current` is true, add an ok badge `learn.current`
        after the level badge.
  - Hover: the card border turns `accent`.
  - Click: toast `common.opens_in_browser`. Don't open the URL.

## 7. Tools

- Header: `tools.title` / `tools.subtitle` (`ago` = `tools_checked_ago`).
- Header action on the right: a secondary button `tools.check_now` with a `refresh-cw` icon. Clicking it spins the
  icon for 1000 ms, then the subtitle uses `ago` = `tools.checked_now`.
- Body: one card holding a list of `tools`, 84 px per row, separated by 1 px lines, padding 0 18. Each row:
  - a 44×44 icon tile with the tool icon.
  - A text column:
    - line 1: the name in Inter SemiBold 15, the version in Mono 12 muted (skip it if empty), and a platform badge
      if `platform` isn't `all` (`tools.windows_only` / `tools.linux_only`, muted);
    - line 2: `desc` in muted 13;
    - line 3 (only if the tool has `newer`): `triangle-alert` 12 plus `tools.legal_note` with `version` = `newer`,
      in `warn` 12.
  - The right side depends on the tool:
    - **installed:** two labelled toggles side by side, 24 px apart: label muted 12 above, toggle below. One is
      `tools.start_with_game`, the other `tools.auto_update`. If `pinned` is true, the auto-update toggle is
      replaced by a pb badge with a `key-round` 12 icon and `tools.pinned`, still under the
      `tools.auto_update` label so the columns line up.
    - **not installed and `external`:** a secondary small button `tools.get` (`name` = the tool name) with
      `external-link`.
    - **not installed** (e.g. waywall on Windows): a muted badge `tools.not_installed` and a primary small button
      `common.install` (fake install).
  - A tool whose `platform` is the other OS is drawn with its name in `muted`.

## 8. Stats

- Header: `stats.title` / `stats.subtitle` (`n` = `stats.total_runs`).
- Header action on the right: a segmented control with `stats.tab_overview` | `stats.tab_leaderboard`.
  - Leaderboard navigates to the **Library** page (SPEC's page transition).
  - The Library header shows the same control with Leaderboard selected; Overview there navigates back to Stats.
- **Overview body:**
  1. **Tile row:** 4 equal tiles, height 112, gap 16, card style. Each has a muted label 13, a big value in Mono
     Bold 28, and a sub line in muted 12:
     - `stats.tile_pb`: value in `pb`, sub = `tiles.pb_category`;
     - `stats.tile_runs`: `runs_month`, sub `runs_month_sub`;
     - `stats.tile_finish`: `finished`, sub `finished_sub`;
     - `stats.tile_avg_enter`: `avg_enter`, sub `avg_enter_sub` in `ahead`, because it starts with "-".
  2. **Card `stats.recent`** (pixel-bullet title), filling the remaining height. The table header row is 32 px,
     muted 12, Inter Medium:
     - columns: `stats.col_date` (80 px), `stats.col_mode` (90 px), `stats.col_result` (170 px), then one column per
       entry in `stats.splits`, right-aligned and sharing the space, then a 90 px action column;
     - 8 data rows, 50 px each, 1 px lines;
     - date in muted 13 and mode in 13;
     - result:
       - finished: the time in Mono 14, `text`; for `pb` runs, `pb` colour plus a pb badge `stats.pb`;
       - not finished: `stats.reset_at` with `split` = `result`, in muted 13;
     - each split cell: the time in Mono 13 with the delta under it in Mono 11 (`ahead` if the 3rd item is true,
       otherwise `behind`). An empty delta shows nothing. A missing split shows "—" in `border` colour.
     - action column: for finished RSG runs only, a secondary small button `stats.submit`. It opens **Submit run**.

## 9. Submit run

- Header: `submit.title` / `submit.subtitle` (`time`, `category`, `date`). Above the title, a ghost small button
  with `chevron-left` and `rail.stats` goes back to Stats.
- The body is two columns with a 20 px gap.
- **Left (flex):**
  - **Card `submit.files`:**
    - a 40 px row per `files` item: `file-check` 16 in `ahead`, the label 14, and the file name in Mono 12 muted on
      the right;
    - then a 1 px line;
    - then a footer row: `folder-open` 16 plus `zip` in Mono 13, `zip_size` in muted 12, the ok badge
      `submit.files_ready`, and a ghost small button `submit.show_folder` on the right.
  - **Card `submit.timing`**, 16 px below: two blocks side by side.
    - `submit.igt`: muted 12 label, then `igt` in Mono Bold 30, `pb`.
    - `submit.rta`: muted 12 label, then `rta` in Mono Bold 22, `text`.
- **Right (fixed 480 px):**
  - **Card `submit.video`:**
    - A text field, 40 px tall, radius 10, bg `bg`, 1 px `border`, padding 0 12, with placeholder
      `submit.video_placeholder` in muted 14.
    - To its right, a secondary button `submit.paste`, which fills in `sample_link`.
    - Web: a real `<input>`. GPUI: a click-to-focus box. Typing is optional; Paste is enough.
    - When filled, the text is Mono 13 `text` and the field border is `accent`.
  - **Card `submit.checklist`**, 16 px below:
    - two checkbox rows (`submit.rule_video`, `submit.rule_files`), 13 px text, wrapping allowed;
    - then a warn note row: `triangle-alert` 16 plus `submit.rule_launcher` in `warn` 13, on bg `warn @ 8%`,
      radius 10, padding 10 12.
  - **Button row** (right-aligned, 16 px below): secondary `submit.copy` with a `copy` icon (toast `submit.copied`),
    and primary `submit.open_src` with `external-link`.
    - The primary button is disabled until the link is filled and both boxes are ticked.
    - When clicked: toast `common.opens_in_browser`.

## 10. Settings

- Header: `settings.title` / `settings.subtitle`.
- The body is two columns of section cards, gap 20, and each column stacks its cards with a 16 px gap:
  - left: General, Game;
  - right: Controls, Updates, Advanced.
- The card title is the pixel bullet plus the section name.
- Section cards use 4 px bottom padding and a 4 px gap under the title, because the rows carry their own spacing
  (otherwise the right column doesn't fit 670 px).
- **Setting rows** are 56 px tall (taller if there's a hint), separated by 1 px lines:
  - a label in Inter Medium 14, with an optional hint in muted 12 under it;
  - a control on the right.
- Rows:
  - **General**
    - `settings.language`: segmented control from `settings.languages`, English selected. It doesn't switch the
      UI.
    - `settings.close_on_launch` with hint `settings.close_on_launch_hint`: toggle.
    - `settings.start_with_system`: toggle.
  - **Game**
    - `settings.java`: value `settings.java_value` in muted 13, plus a `circle-check` 16.
    - `settings.memory` with hint `settings.memory_hint`: segmented control `settings.memory`.
  - **Controls**
    - `settings.keybinds`: segmented control `settings.keybinds`.
    - `settings.sensitivity`: value `settings.sensitivity` in Mono 13 inside a 64 px box (bg `bg`, 1 px border,
      radius 8, height 32, centred).
    - `settings.standard_settings`: toggle.
  - **Updates**
    - `settings.auto_update`: toggle.
    - `settings.channel`: segmented control `settings.channels`.
  - **Advanced**
    - `settings.data_folder`: the path for the current OS in Mono 12 muted, plus a ghost small button with
      `folder-open` (`common.open_folder`).
    - `settings.run_setup`: secondary small button with the same text. It goes to `onboarding-welcome`.
    - `settings.reset_all`: danger small button with the same text. It shows a toast with the same text and does
      nothing else.
- Toggle defaults come from `settings`.

## 11. Game running (full window, no rail)

- Window bg `bg` with the static glow (as in onboarding). No frame loop: everything here is static.
- **Centre card:** 620 px wide, padding 28, centred, 40 px above the vertical centre.
  - Row: a 10×10 square in `ahead` (pixel style), then `game.title` in Inter Bold 28.
  - `game.subtitle` (`mode` = `game.mode`, `ago` = `game.started`) in muted 13.
  - 24 px gap, then a pixel-bullet title `game.session`, then 3 equal mini tiles, gap 12, height 84 (bg `raised`,
    radius 12, padding 14):
    - `game.resets` → `resets`;
    - `game.best` → `best`, in `pb`;
    - `game.nether_entries` → `nether_entries`.
    - Each has a muted 12 label and a value in Mono Bold 22.
  - 24 px gap, then a row: danger button `game.stop` with `circle-x` (it goes to Home), and on its right, `info` 16
    plus `game.window_note` in muted 12, wrapping.
- **Tray mock**, bottom-right:
  - **Fake taskbar:** a strip 36 px tall across the whole window bottom, bg `#050408`, 1 px top `border`. On its
    right, four 16 px icons in `muted`, 14 px apart (`globe`, `video`, `monitor`, then `logo` in `accent` inside a
    28×28 bg `raised` radius 6 tile, which is "our" tray icon), then a clock "21:37" in Mono 12 muted, 16 px from the
    right edge.
  - **Tray menu:** a 264 px wide card (bg `raised`, 1 px border, radius 12, padding 6, shadow if available), 8 px
    above the taskbar, right-aligned with our tray icon.
    - First a 40 px header row: logo 16 in `accent`, plus `tray.status` (`mode` = `game.mode`) in Inter SemiBold
      13.
    - Then a 1 px line, then 36 px item rows: icon 16 muted, label 13. Hover: bg `surface`, radius 8.
      - `tray.show` (house): goes to Home;
      - `tray.ninjabrain` (eye): shows a toast with the item text;
      - a 1 px line;
      - `tray.stop` (circle-x, label in `behind`): goes to Home;
      - `tray.quit` (power): shows a toast with the item text.
  - Above the menu, right-aligned, the caption `game.tray_title` in muted 11.

---

## Screenshots

Every build must be able to produce a 1280×800 PNG of every screen id (deep link, then wait until it's settled,
≥500 ms). Put them in `/tmp/srl/shots-v2/<stack>-<id>.png`, where stack is `web` or `gpui`.

## Language: English + Polish

- **Files:**
  - UI strings: `assets/copy.json` (en) and `assets/copy.pl.json` (pl), with the same keys.
  - Sample content: `assets/screens.json` plus the overlay `assets/screens.pl.json`, which has the same shape:
    - objects merge by key;
    - arrays merge by index;
    - `null` keeps the base value;
    - any other value replaces it.

  Load both and build one data set per language at startup.
- **Lookup:** use the current language's copy first, then English, then the key itself.
- **Current language:**
  - It starts as English.
  - `BAKEOFF_LANG=pl` starts in Polish. Web gets it as `lang` in `config()`, and also accepts `lang=pl` in the URL
    hash, combined like `#screen=settings&lang=pl`.
  - Scenario mode always uses English, so measurements stay comparable.
- **Settings → Language:** the segmented control (`settings.languages`: English, Polski) switches the whole UI
  instantly. That covers rail labels, headers, every screen, the sample content, toasts and the window title. There
  is no transition, and the choice is kept in memory only (prototype).
- **Learn:** when `length` is empty, the thumbnail badge shows `learn.kind_<kind>` (kind stays an id: `video`,
  `series`, `guide`).
- **Layout:** Polish text is about 20–30% longer. Every screen id must still fit in 1280×800 in both languages,
  with no clipped or overlapping text. Allowed fixes are wrapping, ellipsis where a line is a list cell, and small
  spacing changes. Don't resize the measured Home/Library scene. If a Polish string can't fit, report its key and
  screen id instead of changing copy files.
- **Screenshots:** `SHOOT_LANG=pl node bakeoff/tools/shoot-web.mjs …` and `BAKEOFF_LANG=pl bakeoff/tools/shoot-gpui.sh …`.
  Comparison sheets are in `shots/v2/` (English) and `shots/v2-pl/` (Polish).
- **Fixes made for Polish** (both stacks):
  - Onboarding mode rows wrap the description and grow, in both languages, instead of cutting it off.
  - Modes cards: only the `full` subtitle gets an ellipsis; the version and status badge always show.
  - Practice cards: the category badge gets an ellipsis and the downloads count doesn't wrap.
  - Sign-in option cards use 10 px vertical padding so two-line hints don't make them too tall.
  - Shorter Polish copy for `modes.status_update`, `check.toolscreen_detail`, `onboarding.java.memory_hint` and the
    Ranked and Draftout `full` lines.

## Decisions made while building (both stacks)

- There are 20 screen ids (the table above), not 19.
- Welcome: the footer block (button row plus the "already have an instance" link under it) sits 40 px from the
  bottom, so its button row is higher than on the other steps.
- Sign-in option hints wrap instead of using an ellipsis (they need 2 lines in the 440 px Accounts column).
- These buttons do nothing yet, because no behaviour is defined: Play and Verify files (Modes; web's Play opens the
  check), Sign out, Show in folder, Open folder, and "I already have an instance".
- Known GPUI gaps: no letter-spacing (device code), no nearest-neighbour image scaling (avatars are smoothed), no
  rounded clipping (thumbnails get their own corner radii), no visible scrollbar.
- Screenshots: `node bakeoff/tools/shoot-web.mjs <url of scene/dist/index.html> <dir>` (headless Chrome) and
  `bakeoff/tools/shoot-gpui.sh <dir>` (Xvfb).
