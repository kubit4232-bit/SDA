; SDA (Super Download Agent) — Inno Setup инсталлятор для Windows x64
; Компилируется через ISCompact или Inno Setup Compiler

#define MyAppName "Super Download Agent (SDA)"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "SDA Project"
#define MyAppURL "https://example.com/sda"
#define MyAppExeName "sda.exe"

[Setup]
AppId={{E3B4C2D1-5A6F-4B2E-8C9D-0F1A2B3C4D5E}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={pf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=E:\site\IDM\SDA\build\output
OutputBaseFilename=SDA-Setup-x64
Compression=lzma
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
ArchitecturesAllowed=x64
PrivilegesRequired=admin

[Languages]
Name: "russian"; MessagesFile: "compiler:Languages\Russian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "E:\site\IDM\SDA\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "E:\site\IDM\SDA\target\release\msg_host.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "E:\site\IDM\SDA\target\release\sda-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "E:\site\IDM\SDA\target\release\yt-dlp.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "E:\site\IDM\SDA/resources/app.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "E:\site\IDM\SDA\resources\icons\*.svg"; DestDir: "{app}\resources\icons"; Flags: ignoreversion
; Добавьте сюда DLL зависимости при необходимости:
; Source: "E:\site\IDM\SDA\target\release\*.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"
Name: "{group}\SDA GUI"; Filename: "{app}\sda-gui.exe"; IconFilename: "{app}\app.ico"
Name: "{commondesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\app.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\sda-gui.exe"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
