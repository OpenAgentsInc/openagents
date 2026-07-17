export const MOBILE_REGULAR_WORKSPACE_MIN_WIDTH = 768
export const MOBILE_WORKSPACE_SIDEBAR_MIN = 240
export const MOBILE_WORKSPACE_SIDEBAR_MAX = 360
export const MOBILE_WORKSPACE_SIDEBAR_DEFAULT = 288

export type MobileWorkspaceLayoutMode = "compact" | "regular"
export type MobileWorkspaceFocusTarget = "navigation" | "transcript"

export const mobileWorkspaceLayoutMode = (width: number): MobileWorkspaceLayoutMode =>
  Number.isFinite(width) && width >= MOBILE_REGULAR_WORKSPACE_MIN_WIDTH ? "regular" : "compact"

export const clampMobileWorkspaceSidebar = (width: number): number =>
  Math.min(MOBILE_WORKSPACE_SIDEBAR_MAX, Math.max(
    MOBILE_WORKSPACE_SIDEBAR_MIN,
    Number.isFinite(width) ? Math.round(width) : MOBILE_WORKSPACE_SIDEBAR_DEFAULT,
  ))

export const mobileWorkspaceActiveDescendant = (
  mode: MobileWorkspaceLayoutMode,
  drawerOpen: boolean,
  sidebarCollapsed: boolean,
  focusTarget: MobileWorkspaceFocusTarget,
): string => {
  if (mode === "compact") return drawerOpen ? "drawer-root" : "home-root"
  if (focusTarget === "navigation" && !sidebarCollapsed) return "drawer-root"
  return "home-root"
}

