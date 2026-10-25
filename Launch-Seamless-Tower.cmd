@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Launch-Seamless-Tower.ps1"
if errorlevel 1 pause
