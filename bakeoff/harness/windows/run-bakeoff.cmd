@echo off
rem Speedrunluncher bake-off launcher: runs run-bakeoff.ps1 without touching the execution policy.
rem Double-click it, accept the administrator prompt (needed for PresentMon), keep hands off.
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0run-bakeoff.ps1" %*
set RC=%ERRORLEVEL%
endlocal & exit /b %RC%
