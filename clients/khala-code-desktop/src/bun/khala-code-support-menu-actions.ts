export const KHALA_CODE_SUPPORT_MENU_ACTION_DOCS = "khala-code:open-docs"
export const KHALA_CODE_SUPPORT_MENU_ACTION_SUPPORT = "khala-code:open-support"
export const KHALA_CODE_SUPPORT_MENU_ACTION_FEEDBACK = "khala-code:send-feedback"
export const KHALA_CODE_SUPPORT_MENU_ACTION_BUG_REPORT = "khala-code:report-bug"

export type KhalaCodeApplicationMenuSupportDeps = {
  readonly openBugReport: () => unknown
  readonly openDocs: () => unknown
  readonly openFeedback: () => unknown
  readonly openSupport: () => unknown
}

export const handleKhalaCodeSupportMenuAction = (
  action: string | null | undefined,
  deps: KhalaCodeApplicationMenuSupportDeps,
): boolean => {
  if (action === KHALA_CODE_SUPPORT_MENU_ACTION_DOCS || action === "help.docs") {
    void deps.openDocs()
    return true
  }
  if (action === KHALA_CODE_SUPPORT_MENU_ACTION_SUPPORT || action === "help.support") {
    void deps.openSupport()
    return true
  }
  if (action === KHALA_CODE_SUPPORT_MENU_ACTION_FEEDBACK || action === "help.feedback") {
    void deps.openFeedback()
    return true
  }
  if (action === KHALA_CODE_SUPPORT_MENU_ACTION_BUG_REPORT || action === "help.bug_report") {
    void deps.openBugReport()
    return true
  }
  return false
}
