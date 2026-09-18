@echo off
setlocal
chcp 65001 >nul
title LessCam - Virtual Camera Studio
color 0A

echo =====================================================================
echo                LessCam - Virtual Camera Studio
echo       Interfaccia Fluent per Windows 11 - Streaming e Regia Video
echo =====================================================================
echo.

cd /d "%~dp0"

set "PYTHON_CMD="

python --version >nul 2>&1
if %errorlevel% equ 0 (
    set "PYTHON_CMD=python"
    goto :found_python
)

if exist "%LocalAppData%\Programs\Python\Python312\python.exe" (
    set "PYTHON_CMD=%LocalAppData%\Programs\Python\Python312\python.exe"
    goto :found_python
)

if exist "%ProgramFiles%\Python312\python.exe" (
    set "PYTHON_CMD=%ProgramFiles%\Python312\python.exe"
    goto :found_python
)

:not_found
color 0C
echo [ERRORE] Interprete Python non trovato nel sistema!
echo Assicurati che Python 3.10 o superiore sia installato e aggiunto al PATH.
echo.
pause
exit /b 1

:found_python
echo [*] Interprete Python rilevato: %PYTHON_CMD%
echo [*] Avvio di LessCam in corso...
echo.

"%PYTHON_CMD%" main.py

if %errorlevel% neq 0 (
    echo.
    color 0C
    echo =====================================================================
    echo [ATTENZIONE] LessCam si e' chiusa con codice di errore: %errorlevel%
    echo Se mancano dei moduli, esegui:
    echo   pip install -r requirements.txt
    echo =====================================================================
    echo.
    pause
)