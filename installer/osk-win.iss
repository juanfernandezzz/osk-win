; Inno Setup script for the osk-win installer.
; Build with installer/package.ps1 after cargo build --release --locked.

#ifndef AppVersion
  #error Supply /DAppVersion matching the workspace package version
#endif
#ifndef BinaryDir
  #define BinaryDir "..\target\release"
#endif
#ifndef SignedHelper
  #define SignedHelper BinaryDir + "\osk-win-input.exe"
#endif

[Setup]
AppId={{6B0E2A4C-5F3B-4C1E-9A7D-0F5C2D8E7B41}
AppName=osk-win
AppVersion={#AppVersion}
VersionInfoVersion={#AppVersion}.0
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
RestartApplications=no
UninstallDisplayIcon={app}\osk-win.exe

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "admincert"; Description: "Escribir también en apps abiertas como administrador (agrega como confiable el certificado de osk-win-input.exe; su clave privada fue destruida)"; Flags: unchecked

[Files]
Source: "{#BinaryDir}\osk-win.exe"; DestDir: "{app}"; Flags: ignoreversion
; Signed by firmar.ps1; Windows starts it (UIAccess) only once its certificate is trusted.
Source: "{#SignedHelper}"; DestDir: "{app}"; DestName: "osk-win-input.exe"; Flags: ignoreversion
Source: "osk-win-input.cer"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Teclado en pantalla osk-win"; Filename: "{app}\osk-win.exe"; Comment: "Teclado en pantalla"

[Run]
Filename: "{sys}\certutil.exe"; Parameters: "-addstore Root ""{app}\osk-win-input.cer"""; Flags: runhidden; Tasks: admincert; Check: not IsKeyboardUpdate
Filename: "{app}\osk-win.exe"; Description: "{cm:LaunchProgram,osk-win}"; Flags: nowait postinstall skipifsilent
Filename: "{app}\osk-win.exe"; Flags: nowait runasoriginaluser skipifnotsilent; Check: IsKeyboardUpdate

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM osk-win.exe /IM osk-win-input.exe"; Flags: runhidden; RunOnceId: "kill"
Filename: "{sys}\certutil.exe"; Parameters: "-delstore Root ""osk-win input helper"""; Flags: runhidden; RunOnceId: "delcert"

[Code]
function IsKeyboardUpdate: Boolean;
begin
  Result := ExpandConstant('{param:OSKUPDATE|0}') = '1';
end;

function GetWindowThreadProcessId(Wnd: HWND; var Pid: DWORD): DWORD;
  external 'GetWindowThreadProcessId@user32.dll stdcall';

// The UIAccess helper lacks a window for Restart Manager. Only stop keyboard
// and helper processes belonging to the installation being replaced.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Locator, Services, Processes, Proc: Variant;
  Wnd: HWND;
  Pid, ClosingPid: DWORD;
  I: Integer;
  Keyboard, Helper, ExePath: String;
begin
  Result := '';
  ClosingPid := 0;
  Keyboard := ExpandConstant('{app}\osk-win.exe');
  Helper := ExpandConstant('{app}\osk-win-input.exe');
  if not (FileExists(Keyboard) or FileExists(Helper)) then Exit;
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Services := Locator.ConnectServer('.', 'root\CIMV2');
    Wnd := FindWindowByClassName('osk-win-window');
    if (Wnd <> 0) and (GetWindowThreadProcessId(Wnd, Pid) <> 0) then begin
      Proc := Services.Get('Win32_Process.Handle="' + IntToStr(Pid) + '"');
      if not VarIsNull(Proc.ExecutablePath) then
        if CompareText(Proc.ExecutablePath, Keyboard) = 0 then begin
          { WM_CLOSE lets the keyboard save geometry and release modifiers. }
          PostMessage(Wnd, $0010, 0, 0);
          for I := 1 to 40 do begin
            if FindWindowByClassName('osk-win-window') <> Wnd then begin
              ClosingPid := Pid;
              Break;
            end;
            Sleep(50);
          end;
        end;
    end;
    Processes := Services.ExecQuery('SELECT * FROM Win32_Process WHERE Name="osk-win.exe" OR Name="osk-win-input.exe"');
    for I := 0 to Processes.Count - 1 do begin
      Proc := Processes.ItemIndex(I);
      if not VarIsNull(Proc.ExecutablePath) then begin
        ExePath := Proc.ExecutablePath;
        if ((CompareText(ExePath, Keyboard) = 0) and (Proc.ProcessId <> ClosingPid)) or (CompareText(ExePath, Helper) = 0) then
          Proc.Terminate(0);
      end;
    end;
  except
    Log('Process lookup failed: ' + GetExceptionMessage);
    { Restart Manager still handles files in use. }
  end;
end;
