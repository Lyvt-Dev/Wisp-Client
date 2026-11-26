@echo off
title Wisp-Hauptserver
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0skripte\hauptserver.ps1"
echo.
pause
