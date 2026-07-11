import { describe, expect, test } from "bun:test"
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openDesktopCommandBindingStore } from "../src/desktop-command-bindings"

describe("contract openagents_desktop.commands.private_binding_store.v1", () => {
  test("persists overrides privately, survives reopen, exposes conflicts, and resets recoverably", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-command-bindings-"))
    const filePath = path.join(root, "commands", "bindings.json")
    try {
      const first = openDesktopCommandBindingStore(filePath)
      expect(first.snapshot().conflicts).toEqual([])
      expect(first.save({ commandId: "settings.open", chord: "Meta+N" }).conflicts).toEqual([{
        chord: "Meta+N",
        commandIds: ["chat.new", "settings.open"],
      }])
      expect(statSync(filePath).mode & 0o777).toBe(0o600)
      expect(readFileSync(filePath, "utf8")).not.toContain("/Users/")

      const reopened = openDesktopCommandBindingStore(filePath)
      expect(reopened.snapshot().rows.find(value => value.commandId === "settings.open")).toMatchObject({
        overrideBinding: "Meta+N",
        effectiveBindings: [],
        conflict: true,
      })
      expect(reopened.save({ commandId: "settings.open", chord: "Meta+Shift+S" }).conflicts).toEqual([])
      expect(reopened.snapshot().rows.find(value => value.commandId === "settings.open")?.effectiveBindings).toEqual(["Meta+Shift+S"])
      expect(reopened.reset().rows.find(value => value.commandId === "settings.open")).toMatchObject({
        overrideBinding: null,
        effectiveBindings: ["Meta+,", "Control+,"],
        conflict: false,
      })
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
