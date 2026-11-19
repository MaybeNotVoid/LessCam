@echo off
setlocal
chcp 65001 >nul
title Installazione e Clonazione di LessCam in C:\LessCam
color 0B

echo ===========================================================================
echo        LessCam Studio 2.0 (Rust Edition) - Deploy su C:\LessCam
echo ===========================================================================
echo.

set "TARGET_DIR=C:\LessCam"

if exist "%TARGET_DIR%\.git" (
    echo [*] Cartella %TARGET_DIR% gia' esistente. Aggiornamento repository via git pull...
    git -C "%TARGET_DIR%" pull
) else (
    where git >nul 2>&1
    if %errorlevel% equ 0 (
        echo [*] Clonazione di LessCam in %TARGET_DIR%...
        git clone https://github.com/MaybeNotVoid/LessCam.git "%TARGET_DIR%"
    ) else (
        echo [*] Git non rilevato nel PATH, copia diretta dei file in %TARGET_DIR%...
        mkdir "%TARGET_DIR%" 2>nul
        xcopy "%~dp0*" "%TARGET_DIR%\" /E /I /H /Y
    )
)

echo.
echo [*] Avvio dell'installer automatico dei programmi esterni e build Rust in %TARGET_DIR%...
if exist "%TARGET_DIR%\lesscam-rust\installer\Install_LessCam_Rust.bat" (
    call "%TARGET_DIR%\lesscam-rust\installer\Install_LessCam_Rust.bat"
)

echo.
echo [OK] Progetto clonato e configurato in C:\LessCam!
pause
