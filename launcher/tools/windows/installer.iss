; Speedrunning Launcher's installer for Windows (Inno Setup 6.3 or later; saved as UTF-8 with a BOM
; for the Polish text). Per user and without admin rights: the launcher goes to
; %LOCALAPPDATA%\Programs\Speedrunning Launcher, a folder the player can write to, so the launcher's
; own update can swap its exe there (crates/srl-update). The data (game, Java, instances and worlds)
; is in %LOCALAPPDATA%\SpeedrunningLauncher\data, which the uninstaller never deletes.
;
; CI builds it in the `windows` job of .github/workflows/launcher.yml from the packaged folder
; (launcher/dist: the exe, README.txt, the licences, VERSION.txt), with the [workspace.package]
; version from launcher/Cargo.toml. From launcher/:
;   iscc /DAppVersion=0.2.3 tools/windows/installer.iss   ->  speedrunning-launcher-setup.exe

#ifndef AppVersion
  #error Pass the version: iscc /DAppVersion=X.Y.Z installer.iss
#endif
; The folder with what the installer installs, relative to this file.
#ifndef DistDir
  #define DistDir "..\..\dist"
#endif
#define AppName "Speedrunning Launcher"
#define AppExe "speedrunning-launcher.exe"
#define RepoUrl "https://github.com/mlodygbelmondo/speedrunning-launcher"

[Setup]
; Never change the AppId: an installer with another one installs a second copy. Windows' Apps list
; keeps the entry under HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<AppId>_is1, and
; the launcher's update writes its DisplayVersion there (srl-app, backend/update.rs, `apps_entry`).
AppId={{EDEC8318-C819-4254-A613-8235B80AB833}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=mlodygbelmondo
AppPublisherURL={#RepoUrl}
AppSupportURL={#RepoUrl}/issues
AppUpdatesURL={#RepoUrl}/releases
VersionInfoVersion={#AppVersion}
; SignPath signs only files whose product name is the launcher's (SIGNPATH.md).
VersionInfoProductName={#AppName}
VersionInfoDescription={#AppName} Setup
VersionInfoCopyright=GNU GPL v3.0 or later
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\{#AppName}
; The folder holds only the launcher (its data lives elsewhere), so there is nothing to choose.
DisableDirPage=yes
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
; Closes a running launcher before its files are replaced.
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=auto
OutputDir=..\..
OutputBaseFilename=speedrunning-launcher-setup
Compression=lzma2/max
SolidCompression=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"

[CustomMessages]
english.DataKept=Your games, instances and worlds are still in:%n%n%1%n%nDelete that folder yourself if you don't need them any more.
polish.DataKept=Twoje gry, instancje i światy nadal są w folderze:%n%n%1%n%nJeśli nie są Ci już potrzebne, usuń ten folder ręcznie.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#DistDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

; What the launcher's own update leaves next to the exe (crates/srl-update, swap.rs).
[UninstallDelete]
Type: files; Name: "{app}\speedrunning-launcher.old*.exe"
Type: files; Name: "{app}\speedrunning-launcher.exe.new"

[Code]
// The data folder stays: it holds the player's worlds, often many GB. Say where it is instead.
// Roaming is where launchers up to 0.2.3 kept it, and where it stays when moving it failed.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Dir: String;
begin
  if (CurUninstallStep <> usPostUninstall) or UninstallSilent then
    Exit;
  Dir := ExpandConstant('{localappdata}\SpeedrunningLauncher\data');
  if not DirExists(Dir) then
    Dir := ExpandConstant('{userappdata}\SpeedrunningLauncher\data');
  if DirExists(Dir) then
    MsgBox(FmtMessage(CustomMessage('DataKept'), [Dir]), mbInformation, MB_OK);
end;
