/**
 * New Shell Types
 *
 * Types for the tab container shell that switches between Gym and Commander.
 */

// ============================================================================
// Tab Configuration
// ============================================================================

export type TabId = "gym" | "commander"

export interface TabConfig {
  id: TabId
  label: string
  icon: string
}

export const TABS: TabConfig[] = [
  { id: "gym", label: "Gym", icon: "flask" },
  { id: "commander", label: "Commander", icon: "terminal" },
]

// ============================================================================
// Component State
// ============================================================================

export interface NewShellState {
  /** Currently active tab */
  activeTab: TabId
  /** Whether sidebar is collapsed */
  sidebarCollapsed: boolean
}

// ============================================================================
// Component Events
// ============================================================================

export type NewShellEvent =
  | { type: "changeTab"; tab: TabId }
  | { type: "toggleSidebar" }
