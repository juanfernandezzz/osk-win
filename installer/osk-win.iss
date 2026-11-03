; Inno Setup script for the osk-win installer.
; Build: cargo build --release -p osk-win --no-default-features
;        ISCC.exe installer\osk-win.iss   -> installer\Output\osk-win-setup.exe

#define AppVersion "0.3.0"

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

[Tasks]
Name: "admincert"; Description: "Escribir también en apps abiertas como administrador (agrega como confiable el certificado de osk-win-input.exe; su clave privada fue destruida)"; Flags: unchecked

[Files]
Source: "..\target\release\osk-win.exe"; DestDir: "{app}"; Flags: ignoreversion
; Signed by firmar.ps1; Windows starts it (UIAccess) only once its certificate is trusted.
Source: "..\target\release\osk-win-input.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "osk-win-input.cer"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Teclado en pantalla osk-win"; Filename: "{app}\osk-win.exe"; Comment: "Teclado en pantalla"

[Run]
Filename: "{sys}\certutil.exe"; Parameters: "-addstore Root ""{app}\osk-win-input.cer"""; Flags: runhidden; Tasks: admincert
Filename: "{app}\osk-win.exe"; Description: "{cm:LaunchProgram,osk-win}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM osk-win.exe /IM osk-win-input.exe"; Flags: runhidden; RunOnceId: "kill"
Filename: "{sys}\certutil.exe"; Parameters: "-delstore Root ""osk-win input helper"""; Flags: runhidden; RunOnceId: "delcert"

[Code]
// The UIAccess helper has no window, so Restart Manager cannot close it
// and an update would stop at "applications in use".
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM osk-win.exe /IM osk-win-input.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(500);
  Result := '';
end;
