import { setTimeout as sleep } from "node:timers/promises"
import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openLocalTurnJournal } from "./local-turn-journal.ts"
import { makeLocalTurnTextPersistence } from "./local-turn-text-persistence.ts"
import { makeThreadStore } from "./thread-store.ts"

describe("local turn text persistence", () => {
  test("coalesces rapid deltas into one durable checkpoint and flushes the final value", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-turn-text-"))
    try {
      const store = makeThreadStore(path.join(root, "threads.json"))
      const thread = store.newThread()
      const journal = openLocalTurnJournal(path.join(root, "journal.json"))
      const key = { threadRef: thread.id, turnRef: "turn-1", lane: "codex-local" as const }
      journal.accept({ ...key, userMessageKey: "user-1", assistantMessageKey: "assistant-1" })
      const persistence = makeLocalTurnTextPersistence({
        journal,
        store,
        key,
        cadenceMs: 10,
        meta: () => ({ lane: "codex-local", turnRef: "turn-1" }),
      })

      persistence.append("one ")
      persistence.append("two")
      expect(journal.get(key)?.persistedCursor).toBe(0)
      await sleep(20)
      expect(journal.get(key)).toMatchObject({ assistantText: "one two", persistedCursor: 1 })
      persistence.boundary()
      persistence.append("three")
      persistence.complete("one twothree")
      expect(store.open(thread.id)?.notes).toMatchObject([
        { key: "assistant-1", text: "one two" },
        { key: "turn-1-assistant-1", text: "three" },
      ])
      persistence.dispose()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
