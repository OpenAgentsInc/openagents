import { describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { makeThreadStore } from "./thread-store.ts"

describe("H2 local thread fork persistence", () => {
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
})
