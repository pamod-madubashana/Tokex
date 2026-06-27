@echo off
title Cotrex Installer
echo Installing Cotrex...
echo.
powershell -ExecutionPolicy Bypass -File "%~dp0install.ps1"
echo.
pause
