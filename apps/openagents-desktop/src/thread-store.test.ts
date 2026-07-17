import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { makeThreadStore } from "./thread-store.ts"

describe("H2 local thread fork persistence", () => {
  test("restores a verified historical thread under its Desktop-local id", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-restore-"))
    try {
      const store = makeThreadStore(path.join(root, "threads.json"))
      const restored = store.restoreThread({
        id: "desktop-local-older",
        title: "Older chat",
        updatedAt: "2026-07-16T21:00:00.000Z",
        notes: [{ key: "assistant-1", role: "assistant", text: "Restored", timestamp: "16:00" }],
      })
      expect(restored.id).toBe("desktop-local-older")
      expect(store.open("desktop-local-older")).toEqual(restored)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("creates distinct seeded threads while leaving the seed and first fork unmutated", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-history-fork-"))
    try {
      const store = makeThreadStore(path.join(root, "threads.json"))
      const seed = [{ key: "source.1", role: "user" as const, text: "Investigate the parser", timestamp: "09:00" }]
      const first = store.forkThread(seed)
      const second = store.forkThread(seed)
      expect(first.id).not.toBe(second.id)
      expect(first.notes).toEqual(seed)
      expect(second.notes).toEqual(seed)
      expect(first.title).toBe("Fork · Investigate the parser")
      seed[0]!.text = "mutated caller copy"
      expect(store.open(first.id)?.notes[0]?.text).toBe("Investigate the parser")
      expect(store.open(second.id)?.notes[0]?.text).toBe("Investigate the parser")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("upsert replaces a keyed note in place without moving it", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-upsert-"))
    try {
      const store = makeThreadStore(path.join(root, "threads.json"))
      const thread = store.newThread()
      store.append(thread.id, { key: "a", role: "assistant", text: "before", timestamp: "10:00" })
      store.append(thread.id, { key: "tool", role: "system", text: "tool", timestamp: "10:01" })
      const updated = store.upsert(thread.id, { key: "a", role: "assistant", text: "after", timestamp: "10:00" })
      expect(updated?.notes.map(note => `${note.key}:${note.text}`)).toEqual(["a:after", "tool:tool"])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("renames a local thread durably and refuses blank titles without mutation", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-rename-"))
    const file = path.join(root, "threads.json")
    try {
      const store = makeThreadStore(file)
      const thread = store.newThread("Original title")
      const renamed = store.rename(thread.id, "  Release checklist  ")
      expect(renamed?.title).toBe("Release checklist")
      expect(makeThreadStore(file).open(thread.id)?.title).toBe("Release checklist")

      expect(store.rename(thread.id, "   ")).toBeNull()
      expect(makeThreadStore(file).open(thread.id)?.title).toBe("Release checklist")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("deterministic turn keys survive reopen without duplicate prompt or assistant rows", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-restart-"))
    const file = path.join(root, "private", "threads.json")
    try {
      const first = makeThreadStore(file)
      const thread = first.newThread()
      first.upsert(thread.id, { key: "turn.1-user", role: "user", text: "Do the work", timestamp: "10:00" })
      first.upsert(thread.id, { key: "turn.1-assistant", role: "assistant", text: "partial", timestamp: "10:01" })

      const second = makeThreadStore(file)
      second.upsert(thread.id, { key: "turn.1-user", role: "user", text: "Do the work", timestamp: "10:00" })
      second.upsert(thread.id, { key: "turn.1-assistant", role: "assistant", text: "partial continued", timestamp: "10:01" })
      expect(second.open(thread.id)?.notes).toEqual([
        { key: "turn.1-user", role: "user", text: "Do the work", timestamp: "10:00" },
        { key: "turn.1-assistant", role: "assistant", text: "partial continued", timestamp: "10:01" },
      ])
      expect(statSync(file).mode & 0o777).toBe(0o600)
      expect(statSync(path.dirname(file)).mode & 0o777).toBe(0o700)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
