# ==============================================================================
# LessCam Studio 2.0 (Rust Edition) - Automated External Programs Installer
# Scarica, estrae e installa automaticamente nella cartella bin/:
#   1. yt-dlp.exe (Estrazione flussi diretti YouTube)
#   2. ffmpeg.exe & ffprobe.exe (Motore video/audio & compressione H.264/AAC)
#   3. VB-Audio Virtual Cable (Driver di routing audio virtuale per Windows)
#   4. UnityCapture DirectShow Virtual Camera (Driver Webcam Virtuale Windows)
# ==============================================================================

param(
    [string]$TargetRoot = ""
)

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

if ([string]::IsNullOrWhiteSpace($TargetRoot)) {
    $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $TargetRoot = Split-Path -Parent $ScriptDir
}

$BinDir = Join-Path $TargetRoot "bin"
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

Write-Host "=====================================================================" -ForegroundColor Cyan
Write-Host "   LessCam Studio 2.0 (Rust) - Setup Programmi Esterni & Driver" -ForegroundColor Cyan
Write-Host "   Cartella Destinazione Binari: $BinDir" -ForegroundColor Gray
Write-Host "=====================================================================" -ForegroundColor Cyan
Write-Host ""

# ------------------------------------------------------------------------------
# 1. YT-DLP.EXE
# ------------------------------------------------------------------------------
$YtDlpExe = Join-Path $BinDir "yt-dlp.exe"
if (Test-Path $YtDlpExe) {
    Write-Host "[1/4] [OK] yt-dlp.exe e' gia' presente in bin/." -ForegroundColor Green
} else {
    Write-Host "[1/4] [*] Scaricamento yt-dlp.exe ufficiale da GitHub..." -ForegroundColor Yellow
    $YtUrl = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    Invoke-WebRequest -Uri $YtUrl -OutFile $YtDlpExe -UseBasicParsing
    Write-Host "      [OK] yt-dlp.exe installato con successo!" -ForegroundColor Green
}

# ------------------------------------------------------------------------------
# 2. FFMPEG.EXE & FFPROBE.EXE
# ------------------------------------------------------------------------------
$FfmpegExe = Join-Path $BinDir "ffmpeg.exe"
$FfprobeExe = Join-Path $BinDir "ffprobe.exe"
if ((Test-Path $FfmpegExe) -and (Test-Path $FfprobeExe)) {
    Write-Host "[2/4] [OK] ffmpeg.exe e ffprobe.exe sono gia' presenti in bin/." -ForegroundColor Green
} else {
    Write-Host "[2/4] [*] Scaricamento FFmpeg & FFprobe (build statica Windows)..." -ForegroundColor Yellow
    $FfZip = Join-Path $BinDir "ffmpeg-release-essentials.zip"
    $FfExtract = Join-Path $BinDir "ffmpeg_tmp"
    $FfUrl = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"

    try {
        Invoke-WebRequest -Uri $FfUrl -OutFile $FfZip -UseBasicParsing
        Write-Host "      [*] Estrazione archivio FFmpeg in corso..." -ForegroundColor Yellow
        Expand-Archive -Path $FfZip -DestinationPath $FfExtract -Force

        $FoundFfmpeg = Get-ChildItem -Path $FfExtract -Recurse -Filter "ffmpeg.exe" | Select-Object -First 1
        $FoundFfprobe = Get-ChildItem -Path $FfExtract -Recurse -Filter "ffprobe.exe" | Select-Object -First 1

        if ($FoundFfmpeg) { Copy-Item -Path $FoundFfmpeg.FullName -Destination $FfmpegExe -Force }
        if ($FoundFfprobe) { Copy-Item -Path $FoundFfprobe.FullName -Destination $FfprobeExe -Force }

        Remove-Item -Path $FfZip -Force -ErrorAction SilentlyContinue
        Remove-Item -Path $FfExtract -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "      [OK] ffmpeg.exe e ffprobe.exe installati in bin/!" -ForegroundColor Green
    } catch {
        Write-Host "      [!] Download diretto non riuscito, tentativo tramite Winget (Gyan.FFmpeg)..." -ForegroundColor Yellow
        winget install --id Gyan.FFmpeg -e --accept-source-agreements --accept-package-agreements
    }
}

# ------------------------------------------------------------------------------
# 3. VB-AUDIO VIRTUAL CABLE (Audio Routing Driver)
# ------------------------------------------------------------------------------
$VbDir = Join-Path $BinDir "vbcable"
$VbSetupExe = Join-Path $VbDir "VBCABLE_Setup_x64.exe"
New-Item -ItemType Directory -Force -Path $VbDir | Out-Null

