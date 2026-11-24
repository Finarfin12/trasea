; Trasea installer hooks (Tauri NSIS).
; After an install or an upgrade, tell the Windows shell that file icons changed, so the taskbar, Start menu and desktop
; shortcuts show the new Trasea icon instead of a cached icon of an earlier version at the same path.
!macro NSIS_HOOK_POSTINSTALL
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
