; Callmor Remote Desktop Agent — NSIS installer
; Zero-config single .exe: installs the agent, registers a Windows service,
; and starts it. The enrollment token is baked in at download time by the
; API (byte-level replacement of the placeholder below).
;
; Build:   makensis -DVERSION=0.1.0 -DBIN=path/to/callmor-agent.exe installer.nsi
; Output:  callmor-agent-setup-<version>.exe

!ifndef VERSION
    !define VERSION "0.1.0"
!endif

!ifndef BIN
    !error "Define BIN=/path/to/callmor-agent.exe"
!endif

; Directory containing the three mingw-w64 runtime DLLs that callmor-agent.exe
; depends on (libstdc++-6.dll, libgcc_s_seh-1.dll, libwinpthread-1.dll). Built
; by scripts/build-windows-installer.sh.
!ifndef DLLDIR
    !error "Define DLLDIR=/path/containing/mingw-runtime-dlls"
!endif

; MODE=tenant -> bakes in an enrollment-token placeholder (per-tenant download)
; MODE=adhoc  -> writes ADHOC=1; agent will self-register on first run and
;                display an access code + PIN to the user
!ifndef MODE
    !define MODE "tenant"
!endif

!include "MUI2.nsh"

Name "Callmor Remote Desktop Agent ${VERSION}"
!ifndef OUTPUT
    !define OUTPUT "callmor-agent-setup-${VERSION}.exe"
!endif
OutFile "${OUTPUT}"
InstallDir "$PROGRAMFILES64\Callmor"
RequestExecutionLevel admin
Unicode true

; IMPORTANT: no compression so the placeholder token is byte-findable inside
; the installer .exe and can be replaced at download time by the API.
SetCompress off

; Version metadata visible in Explorer properties and scanned by SmartScreen /
; AV engines. Real relief from SmartScreen requires code signing (EV cert), but
; presenting as a named publisher with a clear description reduces false flags.
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName"      "Callmor Remote Desktop"
VIAddVersionKey "CompanyName"      "Callmor"
VIAddVersionKey "LegalCopyright"   "Copyright (C) Callmor"
VIAddVersionKey "FileDescription"  "Callmor Remote Desktop Agent Installer"
VIAddVersionKey "FileVersion"      "${VERSION}"
VIAddVersionKey "ProductVersion"   "${VERSION}"
VIAddVersionKey "InternalName"     "callmor-agent-setup"
VIAddVersionKey "OriginalFilename" "callmor-agent-setup.exe"
BrandingText "Callmor — https://remote.callmor.ai"

; --- UI (silent-ish: welcome + progress + finish only) ---
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"
!define MUI_WELCOMEPAGE_TITLE "Install Callmor Remote Desktop Agent"
!define MUI_WELCOMEPAGE_TEXT "This will install and start the Callmor agent. No configuration is required — this installer was generated specifically for your Callmor tenant."
!define MUI_FINISHPAGE_TITLE "Installation complete"
!define MUI_FINISHPAGE_TEXT "The Callmor agent is running. You should see this machine appear on your Callmor dashboard within a few seconds."
!define MUI_FINISHPAGE_LINK "Open your Callmor dashboard"
!define MUI_FINISHPAGE_LINK_LOCATION "https://remote.callmor.ai"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; --- Install section ---
Section "Install"
    SetOutPath "$INSTDIR"

    ; Copy the agent binary
    File "/oname=callmor-agent.exe" "${BIN}"

    ; Bundle the mingw-w64 runtime DLLs callmor-agent.exe links against.
    ; They live next to the .exe so Windows' DLL search path finds them.
    File "${DLLDIR}\libstdc++-6.dll"
    File "${DLLDIR}\libgcc_s_seh-1.dll"
    File "${DLLDIR}\libwinpthread-1.dll"

    ; Switch to machine-wide vars so $APPDATA -> C:\ProgramData
    SetShellVarContext all
    CreateDirectory "$APPDATA\Callmor"

    ; Write agent.conf. In tenant mode the ENROLLMENT_TOKEN placeholder is
    ; replaced byte-for-byte by the API at download time with the caller's
    ; real token (same length, 36 chars). In adhoc mode we write ADHOC=1
    ; instead, which makes the agent self-register and display a code + PIN
    ; on first run.
    FileOpen $0 "$APPDATA\Callmor\agent.conf" w
    FileWrite $0 "# Callmor Remote Desktop Agent Configuration (Windows)$\r$\n"
    FileWrite $0 "# Auto-written by installer. Will be replaced by the agent on first run$\r$\n"
    FileWrite $0 "# with the machine's permanent credentials.$\r$\n"
    FileWrite $0 "$\r$\n"
    FileWrite $0 "RELAY_URL=wss://relay.callmor.ai$\r$\n"
    FileWrite $0 "API_URL=https://api.callmor.ai$\r$\n"
    !if "${MODE}" == "adhoc"
        FileWrite $0 "ADHOC=1$\r$\n"
    !else
        FileWrite $0 "ENROLLMENT_TOKEN=cle_INSTALLER_TOKEN_PLACEHOLDER_XXXX$\r$\n"
    !endif
    FileClose $0

    ; Register the Windows service
    ; Stop/remove any previous install first (upgrade path)
    nsExec::Exec 'sc stop CallmorAgent'
    Sleep 1500
    nsExec::Exec 'sc delete CallmorAgent'
    Sleep 500

    nsExec::Exec 'sc create CallmorAgent binPath= "\"$INSTDIR\callmor-agent.exe\"" start= auto DisplayName= "Callmor Remote Desktop Agent"'
    nsExec::Exec 'sc description CallmorAgent "Enables remote access via Callmor."'

    ; Start the service (agent will self-enroll on first run)
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
    Delete "$INSTDIR\libstdc++-6.dll"
    Delete "$INSTDIR\libgcc_s_seh-1.dll"
    Delete "$INSTDIR\libwinpthread-1.dll"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    ; Config preserved by default (holds machine credentials). Uncomment to purge:
    ; SetShellVarContext all
    ; Delete "$APPDATA\Callmor\agent.conf"
    ; RMDir "$APPDATA\Callmor"

    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorAgent"
SectionEnd
