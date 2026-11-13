@echo off
rem Double-click: builds and starts the Wisp server (cosmetics and friends), opens a public
rem address and points this PC's launcher at it. Close the window to stop it.
title Wisp server
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0start-wisp-server.ps1"
echo.
pause
