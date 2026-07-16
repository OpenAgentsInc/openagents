import { iconNames } from "@effect-native/core"
import { describe, expect, test } from "vite-plus/test"
import { desktopCanonicalCommandRegistry } from "../desktop-command-contract.ts"

import {
  desktopSidebarDestinationDefinitions,
  projectDesktopSidebarDestinations,
} from "./sidebar-destinations.ts"

describe("Desktop sidebar destination projection", () => {
  test("keeps the exact admitted catalog order and closed icon identities", () => {
    expect(desktopSidebarDestinationDefinitions.map(({ id, label, icon }) => ({ id, label, icon }))).toEqual([
      { id: "workspace-new-chat", label: "New session", icon: "ChatCompose" },
      { id: "shell-settings-toggle", label: "Settings", icon: "Settings" },
    ])
    for (const destination of desktopSidebarDestinationDefinitions) {
      expect(iconNames).toContain(destination.icon)
    }
  })

  test("binds every row to its existing canonical command and typed intent", () => {
    expect(desktopSidebarDestinationDefinitions.map(({ commandId, intent }) => ({ commandId, intent }))).toEqual([
      { commandId: "chat.new", intent: { name: "DesktopNewChat", payload: null } },
      { commandId: "settings.open", intent: { name: "DesktopSettingsToggled", payload: null } },
    ])
    for (const destination of desktopSidebarDestinationDefinitions) {
      const command = desktopCanonicalCommandRegistry.find(candidate => candidate.id === destination.commandId)
      expect(command).toBeDefined()
      expect(destination.intent.name).toBe(command?.intentName)
      expect(destination.intent.payload).toEqual(
        command?.defaultArguments.kind === "workspace" ? command.defaultArguments.workspace : null,
      )
    }
  })

  test("keeps the primary new-session action unselected", () => {
    const destinations = projectDesktopSidebarDestinations("chat")
    expect(destinations.filter(destination => destination.selected)).toEqual([])
    expect(destinations.find(destination => destination.id === "workspace-new-chat")).toMatchObject({
      selected: false,
      accessibilityLabel: "New session",
      accessibilityCurrent: undefined,
      indicator: null,
    })
  })

  test("leaves destinations unselected when a conversation row owns selection", () => {
    const destinations = projectDesktopSidebarDestinations("chat", true)
    expect(destinations.filter(destination => destination.selected)).toEqual([])
  })

  test("uses truthful Settings toggle labels without selecting the primary action", () => {
    const open = projectDesktopSidebarDestinations("chat")
    expect(open.find(destination => destination.id === "shell-settings-toggle")).toMatchObject({
      selected: false,
      accessibilityLabel: "Open Settings",
    })

    const close = projectDesktopSidebarDestinations("settings")
    expect(close.find(destination => destination.id === "shell-settings-toggle")).toMatchObject({
      selected: true,
      accessibilityLabel: "Close Settings",
      accessibilityCurrent: "page",
      indicator: { kind: "current" },
    })
    expect(close.find(destination => destination.id === "workspace-new-chat")?.selected).toBe(false)
  })
})
