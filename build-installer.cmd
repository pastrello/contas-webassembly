@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0installer\Build-Installer.ps1" %*
exit /b %errorlevel%
