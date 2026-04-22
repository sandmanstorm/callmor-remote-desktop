; Callmor-RustDesk installer — wraps upstream rustdesk.exe + pre-configures it
; to use our self-hosted rendezvous server (callmor.ai + our public key).
;
; User downloads a single file from callmor.ai/downloads/rustdesk/windows,
; double-clicks, gets RustDesk installed and connected to our network.
;
; Build:
;   makensis -DVERSION=1.4.6 -DBIN=/path/to/rustdesk.exe -DOUTPUT=callmor-rd.exe rustdesk-installer.nsi

!ifndef VERSION
    !define VERSION "1.4.6"
!endif

!ifndef BIN
    !error "Define BIN=/path/to/rustdesk.exe"
!endif

!ifndef OUTPUT
    !define OUTPUT "callmor-rd-setup-${VERSION}.exe"
!endif

!ifndef SERVER
    !define SERVER "callmor.ai"
!endif

!ifndef KEY
    !define KEY "9LE62rY2BFqC+lw28MhiJEewt4KsQHUCWEUWBZIuxtk="
!endif

Name "Callmor-RustDesk ${VERSION}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Callmor-RustDesk"
RequestExecutionLevel user          ; no admin — installs per-user
Unicode true
AutoCloseWindow true
ShowInstDetails hide
XPStyle on
BrandingText " "
SetCompress off

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName"      "Callmor-RustDesk"
VIAddVersionKey "CompanyName"      "Callmor"
VIAddVersionKey "LegalCopyright"   "Callmor. Bundles RustDesk under AGPL-3.0."
VIAddVersionKey "FileDescription"  "Callmor-RustDesk — self-hosted remote desktop"
VIAddVersionKey "FileVersion"      "${VERSION}"
VIAddVersionKey "ProductVersion"   "${VERSION}"
VIAddVersionKey "InternalName"     "callmor-rd-setup"
VIAddVersionKey "OriginalFilename" "callmor-rd-setup.exe"

Section ""
    ; Extract rustdesk.exe
    SetOutPath "$INSTDIR"
    File "/oname=rustdesk.exe" "${BIN}"

    ; Write the pre-configured RustDesk config so the client comes up already
    ; pointing at our rendezvous server. RustDesk reads RustDesk2.toml from
    ; %APPDATA%\RustDesk\config\ on every launch.
    CreateDirectory "$APPDATA\RustDesk"
    CreateDirectory "$APPDATA\RustDesk\config"

    FileOpen $0 "$APPDATA\RustDesk\config\RustDesk2.toml" w
    FileWrite $0 "rendezvous_server = '${SERVER}'$\r$\n"
    FileWrite $0 "nat_type = 0$\r$\n"
    FileWrite $0 "serial = 0$\r$\n"
    FileWrite $0 "$\r$\n"
    FileWrite $0 "[options]$\r$\n"
    FileWrite $0 "custom-rendezvous-server = '${SERVER}'$\r$\n"
    FileWrite $0 "relay-server = '${SERVER}'$\r$\n"
    FileWrite $0 "api-server = ''$\r$\n"
    FileWrite $0 "key = '${KEY}'$\r$\n"
    FileClose $0

    ; Desktop shortcut
    SetShellVarContext current
    CreateShortcut "$DESKTOP\Callmor-RustDesk.lnk" "$INSTDIR\rustdesk.exe" "" "$INSTDIR\rustdesk.exe" 0

    ; Start Menu shortcut
    CreateDirectory "$SMPROGRAMS\Callmor"
    CreateShortcut "$SMPROGRAMS\Callmor\Callmor-RustDesk.lnk" "$INSTDIR\rustdesk.exe" "" "$INSTDIR\rustdesk.exe" 0

    ; Uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"
    CreateShortcut "$SMPROGRAMS\Callmor\Uninstall Callmor-RustDesk.lnk" "$INSTDIR\uninstall.exe"

    ; Add/Remove Programs entry
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "DisplayName" "Callmor-RustDesk"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "InstallLocation" "$\"$INSTDIR$\""
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "Publisher" "Callmor"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "URLInfoAbout" "https://remote.callmor.ai"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk" "NoRepair" 1

    ; Launch the client
    Exec '"$INSTDIR\rustdesk.exe"'
SectionEnd

Section "Uninstall"
    ; Kill running RustDesk instances
    nsExec::Exec 'taskkill /F /IM rustdesk.exe'
    Sleep 500

    Delete "$INSTDIR\rustdesk.exe"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    Delete "$DESKTOP\Callmor-RustDesk.lnk"
    Delete "$SMPROGRAMS\Callmor\Callmor-RustDesk.lnk"
    Delete "$SMPROGRAMS\Callmor\Uninstall Callmor-RustDesk.lnk"
    RMDir "$SMPROGRAMS\Callmor"

    ; Leave the per-user config intact so reinstall picks up previous state.
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CallmorRustDesk"
SectionEnd
