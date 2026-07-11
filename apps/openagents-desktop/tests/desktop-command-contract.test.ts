import { describe, expect, test } from "bun:test"

import {
  decodeDesktopDeferredCommand,
  desktopCanonicalCommandRegistry,
  desktopCommandIsAvailable,
  normalizeDesktopCommandChord,
  resolveDesktopCommandBindings,
} from "../src/desktop-command-contract"

describe("contract openagents_desktop.commands.canonical_registry.v1", () => {
  test("declares every command field once with unique ids and conflict-free defaults", () => {
    expect(new Set(desktopCanonicalCommandRegistry.map(value => value.id)).size).toBe(desktopCanonicalCommandRegistry.length)
    for (const command of desktopCanonicalCommandRegistry) {
      expect(command).toHaveProperty("scope")
      expect(command).toHaveProperty("availability")
      expect(command).toHaveProperty("authorization")
      expect(command).toHaveProperty("arguments")
      expect(command).toHaveProperty("defaultArguments")
      expect(command).toHaveProperty("result")
      expect(command).toHaveProperty("defaultBindings")
    }
    expect(resolveDesktopCommandBindings([]).conflicts).toEqual([])
  })

  test("normalizes aliases and removes every conflicted chord until the user recovers it", () => {
    expect(normalizeDesktopCommandChord("Ctrl+Shift+k")).toBe("Control+Shift+K")
    const conflicted = resolveDesktopCommandBindings([
      { commandId: "settings.open", chord: "Cmd+N" },
      { commandId: "unknown", chord: "Ctrl+X" },
      { commandId: "chat.new", chord: "not a chord" },
    ])
    expect(conflicted.conflicts).toEqual([{
      chord: "Meta+N",
      commandIds: ["chat.new", "settings.open"],
    }])
    expect(conflicted.bindings.some(value => value.chord === "Meta+N")).toBe(false)
    expect(resolveDesktopCommandBindings([]).bindings.some(value => value.chord === "Meta+N")).toBe(true)
  })

  test("decodes bounded deferred opens and fails closed on unauthorized or unready commands", () => {
    const deferred = decodeDesktopDeferredCommand({
      schema: "openagents.desktop.deferred_command.v1",
      requestRef: "command.fixture.1",
      commandId: "workspace.fleet",
      arguments: { kind: "workspace", workspace: "fleet" },
      source: "second_instance",
      delivery: "dispatch",
    })
    expect(deferred.commandId).toBe("workspace.fleet")
    expect(() => decodeDesktopDeferredCommand({ ...deferred, commandId: "shell.exec" })).toThrow()
    const fleet = desktopCanonicalCommandRegistry.find(value => value.id === "workspace.fleet")!
    expect(desktopCommandIsAvailable(fleet, { sessionReady: true, workspaceReady: true, verifiedOwner: false })).toBe(false)
    expect(desktopCommandIsAvailable(fleet, { sessionReady: true, workspaceReady: false, verifiedOwner: true })).toBe(true)
  })
})
