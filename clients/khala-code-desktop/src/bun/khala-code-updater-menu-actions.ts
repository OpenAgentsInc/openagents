/**
 * Native app-menu action ids and dispatcher for the #8440 in-app updater
 * plumbing. Kept as a small pure module (rather than inline in
 * `src/bun/index.ts`) so the menu-click routing is unit-testable without
 * booting Electrobun's native `ApplicationMenu` bridge.
 */

export const KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES = "khala-code:check-for-updates"
export const KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES = "khala-code:open-release-notes"

export type KhalaCodeApplicationMenuUpdaterDeps = {
  readonly checkForUpdates: () => unknown
  readonly openReleaseNotes: () => unknown
}

/**
 * Returns true when the action was one of the updater menu actions and was
 * dispatched to `deps`; false for any other (or missing) action id, so
 * callers can chain additional menu-action handlers.
 */
export const handleKhalaCodeApplicationMenuAction = (
  action: string | null | undefined,
  deps: KhalaCodeApplicationMenuUpdaterDeps,
): boolean => {
  if (action === KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES) {
    void deps.checkForUpdates()
    return true
  }
  if (action === KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES || action === "help.release_notes") {
    void deps.openReleaseNotes()
    return true
  }
  return false
}
