import { describe, expect, test } from "vite-plus/test"

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
    expect(desktopCanonicalCommandRegistry.map(value => value.id)).toEqual(expect.arrayContaining([
      "chat.new",
      "chat.open",
      "navigation.back",
      "navigation.forward",
      "palette.toggle",
      "settings.open",
      "workspace.choose",
      "workspace.files",
      "workspace.home",
      "workspace.review",
    ]))
    expect(desktopCanonicalCommandRegistry.map(value => value.id)).not.toEqual(expect.arrayContaining([
      "workspace.fleet",
      "workspace.inbox",
      "workspace.terminal",
    ]))
    expect(desktopCanonicalCommandRegistry.filter(value => value.id.startsWith("navigation.")).map(value => ({
      id: value.id,
      bindings: value.defaultBindings,
      palette: value.palette,
    }))).toEqual([
      { id: "navigation.back", bindings: [], palette: false },
      { id: "navigation.forward", bindings: [], palette: false },
    ])
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
      commandId: "workspace.review",
      arguments: { kind: "workspace", workspace: "review" },
      source: "second_instance",
      delivery: "dispatch",
    })
    expect(deferred.commandId).toBe("workspace.review")
    expect(() => decodeDesktopDeferredCommand({ ...deferred, commandId: "shell.exec" })).toThrow()
    expect(() => decodeDesktopDeferredCommand({
      ...deferred,
      commandId: "workspace.fleet",
      arguments: { kind: "workspace", workspace: "fleet" },
    })).toThrow()
    const review = desktopCanonicalCommandRegistry.find(value => value.id === "workspace.review")!
    expect(desktopCommandIsAvailable(review, { sessionReady: true, workspaceReady: false, verifiedOwner: true })).toBe(false)
    expect(desktopCommandIsAvailable(review, { sessionReady: false, workspaceReady: true, verifiedOwner: false })).toBe(true)
  })
})
