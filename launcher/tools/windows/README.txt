Speedrunning Launcher, test build (milestone 2)

Run
  Unzip anywhere and double-click speedrunning-launcher.exe. Enter a player name, pick a mode
  (Ranked, RSG, FSG, SSG or Draftout) and press Download. The launcher installs Java,
  Minecraft, Fabric Loader, the mode's legal mods and its tools, then starts the game. The
  first install downloads about 500 MB; after that Play starts the game at once.

  The account is offline: singleplayer works (RSG, FSG, SSG). Ranked and Draftout start with
  their mods, but playing them online needs a Microsoft account, which comes later.

Tools
  Ninjabrain Bot and Toolscreen start with the game in Ranked, RSG, FSG and SSG, and close when
  it exits. The Tools page installs and updates them and sets which modes start each one.
  Draftout bans Ninjabrain Bot. Toolscreen's log: %USERPROFILE%\.config\toolscreen\logs\latest.log

Where things are
  Game, mods, tools and settings: %APPDATA%\SpeedrunningLauncher\data
  The launcher's log:             %APPDATA%\SpeedrunningLauncher\data\logs\launcher.log
  The game's log:                 %APPDATA%\SpeedrunningLauncher\data\logs\<mode>-latest.log
  To start over, close the launcher and delete that folder.

Command line (no window), for checks and bug reports:
  speedrunning-launcher.exe --install rsg                install or repair a mode
  speedrunning-launcher.exe --status                     what is installed
  speedrunning-launcher.exe --tools                      the state of each tool
  speedrunning-launcher.exe --install-tool toolscreen    install a tool, with its hashes
  speedrunning-launcher.exe --help                       all commands
  The exe is a windowed program, so PowerShell and cmd don't wait for it. In PowerShell pipe
  it to wait and see the output: .\speedrunning-launcher.exe --install rsg | Out-Host
  Set SRL_HOME to a folder to keep the data there instead.

VERSION.txt names the commit this build comes from. The licences of the bundled fonts and icons
are in licenses\, and those of adapted code in THIRD_PARTY_NOTICES.md.
