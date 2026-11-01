; 小舟沙盒 安装包（NSIS）
; 编译：  cargo build --release
;        makensis installer/setup.nsi
; 产物：  dist/小舟沙盒-0.1.0-安装包.exe
;
; 注意：卸载只删程序，**不删用户数据**。保险库在
;   %LOCALAPPDATA%\com.xiaozhou.sandbox\vault
;   %APPDATA%\com.xiaozhou.sandbox\config.json
; 这两个目录要留给用户自己决定。

Unicode true
SetCompressor /SOLID lzma

; 安装目录用纯 ASCII，避免中文路径在各种工具链里出问题；
; 显示名、快捷方式仍然用中文。
!define APP_NAME    "小舟沙盒"
!define APP_DIR     "XiaozhouSandbox"
!define APP_EXE     "xiaozhou-sandbox.exe"
!define APP_VER     "0.1.0"
!define APP_PUB     "小舟沙盒"
!define APP_ID      "com.xiaozhou.sandbox"
!define APP_DESC    "本地加密保险库"

Name "${APP_NAME} ${APP_VER}"
OutFile "..\dist\${APP_NAME}-${APP_VER}-安装包.exe"
InstallDir "$PROGRAMFILES64\${APP_DIR}"
InstallDirRegKey HKLM "Software\${APP_ID}" "InstallDir"
RequestExecutionLevel admin
BrandingText "${APP_NAME} ${APP_VER} · ${APP_DESC}"

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ICON   "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_TITLE "${APP_NAME} 安装完成"
!define MUI_FINISHPAGE_TEXT "程序已安装到：$\r$\n$INSTDIR$\r$\n$\r$\n从开始菜单或桌面快捷方式启动。$\r$\n$\r$\n你的保险库默认存放在 %LOCALAPPDATA%\\${APP_ID}\\vault，卸载程序不会删除它。"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"

Section "主程序" SecMain
  SectionIn RO
  SetShellVarContext all

  SetOutPath "$INSTDIR"
  SetOverwrite try

  File "..\target\release\${APP_EXE}"
  File "..\README.md"
  File "..\SECURITY.md"
  File "..\LICENSE"
  File "..\THIRD-PARTY-NOTICES.md"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; 开始菜单
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk" "$INSTDIR\uninstall.exe"
  ; 桌面
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0

  ; 卸载信息
  WriteRegStr HKLM "Software\${APP_ID}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "DisplayVersion" "${APP_VER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "Publisher" "${APP_PUB}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "NoRepair" 1

  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "EstimatedSize" "$0"
SectionEnd

Section "Uninstall"
  SetShellVarContext all

  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\SECURITY.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.md"
  RMDir "$INSTDIR"

  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
  DeleteRegKey HKLM "Software\${APP_ID}"

  ; 绝不碰用户数据：只在交互式卸载时提示，静默卸载(/S)直接跳过
  IfFileExists "$LOCALAPPDATA\${APP_ID}\*.*" 0 noData
    IfSilent noData 0
    MessageBox MB_ICONINFORMATION|MB_OK "程序已卸载。$\r$\n$\r$\n你的保险库数据没有被删除，仍在：$\r$\n$LOCALAPPDATA\${APP_ID}\vault$\r$\n$\r$\n确认不再需要时请手动删除该目录。密码丢失后这些数据无法恢复。"
  noData:
SectionEnd
