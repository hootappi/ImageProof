@echo off
setlocal

set "SCRIPT_DIR=%~dp0"
set "PS1_FILE=%SCRIPT_DIR%start-web.ps1"

if not exist "%PS1_FILE%" (
  echo start-web.ps1 was not found next to this file.
  echo Expected path: "%PS1_FILE%"
  pause
  exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%PS1_FILE%"
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
  echo.
  echo Launcher exited with code %EXIT_CODE%.
  echo Press any key to close.
  pause >nul
)

exit /b %EXIT_CODE%
