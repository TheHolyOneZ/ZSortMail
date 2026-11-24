!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\ZSortMail.Mailto" "" "URL:ZSortMail mailto"
  WriteRegStr SHCTX "Software\Classes\ZSortMail.Mailto" "URL Protocol" ""
  WriteRegStr SHCTX "Software\Classes\ZSortMail.Mailto\DefaultIcon" "" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr SHCTX "Software\Classes\ZSortMail.Mailto\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
  WriteRegStr SHCTX "Software\Clients\Mail\ZSortMail\Capabilities" "ApplicationName" "ZSortMail"
  WriteRegStr SHCTX "Software\Clients\Mail\ZSortMail\Capabilities" "ApplicationDescription" "Gmail organised around people"
  WriteRegStr SHCTX "Software\Clients\Mail\ZSortMail\Capabilities\URLAssociations" "mailto" "ZSortMail.Mailto"
  WriteRegStr SHCTX "Software\RegisteredApplications" "ZSortMail" "Software\Clients\Mail\ZSortMail\Capabilities"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey SHCTX "Software\Classes\ZSortMail.Mailto"
  DeleteRegKey SHCTX "Software\Clients\Mail\ZSortMail"
  DeleteRegValue SHCTX "Software\RegisteredApplications" "ZSortMail"
!macroend
