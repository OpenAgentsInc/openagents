import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { makeThreadStore } from "./thread-store.ts"

describe("H2 local thread fork persistence", () => {
  test("persists immutable creation time and migrates a legacy row before later activity", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-created-order-"))
    const file = path.join(root, "threads.json")
    try {
      const legacyUpdatedAt = "2026-07-16T20:00:00.000Z"
      writeFileSync(file, JSON.stringify({
        version: 1,
        threads: [{ id: "legacy", title: "Legacy", updatedAt: legacyUpdatedAt, notes: [] }],
      }))
      const store = makeThreadStore(file)
      expect(store.list()[0]?.createdAt).toBe(legacyUpdatedAt)

      const updated = store.append("legacy", {
        key: "message-1",
        role: "user",
        text: "New activity",
        timestamp: "15:30",
      })
      expect(updated?.createdAt).toBe(legacyUpdatedAt)
      expect(updated?.updatedAt).not.toBe(legacyUpdatedAt)
      expect(makeThreadStore(file).open("legacy")?.createdAt).toBe(legacyUpdatedAt)

      const created = store.newThread()
      const forked = store.forkThread([{ key: "seed", role: "user", text: "Seed", timestamp: "15:31" }])
      expect(created.createdAt).toBe(created.updatedAt)
      expect(forked.createdAt).toBe(forked.updatedAt)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

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

  test("retains a restored older-created conversation by recent access", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-restore-lru-"))
    const file = path.join(root, "threads.json")
    try {
      writeFileSync(file, JSON.stringify({
        version: 1,
        threads: Array.from({ length: 5 }, (_, index) => ({
          id: `newer-${index}`,
          title: `Newer ${index}`,
          createdAt: `2026-07-16T2${index}:00:00.000Z`,
          updatedAt: `2026-07-16T2${index}:00:00.000Z`,
          notes: [],
        })),
      }))
      const store = makeThreadStore(file)
      store.restoreThread({
        id: "older-active",
        title: "Older active chat",
        createdAt: "2026-07-16T10:00:00.000Z",
        updatedAt: "2026-07-17T00:00:00.000Z",
        notes: [{ key: "assistant-1", role: "assistant", text: "Completed", timestamp: "19:00" }],
      })

      expect(store.open("older-active")?.createdAt).toBe("2026-07-16T10:00:00.000Z")
      expect(store.list()).toHaveLength(5)
      expect(store.open("newer-0")).toBeNull()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("does not evict an older-created chat between turn completion and continuation", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-active-lru-"))
    const file = path.join(root, "threads.json")
    try {
      writeFileSync(file, JSON.stringify({
        version: 1,
        threads: [
          {
            id: "older-active",
            title: "Fast Follow",
            createdAt: "2026-07-16T10:00:00.000Z",
            updatedAt: "2026-07-16T10:00:00.000Z",
            notes: [],
          },
          ...Array.from({ length: 4 }, (_, index) => ({
            id: `newer-${index}`,
            title: `Newer ${index}`,
            createdAt: `2026-07-16T2${index}:00:00.000Z`,
            updatedAt: `2026-07-16T2${index}:00:00.000Z`,
            notes: [],
          })),
        ],
      }))
      const store = makeThreadStore(file)
      expect(store.append("older-active", {
        key: "turn-1-assistant",
        role: "assistant",
        text: "First turn completed",
        timestamp: "23:59",
      })).not.toBeNull()

      store.newThread("Blank newer chat")

      expect(store.open("older-active")?.notes.at(-1)?.text).toBe("First turn completed")
      expect(store.list()).toHaveLength(5)
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("protects an in-flight Full Auto run across six ordinary chats and restart, then releases it at terminal", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-full-auto-protection-"))
    const file = path.join(root, "threads.json")
    const protectedIds = new Set(["full-auto-active"])
    try {
      writeFileSync(file, JSON.stringify({
        version: 1,
        threads: [{
          id: "full-auto-active",
          title: "Overnight run",
          createdAt: "2026-07-17T00:00:00.000Z",
          updatedAt: "2026-07-17T00:00:00.000Z",
          notes: [{ key: "turn-running", role: "system", text: "turn running", timestamp: "00:00" }],
        }],
      }))
      const store = makeThreadStore(file, { protectedThreadIds: () => protectedIds })

      // Exact escaped incident pressure: six ordinary chats arrive before
      // the run thread is touched again.
      for (let index = 0; index < 6; index += 1) store.newThread(`Ordinary ${index}`)

      expect(store.open("full-auto-active")?.notes[0]?.text).toBe("turn running")
      expect(store.list()).toHaveLength(6)
      expect(store.list().filter(thread => thread.id !== "full-auto-active")).toHaveLength(5)

      const reopened = makeThreadStore(file, { protectedThreadIds: () => protectedIds })
      expect(reopened.open("full-auto-active")).not.toBeNull()

      // Terminal settlement removes the id from the durable protection
      // authority. The next ordinary cache write can evict it normally.
      protectedIds.clear()
      reopened.newThread("After terminal")
      expect(reopened.open("full-auto-active")).toBeNull()
      expect(reopened.list()).toHaveLength(5)
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

  test("upsert derives and persists the first authored title without overwriting a rename", () => {
    const root = mkdtempSync(path.join(tmpdir(), "desktop-thread-title-"))
    const file = path.join(root, "threads.json")
    try {
      const store = makeThreadStore(file)
      const thread = store.newThread()
      const titled = store.upsert(thread.id, {
        key: "turn.1-user",
        role: "user",
        text: "  Diagnose   the sidebar title pipeline  ",
        timestamp: "10:00",
      })
      expect(titled?.title).toBe("Diagnose the sidebar title pipeline")
      expect(makeThreadStore(file).open(thread.id)?.title).toBe("Diagnose the sidebar title pipeline")

      expect(store.rename(thread.id, "Owner title")?.title).toBe("Owner title")
      expect(store.upsert(thread.id, {
        key: "turn.2-user",
        role: "user",
        text: "Do not replace the owner title",
        timestamp: "10:01",
      })?.title).toBe("Owner title")
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
