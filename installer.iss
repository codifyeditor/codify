#define AppName "Codify"
#define AppVersion "0.2.0"
#define AppPublisher "Codify"
#define AppExeName "codify.exe"

[Setup]
AppId={{8C9C04DA-20C6-4DA9-9F8A-8B01A1C0D0F2}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\Codify
DefaultGroupName=Codify
OutputDir=Output
OutputBaseFilename=Codify-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64
UninstallDisplayIcon={app}\{#AppExeName}

[Files]
Source: "target\release\codify.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Codify"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\Codify"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch Codify"; Flags: nowait postinstall skipifsilent
