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
      { id: "workspace-chat", label: "Chat", icon: "Chats" },
      { id: "workspace-home", label: "Project home", icon: "Home" },
      { id: "shell-settings-toggle", label: "Settings", icon: "Settings" },
    ])
    for (const destination of desktopSidebarDestinationDefinitions) {
      expect(iconNames).toContain(destination.icon)
    }
  })

  test("binds every row to its existing canonical command and typed intent", () => {
    expect(desktopSidebarDestinationDefinitions.map(({ commandId, intent }) => ({ commandId, intent }))).toEqual([
      { commandId: "chat.new", intent: { name: "DesktopNewChat", payload: null } },
      { commandId: "chat.open", intent: { name: "DesktopWorkspaceSelected", payload: "chat" } },
      { commandId: "workspace.home", intent: { name: "DesktopWorkspaceSelected", payload: "home" } },
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

  test("projects one selected destination and matching accessibility state", () => {
    const destinations = projectDesktopSidebarDestinations("home")
    expect(destinations.filter(destination => destination.selected).map(destination => destination.id)).toEqual([
      "workspace-home",
    ])
    expect(destinations.find(destination => destination.id === "workspace-home")).toMatchObject({
      accessibilityLabel: "Project home",
      accessibilityCurrent: "page",
    })
    expect(destinations.find(destination => destination.id === "workspace-new-chat")).toMatchObject({
      selected: false,
      accessibilityLabel: "New session",
      accessibilityCurrent: undefined,
      indicator: null,
    })
  })

  test("does not paint Chat active when a nested conversation is the active row", () => {
    const destinations = projectDesktopSidebarDestinations("chat", true)
    expect(destinations.filter(destination => destination.selected)).toEqual([])
    expect(destinations.find(destination => destination.id === "workspace-chat")).toMatchObject({
      selected: false,
      accessibilityCurrent: undefined,
      indicator: null,
    })
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
