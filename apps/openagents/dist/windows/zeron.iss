; OpenAgents (the OpenAgents desktop app, a Zeron fork) for Windows — per-user installer (Inno Setup 6).
; Identity values mirror crates/brand/src/lib.rs. Not yet verified on Windows.
;
; Built by scripts/package-windows.ps1, which passes the version, the package
; architecture, and the staged portable directory:
;   ISCC.exe /DAppVersion=0.2.97 /DArch=x86_64 /DPackageDir=<stage> /DOutputDir=<out> zeron.iss
;
; Installs into %LOCALAPPDATA%\Programs\OpenAgents without elevation, like VS
; Code's user setup: the directory stays writable by its user, so the in-app
; updater (crates/update/src/windows.rs) can replace zeron.exe in place. The
; staged directory already carries zeron-update.json, which marks the install
; as update-managed. Re-running a newer installer upgrades in place; user data
; lives in %LOCALAPPDATA%\OpenAgents Desktop and is never touched here.

#ifndef AppVersion
  #error AppVersion must be defined (/DAppVersion=x.y.z)
#endif
#ifndef Arch
  #error Arch must be defined (/DArch=x86_64 or /DArch=aarch64)
#endif
#ifndef PackageDir
  #error PackageDir must be defined (/DPackageDir=<staged package directory>)
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

#if Arch == "aarch64"
  #define ArchAllowed "arm64"
#else
  #define ArchAllowed "x64compatible"
#endif

[Setup]
; Never change AppId: it identifies the installation across upgrades, and
; crates/update/src/windows.rs refreshes DisplayVersion under this key after
; in-app updates.
AppId={{0FBFAAA3-AC9B-4542-8219-5C1638A1FB18}
AppName=OpenAgents
AppVersion={#AppVersion}
AppVerName=OpenAgents {#AppVersion}
AppPublisher=OpenAgents
AppPublisherURL=https://openagents.com/download
AppSupportURL=https://openagents.com/docs
AppUpdatesURL=https://openagents.com/download
VersionInfoVersion={#AppVersion}
PrivilegesRequired=lowest
DefaultDirName={autopf}\OpenAgents
DisableProgramGroupPage=yes
DisableDirPage=auto
DisableReadyPage=yes
ArchitecturesAllowed={#ArchAllowed}
ArchitecturesInstallIn64BitMode={#ArchAllowed}
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=openagents-desktop-{#AppVersion}-windows-{#Arch}-setup
SetupIconFile=..\coder\coder.ico
UninstallDisplayIcon={app}\zeron.exe
UninstallDisplayName=OpenAgents
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
; A running OpenAgents is closed through the Restart Manager before its files are
; replaced; the updated app starts again from the finish page.
CloseApplications=yes
RestartApplications=no

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PackageDir}\zeron.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\zeron-update.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PackageDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\OpenAgents"; Filename: "{app}\zeron.exe"
Name: "{autodesktop}\OpenAgents"; Filename: "{app}\zeron.exe"; Tasks: desktopicon

[Registry]
; openagents:// conversation links — the scheme macOS registers in Info.plist
; and Linux in zeron.desktop.
Root: HKCU; Subkey: "Software\Classes\openagents"; ValueType: string; ValueName: ""; ValueData: "URL:OpenAgents"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\openagents"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\openagents\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\zeron.exe"",0"
Root: HKCU; Subkey: "Software\Classes\openagents\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\zeron.exe"" ""%1"""

[Run]
Filename: "{app}\zeron.exe"; Description: "{cm:LaunchProgram,OpenAgents}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Leftovers of in-app updates (crates/update/src/windows.rs).
Type: files; Name: "{app}\zeron.exe.old"
Type: files; Name: "{app}\.zeron-update-incoming.exe"
Type: filesandordirs; Name: "{app}\.zeron-update-*"
