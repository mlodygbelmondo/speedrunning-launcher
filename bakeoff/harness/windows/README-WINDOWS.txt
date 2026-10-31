Speedrunluncher UI bake-off: Windows test pack
==============================================

This folder contains three tiny test apps that draw the same launcher screen
with three different UI stacks (Tauri, Electron, GPUI), plus a script that
measures them. It takes about 20 minutes and needs no installation.

Before you start
----------------
- Plug in the laptop (if it is one) and keep the PC awake.
- Close games, browsers, Discord, OBS, Steam and other heavy apps.
- Put the high-refresh monitor as your PRIMARY display (the test windows open there).
- Make sure nothing will pop up on its own (updates, notifications) if you can.

How to run
----------
1. Extract this zip into any folder (right-click -> Extract All). Don't run it
   from inside the zip.
2. Double-click run-bakeoff.cmd.
3. Accept the Windows administrator prompt. Admin rights are only needed for
   PresentMon (Intel's open-source frame-timing tool, downloaded once from
   GitHub into tools\ and checked against a pinned SHA-256).
4. Don't touch the mouse or keyboard for ~20 minutes. Windows will open,
   animate, minimise and close by themselves; that's expected.
5. At the end the window says:

       Send this file back: ...\results-windows-<PC name>-<date>.json

   Send that .json file back (the .md next to it is a readable summary).

Options (advanced)
------------------
Run from PowerShell in this folder, for example:

    powershell -ExecutionPolicy Bypass -File run-bakeoff.ps1 -Runs 1 -PhaseMs 5000
    powershell -ExecutionPolicy Bypass -File run-bakeoff.ps1 -Apps tauri,gpui
    powershell -ExecutionPolicy Bypass -File run-bakeoff.ps1 -NoPresentMon   (no admin needed, fewer metrics)

Troubleshooting
---------------
- "Windows protected your PC" (SmartScreen): click "More info" -> "Run anyway".
  The apps are unsigned test builds from the project's GitHub Actions.
- If you decline the admin prompt you can still continue without PresentMon,
  but the most important numbers (presented frames) will be missing.
- If an app fails, the script notes it in the results and continues. Send the
  file anyway.
