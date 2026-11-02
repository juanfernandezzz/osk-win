; Inno Setup script for the osk-win installer.
; Build: cargo build --release -p osk-win --no-default-features
;        ISCC.exe installer\osk-win.iss   -> installer\Output\osk-win-setup.exe

#define AppVersion "0.1.0"

[Setup]
AppId={{6B0E2A4C-5F3B-4C1E-9A7D-0F5C2D8E7B41}
AppName=osk-win
AppVersion={#AppVersion}
AppPublisher=osk-win
AppPublisherURL=https://github.com/juanfernandezzz/osk-win
DefaultDirName={autopf}\osk-win
DefaultGroupName=osk-win
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=osk-win-setup
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
UninstallDisplayIcon={app}\osk-win.exe

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\target\release\osk-win.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Teclado en pantalla osk-win"; Filename: "{app}\osk-win.exe"; Comment: "Teclado en pantalla"

[Run]
Filename: "{app}\osk-win.exe"; Description: "{cm:LaunchProgram,osk-win}"; Flags: nowait postinstall skipifsilent
