# 04 — Practice maps and tutorials catalogue

Research date: **2026-10-23**. Scope: Minecraft Java **1.16.1** (RSG, FSG, MCSR Ranked).

## How this was checked

- **GitHub repos and releases.** Stars, licence, latest release and per-asset download counts come from the GitHub REST API (`gh api`) on 2026-10-23. "Downloads" means the total across all release assets and versions. Every direct asset URL in the tables was checked live with a ranged GET (HTTP 206).
- **Checksums.** SHA-256 values come from the GitHub asset `digest` field. Four of them were confirmed by downloading the files: LBP 3.15.0, Zero Practice 1.2.2, Blaze Practice (Mescht) 1.1.0 and Ryguy End Practice 3.4.0. For those four the Minecraft version was also read from `level.dat`, and all four are 1.16.1.
- **Primary curated lists:**
  - The minecraftspeedrunning.com pages ([Maps](https://www.minecraftspeedrunning.com/public-resources/maps), [Tools and Resources](https://www.minecraftspeedrunning.com/public-resources/tools-and-resources), [Videos and Guides](https://www.minecraftspeedrunning.com/public-resources/videos-and-guides), [Documents](https://www.minecraftspeedrunning.com/public-resources/documents)). This is the site the speedrun.com MC moderators link to.
  - The speedrun.com MC resources list. The HTML page returns 403 to bots, so it was read through `https://www.speedrun.com/api/v2/GetResourceList?gameId=j1npme6p`.
  - The bundled catalogue in [cylorun/Map-Check](https://github.com/cylorun/Map-Check/blob/HEAD/maps.json).
  - The [MCSR Ranked wiki](https://wiki.mcsrranked.com/) (source: [MCSR-Ranked/Wiki](https://github.com/MCSR-Ranked/Wiki)).
- **YouTube.** Every video was checked through oEmbed or the watch page. That gave the title, channel, publish date and view count, and showed whether the author has marked it OUTDATED.
- **Licence wording used below.** *No licence – link only / ask author* means the repo has no LICENSE file and the README gives no redistribution terms. Legally that means all rights are reserved.
- **Status markers:** ✅ verified live today · ⚠️ verified, but there is a caveat · ❌ dead or stale link · ❔ not verified (no download found, or not tested).

---

## 1. Practice maps and practice tools

### 1.1 Core practice maps (recommended set)

These are the maps on the moderators' [curated list](https://www.minecraftspeedrunning.com/public-resources/maps) and in the Map-Check catalogue. The curated list says: *"The MCSR Practice Map is recommended, as it contains most of the below maps."*

| # | Name | Author(s) / credits | Practises | MC | Required mods | Download (direct asset) | Licence / redistribution | Last release | Popularity (2026-10-23) | Install |
|---|---|---|---|---|---|---|---|---|---|---|
| M1 | **The MCSR Practice Map** v2.0.0 ✅ | Dibedy (hub, OW practice, queue games). Bundled sections: Mescht (portal, zero, search crafting, fortress), LlamaPag (bastion). | All-in-one: portals; zero/one-cycle and zero setup; overworld (shipwreck, desert temple, village, RP+BT, BT looting, crafting, inventory sorting); movement (pearls, bridging, boats); search crafting; bastion; fortress spawner; Ranked queue mini-games | 1.16.1 only | none | [MCSR.Practice.v2.0.0.zip](https://github.com/Dibedy/The-MCSR-Practice-Map/releases/download/latest/MCSR.Practice.v2.0.0.zip) (92.5 MB). Repo: [Dibedy/The-MCSR-Practice-Map](https://github.com/Dibedy/The-MCSR-Practice-Map) | No licence – link only / ask author. ⚠️ This is a compilation, so permission for mirroring would also involve Mescht and LlamaPag. | 2026-01-12 (release tag is literally `latest` and gets moved) | ★133; 272,947 total downloads (169,389 on v2.0.0). Top recommendation on the moderators' page. Linked from the MCSR Wiki YouTube videos, the draconix 2026 guide and the HiTrippy guide. | Zip with one world folder → extract into `saves/`. Ko-fi: `dibedy`. |
| M2 | **Llama's Bastion Practice (LBP)** 3.15.0 ✅ | LlamaPag ("Llama") | All 4 bastion types (bridge, housing, stables, treasure), with full control over generation and loadouts | 1.16.1 (README: "does not work in 1.16.2+") | none | [LBP.3.15.0.zip](https://github.com/LlamaPag/bastion/releases/download/3.15.0/LBP.3.15.0.zip) (26.3 MB). Repo: [LlamaPag/bastion](https://github.com/LlamaPag/bastion). Explainer videos: [overview](https://youtu.be/jlA-jW7VGqw), [loadouts](https://youtu.be/uTn9LwB5WAE) | No licence – link only / ask author | 2026-06-18 (the previous release, 3.14.0, was 2023-03-26) | ★80; **466,710** downloads, the most downloaded MCSR map. Recommended in nearly every bastion guide. | Zip → extract to `saves/` (one folder, `LBP 3.15.0/`) |
| M3 | **Ryguy2k4's End Practice** v3.4.0 ✅ | ryguy2k4 | One-cycle (N/S and diagonal perches, beds/anchors, custom fountains), perch practice across 5 end islands, dragon options | 1.16.1 | none | [_Ryguy2k4_End_Practice_v3.4.0-1.16.1.zip](https://github.com/ryguy2k4/ryguy2k4endpractice/releases/download/v3.4.0/_Ryguy2k4_End_Practice_v3.4.0-1.16.1.zip) (15.6 MB). [Tutorial video](https://youtu.be/2IKzwVIvJdQ) | No licence – link only / ask author | 2023-01-11 (stable; the end fight in 1.16.1 hasn't changed) | ★51; 374,206 downloads | Zip → `saves/`. The release also ships a non-1.16.1 asset, so pick the asset matching `-1.16.1.zip` (Map-Check uses that pattern). |
| M4 | **Zero Practice** v1.2.2 ✅ | Mescht | Zero cycle: tower type and location, dragon rotation, damage and knockback display, loadouts | 1.16.1 | none | [Zero.Practice.v1.2.2.zip](https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/Zero.Practice.v1.2.2.zip) (9.1 MB). Repo: [Mescht/Zero-Practice](https://github.com/Mescht/Zero-Practice) | No licence – link only / ask author | 2025-08-23 | ★14; 116,813 downloads. Linked from the MCSR Wiki "How to ZERO CYCLE" video (Feb 2026). | Zip → `saves/`. To carry loadouts across updates, copy `data/command_storage_zero_practice_loadouts.dat` from the old world. Ko-fi: `mescht`. |
| M5 | **Blaze Practice** (Mescht) v1.1.0 ✅ | Mescht | Blaze spawner split: loadouts, item randomiser, blocked-spawn-space visualiser, spawning-potential calculator | 1.16.1 | none | [Blaze.Practice.v1.1.0.zip](https://github.com/Mescht/Blaze-Practice/releases/download/v1.1.0/Blaze.Practice.v1.1.0.zip) (3.7 MB). Repo: [Mescht/Blaze-Practice](https://github.com/Mescht/Blaze-Practice) | No licence – link only / ask author | 2026-05-21 | ★5; 8,221 downloads. It is the fortress map in Map-Check, and JackoWacko62's 2026 spawner-setup video links it. | Zip → `saves/`. Loadouts live in `data/command_storage_inventory.dat`. |
| M6 | **Blaze Practice** (Semperzz) v1.3 ✅ | Semperzz | Blaze fights: open spawner, closed spawner and double spawner; 3 loadouts | 1.16.1 | none | [Blaze.Practice.zip](https://github.com/Semperzz/Blaze-Practice/releases/download/v1.3/Blaze.Practice.zip) (9.2 MB). [Video](https://youtu.be/CNqrnlgCRXk) | No licence – link only / ask author | 2023-04-14 | ★11; 87,992 downloads. Listed as "Fortress" on the moderators' page. | Zip → `saves/` |
| M7 | **Portal Practice v2** v2.8 ✅ | Semperzz & ItzToxic | Building nether portals (ruined portal completions, etc.) | 1.16.1 | none | [Portal.Practice.v2.zip](https://github.com/Semperzz/Portal-Practice/releases/download/v2.8/Portal.Practice.v2.zip) (27.5 MB). [Video](https://youtu.be/WUr4MMiQwhI) | No licence – link only / ask author | 2023-02-11 | ★32; 290,466 downloads | Zip → `saves/`. README tip: render distance 16 if the inventory doesn't load. |
| M8 | **Portal Practice** (Mescht) v1.3.1 ✅ | Mescht | Portals: timer, variants, randomiser, editing mode, crying obsidian, portal-break practice | 1.16.1 | none | [Portal-Practice.v1.3.1.zip](https://github.com/Mescht/Portal-Practice/releases/download/v.1.3.1/Portal-Practice.v1.3.1.zip) (3.6 MB) | No licence – link only / ask author | 2025-01-18. ⚠️ The README mentions v1.4.1, but the newest *release* is v1.3.1. | ★2; 3,639 downloads. This is the portal section bundled in M1. | Zip → `saves/`. Import via `command_storage_portal_import.dat` + `/trigger import`. |
| M9 | **OW Practice** v2.0 ✅ | 7rowl. Credits: Semperzz (command blocks, rooms), Angelohasleft (crafting), GNG/Ravalle (testing). | Overworld: buried treasure × ruined portal, BT × lava pool, crafting, zero-cycle portal-room prep | 1.16.1 | none | [OW.Practice.v2.0.zip](https://github.com/7rowl/OWPractice/releases/download/v2.0/OW.Practice.v2.0.zip) (21.3 MB) | No licence – link only / ask author | 2022-07-25. ⚠️ The speedrun.com resource still links the older v1.0. | ★25; 115,313 downloads | Zip → `saves/` |
| M10 | **Crafting Practice v2** v2.1 ✅ | Semperzz. Inspired by Fyroah's original crafting map; contributor Bugbu6. | Crafting sequences and inventory speed | 1.16.1 | none | [Crafting.Practice.v2.zip](https://github.com/Semperzz/Crafting-Practice-v2/releases/download/v2.1/Crafting.Practice.v2.zip) (7.0 MB) | No licence – link only / ask author | 2023-01-04 | ★9; 96,976 downloads | Zip → `saves/` |
| M11 | **Zero Sorting Practice** v1.5 ✅ | Semperzz | Sorting the inventory before a zero cycle | 1.16.1 | Optional `playerattribute-1.0.jar` ships in the same release | [Zero.Sorting.zip](https://github.com/Semperzz/Zero-Sorting-Practice/releases/download/v1.5/Zero.Sorting.zip) (13.9 MB) | No licence – link only / ask author | 2023-01-20 | 49,558 downloads (in Map-Check) | Zip → `saves/` |
| M12 | **BT Practice (mapless buried treasure)** v1.3 ✅ | Mescht | Finding buried treasure without a map | 1.16.1 | none | [Map zip](https://github.com/Mescht/BTPractice/releases/download/v1.3/BTPractice-Map_v1.3.zip) / [datapack zip](https://github.com/Mescht/BTPractice/releases/download/v1.3/BTPractice-Datapack_v1.3.zip). [Demo](https://youtu.be/kI5G3Cwn0M4) | No licence – link only / ask author | 2023-05-19 | ★8; 72,381 downloads | Either the map zip → `saves/`, or the datapack → `saves/<world>/datapacks/` (cheats needed). The world grows large, so the README suggests deleting `region/`. |
| M13 | **End Portal Fill** v2 ✅ | cylorun | Filling end-portal frames quickly | 1.16.1 (per Map-Check) | none | [EndPortal.v2.zip](https://github.com/cylorun/End-Portal-Fill/releases/download/Minecraft/EndPortal.v2.zip) (5.3 MB) | No licence (no README) – link only / ask author | 2022-08-27 | 55,225 downloads | Zip → `saves/` |
| M14 | **Vine Seed SSG Practice** v1.7.0 ⚠️ | bdamja & r0hkx | Set-seed (vine seed) full OW + nether, portals | **1.16.5** (SSG, not 1.16.1 RSG) | none | [Vine.SSG.Practice.v1.7.0-1.16.5.zip](https://github.com/r0hkx/vine-ssg-practice/releases/download/v1.7.0/Vine.SSG.Practice.v1.7.0-1.16.5.zip) (32.5 MB) | No licence – link only / ask author | 2022-08-16 | ★10; 16,599 downloads. On the moderators' list. | Zip → `saves/` of a **1.16.5** instance |

### 1.2 Add-ons, secondary and niche maps

| # | Name | Author | Practises | MC / reqs | Source URL | Licence | Last update | Signals | Install | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | **LBP Extensions** v1.2.0 | awesome-03 | Datapack for LBP: save states, saturation selector, Ranked guaranteed-loot option, pickaxe reset modes | 1.16.1, needs M2 | [lbp_extensions.zip](https://github.com/awesome-03/lbp-extensions/releases/download/v1.2.0/lbp_extensions.zip) | No licence – link only / ask author | 2026-08-31 | ★4; 657 downloads (new) | Datapack → `saves/<LBP world>/datapacks/`, then `/reload` | ✅ |
| A2 | **Llama's (preemptive) Practice Map** | Llama | Preemptive stronghold navigation ("three spikes"; use 175 % entity distance) | 1.16.1 | [Mediafire page](https://www.mediafire.com/file/lzbecct99g8p22i/Lama%27s_Practice_Map.zip) (linked by meebie and MCSR Wiki stronghold videos) | No licence – link only / ask author | unknown | Recommended in [meebie's preemptive video](https://youtu.be/LtouvpxyDGY) and [MCSR Wiki stronghold](https://youtu.be/9dZWe7dNNMg) | Zip → `saves/` | ⚠️ The Mediafire landing page is live, but **can't be auto-downloaded** (interactive page). Show it as link-only, or ask Llama to publish a GitHub release. |
| A3 | **Crafting practice & more** | romu (romuuuuu) | Crafts, exploding chalice, spawner | 1.16.1 (assumed) | [_craftingworld.zip](https://github.com/romuuuuu/crafingworld/releases/download/g/_craftingworld.zip) (63 MB) | No licence – link only / ask author | 2026-03-30 | 56,610 downloads (via Map-Check). Placeholder repo description. | Zip → `saves/` | ✅ link. ⚠️ Low documentation. |
| A4 | **Random Seed Glitchless 1.16 Practice Datapack** | SethBling | In-chat buttons that teleport to random structures / set up splits | 1.16 | [speedrun_practice.zip](https://sethbling.s3-us-west-2.amazonaws.com/Downloads/DataPacks/speedrun_practice.zip) | No licence stated – link only | 2020 (src entry 2020-10-17) | Historic; on the speedrun.com resources list | Datapack → world `datapacks/` | ✅ link. ⚠️ Old; superseded by MPK and M1. |
| A5 | **Twig's Speedrun Practice Map** | Twwig | End, blazes, MLG water, etc. | 1.16 | Only the [video](https://youtu.be/wTzN97UnOs0) is listed on speedrun.com; no download link was found | unknown | 2020 | Historic | — | ❔ Unverified download |
| A6 | **Preemptive-Practice v1** | S1r0ke | Seed-specific stronghold navigation datapack covering all 128 strongholds; includes a generator | 1.16.1 | [S1r0ke/Preemptive-Practice](https://github.com/S1r0ke/Preemptive-Practice) (no releases; datapacks are in the repo) | No licence – link only / ask author | 2026-09-15 | 0 stars (new) | Datapack | ⚠️ New, unvetted. Watchlist. |
| A7 | **Killing Animals Practice** v1.0 | ryvelina | Killing and cooking food animals | 1.16.1 | [ryvelina/Killing-Animals-Practice-Map](https://github.com/ryvelina/Killing-Animals-Practice-Map) (zip committed in repo) | No licence | 2026-08-01 | 0 stars | Zip → `saves/` | ⚠️ New, unvetted. Watchlist. |
| A8 | **creeper_bs_practice** | ThatEpicCreeper | "Random BS" practice | 1.16.1 | [ThatEpicCreeper/mcsr_bs_practice](https://github.com/ThatEpicCreeper/mcsr_bs_practice/releases) | No licence | 2026-09-16 | 0 stars, 2 downloads | Zip | ⚠️ Unvetted. Don't list yet. |
| A9 | **HDWGH Practice v2** (AA category) | acewhite1010 (remake of fyroah's map) | How Did We Get Here (All Advancements) | 1.16.1 / 1.19 | [Releases](https://github.com/acewhite1010/hdwgh-practice-v2-1.16.1/releases) | No licence | 2024-07-18 | 2,216 downloads; on the speedrun.com list | Zip | ✅ Out of scope (AA), optional |
| A10 | **Ryguy2k4 Hotkey Practice** v2.1 | ryguy2k4 | Hotkey and inventory practice | 1.16 | [Discord CDN link](https://cdn.discordapp.com/attachments/405839885509984256/821896625634410546/HotkeyPractice_v2.1.zip) | — | 2021 | — | — | ❌ **Dead (HTTP 404)**. Discord CDN links expire. |
| A11 | Old speedrun.com Mediafire mirrors for Semperzz Blaze/Portal and the 1.17 SSG All-in-One map | — | — | — | e.g. [Blaze mirror](https://download2282.mediafire.com/40enns7ez7kg/2c6vo3qt9hv3qp5/Blaze+Practice.zip) | — | 2021–22 | — | — | ❌ **Stale.** They redirect to Mediafire's `download_repair.php` HTML. Use the GitHub releases (M6, M7). |

### 1.3 Practice mods, kits and tools

| # | Name | Author | What it practises | MC / reqs | Source URL | Licence | Last release | Signals | Install | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| T1 | **MiniPracticeKit (MPK)** v0.6 | Knawk | Vanilla command-block kit: presets for each split (bastion and fortress terrain, Eye Spy / stronghold entry, blind travel / nether exit, zero-cycle setups), custom inventories, structure locate. Also works in your **Ranked instance**. | 1.15–1.20.4, no mods | [hotbar.nbt](https://github.com/Knawk/mc-MiniPracticeKit/raw/master/hotbar.nbt). [Demo playlist](https://www.youtube.com/playlist?list=PL3xWUYc3vlEl4QRGBrZFiGGBLKh6V3KhS). [Setup video (Calisthenicuber, Oct 2026)](https://www.youtube.com/watch?v=cUDfyIwuZO4) | No licence – link only / ask author | v0.6 (last commit 2024-04-10; no GitHub releases) | ★84. In the draconix 2026 guide and on the speedrun.com list. | Copy `hotbar.nbt` into the instance's `.minecraft/`. ⚠️ This **overwrites** the user's saved hotbars, so the launcher must back up the existing file first. | ✅ |
| T2 | **peepoPractice** v2.2.1 | faluhub (Quesia) | Fabric mod with split practice (bastion, fortress, stronghold, end…), loadout editor, random tools, custom categories in JSON | 1.16.1 Fabric + SpeedRunIGT | [peepopractice-2.2.1.jar](https://github.com/faluhub/peepoPractice/releases/download/v2.2.1/peepopractice-2.2.1.jar) (33.6 MB) | No licence – link only / ask author | 2024-05-07 | ★55; 35,623 downloads | Mod → `mods/` of a separate practice instance. **Not** on the legal-mods list, so never install it into a speedrun.com or Ranked instance. Ko-fi: `quesia`. | ✅ |
| T3 | **StrongholdTrainer** v1.0 | mjtb49 (Matthew Bolan) and contributors | Stronghold navigation with timing and stats; ML model gives feedback on navigation | 1.16.1 Fabric, x86-64 | [stronghold-trainer-1.0.jar](https://github.com/mjtb49/StrongholdTrainer/releases/download/v1.0/stronghold-trainer-1.0.jar) (180 MB) | **MIT** (mirroring allowed with the licence notice) | 2021-09-04 | ★26; 11,960 downloads | Mod → separate practice instance | ✅ Old but functional for 1.16.1 |
| T4 | **ZeroPracticeSeedFilter** v1.0.0 | repeater64 | Background app that puts filtered seeds on the clipboard, so you get the zero-cycle types you want with MPK | Java 17; used alongside MPK | [ZeroPracticeSeedFilter-1.0.0.jar](https://github.com/repeater64/ZeroPracticeSeedFilter/releases/download/v1.0.0/ZeroPracticeSeedFilter-1.0.0.jar) | **GPL-3.0** | 2026-01-27 | ★3; 190 downloads | External tool | ✅ |
| T5 | MpkSeedFilter | repeater64 | General MPK seed filtering (successor to T4?) | — | [repeater64/MpkSeedFilter](https://github.com/repeater64/MpkSeedFilter) | No licence | WIP, no release (2026-03-29) | 0 stars | — | ❔ WIP |
| T6 | **MCSR Ranked built-in practice** | RedLime, OliverMCSR / MCSR Ranked team | **Private Rooms:** set seed, choice of category; Stone Pickaxe tier and up can pick seed type and **bastion type**; "Beginner Mode" rule. **Weekly Race → Practice mode**: any week's seed, cheats on. **Replays** of your own and opponents' runs. | Ranked mod (1.16.1) | [Private rooms](https://wiki.mcsrranked.com/gameplay/private_rooms) · [Weekly race](https://wiki.mcsrranked.com/gameplay/weekly_race) · [Replays](https://wiki.mcsrranked.com/gameplay/replay_system) · [Download](https://mcsrranked.com/download) | Proprietary. Link only; the launcher should install Ranked only through official channels. | Wiki updated 2026-10 | Very high | Install the Ranked instance (see [wiki install](https://wiki.mcsrranked.com/install/)) | ✅ Seed-type and bastion-type choice is **tier-gated** (paid [special tiers](https://store.mcsrranked.com/category/tiers)). |
| T7 | **Map-Check** v4.4.0 (prior art) | cylorun | Desktop tool that installs the practice maps above into selected instances (it resolves each repo's latest GitHub release at runtime) | Java | [Map-Check-4.4.0-all.jar](https://github.com/cylorun/Map-Check/releases/download/v4.4.0/Map-Check-4.4.0-all.jar) · [catalogue](https://github.com/cylorun/Map-Check/blob/HEAD/maps.json) | No licence | 2026-09-09 | ★40; 119,371 downloads. Recommended on the moderators' page. | — | ✅ **Closest prior art to our feature.** Coordinate with cylorun rather than compete. |
| T8 | **Ninjabrain Bot** 1.5.2 (measuring tool, not a practice tool) | Ninjabrain1 | Stronghold calculator. There is **no built-in practice mode**. To practise measuring: MPK "Eye Spy" preset + Ninjabrain Bot + Toolscreen EyeZoom / boat eye. | Java app | [Ninjabrain-Bot-1.5.2.jar](https://github.com/Ninjabrain1/Ninjabrain-Bot/releases/download/1.5.2/Ninjabrain-Bot-1.5.2.jar) · [Boat measurements wiki](https://github.com/Ninjabrain1/Ninjabrain-Bot/wiki/Boat-measurements) | **GPL-3.0** | 2026-03-24 | ★837; 545,660 downloads | External tool | ✅ No dedicated eye-measuring / triangulation practice map was found (searched GitHub, YouTube and the moderators' lists). |
| T9 | **Toolscreen** v1.5.0 (pre-release, 2026-10-10) | jojoe77777 | Not practice. Overlays, EyeZoom, instant resizing (replaces Jingle's resizing for many players). Windows only. | Win 10/11 | [jojoe77777/Toolscreen](https://github.com/jojoe77777/Toolscreen/releases/latest) | **MIT** | 2026-10-10 | ★196 | Installer into the instance folder | ✅ Listed for the tutorial section and setup flows |
| T10 | Search Crafting language optimiser | bezz | Picks the best game language for search crafting | Web | [searchcrafting.bezz.dev](https://searchcrafting.bezz.dev/) · [Search crafting doc](https://docs.google.com/document/d/1jSeciLoEgSwWWCdNk0dKignzxJskxJ5_zeCQmcdGmTg/edit) | — | live | Moderators' list | Link | ✅ |
| T11 | MCSRRankedPractice | mspi21 | Fabric mod that replaces random seeds with "God seed" generation | 1.16.1 | [mspi21/MCSRRankedPractice](https://github.com/mspi21/MCSRRankedPractice) | Custom/"other" LICENSE file | 2023-04-23 (no releases) | ★2 | Build from source | ❔ Stale. Don't list. |
| T12 | Auto-FSG / FSG filter | Kyu / hube12 | FSG seed generation (not practice) | 1.16.1 Fabric | [Kyu/auto-fsg](https://github.com/Kyu/auto-fsg) | MIT | 2021-04-13 | ★5 | — | ⚠️ Historic. FSG now uses other verified seed services. Out of scope for "practice". |

**Practice coverage, by split (for the launcher's "what do I practise?" filter):**

| Split | Best options |
|---|---|
| Overworld (RP, BT, shipwreck, village, temple, crafting) | M1 (OW section), M9, M12, M10, MPK |
| Portal building | M1, M7, M8 |
| Bastion (bridge, housing, stables, treasure) | M2 (+A1), M1, peepoPractice, MPK, Ranked private rooms (bastion type, tier-gated) |
| Fortress / blazes | M5, M6, M1, peepoPractice, MPK |
| Blind travel / nether exit | MPK presets; [blind coords image](https://imgur.com/gallery/i3fIanf) (Sharpieman20 & k4yfour) |
| Eye measuring / triangulation | MPK "Eye Spy" + Ninjabrain Bot (no dedicated map exists) |
| Stronghold navigation / preemptive | A2, StrongholdTrainer, A6 (unvetted), MPK |
| Zero cycle | M4, M1, M11, MPK + T4 |
| One cycle / end fight | M3, M1 |
| Whole runs on known seeds | Ranked Weekly Race practice mode, Ranked replays |

---

## 2. Tutorials

Accuracy key: **Current** means it matches the 2026 meta and tools (SeedQueue, Toolscreen, boat eye, Ranked). **Mostly** means the core content is still valid but the tooling or details have moved on. **Outdated** means the author flags it or it has been superseded; link it only as history.

### 2.1 Start here: setup and full beginner paths

| # | Title | Creator | Format | URL | Topic | Level | Date | Still accurate? |
|---|---|---|---|---|---|---|---|---|
| U1 | How to setup Minecraft (and Boateye) for Speedrunning – Full comprehensive guide [2026] | draconix | Video (25 min, chapters) | https://www.youtube.com/watch?v=l-q-_4R8_6M | Prism, mods, Java, StandardSettings, SeedQueue, practice tools, pixel-perfect eye measuring, Toolscreen, srcom submission, **OBS**, separate srcom vs Ranked instances | Beginner | 2026-04-01 (208k views) | **Current.** It is the de facto standard: osh's and Montage's videos and the Toolscreen README all point here. |
| U2 | The Ultimate Guide to Minecraft Speedrunning (video) + written guide on Metafy | Couriway (written guide contributions acknowledged by draconix) | Video + long written guide (7 chapters: setup, resetting and overworlds, nether 1 and 2, post-blind, end) | Video: https://www.youtube.com/watch?v=qo475jHhxGg · Written: https://metafy.gg/guides/view/ultimate-minecraft-speedrun-guide-cIzfjeTmwOm | Whole run | Beginner → Intermediate | Video 2025-10-22; written guide updated 2026-04-02 | **Current.** Free, but the written guide needs a Metafy sign-in. rekrap1's popular 2023 series now points here. |
| U3 | Minecraft 2025 Speedrunning Guide for RANKED/RSG (4 parts) | Mikeyboie | Video series | [P1 Setup + Overworlds](https://www.youtube.com/watch?v=egyiA_8FztM) · [P2 Bastions](https://www.youtube.com/watch?v=CRwiJcWWUlY) · [P3 Fortress + Calculation](https://www.youtube.com/watch?v=JsFcAeBXVpk) · [P4 Stronghold + End](https://www.youtube.com/watch?v=4It26dOki7g) | Whole run, Ranked seed types | Beginner | 2025-06-28 → 2025-12-22 | **Mostly.** Content is valid; for setup, prefer U1 or U4 (Toolscreen era). |
| U4 | MCSR Setup Guide: Ninjabrainbot, Boat Eye, Toolscreen, Standard Settings, Practice Maps | HiTrippy | Video (35 min) | https://www.youtube.com/watch?v=Uvy0LzZuCxU | **Ranked** setup end-to-end, pie directory, practice-map install | Beginner | 2026-05-25 | **Current** |
| U5 | How to set up Minecraft for MCSR Ranked – A full Guide | Stuff2Simon | Video | https://www.youtube.com/watch?v=3W2UL4CkAPE | Ranked setup | Beginner | 2026-03-28 | Current (smaller channel; alternative to U4) |
| U6 | MCSR Ranked Wiki – Install / Gameplay | MCSR Ranked team + contributors | Docs site (VitePress, open source) | https://wiki.mcsrranked.com/ (install: https://wiki.mcsrranked.com/install/) | Install (MCSR Launcher, Prism, Modrinth, vanilla), rules, RNG standardisation, seed types, private rooms | Beginner | Repo active Oct 2026 | **Current** for Ranked mechanics. ⚠️ Its *tutorial* pages (bastion, fortress, stronghold, end…) are **empty stubs** (0-byte files in [the repo](https://github.com/MCSR-Ranked/Wiki/tree/HEAD/docs/gameplay/tutorial)), so don't link those. |
| U7 | minecraftspeedrunning.com – Tools and Resources | speedrun.com MC moderators | Curated link hub | https://www.minecraftspeedrunning.com/public-resources/tools-and-resources | Official tool list (launcher, Java, ModCheck, MapCheck, Jingle, Julti, Ninjabrain, OBS, PaceMan, Ranked) | All | Footer © 2025; src entry updated 2025-11-25 | **Current** as a hub, but some linked videos there are flagged outdated (U18, U19). |
| U8 | (outdated) Minecraft Speedrun Setup Guide 2025 | osh | Video | https://www.youtube.com/watch?v=OEpZlv6cQsI | Setup | Beginner | 2024-09-14 (468k views) | **Outdated.** Author points to U1. |
| U9 | 2023 How to Setup Minecraft Speedrunning Guide | k4yfour | Video | https://www.youtube.com/watch?v=VL8Syekw4Q0 | Setup (MultiMC era) | Beginner | 2022-12-04 (558k views) | **Outdated** for tooling (pre-SeedQueue). The moderators' page still lists it for *other versions/categories*. |
| U10 | How to Submit Minecraft: Java Edition Speedruns | Ismexion | Video | https://youtu.be/z2n2fCV1GY0 | speedrun.com submission | Beginner | 2025-01-29 | Current (linked from U1) |
| U11 | Feinberg Teaches Chat How to Speedrun Minecraft | feinberg 2 funny (clips channel) | Video (stream VOD) | https://www.youtube.com/watch?v=8OXosuCKf00 | Whole run, informal | Beginner | 2026-02-10 (138k views) | Entertainment/teaching mix. Optional. |

### 2.2 Routing and splits

| # | Title | Creator | Format | URL | Topic | Level | Date | Accurate? |
|---|---|---|---|---|---|---|---|---|
| R1 | How to SPEEDRUN BRIDGE – 2026 | MCSR Wiki (Dibedy), "reviewed by top runners" | Video | https://www.youtube.com/watch?v=FoNy438g1GM | Bridge bastion routes | Beginner→Int. | 2026-02-27 | **Current** (replaces the 2024 bridge video, which is marked OUTDATED) |
| R2 | How to SPEEDRUN HOUSING – 2026 | MCSR Wiki | Video | https://www.youtube.com/watch?v=y7fG2L4FZLU | Housing routes | Beginner→Int. | 2026-03-09 | **Current** |
| R3 | How to SPEEDRUN Bastions – STABLES | MCSR Wiki | Video | https://www.youtube.com/watch?v=fjkkLdWYRmY | Stables routes | Int. | 2024-05-20 | Mostly (pair with R9) |
| R4 | How to SPEEDRUN Bastions – TREASURE | MCSR Wiki | Video | https://www.youtube.com/watch?v=u4-KxRhNsUc | Treasure routes | Int. | 2024-05-20 | Mostly |
| R5 | How to SPEEDRUN – The FORTRESS | MCSR Wiki | Video | https://www.youtube.com/watch?v=9LpyDBPC3u4 | Fortress navigation and exit | Int. | 2024-07-26 | Mostly |
| R6 | How to Complete RUINED PORTALS | MCSR Wiki | Video | https://www.youtube.com/watch?v=FkLYnUw39zU | Every RP completion | Beginner→Int. | 2024-11-14 | Current |
| R7 | How to SPEEDRUN – The STRONGHOLD | MCSR Wiki | Video | https://www.youtube.com/watch?v=9dZWe7dNNMg | Stronghold navigation, preemptive | Int. | 2024-10-30 | Current |
| R8 | How to ZERO CYCLE – ft. Doogile | MCSR Wiki | Video | https://www.youtube.com/watch?v=CSdkCmZ69RI | Zero cycle | Int.→Adv. | 2026-02-15 (167k views) | **Current** |
| R9 | MCSR Ranked Explanations: [Bridge (7rowl)](https://www.youtube.com/watch?v=SDBdWHKLvNg) · [Housing (Big Big Mongey)](https://www.youtube.com/watch?v=OxZkC67rops) · [Stables (Silverr)](https://www.youtube.com/watch?v=jc8156m-URU) · [Treasure (Beefsalad)](https://www.youtube.com/watch?v=G6bbyQsxDEI) · [Fortress (Hax)](https://www.youtube.com/watch?v=Ts2cRIz-MOc) · [Stronghold (Doogile)](https://www.youtube.com/watch?v=iI8bntMXJzE) | MCSR Ranked (official channel) featuring top runners | Video series | see links | Bastions, fortress, stronghold | Int. | 2025-05-13 | **Current** |
| R10 | Bastion guides: [Stables – all useful routes](https://www.youtube.com/watch?v=D8Mt1bNNWtg) · [Housing – all useful routes](https://www.youtube.com/watch?v=O4-U4Lwzc-Y) · ["You Only Need 4 Routes"](https://www.youtube.com/watch?v=NKWnd0LJbbY) | HiTrippy | Video | see links | Bastion routes; one route per type for beginners | Beginner→Int. | 2026-04 → 2026-06 | **Current** |
| R11 | Stables Triple to Gap Updated Guide (all variants) | meebie | Video | https://www.youtube.com/watch?v=D4Ulw7VepJc | Stables TTG | Adv. | 2026-01-24 | Current |
| R12 | ADVANCED [Bridge](https://www.youtube.com/watch?v=jIf98VTLOs0) / [Treasure](https://www.youtube.com/watch?v=rpxJ6EVRzbo) guides; [Complete Fortress Guide](https://www.youtube.com/watch?v=71ABl45UyAE); [Movement Guide pt 1](https://www.youtube.com/watch?v=BvNihmsvKWQ) | Calisthenicuber | Video | see links | Ranked-oriented advanced routing, movement | Adv. | 2026-03 → 2026-07 | Current |
| R13 | ALL Optimal Spawner Setups | JackoWacko62 | Video | https://www.youtube.com/watch?v=n11v-59LJnA | Blaze spawner setups | Int. | 2026-02-20 | Current |
| R14 | Preemptive Stronghold Navigation in 3 minutes | meebie | Video | https://www.youtube.com/watch?v=LtouvpxyDGY | Preemptive nav (beginner) | Int. | 2026-01-26 | Current |
| R15 | Pre-Emptive Guide (Piedar) | HiTrippy | Video | https://www.youtube.com/watch?v=gHHRAY3d-20 | Preemptive with the pie chart | Int. | 2026-07-05 | Current |
| R16 | A Deeper Look Into Preemptive Navigation · Preemptive Nav: Setup | Mimi. Earlier research credited to WillBe, Ninjabrain and Bolan. | Google Docs | [Doc 1](https://docs.google.com/document/d/1Xnmki5jOwuiwVnyv1b3VJLpiDWfNixgKW3zQowmpsYo/edit) · [Doc 2](https://docs.google.com/document/d/1NEJ_BaQOqyDlt-h2GiUg4zXlqBHv8YfMVdGpQhDLD8U/edit) | Stronghold generation and preemptive | Adv. | ❔ undated | Current (cited by R7) ✅ live |
| R17 | How to One Cycle the Ender Dragon in 100 Seconds | k4yfour | Video | https://www.youtube.com/watch?v=u9UVwwWxN_k | One cycle (all versions incl. 26.1+) | Beginner | 2025-12-26 | **Current** |
| R18 | How to One Cycle | Marcus Fireworks ("Fireworkss") | Video | https://youtu.be/JaVyuTyDxxs | One cycle | Int. | 2023-05-26 (235k views) | Current (on the moderators' list) |
| R19 | Never Fail a 1 Cycle Again | HiTrippy | Video | https://www.youtube.com/watch?v=9HpFTWRUtxw | One-cycle consistency | Int. | 2026-05-30 | Current |
| R20 | Speedrunners Guide to End Fights Part 1: Mechanics · A Complete Speedrunners Guide to the Fast Zero Cycle | T_Wagz | Video | [End fights](https://youtu.be/6V2sQthCzOc) · [Zero cycle](https://www.youtube.com/watch?v=iClDGWL0e5s) | Dragon mechanics, zero cycle | Adv. | 2021-09-09 / 2021-06-14 | Mostly. The mechanics haven't changed; for zero technique, prefer R8. |
| R21 | How to find buried treasure without a map (Dec 2021) | MoleyG | Video | https://youtu.be/ho1rwmooHRg | Mapless BT | Int. | 2021-12-07 | Mostly (still on the moderators' list; practise with M12) |
| R22 | k4yfour classics: [Bastion Treasure](https://www.youtube.com/watch?v=np6fc_z9LUY) / [Stables](https://www.youtube.com/watch?v=WIN-ZtUJfIc) / [Bridge](https://www.youtube.com/watch?v=Dts81nEnzuQ) / [Housing](https://www.youtube.com/watch?v=S0G5asEjrgk); [Fortresses & Blazes](https://www.youtube.com/watch?v=pmx9LyUvLTk); [Stronghold portal room](https://youtu.be/kQCt6JgWr6E); [Bastion X-ray/microlensing](https://youtu.be/jvTfMLPnMSw); [Find fortresses/bastions & blind travel](https://youtu.be/Ou58P7e-ZY0) | k4yfour | Video | see links | Fundamentals | Beginner | 2020-11 → 2021-12 | **Outdated / historic.** Routes have been superseded by R1–R12, and the blind-travel video is self-flagged "OUTDATED". The fundamentals are still useful. |
| R23 | Speedrunners Guide to Nether Travel | T_Wagz | Video | https://www.youtube.com/watch?v=12efBB7SW9c | Blind / educated travel theory | Int. | 2020-11-22 | Mostly (theory) |
| R24 | The BEST Minecraft Speedrun Bastion Routes | Couriway | Video | https://www.youtube.com/watch?v=5QIMHxpfUkg | Bastion routes (2021) | — | 2021-05-01 | **Outdated** (historic) |
| R25 | How to Speedrun Minecraft (overworld / nether / the end) | rekrap1 | Video ×3 | [OW](https://www.youtube.com/watch?v=FGfuoLTma14) · [Nether](https://www.youtube.com/watch?v=tuoJpIQu8l8) · [End](https://www.youtube.com/watch?v=Gp7Qsab8JNY) | Whole run, very approachable | Beginner | 2023-05/06 (1.1–1.4M views each) | **Superseded.** Each description says "UPDATED GUIDE" → U2. |
| R26 | "How Do I Improve in MCSR?" | JackoWacko62 | Video | https://www.youtube.com/watch?v=mYULe_lELTk | Practice habits, mindset | All | 2026-03-06 | Current |

### 2.3 Eye measuring, Ninjabrain Bot and blind/stronghold tools

| # | Title | Creator | Format | URL | Topic | Level | Date | Accurate? |
|---|---|---|---|---|---|---|---|---|
| E1 | How to Find the Stronghold with Ninjabrain Bot | k4yfour | Video | https://www.youtube.com/watch?v=XmikooWmIAM | Ninjabrain basics (1.14 → 26.1+) | Beginner | 2026-01-06 (151k) | **Current** |
| E2 | (2025) Locating the stronghold with Ninjabrain Bot | olock | Video | https://www.youtube.com/watch?v=Gp6EnDs24NI | Ninjabrain usage (on the moderators' list) | Beginner | 2025-01-06 (170k) | Mostly. The description now redirects boat-eye setup to U1 (Toolscreen replaces Jingle for this). |
| E3 | Ninjabrain Bot (Boat Eye Tutorial) | HiTrippy | Video | https://www.youtube.com/watch?v=SxVJD7rW-x8 | Boat eye, eye wiggle | Int. | 2026-07-20 | **Current** |
| E4 | Boat measurements (official wiki) | Ninjabrain1 | Docs | https://github.com/Ninjabrain1/Ninjabrain-Bot/wiki/Boat-measurements | Boat eye settings, sensitivity values | Int. | live | **Current** (the old URL `.../Boat-measurements-with-Ninjabrain-Bot` redirects) |
| E5 | Ninjabrain Bot README | Ninjabrain1 | Docs | https://github.com/Ninjabrain1/Ninjabrain-Bot | Install, hotkeys, F3+C flow, theory | Beginner | 1.5.2 (2026-03) | Current |
| E6 | Boat Eye Setup Guide (OUTDATED, READ DESC) | Montage | Video | https://www.youtube.com/watch?v=l1Z2t9e6Qko | Old DPI / cursor-speed method | — | 2025-12-25 | **Outdated.** The author points to U1 (Toolscreen at 11:53). |
| E7 | How to set up Toolscreen · Advanced Tutorial for Toolscreen | jojoe (Toolscreen author) | Video | [Setup](https://www.youtube.com/watch?v=YqS-fxPx_jo) · [Advanced](https://www.youtube.com/watch?v=LG13ljK9RPs) | EyeZoom, overlays, Ninjabrain overlay, rebinding | Beginner / Int. | 2026-02-14 / 2026-03-30 | **Current** (Windows only) |
| E8 | Toolscreen sensitivity calculator | qMaxXen | Interactive web tool | https://qmaxxen.github.io/Toolscreen-sens-calc/ | Matching mouse feel in EyeZoom | Int. | live | Current |
| E9 | What to do about eye wiggle | frontcage forum (Discourse) | Forum guide | https://frontcage.com/t/what-to-do-about-eye-wiggle/14 | Eye wiggle | Int. | ❔ | Current (cited by U1) |
| E10 | Stronghold ring blind coords | Sharpieman20 & k4yfour | Image | https://imgur.com/gallery/i3fIanf | Blind coordinates per ring | Beginner | old | Current (geometry hasn't changed) |
| E11 | Triangulation distance estimate · Axis Calculated · Perfect Travel offsets · Divine table | community (moderators' Documents page) | Images / Sheets | [Docs page](https://www.minecraftspeedrunning.com/public-resources/documents) · [Perfect Travel sheet](https://docs.google.com/spreadsheets/d/1sVIvSLt0TXVE_MteRpL8r4FiOF_UJ6Lm/edit) · [Axis Calculated](https://docs.google.com/spreadsheets/d/192yx2gfr7Ws4iFFgQoRMviSsHFeU9DnaLSzBFo3U59Q/edit) | Advanced stronghold techniques | Adv. | old | Valid reference material ✅ live |
| E12 | How to Speedrun Minecraft: Ninjabrain Bot & Jingle Setup | Spaceman | Video | https://www.youtube.com/watch?v=_Ro-6bSRghQ | Ninjabrain + Jingle | Beginner | 2026-01-16 | Mostly. Jingle still works, but most 2026 guides moved resizing to Toolscreen. Useful on Linux and macOS, where Toolscreen is unavailable. |

### 2.4 Reset tools, OBS, settings, keybinds, platform

| # | Title | Creator | Format | URL | Topic | Level | Date | Accurate? |
|---|---|---|---|---|---|---|---|---|
| S1 | SeedQueue (intro video) + [SeedQueue wiki](https://github.com/contariaa/seedqueue/wiki) (Config, Customization, Troubleshooting) | contaria (KingContaria) | Video + wiki | https://www.youtube.com/watch?v=fGu2MYZxh_c | Background world generation, wall | Int. | Video 2024-08-09; wiki 2024-08-10; mod v1.6 (2025-11) | Current (the mod is MIT; the moderators say 1.16.1 runners should use SeedQueue instead of multi-instance Julti) |
| S2 | SeedQueue settings website | maskersss | Interactive tool | http://maskers.xyz/sq-settings | Recommended SeedQueue config per PC | Int. | live | Current (cited by U1) |
| S3 | MCSR Java settings guide (GraalVM, ZGC, args) | maskersss | Gist (Markdown) | https://gist.github.com/maskersss/5847d594fc6ce4feb66fbd2d3fda281d | Java / JVM args | Int. | updated 2026-08-10 | **Current** |
| S4 | Jingle README | DuncanRuns | Docs | https://github.com/DuncanRuns/Jingle | Single-instance utility: borderless, submission packaging, plugins, OBS link | Beginner | v2.0.0+pre6 (2026-09-19) | Current |
| S5 | Julti + "How to setup Multi-Instance resetting" | DuncanRuns / draconix | Docs + video | [Julti](https://github.com/DuncanRuns/Julti) · [video](https://www.youtube.com/watch?v=_8gQkgZcTKo) | Multi-instance wall | Int. | Julti 1.6.1 (2025-01-01); video 2024-05-27 | **Outdated for 1.16.1** (the video is self-flagged and the moderators recommend SeedQueue). Still relevant for other versions. |
| S6 | OBS setup | draconix (U1 chapter 23:05) · [OBS Studio](https://obsproject.com/) | Video chapter | https://www.youtube.com/watch?v=l-q-_4R8_6M&t=1385s | Recording for srcom | Beginner | 2026-04-01 | Current. No good *standalone* 2026 MCSR OBS guide was found; the projector guide (TobiasIsGood) is self-flagged OUTDATED. |
| S7 | The Best Minecraft Speedrunning Settings + My keybinds | Stuff2Simon | Video | https://www.youtube.com/watch?v=mswDTtBQ_2U | Options, keybinds | Beginner | 2026-03-29 | Current |
| S8 | Best Speedrunning Keybinds | HiTrippy | Video | https://www.youtube.com/watch?v=UsdEzcsCafo | Keybinds and options for Ranked | Beginner | 2026-06-10 | Current |
| S9 | speedrun.com Rules (rebinding, macros, legal mods) + [legal mods list](https://mods.tildejustin.dev/) | speedrun.com MC moderators | Docs | https://www.minecraftspeedrunning.com/public-resources/rules | What is legal | All | live | Current. **The launcher must not ship illegal mods into run instances.** |
| S10 | linux-mcsr | its-saanvi + contributors | Docs repo | https://github.com/its-saanvi/linux-mcsr | MCSR on Linux (no Toolscreen there) | Int. | 2026-09 | Current (GPL-3.0) |
| S11 | launcher guide (other legal launchers) | community (linked by draconix) | Google Doc | https://docs.google.com/document/d/1oAXkyuY__SaTbPMlYWpV2i93D57AAKljx6WjCsJTU0w/edit | Launcher alternatives | Int. | ❔ | ✅ live. Relevant for our own launcher's legality story. |
| S12 | MCSR Launcher (prior art) | MCSRLauncher team | App + site | https://mcsrlauncher.github.io/ · [repo](https://github.com/MCSRLauncher/Launcher) | Speedrun-focused launcher (Kotlin, GPL-3.0); the Ranked wiki has an install page for it | — | 0.8.10-beta (2026-10-04) | Competing/adjacent project. Worth studying. |

**Gaps noted:**
- No dedicated *eye-measuring practice map* exists.
- The Ranked wiki tutorial section is empty.
- No standalone 2026 OBS guide for MCSR was found.
- No live text site called "MCSR Wiki" was found. "MCSR Wiki" is Dibedy's YouTube channel: https://www.youtube.com/@MinecraftSpeedrunningWiki. Its 2024 videos say the written wiki is "in the making".

---

## 3. Licensing and redistribution assessment

| Category | Items | What the launcher may do |
|---|---|---|
| **No licence** (all rights reserved by default) | All practice **maps** (M1–M14, A1–A8), MPK, peepoPractice, Map-Check | **Link only and auto-download from the author's original GitHub release URL.** This is what Map-Check already does with community acceptance, and it keeps the authors' download counters accurate. **Do not mirror or re-host** without written permission. Display the credits, the repo link and the support link. |
| **Compilations** | M1 (includes Mescht's and Llama's work, credited in its README) | Same as above. Any mirroring permission would need Dibedy *and* the bundled authors. |
| **MIT** | StrongholdTrainer, Toolscreen, Jingle, Julti, SeedQueue | Mirroring is allowed if the copyright and licence notice is kept. Downloading from the source is still preferred. |
| **GPL-3.0** | Ninjabrain Bot, ZeroPracticeSeedFilter, MCSR Launcher, linux-mcsr | Redistribution is allowed with the licence and a source offer. Linking or downloading from upstream is simplest. |
| **Proprietary / service** | MCSR Ranked mod and paid tiers, YouTube videos, Metafy guide, Google Docs | Link only. Embed YouTube only through the official player. |
| **Unreachable without a browser** | Mediafire (A2), Discord CDN (A10 is dead) | Link to the landing page only, or ask the author to publish a GitHub release. |

Other concerns:
- **Minecraft Usage Guidelines.** Maps are derivative works of Minecraft and must remain free and non-commercial. The launcher must not paywall map downloads.
- **Mutable tags.** M1 publishes under a moving tag named `latest`, and LBP's previous release was 3 years old. Always resolve the release through the GitHub API, then **pin by SHA-256** (GitHub now exposes an asset `digest`; it was `null` for some older assets, e.g. M3 and M7, so compute and store our own hash).
- **Updates must not destroy user data.** Several maps keep loadouts and PBs in `data/command_storage_*.dat` (M4, M5, M8). The launcher should install a new version as a *new* folder, and offer to copy known loadout files, as the authors document.
- **MPK overwrites `hotbar.nbt`.** Back up the existing file and offer a restore.
- **Practice mods (peepoPractice, StrongholdTrainer) are not legal for runs.** Install them only into a dedicated "Practice" instance, never into an RSG or Ranked instance.

---

## 4. Proposed catalogue schema and hosting

### 4.1 Hosting and update model

- **Repository and file layout.** Use a public GitHub repo, e.g. `speedrunluncher/catalog`:
  - `entries/maps/*.json`, `entries/tools/*.json` and `entries/tutorials/*.json`, one file per entry, so PRs stay small.
  - `schema/catalog-entry.schema.json` (JSON Schema 2020-12).
  - A built artefact, `dist/catalog.v1.json`, published to GitHub Pages and also attached to a GitHub Release.
- **CI on every PR:**
  1. Schema validation.
  2. Link-health checks: HTTP 200/206 for every URL, plus a YouTube oEmbed check.
  3. For `github-release` sources: resolve the asset, download it, verify the declared `sha256` and `size`, check that the zip has exactly one top-level folder containing `level.dat`, and read `Version.Name` from `level.dat`.
- **Nightly scheduled Action.** It polls each source repo for new releases and opens an *automated PR* with the new version block (url, sha256, size, date). A maintainer reviews before merge, so an upstream re-upload can't silently change what users get.
- **Signing.** Sign `catalog.v1.json` (minisign or sigstore). The launcher embeds the public key and rejects unsigned or invalid catalogues.
- **Launcher behaviour:**
  - Fetch with `If-None-Match` (ETag) on start, at most once every 6 h.
  - Fall back to a snapshot bundled in each launcher build when offline.
  - Always download from `url` and verify `sha256` before extracting.
  - Never extract over an existing folder.
- **Community PRs.** Use a PR template with a checklist: credits, licence status, permission evidence, practice tags, tested on 1.16.1. Add CODEOWNERS and label new entries `status: unvetted` until a maintainer plays them.
- **Prior art to align with:**
  - [mcsr-meta mods.json](https://github.com/tildejustin/mcsr-meta) (fields `url`, `sha1`, `sha512`, `size`, `target_version`).
  - Map-Check's `maps.json` (`repository`, `assetPattern`).

  Consider proposing a shared "mcsr practice maps" meta file with cylorun and tildejustin rather than forking yet another list.

### 4.2 Being polite to creators

1. **Before listing:** open a GitHub issue or send a Discord DM to each author. Explain the one-click install, show exactly how they will be credited, and ask for:
   - (a) permission to auto-download from their release URL, which is the default request;
   - (b) optionally, permission to mirror as a fallback;
   - (c) their preferred credit name and support links.

   Record the answer in `licence.permission` (date plus a link to the issue or message).
2. **Credit prominently.** Put author names and roles in the entry card, link the repo and the author's YouTube or Twitch, and add a "Support the creator" button (Ko-fi links found: Dibedy `ko-fi.com/dibedy`, Mescht `ko-fi.com/mescht`, faluhub `ko-fi.com/quesia`; jojoe has a Patreon).
3. **Download from the original host,** so the author's GitHub download counts and analytics stay meaningful. Don't strip READMEs from the zip.
4. **Honour opt-outs within 48 h.** Keep an `optOut` flag, and if an author says no, show a "visit page" link only.
5. **Send feedback upstream.** Put "Report a bug" links on the author's GitHub Issues, not ours.

### 4.3 JSON Schema (abridged)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://speedrunluncher.github.io/catalog/schema/catalog-entry.v1.json",
  "title": "Speedrunluncher catalogue entry",
  "type": "object",
  "required": ["id", "kind", "name", "summary", "credits", "links", "licence", "tags", "status"],
  "properties": {
    "id":       { "type": "string", "pattern": "^[a-z0-9-]+$" },
    "kind":     { "enum": ["map", "datapack", "mod", "hotbar", "tool", "tutorial"] },
    "name":     { "type": "string" },
    "summary":  { "type": "string", "maxLength": 280 },
    "credits":  { "type": "array", "minItems": 1, "items": {
      "type": "object", "required": ["name", "role"],
      "properties": {
        "name": { "type": "string" },
        "role": { "enum": ["author", "co-author", "contributor", "bundled-section", "inspiration", "tester", "presenter", "editor"] },
        "note": { "type": "string" },
        "links": { "type": "object", "additionalProperties": { "type": "string", "format": "uri" } }
      } } },
    "links": { "type": "object", "required": ["homepage"], "properties": {
      "homepage": { "type": "string", "format": "uri" },
      "issues":   { "type": "string", "format": "uri" },
      "video":    { "type": "string", "format": "uri" },
      "support":  { "type": "array", "items": { "type": "object", "properties": { "label": { "type": "string" }, "url": { "type": "string", "format": "uri" } } } }
    } },
    "licence": { "type": "object", "required": ["spdx", "redistribution"], "properties": {
      "spdx": { "type": ["string", "null"], "description": "SPDX id, or null when none stated" },
      "redistribution": { "enum": ["open-licence", "permission-granted", "link-only", "proprietary"] },
      "autoDownloadFromSource": { "type": "boolean" },
      "mirrorAllowed": { "type": "boolean" },
      "permission": { "type": ["object", "null"], "properties": {
        "grantedBy": { "type": "string" }, "date": { "type": "string", "format": "date" }, "evidence": { "type": "string", "format": "uri" } } },
      "optOut": { "type": "boolean", "default": false },
      "notes": { "type": "string" }
    } },
    "source": { "type": "object", "description": "maps/tools only", "properties": {
      "type": { "enum": ["github-release", "github-raw", "direct", "external-page"] },
      "repo": { "type": "string" },
      "assetPattern": { "type": "string" },
      "pageUrl": { "type": "string", "format": "uri" }
    } },
    "versions": { "type": "array", "description": "newest first; maps/tools only", "items": {
      "type": "object", "required": ["version", "releasedAt", "url", "sha256", "size", "minecraft", "install"],
      "properties": {
        "version":    { "type": "string" },
        "releasedAt": { "type": "string", "format": "date" },
        "url":        { "type": "string", "format": "uri" },
        "sha256":     { "type": "string", "pattern": "^[a-f0-9]{64}$" },
        "size":       { "type": "integer" },
        "minecraft":  { "type": "array", "items": { "type": "string" } },
        "requires":   { "type": "array", "items": { "type": "object", "properties": {
          "id": { "type": "string" }, "kind": { "enum": ["mod", "loader", "catalog-entry", "java"] }, "version": { "type": "string" } } } },
        "install": { "type": "object", "required": ["method"], "properties": {
          "method": { "enum": ["extract-world", "add-datapack", "add-mod", "replace-hotbar", "run-external", "open-link"] },
          "target": { "enum": ["saves", "world-datapacks", "mods", "dotminecraft", "none"] },
          "rootFolder": { "type": "string", "description": "expected single top-level folder in zip" },
          "instanceKind": { "enum": ["any-1.16.1", "practice-only", "ranked", "rsg"] },
          "preserveOnUpdate": { "type": "array", "items": { "type": "string" }, "description": "relative paths to copy from the previous version (loadouts, PBs)" },
          "backupBeforeInstall": { "type": "array", "items": { "type": "string" } }
        } }
      } } },
    "tutorial": { "type": "object", "description": "tutorials only", "properties": {
      "format": { "enum": ["video", "video-series", "written", "google-doc", "docs-site", "interactive", "image"] },
      "url": { "type": "string", "format": "uri" },
      "parts": { "type": "array", "items": { "type": "object", "properties": { "title": { "type": "string" }, "url": { "type": "string", "format": "uri" } } } },
      "publishedAt": { "type": "string", "format": "date" },
      "durationMin": { "type": "number" },
      "language": { "type": "string", "default": "en" },
      "requiresAccount": { "type": "boolean" }
    } },
    "tags": { "type": "object", "properties": {
      "categories": { "type": "array", "items": { "enum": ["rsg-1.16.1", "fsg", "ranked", "ssg", "aa"] } },
      "splits": { "type": "array", "items": { "enum": ["setup", "overworld", "portal", "bastion", "fortress", "blind-travel", "eye-measuring", "stronghold", "end", "zero-cycle", "one-cycle", "crafting", "movement", "resetting", "obs", "settings"] } },
      "difficulty": { "enum": ["beginner", "intermediate", "advanced"] },
      "keywords": { "type": "array", "items": { "type": "string" } }
    } },
    "popularity": { "type": "object", "properties": {
      "githubStars": { "type": "integer" }, "downloads": { "type": "integer" }, "views": { "type": "integer" },
      "recommendedBy": { "type": "array", "items": { "type": "string", "format": "uri" } },
      "asOf": { "type": "string", "format": "date" } } },
    "status": { "type": "object", "required": ["review", "lastVerified"], "properties": {
      "review": { "enum": ["curated", "unvetted", "deprecated", "dead"] },
      "accuracy": { "enum": ["current", "mostly", "outdated", "n/a"] },
      "accuracyNote": { "type": "string" },
      "supersededBy": { "type": "string" },
      "lastVerified": { "type": "string", "format": "date" },
      "linkHealth": { "enum": ["ok", "redirect", "interactive-only", "dead"] }
    } }
  }
}
```

### 4.4 Example entries (real data, verified 2026-10-23)

```json
[
  {
    "id": "llamas-bastion-practice",
    "kind": "map",
    "name": "Llama's Bastion Practice",
    "summary": "Full-control bastion practice for all four bastion types with configurable loadouts.",
    "credits": [
      { "name": "LlamaPag (Llama)", "role": "author",
        "links": { "github": "https://github.com/LlamaPag", "youtube": "https://www.youtube.com/@llama9600" } }
    ],
    "links": {
      "homepage": "https://github.com/LlamaPag/bastion",
      "issues": "https://github.com/LlamaPag/bastion/issues",
      "video": "https://youtu.be/jlA-jW7VGqw",
      "support": []
    },
    "licence": {
      "spdx": null,
      "redistribution": "link-only",
      "autoDownloadFromSource": true,
      "mirrorAllowed": false,
      "permission": null,
      "optOut": false,
      "notes": "No licence in repo. Ask author before mirroring; auto-download from GitHub release only."
    },
    "source": { "type": "github-release", "repo": "LlamaPag/bastion", "assetPattern": "^LBP[._ ].*\\.zip$" },
    "versions": [
      {
        "version": "3.15.0",
        "releasedAt": "2026-06-18",
        "url": "https://github.com/LlamaPag/bastion/releases/download/3.15.0/LBP.3.15.0.zip",
        "sha256": "aa6ca8dfcd22d4ebf28771722fc51e9cdc3022f684aa6aa7d37761fc2350c925",
        "size": 26327558,
        "minecraft": ["1.16.1"],
        "requires": [],
        "install": { "method": "extract-world", "target": "saves", "rootFolder": "LBP 3.15.0", "instanceKind": "any-1.16.1", "preserveOnUpdate": [] }
      }
    ],
    "tags": { "categories": ["rsg-1.16.1", "ranked"], "splits": ["bastion"], "difficulty": "beginner", "keywords": ["bridge", "housing", "stables", "treasure"] },
    "popularity": { "githubStars": 80, "downloads": 466710, "recommendedBy": ["https://www.minecraftspeedrunning.com/public-resources/maps"], "asOf": "2026-10-23" },
    "status": { "review": "curated", "accuracy": "n/a", "lastVerified": "2026-10-23", "linkHealth": "ok" }
  },
  {
    "id": "mescht-zero-practice",
    "kind": "map",
    "name": "Zero Practice",
    "summary": "Zero-cycle practice with tower type/location, dragon rotation, damage display and custom loadouts.",
    "credits": [
      { "name": "Mescht", "role": "author",
        "links": { "github": "https://github.com/Mescht", "youtube": "https://www.youtube.com/@meescht" } }
    ],
    "links": {
      "homepage": "https://github.com/Mescht/Zero-Practice",
      "issues": "https://github.com/Mescht/Zero-Practice/issues",
      "support": [ { "label": "Ko-fi", "url": "https://ko-fi.com/mescht" } ]
    },
    "licence": { "spdx": null, "redistribution": "link-only", "autoDownloadFromSource": true, "mirrorAllowed": false, "permission": null, "optOut": false },
    "source": { "type": "github-release", "repo": "Mescht/Zero-Practice", "assetPattern": "^Zero\\.Practice\\.v.*\\.zip$" },
    "versions": [
      {
        "version": "1.2.2",
        "releasedAt": "2025-08-23",
        "url": "https://github.com/Mescht/Zero-Practice/releases/download/v1.2.2/Zero.Practice.v1.2.2.zip",
        "sha256": "65892e292c49accaf959c98d0f9e2853b3c77987485b029f2959b9a407f55690",
        "size": 9079699,
        "minecraft": ["1.16.1"],
        "requires": [],
        "install": {
          "method": "extract-world", "target": "saves", "rootFolder": "Zero Practice v1.2.2", "instanceKind": "any-1.16.1",
          "preserveOnUpdate": ["data/command_storage_zero_practice_loadouts.dat"]
        }
      }
    ],
    "tags": { "categories": ["rsg-1.16.1", "ranked"], "splits": ["end", "zero-cycle"], "difficulty": "advanced", "keywords": ["dragon", "tower"] },
    "popularity": { "githubStars": 14, "downloads": 116813, "recommendedBy": ["https://www.youtube.com/watch?v=CSdkCmZ69RI"], "asOf": "2026-10-23" },
    "status": { "review": "curated", "accuracy": "n/a", "lastVerified": "2026-10-23", "linkHealth": "ok" }
  },
  {
    "id": "draconix-setup-guide-2026",
    "kind": "tutorial",
    "name": "How to setup Minecraft (and Boateye) for Speedrunning – Full comprehensive guide [2026]",
    "summary": "End-to-end setup: launcher, legal mods, Java, StandardSettings, SeedQueue, practice tools, pixel-perfect eye measuring with Toolscreen, OBS and srcom submission.",
    "credits": [ { "name": "draconix", "role": "author", "links": { "youtube": "https://www.youtube.com/@draconix_" } } ],
    "links": { "homepage": "https://www.youtube.com/watch?v=l-q-_4R8_6M" },
    "licence": { "spdx": null, "redistribution": "proprietary", "autoDownloadFromSource": false, "mirrorAllowed": false, "permission": null, "optOut": false, "notes": "Link / official YouTube embed only." },
    "tutorial": {
      "format": "video",
      "url": "https://www.youtube.com/watch?v=l-q-_4R8_6M",
      "publishedAt": "2026-04-01",
      "durationMin": 25,
      "language": "en",
      "requiresAccount": false,
      "parts": [
        { "title": "SeedQueue", "url": "https://www.youtube.com/watch?v=l-q-_4R8_6M&t=364s" },
        { "title": "Practice tools", "url": "https://www.youtube.com/watch?v=l-q-_4R8_6M&t=590s" },
        { "title": "Pixel perfect eye measuring setup", "url": "https://www.youtube.com/watch?v=l-q-_4R8_6M&t=713s" },
        { "title": "OBS", "url": "https://www.youtube.com/watch?v=l-q-_4R8_6M&t=1385s" }
      ]
    },
    "tags": { "categories": ["rsg-1.16.1", "ranked"], "splits": ["setup", "eye-measuring", "resetting", "obs", "settings"], "difficulty": "beginner", "keywords": ["toolscreen", "boat eye", "seedqueue", "standardsettings"] },
    "popularity": { "views": 207937, "recommendedBy": ["https://github.com/jojoe77777/Toolscreen", "https://www.youtube.com/watch?v=OEpZlv6cQsI"], "asOf": "2026-10-23" },
    "status": { "review": "curated", "accuracy": "current", "lastVerified": "2026-10-23", "linkHealth": "ok" }
  }
]
```
