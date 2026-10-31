# 05 - Visual directions for Speedrunluncher

> The reference images (other apps' screenshots and Dribbble shots) are not in the public repository: they
> belong to their authors. The source links below lead to the originals.


Collected 2026-10-23. There are 36 reference images in 6 style directions. Performance and smoothness are top priorities, so every direction has a performance cost rating.

- Gallery (open in a browser): `/home/piotr/.t3/projects/speedrunluncher/research/visual/gallery.html`
- Overview contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-all.png`
- Images: `/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/`

## Celestial Launcher: what it is

Celestial Launcher is the free desktop mod manager for League of Legends custom skins, made by **Divine Skins** (divineskins.gg; signed by Divine Studio LLC). It runs on Windows 10/11 x64 and Apple Silicon macOS. You browse skins on the site or in the in-app store, install them in 2 clicks, organize them into profiles, folders and variants, repair them after patches, and press **Start Patcher** before a game. The installer names (`Celestial_<version>_x64-setup.exe`, `_arm64.dmg`) follow the Tauri bundler convention, so it is probably a Tauri (web UI) app. That is unverified.
- Download page: https://www.divineskins.gg/install-mods
- Releases repo: https://github.com/DivineSkins/celestial-releases (the README has no screenshots)
- Screenshots 01-06 are frames from the official walkthrough video https://www.youtube.com/watch?v=cLa2mmn2DEI (1080p, cropped to the app window). This was the only good source of real in-app screenshots. Lookalike repos (sxrmss/celestial-releases, BeauxMarka/Celestial-Manager) are unofficial copies and were ignored.

**What defines its look:** a near-black violet-tinted base (#0A090E), one violet accent (#8B5CF6-ish; the Divine brand colour is #783CB5) and green only for 'valid' states. It has a custom frameless title bar, a slim left icon rail, and a large hero carousel with a mascot. Below that is a grid of art-heavy cards with creator pills, and a right info column (sign in, play time, launcher time, NEXT LEAGUE PATCH card, social icons). Settings use small tracked-caps section labels. The first run is a 6-step stepper (Welcome, Language, Layout, League, Storage, Game Files) where you can pick sidebar or top-bar navigation and the corner style. The polish comes from restraint: one accent, quiet surfaces, and soft violet radial glows. It does not get it from heavy effects. That keeps it cheap to render.

## A - Celestial Violet Noir

Celestial's own look. Near-black with a violet tint, a slim icon rail on the left, a custom title bar, a wide hero banner with a slider, a grid of art cards with small creator pills, and a right-hand info column (sign in, play time, next patch). Settings are grouped under small tracked-caps labels (LEAGUE OF LEGENDS, LOCALIZATION). The first-run flow is a 6-step stepper with one centred card per step. The only colour is one violet accent plus a soft radial violet glow.

**Palette:** `#0A090E` app background, `#15131D` surface, `#1B1327` raised card, `#26222F` border, `#2E0754` violet glow, `#8B5CF6` accent (Divine brand #783CB5), `#22C55E` success / valid, `#ECEAF2` text, `#8A8796` muted text

**Typography:** Geometric sans (Manrope/Inter style), bold display headings, 11-12px uppercase tracked section labels.

**Motion:** Calm: opacity fades, carousel slides, a glow on the selected card. No video.

**Performance cost:** LOW. The surfaces are flat and dark. The glow can be one static radial gradient or a pre-rendered image. Keep blur off the window.

**Fit for a speedrun launcher:** Very good. Swap skin cards for instances, seeds and categories. Use the right column for PB, last run and the PaceMan status, and turn 'Start Patcher' into a big 'Launch' button. This direction is the closest to what Piotr already likes.

Contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-A.png`

| # | Image | Source | Credit |
|---|---|---|---|
| 01 | Celestial Launcher: home (hero carousel, skin grid, right info panel)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/01-celestial-home.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Divine Skins, frame from the official 'How to install Celestial' video |
| 02 | Celestial Launcher: library with toolbar (profiles, import, repair, Start Patcher)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/02-celestial-library.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Divine Skins, video frame |
| 03 | Celestial onboarding: layout picker (sidebar or top bar, corner style)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/03-celestial-onboarding-layout.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Divine Skins, video frame |
| 04 | Celestial onboarding: storage step with 6-step stepper<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/04-celestial-onboarding-storage.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Divine Skins, video frame |
| 05 | Celestial settings: game path, auto-detect, localization<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/05-celestial-settings.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Divine Skins, video frame |
| 06 | divineskins.gg/install-mods: Celestial landing page (purple theme)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/06-celestial-divineskins-site.png` | https://www.divineskins.gg/install-mods | Divine Skins, video frame |
| 07 | Minecraft Launcher Concept (dark, violet + green)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/07-dribbble-mc-launcher-concept-docy.png` | https://dribbble.com/shots/22562936-Minecraft-Launcher-Concept | docy on Dribbble |

## B - Cinematic Hextech Game Client

AAA game-client chrome: full-bleed key art, ornamented gold frames, all-caps serif headings, top navigation tabs, a persistent friends rail, and big PLAY buttons. Battle.net and Hytale are calmer versions of the same idea.

**Palette:** `#010A13` LoL near-black, `#0A1428` navy, `#1E2328` panel, `#785A28` dark gold, `#C8AA6E` gold, `#0AC8B9` hextech teal, `#F0E6D2` parchment text

**Typography:** Display serif in caps (Beaufort in LoL) with a humanist sans for body text (Spiegel). Wide letter-spacing on the CTAs.

**Motion:** Heavy: looping video backgrounds, particles, sounds on hover, ornate transitions.

**Performance cost:** HIGH. Video backgrounds, large layered PNGs, many shadows and glows. The LoL client is famous for being heavy. Only cheap if the art is a still image and the ornaments are flat SVG.

**Fit for a speedrun launcher:** Aspirational but asset-hungry. Speedrunners have no splash art. A light version could use still Minecraft biome or structure renders behind the hero, with the gold swapped for one accent colour.

Contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-B.png`

| # | Image | Source | Credit |
|---|---|---|---|
| 08 | League of Legends client: home (Eclipse Diana splash)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/08-lol-client-home.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Riot Games, frame from the Divine Skins video |
| 09 | League of Legends client: game mode select<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/09-lol-client-modes.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Riot Games, frame from the Divine Skins video |
| 10 | League of Legends client: champ-select loadout<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/10-lol-client-loadout.png` | https://www.youtube.com/watch?v=cLa2mmn2DEI | Riot Games, frame from the Divine Skins video |
| 11 | Battle.net app (2.0 redesign): game grid + friends rail<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/11-battlenet-app.jpg` | https://www.pcgamer.com/battlenet-just-got-a-total-makeover/ | Blizzard, via PC Gamer |
| 12 | Hytale launcher: home / play (official)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/12-hytale-launcher-home.png` | https://nodecraft.com/support/games/hytale/game-information/how-to-use-the-hytale-launcher | Hypixel Studios, screenshot via Nodecraft guide (red highlight box is theirs) |
| 13 | Warframe game launcher concept<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/13-dribbble-warframe-launcher.jpg` | https://dribbble.com/shots/6620996-Warframe-Game-Launcher-Design-Concept | Fabio Santos on Dribbble |
| 14 | Hytale launcher play concept<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/14-dribbble-hytale-launcher-concept.jpg` | https://dribbble.com/shots/15418211-Hytale-Launcher-Play-Concept | Morena Daniel on Dribbble |

## C - Glass and Aurora

Translucent frosted panels over soft aurora or photo gradients. Thin 1px light borders, rounded 16-24px corners, macOS-style sidebars.

**Palette:** `#0E1033` deep indigo, `#1D244E` indigo surface, `#42456A` glass tint, `#9B61AB` aurora magenta, `#B998B9` aurora pink, `#43649A` aurora blue, `#A1BAD2` ice highlight

**Typography:** SF Pro / Inter / Geist, light weights, generous spacing.

**Motion:** Floating, slow gradient drift, parallax, blur that changes with depth.

**Performance cost:** HIGHEST. backdrop-filter blur on large panes forces repaints whenever anything behind them moves, and it is costly in WebView2/Electron on low-end GPUs. Animated gradients repaint every frame. Mitigate with one pre-blurred static background image, opaque cards with only a tinted border, or native Windows Mica/Acrylic window effects instead of CSS blur.

**Fit for a speedrun launcher:** Pretty, but it fights the performance priority. Use it only as a static aurora background behind opaque surfaces, for example on the onboarding or splash screen.

Contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-C.png`

| # | Image | Source | Credit |
|---|---|---|---|
| 15 | Loom: network discovery for macOS (glass, aurora gradient)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/15-dribbble-loom-glass-macos.png` | https://dribbble.com/shots/27757812-Loom-Network-Discovery-for-macOS | Rahman Ash on Dribbble |
| 16 | Lunea: video client for macOS (glass over photo)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/16-dribbble-lunea-glass-video-client.png` | https://dribbble.com/shots/27609236-Lunea-A-Modern-Video-Client-for-macOS | Rahman Ash on Dribbble |
| 17 | Nova: space exploration dashboard (dark glass, cinematic)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/17-dribbble-nova-space-dashboard.png` | https://dribbble.com/shots/27303216-Nova-Space-Exploration-Dashboard-Concept | Dmytro Demianenko on Dribbble |

## D - Speedrun Data / Timer-Brutalist

Tools speedrunners actually use: dense tables, tabular numbers, red and green deltas, live pace rows, small icons for splits (nether, bastion, fortress, stronghold) and line charts. Almost no decoration. Ninjabrain Bot is a tiny always-on-top overlay.

**Palette:** `#141417` MCSR Ranked bg, `#1E1E22` row surface, `#29282C` divider, `#0E1625` PaceMan navy, `#6340C6` PaceMan purple, `#4ADE80` win / ahead, `#F87171` loss / behind, `#C084FC` elo / PB highlight

**Typography:** Monospace or tabular-figure sans (JetBrains Mono, Inter with tnum). Bold timers, small labels.

**Motion:** Near-instant. Number ticks and row highlight flashes, nothing ornamental.

**Performance cost:** LOWEST. Flat colours, text, 1px dividers. Easy to keep at 60fps or higher even with live data.

**Fit for a speedrun launcher:** Native fit for the content: splits, PBs, seed queue, live pace, Ranked elo. On its own it can feel utilitarian, so it works best as the data layer inside A or F.

Contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-D.png`

| # | Image | Source | Credit |
|---|---|---|---|
| 18 | MCSR Ranked: home (pixel logo, blurred nether bg)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/18-mcsr-ranked-home.png` | https://mcsrranked.com/ | MCSR Ranked, headless screenshot taken 2026-10-23 |
| 19 | MCSR Ranked: player stats (match list, elo chart, leaderboard rail)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/19-mcsr-ranked-player-stats.png` | https://mcsrranked.com/stats/Feinberg | MCSR Ranked, headless screenshot taken 2026-10-23 |
| 20 | PaceMan: live pace table<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/20-paceman-live.png` | https://paceman.gg/ | PaceMan, headless screenshot taken 2026-10-23 |
| 21 | PaceMan: stats leaderboards (dense tables, coloured splits)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/21-paceman-stats-leaderboards.png` | https://paceman.gg/stats/ | PaceMan, headless screenshot taken 2026-10-23 |
| 22 | Ninjabrain Bot: stronghold calculator overlay<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/22-ninjabrain-bot-overlay.png` | https://github.com/Ninjabrain1/Ninjabrain-Bot | Ninjabrain1, last frame of README demo.gif (700px, low-res) |
| 23 | Speedrun Settings (recording app concept, green)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/23-dribbble-speedrun-settings.png` | https://dribbble.com/shots/4076757-Speedrun-Settings | Thomas Hajny on Dribbble (800px) |

## E - Modern Minecraft Launcher (MC-native)

What the current Minecraft launchers look like: instance grids with mod-loader badges, mod lists with toggles, Jump-back-in rows, a friends sidebar, profile wizards. Optional pixel-font accents and block textures (MCSR Ranked's ON/OFF list).

**Palette:** `#17191C` Modrinth bg, `#292D30` Modrinth card, `#1BD96A` Modrinth green, `#141618` Prism bg, `#0B0B0D` Feather black, `#E33B3B` Feather red, `#120D08` MC dirt-dark, `#6B7D4A` MC grass

**Typography:** Inter or Montserrat for the UI, plus a pixel display face (Monocraft or Minecraft-style) only for logos, headers or timers.

**Motion:** Moderate: hover lift and scale on cards, skeleton loaders.

**Performance cost:** LOW-MEDIUM. Mostly flat. Costs come from many large thumbnails (lazy-load and decode off the main thread) and from in-game style menus that blur the world.

**Fit for a speedrun launcher:** Players already know this language. Good for the instance and mod-management screens. On its own it does not stand out, so borrow pixel accents for identity.

Contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-E.png`

| # | Image | Source | Credit |
|---|---|---|---|
| 24 | Modrinth App: home (Jump in, library grid, friends)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/24-modrinth-app-home.png` | https://modrinth.com/app | Modrinth |
| 25 | Modrinth App: instance page<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/25-modrinth-app-instance.png` | https://modrinth.com/app | Modrinth |
| 26 | Prism Launcher: dark theme instance grid (Linux)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/26-prism-launcher-dark.png` | https://prismlauncher.org/ | Prism Launcher |
| 27 | XMCL (X Minecraft Launcher): home<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/27-xmcl-home.png` | https://github.com/Voxelum/x-minecraft-launcher | Voxelum / XMCL |
| 28 | GDLauncher (Carbon): library<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/28-gdlauncher-home.jpg` | https://gdlauncher.com/ | GDLauncher |
| 29 | Feather Client: mod manager<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/29-feather-mod-manager.png` | https://feathermc.com/ | Feather |
| 30 | Feather Client: in-game mod menu<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/30-feather-ingame-mod-menu.png` | https://feathermc.com/ | Feather |
| 31 | Lunar Client launcher: Create New Profile wizard<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/31-lunar-profile-wizard.png` | https://www.lunarclient.com/news/lunar-client-monthly-recap-april-2026 | Lunar Client |
| 32 | MCSR Ranked: in-game pixel settings list (ON/OFF toggles)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/32-mcsr-ranked-pixel-settings.png` | https://mcsrranked.com/ | MCSR Ranked site asset |
| 33 | Minecraft Gaming Dashboard UI<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/33-dribbble-mc-gaming-dashboard.png` | https://dribbble.com/shots/26549009-Minecraft-Gaming-Dashboard-UI | Erik Padamans on Dribbble |

## F - Bento Dashboard

A home screen made of tiles in different sizes. Each tile holds one thing: a big number, a sparkline, a launch button or a list. RAGEBITE's game-launcher dashboard is the gaming version.

**Palette:** `#08090B` base, `#22232E` tile, `#423555` tile alt, `#5147CA` indigo accent, `#2FAFEB` cyan data, `#B0444A` alert red, `#DEF0FA` light-mode tile

**Typography:** Inter / Geist, very large numbers next to small captions.

**Motion:** Tiles enter in a staggered sequence with transform and opacity only. Hover lifts by a few pixels.

**Performance cost:** LOW if the tiles are opaque and shadows are small or absent. Avoid blur and large spread shadows on every tile.

**Fit for a speedrun launcher:** Great for the home screen: a launch tile, a PB tile, a 'today's runs' sparkline, a PaceMan live tile, a Ranked elo tile, a seed-queue tile. Pairs naturally with A's palette and D's data style.

Contact sheet: `/home/piotr/.t3/projects/speedrunluncher/research/visual/sheet-F.png`

| # | Image | Source | Credit |
|---|---|---|---|
| 34 | Bento dashboard UI (dark teal tiles)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/34-dribbble-bento-dashboard-kreyda.jpg` | https://dribbble.com/shots/25960339-Bento-dashboard-UI | Serhii Antoniuk for Kreyda on Dribbble |
| 35 | Bento grid dashboard (purple tiles)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/35-dribbble-bento-revops.png` | https://dribbble.com/shots/25765472-Bento-grid-dashboard-for-revops-startup | Robin Holesinsky on Dribbble |
| 36 | Game Launcher Dashboard (dense dark bento, RAGEBITE)<br>`/home/piotr/.t3/projects/speedrunluncher/research/visual/refs/36-dribbble-ragebite-launcher-dashboard.png` | https://dribbble.com/shots/23197065-Game-Launcher-Dashboard | Boris Batocanin for RAGEBITE on Dribbble |

## Gaps and quality notes

- **Low resolution:** 22 Ninjabrain Bot (700x394, a GIF frame), 23 Speedrun Settings (800x600), 29 Feather mod manager (938x541), 26 Prism (802x629).
- **Video frames:** 01-10 come from a compressed YouTube 1080p stream (the app window is about 1400px wide), so fine text is slightly soft and the sampled hex values are approximate.
- **Annotation:** 12 Hytale has a small red highlight box around the settings gear, added by the Nodecraft guide.
- **Missing:** Steam (new library / Big Picture), Epic Games Launcher, Valorant / Riot Client, HoYoPlay, Wuthering Waves launcher, Rockstar launcher, Badlion and the official Minecraft launcher. The official sites only had marketing art or SPA pages without client screenshots, and the web-search budget ran out before press articles with screenshots could be found. speedrun.com blocked the headless browser with a Cloudflare check. No dedicated 'MCSR Launcher' app was found. MCSR Ranked and PaceMan are web or mod UIs.
- The HoYoPlay and Wuthering Waves website captures were not usable and were discarded.
