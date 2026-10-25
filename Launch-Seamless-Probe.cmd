@echo off
setlocal
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0Launch-Seamless-Probe.ps1" %*
set "tower_exit_code=%errorlevel%"
if not "%tower_exit_code%"=="0" pause
exit /b %tower_exit_code%
