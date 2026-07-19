import { describe, expect, test } from "vite-plus/test"

import type { DesktopCommandBindingProjection } from "../desktop-command-contract.ts"
import { desktopCommandShortcutMatches, type DesktopKeyboardShortcutEvent } from "./command-shortcuts.ts"

const event = (overrides: Partial<DesktopKeyboardShortcutEvent> = {}): DesktopKeyboardShortcutEvent => ({
  key: "e",
  metaKey: true,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  defaultPrevented: false,
  repeat: false,
  ...overrides,
})

const projection = (overrides: Partial<DesktopCommandBindingProjection["rows"][number]> = {}): DesktopCommandBindingProjection => ({
  schema: "openagents.desktop.command_bindings.v1",
  rows: [{
    commandId: "workspace.files",
    label: "Toggle Files",
    defaultBindings: ["Meta+E", "Control+E"],
    overrideBinding: null,
    effectiveBindings: ["Meta+E", "Control+E"],
    conflict: false,
    ...overrides,
  }],
  conflicts: [],
})

describe("Desktop effective command shortcuts", () => {
  test("selects the platform default without admitting the other primary modifier", () => {
    expect(desktopCommandShortcutMatches(projection(), "workspace.files", "darwin", event(), false)).toBe(true)
    expect(desktopCommandShortcutMatches(projection(), "workspace.files", "darwin", event({ metaKey: false, ctrlKey: true }), false)).toBe(false)
    expect(desktopCommandShortcutMatches(projection(), "workspace.files", "linux", event({ metaKey: false, ctrlKey: true }), false)).toBe(true)
  })

  test("honors an override and refuses the displaced default", () => {
    const overridden = projection({ overrideBinding: "Meta+J", effectiveBindings: ["Meta+J"] })
    expect(desktopCommandShortcutMatches(overridden, "workspace.files", "darwin", event(), false)).toBe(false)
    expect(desktopCommandShortcutMatches(overridden, "workspace.files", "darwin", event({ key: "j" }), false)).toBe(true)
  })

  test("refuses conflicts, editable targets, prevented events, and key repeat", () => {
    expect(desktopCommandShortcutMatches(projection({ conflict: true, effectiveBindings: [] }), "workspace.files", "darwin", event(), false)).toBe(false)
    expect(desktopCommandShortcutMatches(projection(), "workspace.files", "darwin", event(), true)).toBe(false)
    expect(desktopCommandShortcutMatches(projection(), "workspace.files", "darwin", event({ defaultPrevented: true }), false)).toBe(false)
    expect(desktopCommandShortcutMatches(projection(), "workspace.files", "darwin", event({ repeat: true }), false)).toBe(false)
  })
})
