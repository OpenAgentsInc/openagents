import { KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES, KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES } from "./khala-code-updater-menu-actions.js"

type MenuItem =
  | { type: "divider" | "separator" }
  | {
      type?: "normal"
      label?: string
      role?: string
      action?: string
      accelerator?: string
      submenu?: Array<MenuItem>
    }

/**
 * Diagnostics/debug-log export menu actions (issue #8441). Native menu
 * commands give the user a recovery path that does not depend on the
 * (possibly unresponsive) webview content — see src/bun/index.ts's
 * `application-menu-clicked` handler, which routes these through the same
 * diagnostics service used by the in-window recovery overlay and the native
 * unresponsive dialog.
 */
export const KHALA_CODE_DESKTOP_MENU_ACTION_RESTART = "khala.diagnostics.relaunch"
export const KHALA_CODE_DESKTOP_MENU_ACTION_EXPORT_DEBUG_LOGS = "khala.diagnostics.export"

export const khalaCodeDesktopApplicationMenu: Array<MenuItem> = [
  {
    label: "Khala Code",
    submenu: [
      { role: "about" },
      { type: "divider" },
      { role: "hide", accelerator: "CommandOrControl+H" },
      { role: "hideOthers", accelerator: "CommandOrControl+Alt+H" },
      { role: "showAll" },
      { type: "divider" },
      { role: "quit", accelerator: "CommandOrControl+Q" },
    ],
  },
  {
    label: "Edit",
    submenu: [
      { role: "undo", accelerator: "CommandOrControl+Z" },
      { role: "redo", accelerator: "CommandOrControl+Shift+Z" },
      { type: "divider" },
      { role: "cut", accelerator: "CommandOrControl+X" },
      { role: "copy", accelerator: "CommandOrControl+C" },
      { role: "paste", accelerator: "CommandOrControl+V" },
      {
        role: "pasteAndMatchStyle",
        accelerator: "CommandOrControl+Alt+Shift+V",
      },
      { role: "delete" },
      { type: "divider" },
      { role: "selectAll", accelerator: "CommandOrControl+A" },
    ],
  },
  {
    label: "Window",
    submenu: [
      { role: "minimize", accelerator: "CommandOrControl+M" },
      { role: "zoom" },
      { type: "divider" },
      { role: "bringAllToFront" },
    ],
  },
  {
    label: "Help",
    submenu: [
      { label: "Check for Updates…", action: KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES },
      { label: "Release Notes", action: KHALA_CODE_UPDATER_MENU_ACTION_RELEASE_NOTES },
      { type: "divider" },
      {
        label: "Restart Khala Code",
        action: KHALA_CODE_DESKTOP_MENU_ACTION_RESTART,
      },
      {
        label: "Export Debug Logs…",
        action: KHALA_CODE_DESKTOP_MENU_ACTION_EXPORT_DEBUG_LOGS,
      },
    ],
  },
]
