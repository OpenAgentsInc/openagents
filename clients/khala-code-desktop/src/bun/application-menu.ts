import { KHALA_CODE_UPDATER_MENU_ACTION_CHECK_FOR_UPDATES } from "./khala-code-updater-menu-actions.js"

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
    label: "File",
    submenu: [
      { label: "New Session", action: "session.new_chat", accelerator: "CommandOrControl+N" },
      { label: "New Window", action: "window.new", accelerator: "CommandOrControl+Shift+N" },
      { label: "Open Project Home", action: "view.home", accelerator: "Alt+7" },
      { type: "divider" },
      { label: "Settings", action: "view.settings", accelerator: "Alt+5" },
      { type: "divider" },
      { role: "close", accelerator: "CommandOrControl+W" },
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
    label: "View",
    submenu: [
      { label: "Command Palette", action: "palette.open", accelerator: "CommandOrControl+K" },
      { type: "divider" },
      { label: "Chat", action: "view.chat", accelerator: "Alt+1" },
      { label: "Fleet", action: "view.fleet", accelerator: "Alt+2" },
      { label: "Forum", action: "view.forum", accelerator: "Alt+3" },
      { label: "Inbox", action: "view.inbox", accelerator: "Alt+4" },
      { label: "Editor", action: "view.editor", accelerator: "Alt+6" },
      { label: "Review", action: "view.review", accelerator: "Alt+8" },
      { label: "Terminal", action: "view.terminal", accelerator: "Alt+9" },
      { type: "divider" },
      // "reload"/"togglefullscreen"/"toggleDevTools" are NOT real Electrobun
      // menu roles (the real role set only has "toggleFullScreen", capital
      // F, for fullscreen; there is no native reload or devtools role at
      // all) -- using them here rendered as blank, non-functional menu rows.
      // Reload and Toggle Developer Tools now round-trip through the same
      // `nativeWindowAction` RPC a hotkey/palette selection uses (see
      // src/bun/index.ts and src/ui/main.ts); Toggle Full Screen uses the
      // real native role.
      { label: "Reload", action: "app.reload_webview", accelerator: "CommandOrControl+R" },
      {
        label: "Toggle Developer Tools",
        action: "dev.toggle_devtools",
        accelerator: "CommandOrControl+Alt+I",
      },
      { type: "divider" },
      { label: "Zoom In", action: "view.zoom_in", accelerator: "CommandOrControl+Plus" },
      { label: "Zoom Out", action: "view.zoom_out", accelerator: "CommandOrControl+-" },
      { label: "Actual Size", action: "view.zoom_reset", accelerator: "CommandOrControl+0" },
      { type: "divider" },
      { role: "toggleFullScreen", accelerator: "Control+CommandOrControl+F" },
    ],
  },
  {
    label: "Go",
    submenu: [
      { label: "Previous Session", action: "session.previous", accelerator: "Alt+Left" },
      { label: "Next Session", action: "session.next", accelerator: "Alt+Right" },
      { type: "divider" },
      { label: "Previous Message", action: "message.previous", accelerator: "Alt+Up" },
      { label: "Next Message", action: "message.next", accelerator: "Alt+Down" },
      { type: "divider" },
      { label: "Refresh Terminals", action: "terminal.refresh" },
      { label: "Clean Exited Terminals", action: "terminal.clean" },
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
      { label: "Release Notes", action: "help.release_notes" },
      { type: "divider" },
      { label: "Documentation", action: "help.docs" },
      { label: "Support", action: "help.support" },
      { label: "Send Feedback", action: "help.feedback" },
      { label: "Report Bug", action: "help.bug_report" },
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
