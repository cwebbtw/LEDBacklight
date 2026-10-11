@echo off
start "" "%~dp0target\release\backlight.exe" --installed-strip --ddp 192.168.4.23:4048 %*
