@echo off
title Wisp Server einrichten
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0skripte\einrichten.ps1"
echo.
pause
