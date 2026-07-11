/**
 * Closed Desktop command registry. Entries name existing typed intents only;
 * it never carries a callback, host API, shell command, or user-entered route.
 * Palette buttons therefore dispatch the exact same intent as the visible UI.
 */
export const desktopCommandRegistry = [
  { id: "chat.new", label: "New chat", intentName: "DesktopNewChat", payload: null },
  { id: "workspace.fleet", label: "Open fleet", intentName: "DesktopWorkspaceSelected", payload: "fleet" },
  { id: "chat.open", label: "Open chat", intentName: "DesktopWorkspaceSelected", payload: "chat" },
  { id: "workspace.files", label: "Open Files", intentName: "DesktopWorkspaceSelected", payload: "files" },
  { id: "workspace.review", label: "Review changes", intentName: "DesktopWorkspaceSelected", payload: "review" },
  { id: "workspace.choose", label: "Choose workspace folder", intentName: "DesktopWorkspacePickerRequested", payload: null },
  { id: "settings.open", label: "Open Settings", intentName: "DesktopSettingsToggled", payload: null },
] as const

export type DesktopCommand = (typeof desktopCommandRegistry)[number]
