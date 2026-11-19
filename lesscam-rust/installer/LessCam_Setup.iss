; ==============================================================================
; LessCam Studio 2.0 (Rust Edition) - Inno Setup Script (.iss)
; Impacchetta l'eseguibile nativo Rust + FFmpeg + yt-dlp + VB-Audio Cable +
; Driver DirectShow Virtual Camera con installazione predefinita in C:\LessCam
; ==============================================================================

#define MyAppName "LessCam Studio 2.0 (Rust Edition)"
#define MyAppVersion "2.0.0"
#define MyAppPublisher "LessCam Studio"
#define MyAppURL "https://github.com/MaybeNotVoid/LessCam"
#define MyAppExeName "lesscam.exe"

[Setup]
AppId={{8F94A4D1-6E2A-4D87-9B72-5C82A0E12026}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName=C:\LessCam
DefaultGroupName={#MyAppName}
PrivilegesRequired=admin
OutputDir=.\output
OutputBaseFilename=LessCam_Studio_2.0_Rust_Setup_x64
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "install_vbcable"; Description: "Installa il driver audio virtuale VB-Audio Virtual Cable (Consigliato)"; GroupDescription: "Programmi e Driver Esterni Necessari:"
Name: "install_vcam"; Description: "Registra il driver DirectShow Webcam Virtuale (UnityCapture / OBS VirtualCam)"; GroupDescription: "Programmi e Driver Esterni Necessari:"

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\bin\yt-dlp.exe"; DestDir: "{app}\bin"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\bin\ffmpeg.exe"; DestDir: "{app}\bin"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\bin\ffprobe.exe"; DestDir: "{app}\bin"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\bin\vbcable\*"; DestDir: "{app}\bin\vbcable"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "..\bin\unitycapture\*"; DestDir: "{app}\bin\unitycapture"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "setup_dependencies.ps1"; DestDir: "{app}\installer"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
; 1. Scarica/verifica binari esterni mancanti (yt-dlp, ffmpeg, ffprobe, driver)
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\installer\setup_dependencies.ps1"" -TargetRoot ""{app}"""; StatusMsg: "Configurazione di FFmpeg, yt-dlp e driver multimediali..."; Flags: runhidden waituntilterminated
; 2. Registra filtro DirectShow Virtual Camera
Filename: "{sys}\regsvr32.exe"; Parameters: "/s ""{app}\bin\unitycapture\UnityCaptureFilter64.dll"""; StatusMsg: "Registrazione driver DirectShow Virtual Camera..."; Tasks: install_vcam; Flags: waituntilterminated skipifdoesntexist
; 3. Avvia LessCam al termine
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
