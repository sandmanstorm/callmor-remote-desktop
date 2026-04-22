; Callmor Remote Desktop Agent — NSIS installer
; Single .exe that installs the agent, registers a Windows service,
; and prompts the user for MACHINE_ID + AGENT_TOKEN.
;
; Build:   makensis -DVERSION=0.1.0 -DBIN=path/to/callmor-agent.exe installer.nsi
; Output:  callmor-agent-setup-<version>.exe

!ifndef VERSION
    !define VERSION "0.1.0"
!endif

!ifndef BIN
    !error "Define BIN=/path/to/callmor-agent.exe"
!endif

!include "MUI2.nsh"
!include "LogicLib.nsh"

Name "Callmor Remote Desktop Agent ${VERSION}"
!ifndef OUTPUT
    !define OUTPUT "callmor-agent-setup-${VERSION}.exe"
!endif
OutFile "${OUTPUT}"
InstallDir "$PROGRAMFILES64\Callmor"
RequestExecutionLevel admin
Unicode true

; --- UI ---
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
Page custom ConfigPage ConfigPageLeave
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; --- Variables for config page ---
Var Dialog
Var MachineIdText
Var AgentTokenText
Var RelayUrlText
Var ApiUrlText
Var MachineId
Var AgentToken
Var RelayUrl
Var ApiUrl

; --- Custom page: configuration ---
Function ConfigPage
    !insertmacro MUI_HEADER_TEXT "Agent Configuration" "Paste your MACHINE_ID and AGENT_TOKEN from the Callmor dashboard."

    nsDialogs::Create 1018
    Pop $Dialog
    ${If} $Dialog == error
        Abort
    ${EndIf}

    ${NSD_CreateLabel} 0 0 100% 20u "Machine ID (UUID):"
    Pop $0
    ${NSD_CreateText} 0 20u 100% 14u ""
    Pop $MachineIdText

    ${NSD_CreateLabel} 0 42u 100% 20u "Agent Token (starts with cmt_):"
    Pop $0
    ${NSD_CreateText} 0 62u 100% 14u ""
    Pop $AgentTokenText

    ${NSD_CreateLabel} 0 84u 100% 20u "Relay URL:"
    Pop $0
    ${NSD_CreateText} 0 104u 100% 14u "wss://relay.callmor.ai"
    Pop $RelayUrlText

    ${NSD_CreateLabel} 0 126u 100% 20u "API URL:"
    Pop $0
    ${NSD_CreateText} 0 146u 100% 14u "https://api.callmor.ai"
    Pop $ApiUrlText

    nsDialogs::Show
FunctionEnd

Function ConfigPageLeave
    ${NSD_GetText} $MachineIdText $MachineId
    ${NSD_GetText} $AgentTokenText $AgentToken
    ${NSD_GetText} $RelayUrlText $RelayUrl
    ${NSD_GetText} $ApiUrlText $ApiUrl

    ${If} $MachineId == ""
        MessageBox MB_ICONEXCLAMATION "Machine ID is required."
        Abort
    ${EndIf}
    ${If} $AgentToken == ""
        MessageBox MB_ICONEXCLAMATION "Agent Token is required."
        Abort
    ${EndIf}
FunctionEnd

; --- Install section ---
Section "Install"
    SetOutPath "$INSTDIR"

    ; Copy the agent binary
    File "/oname=callmor-agent.exe" "${BIN}"

    ; Switch to machine-wide vars so $APPDATA -> C:\ProgramData
    SetShellVarContext all
    CreateDirectory "$APPDATA\Callmor"

    ; Write agent.conf with user-provided values
    FileOpen $0 "$APPDATA\Callmor\agent.conf" w
    FileWrite $0 "# Callmor Remote Desktop Agent Configuration (Windows)$\r$\n"
    FileWrite $0 "# Edit these and restart the service: sc stop CallmorAgent & sc start CallmorAgent$\r$\n"
    FileWrite $0 "$\r$\n"
    FileWrite $0 "RELAY_URL=$RelayUrl$\r$\n"
    FileWrite $0 "API_URL=$ApiUrl$\r$\n"
    FileWrite $0 "MACHINE_ID=$MachineId$\r$\n"
    FileWrite $0 "AGENT_TOKEN=$AgentToken$\r$\n"
    FileClose $0

    ; Register the Windows service
    ; Stop/remove any previous install first (upgrade path)
    nsExec::Exec 'sc stop CallmorAgent'
    Sleep 1500
    nsExec::Exec 'sc delete CallmorAgent'
    Sleep 500

    nsExec::Exec 'sc create CallmorAgent binPath= "\"$INSTDIR\callmor-agent.exe\"" start= auto DisplayName= "Callmor Remote Desktop Agent"'
    nsExec::Exec 'sc description CallmorAgent "Enables remote access via Callmor."'

    ; Start the service
    nsExec::Exec 'sc start CallmorAgent'

    ; Uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Registry: Add/Remove Programs entry
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "DisplayName" "Callmor Remote Desktop Agent"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "InstallLocation" "$\"$INSTDIR$\""
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "DisplayVersion" "${VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "Publisher" "Callmor"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "URLInfoAbout" "https://callmor.ai"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent" "NoRepair" 1
SectionEnd

; --- Uninstaller ---
Section "Uninstall"
    ; Stop and remove service
    nsExec::Exec 'sc stop CallmorAgent'
    Sleep 1500
    nsExec::Exec 'sc delete CallmorAgent'

    ; Remove files
    Delete "$INSTDIR\callmor-agent.exe"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    ; Config preserved by default (respects user edits). Uncomment to also remove:
    ; SetShellVarContext all
    ; Delete "$APPDATA\Callmor\agent.conf"
    ; RMDir "$APPDATA\Callmor"

    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent"
SectionEnd
