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
    ],
  },
]
