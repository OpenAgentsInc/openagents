import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { reconcileLocalTurns } from "./local-turn-recovery.ts"
import { openLocalTurnJournal } from "./local-turn-journal.ts"
import { makeThreadStore } from "./thread-store.ts"

describe("local turn native-state recovery", () => {
  test.each(["completed", "running"] as const)("does not fabricate a continuation when app-server reports %s", async nativeState => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-native-recovery-"))
    try {
      const store = makeThreadStore(path.join(root, "threads.json"))
      const thread = store.newThread()
      const journal = openLocalTurnJournal(path.join(root, "turns.json"))
      const key = { threadRef: thread.id, turnRef: "turn-1", lane: "codex-local" as const }
      journal.accept({ ...key, userMessageKey: "turn-1-user", assistantMessageKey: "turn-1-assistant", accountRef: "codex-current", model: "gpt-5.6-sol" })
      journal.recordDispatch(key, "codex-current")
      journal.recordProviderSession(key, { accountRef: "codex-current", providerSessionRef: "provider-thread-1" })
      let syntheticRuns = 0
      const outcomes = await reconcileLocalTurns({
        journal,
        store,
        codex: { runTurn: async () => { syntheticRuns += 1; throw new Error("must not run") } },
        codexState: async threadId => { expect(threadId).toBe("provider-thread-1"); return nativeState },
      })
      expect(syntheticRuns).toBe(0)
      expect(outcomes).toEqual([{ key: expect.objectContaining(key), state: nativeState === "completed" ? "completed" : "interrupted" }])
      expect(journal.get(key)?.disposition).toBe(nativeState === "completed" ? "resumed_after_restart" : "interrupted_by_restart")
      expect(store.open(thread.id)?.notes.at(-1)?.text).toContain(nativeState === "completed" ? "confirmed" : "replay cursor")
    } finally { rmSync(root, { recursive: true, force: true }) }
  })
})
