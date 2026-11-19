@echo off
setlocal
chcp 65001 >nul
title LessCam Studio 2.0 (Rust Edition)
color 0B

cd /d "%~dp0"

if not exist "bin\yt-dlp.exe" (
    echo [*] Primo avvio: configurazione programmi esterni in bin/...
    powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0installer\setup_dependencies.ps1" -TargetRoot "%~dp0"
)

if exist "%~dp0lesscam.exe" (
    start "" "%~dp0lesscam.exe"
    exit /b 0
)

if exist "%~dp0target\release\lesscam.exe" (
    start "" "%~dp0target\release\lesscam.exe"
    exit /b 0
)

echo [*] Compilazione ed esecuzione di LessCam Studio 2.0 (Rust Edition)...
cargo run --release
if %errorlevel% neq 0 (
    color 0C
    echo [ERRORE] Esegui prima installer\Install_LessCam_Rust.bat per installare tutti i componenti.
    pause
)
