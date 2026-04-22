@echo off
REM Callmor Remote Desktop Agent — Windows installer (unsigned)
REM Run this as Administrator.

setlocal

set "INSTALL_DIR=%ProgramFiles%\Callmor"
set "DATA_DIR=%ProgramData%\Callmor"

REM Check admin
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo This installer must be run as Administrator.
    echo Right-click install.bat and select "Run as administrator".
    pause
    exit /b 1
)

echo Installing Callmor Agent to %INSTALL_DIR%...

if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"

copy /Y "%~dp0callmor-agent.exe" "%INSTALL_DIR%\callmor-agent.exe" >nul

REM Install config if it doesn't exist
if not exist "%DATA_DIR%\agent.conf" (
    copy /Y "%~dp0agent.conf.template" "%DATA_DIR%\agent.conf" >nul
    echo.
    echo Config file created: %DATA_DIR%\agent.conf
    echo.
    echo You MUST edit this file and paste your MACHINE_ID and AGENT_TOKEN
    echo from the Callmor dashboard before starting the service.
    echo.
)

REM Register the Windows service
sc query CallmorAgent >nul 2>&1
if %errorLevel% equ 0 (
    echo Service already exists; stopping before update...
    sc stop CallmorAgent >nul 2>&1
    timeout /t 2 /nobreak >nul
) else (
    echo Creating Windows service "CallmorAgent"...
    sc create CallmorAgent binPath= "\"%INSTALL_DIR%\callmor-agent.exe\"" start= auto DisplayName= "Callmor Remote Desktop Agent"
    sc description CallmorAgent "Enables remote access via Callmor. See %DATA_DIR%\agent.conf"
)

echo.
echo =====================================================
echo  Callmor Agent installed.
echo.
echo  Config:  %DATA_DIR%\agent.conf
echo  Binary:  %INSTALL_DIR%\callmor-agent.exe
echo.
echo  To start the agent:
echo    1. Edit %DATA_DIR%\agent.conf — paste MACHINE_ID and AGENT_TOKEN.
echo    2. Run:  sc start CallmorAgent
echo.
echo  To uninstall:  run uninstall.bat as Administrator.
echo =====================================================
echo.
pause
