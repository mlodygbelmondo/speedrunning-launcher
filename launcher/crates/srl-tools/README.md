# srl-tools

The speedrun tools a mode needs: installed at legal versions, started with the game, stopped when
it exits. The API is documented at the top of `src/lib.rs`; each tool's module says where its
facts come from.

## The tools

| Tool | Runs on | Version | Source and check | Licence |
|---|---|---|---|---|
| Ninjabrain Bot (Ninjabrain1) | Windows, Linux, macOS | 1.5.2 | `Ninjabrain-Bot-1.5.2.jar` from its GitHub release, sha256 `98099f63…71450ff` (GitHub's asset digest, pinned in code) | GPL-3.0 |
| Toolscreen (jojoe77777) | Windows x64 | 1.4.7 | `Toolscreen-1.4.7-double-click-me.jar`, sha256 `bfadb621…8cc3e`; inside it `Toolscreen.dll` (= legal-dlls.csv `Toolscreen.dll 1.4.4`) and `liblogger_x64.dll` (`1.0.2`), both by sha512 | MIT |
| Toolscreen | Windows ARM64 | 1.5.0 (prerelease, the only ARM64 build) | `Toolscreen-1.5.0-double-click-me.jar`, sha256 `fc19cdb8…fe822e`; `Toolscreen_arm64.dll` (`1.5.0-arm64`), `liblogger_arm64.dll` (`1.1.0`) | MIT |
| waywall (tesselslate) | Linux, Wayland session | whatever the system has | not installed by the launcher: found on `PATH`; the patched GLFW is checked against legal-dlls.csv | GPL-3.0 |

Why these versions:

- **Ninjabrain Bot:** speedrun.com rules v8 (2026-10-17), A.11.7.a: "NinjabrainBot (up to
  v1.5.2)". 1.5.2 is also its newest release.
- **Toolscreen:** legal-dlls.csv (the legal-builds repository, commit bc4735d, 2026-10-19)
  lists Toolscreen.dll 1.0.0 to 1.4.4 and 1.5.0/1.5.1 by sha512. Releases 1.4.5 to 1.4.7 carry the
  1.4.4 DLL unchanged (same sha512), so they are legal. 1.5.1 is in the CSV but not on GitHub.
  The launcher takes the newest pinned release that is not a prerelease and whose two DLLs for
  this CPU are in the current CSV; a prerelease only when nothing else qualifies.

## Per mode

| | Ranked | RSG | FSG | SSG | Draftout |
|---|---|---|---|---|---|
| Ninjabrain Bot | on | on | on | on | banned (draftoutmc.com rules) |
| Toolscreen (Windows) | on | on | on | on | not set up yet (26.3 needs Toolscreen's Vulkan layer) |
| waywall (Linux) | off | off | off | off | off |

waywall is off by default because it needs the player's own setup (a Wayland session, the
patched GLFW, a config). The player turns it on per mode.

## How a launch uses them

1. `Tools::install_selected` (after the mods, `Stage::Tools`): downloads what is missing or old.
   With waywall on for a 1.16.1 mode it also refreshes legal-dlls.csv (on every OS), so the GLFW
   can be checked; when no list can be had, the note says the GLFW couldn't be checked rather
   than that it isn't legal.
2. `Tools::prepare`: checks Ninjabrain Bot's jar (sha256), Toolscreen's DLLs (sha512 against the
   pin and the cached legal-dlls.csv), finds waywall and the GLFW, and wraps the command:
   `waywall wrap -- <java> -Dorg.lwjgl.glfw.libname=<glfw> <args>` for 1.16.1,
   `-DMC_DEBUG_ENABLED -DMC_DEBUG_PREFER_WAYLAND` for 26.x, with `__GL_THREADED_OPTIMIZATIONS=0`.
3. `GameInstaller::spawn` starts that command.
4. `Tools::start`: injects liblogger, then Toolscreen.dll, into the game's JVM (Windows), and
   starts Ninjabrain Bot with the game's Java unless one already runs. When waywall wraps the
   game and the player's waywall config starts Ninjabrain itself (some `.lua` file in the config
   directory, or one level below, calls `waywall.exec` and some names a Ninjabrain jar, also
   through a variable), the launcher leaves it to the config.
5. When the game exits, the session closes Ninjabrain Bot (its window, then a kill after 5 s),
   unless the player had started it or another running game still uses it.
6. A Ninjabrain Bot the launcher started has a pid file, `logs/tools/ninjabrain-bot.pid`. If the
   launcher closes or crashes while the game runs, the next launcher start takes it over
   (`Tools::adopt_earlier`): it stops it at once when no game of that session runs any more,
   else once those games are gone, and a Play meanwhile shares it.

A tool that can't run never blocks the game: it is left out with a `ToolNote`, which has a copy
key (`tool.note.*`) for the UI.

## Command line

```sh
CARGO_TARGET_DIR=$HOME/.cache/srl-target/tools srl-cpu cargo build -j 4 -p srl-tools
srl-tools status
srl-tools install ninjabrain-bot
srl-tools fetch-toolscreen [x64|arm64]     # downloads and checks the DLLs on any OS
srl-tools run-ninjabrain <java> --for 12 --ready-cmd 'import -window root shot.png'
srl-tools launch rsg [--tools ninjabrain-bot,waywall] [--glfw PATH] --ready-cmd CMD
```

Run anything with a window inside Xvfb (`env -u WAYLAND_DISPLAY DISPLAY=:N`).

## What to check on Windows

Toolscreen's injection ran only under wine here (a test DLL into notepad, `win::tests`), never
into a real game. On a Windows 10/11 x64 PC with the launcher:

1. Tools: Toolscreen shows 1.4.7 after its install; `tools\toolscreen\1.4.7\x64\` holds
   `Toolscreen.dll` and `liblogger_x64.dll`.
2. Play RSG. The Toolscreen popup appears in the top-left corner of the game; Ctrl+I opens its
   settings; Thin, Wide and EyeZoom switch at once.
3. `%USERPROFILE%\.config\toolscreen\logs\latest.log` exists and shows the init lines.
4. Windows Defender or another antivirus doesn't block or quarantine the launcher when it injects
   (Toolscreen's own installer asks for Defender exclusions for the same reason). Note what it
   says if it does.
5. Ninjabrain Bot opens with the game, without a console window, and closes when you quit the
   game. If you started Ninjabrain yourself first, the launcher leaves it open.
6. Stop from the launcher: the game closes as with its window's X (its `logs\latest.log` ends
   with `Stopping!`), not killed.
7. A Ninjabrain Bot of your own with another version (e.g. 1.5.1) is detected as running (its
   window title starts with "Ninjabrain Bot").
