import type { IconName } from "@effect-native/core"
import {
  desktopCanonicalCommandRegistry,
  type DesktopCommandId,
} from "../desktop-command-contract.ts"

/**
 * The currently admitted Desktop sidebar catalog. This is presentation data
 * over existing command/intent authority, not a second navigation registry.
 * Compatibility and React renderers can lower this same projection into their
 * respective controls.
 */
export type DesktopSidebarWorkspace = "chat" | "home" | "settings"
export type DesktopSidebarIconName = Extract<IconName, "ChatCompose" | "Home" | "Settings">

export type DesktopSidebarIntent =
  | Readonly<{ name: "DesktopNewChat"; payload: null }>
  | Readonly<{ name: "DesktopWorkspaceSelected"; payload: "home" }>
  | Readonly<{ name: "DesktopSettingsToggled"; payload: null }>

export type DesktopSidebarDestinationDefinition = Readonly<{
  id:
    | "workspace-new-chat"
    | "workspace-home"
    | "shell-settings-toggle"
  commandId: DesktopCommandId
  label: "New session" | "Project home" | "Settings"
  icon: DesktopSidebarIconName
  workspace: DesktopSidebarWorkspace | null
  intent: DesktopSidebarIntent
}>

export type DesktopSidebarDestination = DesktopSidebarDestinationDefinition & Readonly<{
  selected: boolean
  accessibilityLabel: string
  accessibilityCurrent: "page" | undefined
  /** Typed visual state; no unread/status badge is invented without authority. */
  indicator: Readonly<{ kind: "current" }> | null
}>

const desktopSidebarDestinationCatalog = [
  {
    id: "workspace-new-chat",
    commandId: "chat.new",
    label: "New session",
    icon: "ChatCompose",
    workspace: null,
  },
  {
    id: "workspace-home",
    commandId: "workspace.home",
    label: "Project home",
    icon: "Home",
    workspace: "home",
  },
  {
    id: "shell-settings-toggle",
    commandId: "settings.open",
    label: "Settings",
    icon: "Settings",
    workspace: "settings",
  },
] as const satisfies ReadonlyArray<Omit<DesktopSidebarDestinationDefinition, "intent">>

const sidebarIntentFromCanonicalCommand = (commandId: DesktopCommandId): DesktopSidebarIntent => {
  const command = desktopCanonicalCommandRegistry.find(candidate => candidate.id === commandId)
  if (command === undefined) throw new Error(`Missing canonical Desktop command: ${commandId}`)
  if (command.intentName === "DesktopNewChat" && command.defaultArguments.kind === "none") {
    return { name: "DesktopNewChat", payload: null }
  }
  if (command.intentName === "DesktopSettingsToggled" && command.defaultArguments.kind === "none") {
    return { name: "DesktopSettingsToggled", payload: null }
  }
  if (command.intentName === "DesktopWorkspaceSelected" && command.defaultArguments.kind === "workspace" &&
    command.defaultArguments.workspace === "home") {
    return { name: "DesktopWorkspaceSelected", payload: command.defaultArguments.workspace }
  }
  throw new Error(`Canonical Desktop command ${commandId} is not an admitted sidebar intent`)
}

export const desktopSidebarDestinationDefinitions: ReadonlyArray<DesktopSidebarDestinationDefinition> =
  desktopSidebarDestinationCatalog.map(destination => ({
    ...destination,
    intent: sidebarIntentFromCanonicalCommand(destination.commandId),
  }))

const destinationAccessibilityLabel = (
  destination: DesktopSidebarDestinationDefinition,
  selected: boolean,
): string => destination.id === "shell-settings-toggle"
  ? selected ? "Close Settings" : "Open Settings"
  : destination.label

export const projectDesktopSidebarDestinations = (
  activeWorkspace: DesktopSidebarWorkspace,
  nestedSelectionActive = false,
): ReadonlyArray<DesktopSidebarDestination> =>
  desktopSidebarDestinationDefinitions.map((destination) => {
    const selected = destination.workspace !== null && destination.workspace === activeWorkspace &&
      !(activeWorkspace === "chat" && nestedSelectionActive)
    return {
      ...destination,
      selected,
      accessibilityLabel: destinationAccessibilityLabel(destination, selected),
      accessibilityCurrent: selected ? "page" : undefined,
      indicator: selected ? { kind: "current" } : null,
    }
  })