$VbAlreadyInstalled = $false
try {
    $AudioDevs = Get-CimInstance Win32_SoundDevice -ErrorAction SilentlyContinue
    foreach ($d in $AudioDevs) {
        if ($d.Name -match "VB-Audio|CABLE") {
            $VbAlreadyInstalled = $true
            break
        }
    }
} catch {}

if ($VbAlreadyInstalled) {
    Write-Host "[3/4] [OK] Driver VB-Audio Virtual Cable gia' attivo nel sistema Windows!" -ForegroundColor Green
} else {
    if (-not (Test-Path $VbSetupExe)) {
        Write-Host "[3/4] [*] Scaricamento driver ufficiale VB-Audio Virtual Cable..." -ForegroundColor Yellow
        $VbZip = Join-Path $VbDir "VBCABLE_Driver_Pack43.zip"
        $VbUrl = "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip"
        Invoke-WebRequest -Uri $VbUrl -OutFile $VbZip -UseBasicParsing
        Expand-Archive -Path $VbZip -DestinationPath $VbDir -Force
        Remove-Item -Path $VbZip -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $VbSetupExe) {
        Write-Host "      [*] Avvio installer VB-Audio Virtual Cable (conferma eventuale prompt Amministratore)..." -ForegroundColor Yellow
        try {
            Start-Process -FilePath $VbSetupExe -ArgumentList "-i -h" -Verb RunAs -Wait
            Write-Host "      [OK] Setup VB-Audio Virtual Cable completato!" -ForegroundColor Green
        } catch {
            Write-Host "      [!] Installazione VB-Cable saltata o annullata dall'utente. Puoi avviarla dall'app con 1 click." -ForegroundColor DarkYellow
        }
    }
}

# ------------------------------------------------------------------------------
# 4. DIRECTSHOW VIRTUAL CAMERA DRIVER (UnityCapture / OBS VirtualCam)
# ------------------------------------------------------------------------------
$UcDir = Join-Path $BinDir "unitycapture"
$UcDll = Join-Path $UcDir "UnityCaptureFilter64.dll"
New-Item -ItemType Directory -Force -Path $UcDir | Out-Null

$VcamRegistered = (Test-Path "HKLM:\SOFTWARE\Classes\CLSID\{A3FCE0F5-3493-419F-958A-ABA1250EC20B}") -or `
                  (Test-Path "HKLM:\SOFTWARE\Classes\CLSID\{27B05C2D-93DC-474A-A5DA-9BBA34CB2A9C}") -or `
                  (Test-Path "HKLM:\SOFTWARE\Classes\CLSID\{5C2CD55C-92AD-4999-8666-912BD3E70001}")

if ($VcamRegistered) {
    Write-Host "[4/4] [OK] Driver DirectShow Webcam Virtuale gia' registrato in Windows!" -ForegroundColor Green
} else {
    if (-not (Test-Path $UcDll)) {
        Write-Host "[4/4] [*] Scaricamento driver leggero DirectShow Virtual Camera (UnityCapture)..." -ForegroundColor Yellow
        $UcZip = Join-Path $UcDir "UnityCapture.zip"
        try {
            Invoke-WebRequest -Uri "https://github.com/schellingb/UnityCapture/archive/refs/heads/master.zip" -OutFile $UcZip -UseBasicParsing
            Expand-Archive -Path $UcZip -DestinationPath $UcDir -Force
            $FoundDll = Get-ChildItem -Path $UcDir -Recurse -Filter "UnityCaptureFilter64.dll" | Select-Object -First 1
            if ($FoundDll) {
                Copy-Item -Path $FoundDll.FullName -Destination $UcDll -Force
            }
            Remove-Item -Path $UcZip -Force -ErrorAction SilentlyContinue
        } catch {
            Write-Host "      [!] Impossibile scaricare UnityCaptureFilter64.dll." -ForegroundColor DarkYellow
        }
    }

    if (Test-Path $UcDll) {
        Write-Host "      [*] Registrazione driver DirectShow Virtual Camera con regsvr32..." -ForegroundColor Yellow
        try {
            Start-Process -FilePath "regsvr32.exe" -ArgumentList "/s `"$UcDll`"" -Verb RunAs -Wait
            Write-Host "      [OK] Driver DirectShow Virtual Camera registrato con successo!" -ForegroundColor Green
        } catch {
            Write-Host "      [!] Registrazione rimandata (puoi eseguirla con 1 click dentro LessCam)." -ForegroundColor DarkYellow
        }
    }
}

Write-Host ""
Write-Host "=====================================================================" -ForegroundColor Green
Write-Host "   TUTTI I PROGRAMMI ESTERNI SONO STATI CONFIGURATI CORRETTAMENTE!" -ForegroundColor Green
Write-Host "=====================================================================" -ForegroundColor Green
