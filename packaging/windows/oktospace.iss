; OktoSpace setup for Windows (Inno Setup 6). Installs for the current user, without
; administrator rights, into the same folder OktoSpace installs itself to, so it knows it's the
; installed copy and keeps itself up to date from there.
;
;   iscc /DVersion=0.1.1 /DArch=x64 /DSource=<folder with the .exe files> /O<output folder> oktospace.iss

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef Source
  #define Source "..\..\target\release"
#endif

[Setup]
AppId={{8C2F6E6A-5B1D-4C51-9F0B-6D3B2A9E7C41}
AppName=OktoSpace
AppVersion={#Version}
AppVerName=OktoSpace {#Version}
AppPublisher=OktoSpace contributors
AppPublisherURL=https://github.com/emircesur/OktoSpace
AppSupportURL=https://github.com/emircesur/OktoSpace/issues
AppUpdatesURL=https://github.com/emircesur/OktoSpace/releases
VersionInfoVersion={#Version}
DefaultDirName={localappdata}\OktoSpace\app
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputBaseFilename=oktospace-{#Version}-windows-{#Arch}-setup
SetupIconFile=..\..\assets\oktospace.ico
UninstallDisplayIcon={app}\bin\oktospace.exe
UninstallDisplayName=OktoSpace
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
CloseApplications=yes
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[CustomMessages]
english.StartAtLogin=Start OktoSpace in the background when I sign in
turkish.StartAtLogin=Oturum açtığımda OktoSpace'i arka planda başlat

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startup"; Description: "{cm:StartAtLogin}"; Flags: unchecked

[Files]
Source: "{#Source}\oktospace.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#Source}\oktospace-cli.exe"; DestDir: "{app}\bin"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\OktoSpace"; Filename: "{app}\bin\oktospace.exe"
Name: "{userdesktop}\OktoSpace"; Filename: "{app}\bin\oktospace.exe"; Tasks: desktopicon

[Run]
; Through OktoSpace, so its own "start at login" setting matches.
Filename: "{app}\bin\oktospace-cli.exe"; Parameters: "autostart on"; Flags: runhidden; Tasks: startup
Filename: "{app}\bin\oktospace.exe"; Description: "{cm:LaunchProgram,OktoSpace}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\bin\oktospace-cli.exe"; Parameters: "autostart off"; Flags: runhidden; RunOnceId: "AutostartOff"

[UninstallDelete]
; Left behind by self-updates.
Type: files; Name: "{app}\bin\*.old"
Type: files; Name: "{app}\bin\*.new"
