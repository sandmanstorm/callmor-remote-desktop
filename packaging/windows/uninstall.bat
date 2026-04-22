@echo off
REM Callmor Remote Desktop Agent — uninstaller
setlocal

net session >nul 2>&1
if %errorLevel% neq 0 (
    echo This must be run as Administrator.
    pause
    exit /b 1
)

echo Stopping and removing CallmorAgent service...
sc stop CallmorAgent >nul 2>&1
sc delete CallmorAgent >nul 2>&1

echo Removing binary...
del /Q "%ProgramFiles%\Callmor\callmor-agent.exe" 2>nul
rmdir "%ProgramFiles%\Callmor" 2>nul

echo.
echo Callmor Agent removed.
echo.
echo Config preserved at %ProgramData%\Callmor\agent.conf
echo (Delete that folder manually if you want a clean uninstall.)
echo.
pause
