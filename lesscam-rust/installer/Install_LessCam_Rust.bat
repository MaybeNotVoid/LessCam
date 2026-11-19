@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul
title LessCam Studio 2.0 (Rust Edition) - Installer Completo & Dipendenze
color 0B

echo ===========================================================================
echo          LessCam Studio 2.0 (Rust Edition) - Installer Automatico
echo    Installa FFmpeg, yt-dlp, VB-Audio Cable, VirtualCam e compila in Rust
echo ===========================================================================
echo.

cd /d "%~dp0\.."
set "RUST_PROJECT_DIR=%CD%"

:: 1. Installazione programmi esterni necessari (FFmpeg, yt-dlp, VB-Cable, VirtualCam)
echo [FASE 1/3] Configurazione e download programmi esterni necessari...
powershell -NoProfile -ExecutionPolicy Bypass -File "%RUST_PROJECT_DIR%\installer\setup_dependencies.ps1" -TargetRoot "%RUST_PROJECT_DIR%"
echo.

:: 2. Verifica toolchain Rust (Cargo / Rustc)
echo [FASE 2/3] Verifica compilatore Rust (cargo)...
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    ) else (
        echo [!] Toolchain Rust non trovata. Scaricamento e installazione automatica di Rustup...
        powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile '$env:TEMP\rustup-init.exe'; Start-Process -FilePath '$env:TEMP\rustup-init.exe' -ArgumentList '-y' -Wait"
        set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    )
)

:: 3. Compilazione Release nativa ad alte prestazioni
echo [FASE 3/3] Compilazione nativa di LessCam in Rust (--release)...
cargo build --release --manifest-path "%RUST_PROJECT_DIR%\Cargo.toml"
if %errorlevel% neq 0 (
    color 0C
    echo.
    echo [ERRORE] La compilazione di LessCam Rust non e' riuscita.
    pause
    exit /b 1
)

if exist "%RUST_PROJECT_DIR%\target\release\lesscam.exe" (
    copy /Y "%RUST_PROJECT_DIR%\target\release\lesscam.exe" "%RUST_PROJECT_DIR%\lesscam.exe" >nul
)

color 0A
echo.
echo ===========================================================================
echo   [COMPLETATO] LessCam Studio 2.0 (Rust) e' installato e pronto all'uso!
echo   Eseguibile: %RUST_PROJECT_DIR%\lesscam.exe
echo   Binari esterni installati in: %RUST_PROJECT_DIR%\bin\
echo ===========================================================================
echo.
pause
