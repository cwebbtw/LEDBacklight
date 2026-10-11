@echo off
if exist "%~dp0target\release\backlight-next.exe" (
    move /y "%~dp0target\release\backlight-next.exe" "%~dp0target\release\backlight.exe" >nul 2>&1
    if errorlevel 1 (
        echo Close the existing Backlight window, then run this launcher again.
        pause
        exit /b 1
    )
)
start "" "%~dp0target\release\backlight.exe" --installed-strip --ddp 192.168.4.23:4048 --wow %*
