; NSIS installer for mcomix-rs (Windows).
; Run from the crate root: makensis -DVERSION=x.y.z packaging/windows/installer.nsi

!ifndef VERSION
  !define VERSION "0.1.0"
!endif

!define APPNAME "MComix3"
!define BIN "mcomix-rs.exe"
!define SOURCE_DIR "dist\mcomix-rs-win64"
!define REGKEY "Software\${APPNAME}"

Name "${APPNAME} ${VERSION}"
OutFile "dist\mcomix-rs-setup-${VERSION}.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "${REGKEY}" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
    SetOutPath "$INSTDIR"
    File /r "${SOURCE_DIR}\*.*"

    ; Start menu shortcut
    CreateDirectory "$SMPROGRAMS\${APPNAME}"
    CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\${BIN}"
    CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall.lnk" "$INSTDIR\uninstall.exe"

    ; Uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "${REGKEY}" "InstallDir" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
        "DisplayName" "${APPNAME}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
        "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
        "DisplayVersion" "${VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" \
        "Publisher" "MComix3 developers"
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\uninstall.exe"
    RMDir /r "$INSTDIR"
    Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
    Delete "$SMPROGRAMS\${APPNAME}\Uninstall.lnk"
    RMDir "$SMPROGRAMS\${APPNAME}"
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"
    DeleteRegKey HKLM "${REGKEY}"
SectionEnd
