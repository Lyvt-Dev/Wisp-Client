@echo off
title Wisp Server aktualisieren
powershell -NoProfile -ExecutionPolicy Bypass -Command ". \"%~dp0skripte\common.ps1\"; Build-Servers; Good \"Aktualisiert. Starte laufende Server neu.\""
echo.
pause
