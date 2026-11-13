#ifndef AppVersion
  #error AppVersion must be supplied by build-installer.ps1
#endif
#ifndef LauncherBinary
  #error LauncherBinary must be supplied by build-installer.ps1
#endif
#ifndef EngineDirectory
  #error EngineDirectory must be supplied by build-installer.ps1
#endif
#ifndef InstallerOutput
  #define InstallerOutput "..\..\dist"
#endif

[Setup]
AppId={{4F431DD9-8AFC-4D30-855B-C06683C43DA7}
AppName=Wisp Client
AppVersion={#AppVersion}
AppPublisher=Lyvt-Dev
AppPublisherURL=https://github.com/Lyvt-Dev/Wisp-Client
DefaultDirName={localappdata}\Programs\Wisp Client
DefaultGroupName=Wisp Client
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
OutputDir={#InstallerOutput}
OutputBaseFilename=Wisp-Client-{#AppVersion}-Setup-x64
SetupIconFile=..\icons\wisp.ico
UninstallDisplayIcon={app}\Wisp Client.exe
VersionInfoDescription=Wisp Client Setup
VersionInfoProductName=Wisp Client
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
CloseApplicationsFilter=Wisp Client.exe,wisp-client.exe,wisp-prepare.exe
RestartApplications=no
UninstallDisplayName=Wisp Client

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#LauncherBinary}"; DestDir: "{app}"; DestName: "Wisp Client.exe"; Flags: ignoreversion
Source: "{#EngineDirectory}\*"; DestDir: "{app}\engine"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\launcher\docs\microsoft-sign-in.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\..\UPSTREAM.md"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\..\crates\wisp-theme\fonts\OFL-*.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\..\crates\wisp-theme\icons\glass\LICENSE-hugeicons.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion
Source: "..\..\crates\wisp-theme\icons\pixel\LICENSE-pixelarticons.txt"; DestDir: "{app}\licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\Wisp Client"; Filename: "{app}\Wisp Client.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Wisp Client"; Filename: "{app}\Wisp Client.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\Wisp Client.exe"; Description: "{cm:LaunchProgram,Wisp Client}"; Flags: nowait postinstall skipifsilent
