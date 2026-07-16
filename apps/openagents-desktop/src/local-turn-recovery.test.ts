import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { localThreadRefForProviderSession, reconcileLocalTurns } from "./local-turn-recovery.ts"
import { openLocalTurnJournal } from "./local-turn-journal.ts"
import { makeThreadStore } from "./thread-store.ts"

describe("local turn native-state recovery", () => {
  test("maps a provider history id back to the latest Desktop-local owner", () => {
    const record = (threadRef: string, turnRef: string, updatedAt: string) => ({
      schema: "openagents.desktop.local_turn_record.v1" as const,
      threadRef,
      turnRef,
      lane: "codex-local",
      userMessageKey: `${turnRef}-user`,
      assistantMessageKey: `${turnRef}-assistant`,
      accountRef: "codex-current",
      providerSessionRef: "provider-thread-older",
      model: "gpt-5.6-sol",
      phase: "completed" as const,
      persistedCursor: 0,
      assistantText: "",
      assistantSegments: [],
      recoveryGeneration: 0,
      disposition: "completed" as const,
      createdAt: updatedAt,
      updatedAt,
    })
    expect(localThreadRefForProviderSession([
      record("desktop-local-old", "turn-1", "2026-07-15T10:00:00.000Z"),
      record("desktop-local-current", "turn-2", "2026-07-16T10:00:00.000Z"),
    ], "provider-thread-older")).toBe("desktop-local-current")
    expect(localThreadRefForProviderSession([], "provider-thread-older")).toBeNull()
  })

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
