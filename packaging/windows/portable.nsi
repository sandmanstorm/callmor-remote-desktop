; Callmor Remote Desktop — portable launcher
;
; Single .exe the user downloads. Silently extracts the agent binary + the
; three mingw runtime DLLs to %LOCALAPPDATA%\Callmor\, then launches
; callmor-agent.exe in portable (GUI) mode. No UAC prompt — runs as the
; current user. The user sees the app window appear within a second, like
; AnyDesk. A button inside the GUI handles the optional service install.

!ifndef VERSION
    !define VERSION "0.1.0"
!endif

!ifndef BIN
    !error "Define BIN=/path/to/callmor-agent.exe"
!endif

!ifndef DLLDIR
    !error "Define DLLDIR=/path/containing/mingw-runtime-dlls"
!endif

Name "Callmor Remote Desktop"
!ifndef OUTPUT
    !define OUTPUT "callmor-portable-${VERSION}.exe"
!endif
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Callmor"
RequestExecutionLevel user      ; no admin prompt
Unicode true
; Show a small progress window during extraction — without it the user
; gets zero feedback between double-click and the GUI appearing (~1s).
; Auto-close so it feels like "clicking a .exe and it opens".
AutoCloseWindow true
ShowInstDetails hide
XPStyle on
BrandingText " "
SetCompress off

; Metadata for Explorer properties / SmartScreen heuristics
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName"      "Callmor Remote Desktop"
VIAddVersionKey "CompanyName"      "Callmor"
VIAddVersionKey "LegalCopyright"   "Copyright (C) Callmor"
VIAddVersionKey "FileDescription"  "Callmor Remote Desktop"
VIAddVersionKey "FileVersion"      "${VERSION}"
VIAddVersionKey "ProductVersion"   "${VERSION}"
VIAddVersionKey "InternalName"     "callmor"
VIAddVersionKey "OriginalFilename" "callmor.exe"

Section ""
    ; Extract to per-user dir (writable without admin)
    SetOutPath "$INSTDIR"
    File "/oname=callmor-agent.exe" "${BIN}"
    File "${DLLDIR}\libstdc++-6.dll"
    File "${DLLDIR}\libgcc_s_seh-1.dll"
    File "${DLLDIR}\libwinpthread-1.dll"

    ; Launch the GUI, detach, and exit. The GUI takes over from here.
    Exec '"$INSTDIR\callmor-agent.exe"'
SectionEnd
