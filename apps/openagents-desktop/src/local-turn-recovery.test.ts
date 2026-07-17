import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { describe, expect, test } from "vite-plus/test"

import {
  filterLocallyOwnedCodexHistoryCatalog,
  filterLocallyOwnedCodexHistorySearch,
  localThreadRefForProviderSession,
  providerSessionRefsForLocalThreads,
  reconcileLocalTurns,
} from "./local-turn-recovery.ts"
import { openLocalTurnJournal } from "./local-turn-journal.ts"
import { makeThreadStore } from "./thread-store.ts"

const turnRecord = (
  threadRef: string,
  turnRef: string,
  updatedAt: string,
  providerSessionRef = "provider-thread-older",
) => ({
  schema: "openagents.desktop.local_turn_record.v1" as const,
  threadRef,
  turnRef,
  lane: "codex-local",
  userMessageKey: `${turnRef}-user`,
  assistantMessageKey: `${turnRef}-assistant`,
  accountRef: "codex-current",
  providerSessionRef,
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

describe("local turn native-state recovery", () => {
  test("maps a provider history id back to the latest Desktop-local owner", () => {
    expect(localThreadRefForProviderSession([
      turnRecord("desktop-local-old", "turn-1", "2026-07-15T10:00:00.000Z"),
      turnRecord("desktop-local-current", "turn-2", "2026-07-16T10:00:00.000Z"),
    ], "provider-thread-older")).toBe("desktop-local-current")
    expect(localThreadRefForProviderSession([], "provider-thread-older")).toBeNull()
  })

  test("collapses a provider-history duplicate while its Desktop-local owner is retained", () => {
    const records = [
      turnRecord("desktop-local-current", "turn-1", "2026-07-16T10:00:00.000Z", "provider-duplicate"),
      turnRecord("desktop-local-evicted", "turn-2", "2026-07-15T10:00:00.000Z", "provider-history-only"),
    ]
    const localThreadRefs = new Set(["desktop-local-current"])
    expect(providerSessionRefsForLocalThreads(records, localThreadRefs)).toEqual(new Set(["provider-duplicate"]))

    const agent = (threadRef: string, title: string) => ({
      threadRef,
      parentThreadRef: null,
      title,
      status: "completed" as const,
      createdAt: "2026-07-16T10:00:00.000Z",
      updatedAt: "2026-07-16T10:00:00.000Z",
      depth: 0,
      descendantCount: 0,
      model: null,
      role: null,
      nickname: null,
      agentPath: null,
      sourceVersion: null,
      reasoning: null,
      source: "codex" as const,
    })
    const duplicate = agent("provider-duplicate", "Untitled Codex chat")
    const historyOnly = agent("provider-history-only", "Older provider chat")
    const catalog = filterLocallyOwnedCodexHistoryCatalog(
      { roots: [duplicate, historyOnly], agents: [duplicate, historyOnly] },
      records,
      localThreadRefs,
    )
    expect(catalog.roots.map(root => root.threadRef)).toEqual(["provider-history-only"])
    // The full agent graph remains loss-accounted; only the duplicate
    // top-level navigation projection is collapsed into its local owner.
    expect(catalog.agents).toHaveLength(2)

    const result = (threadRef: string, title: string) => ({
      threadRef,
      rootThreadRef: threadRef,
      source: "codex" as const,
      title,
      matchKind: "title" as const,
      matchItemRef: null,
      matchSequence: null,
      snippet: title,
      updatedAt: "2026-07-16T10:00:00.000Z",
      score: 1,
    })
    const search = filterLocallyOwnedCodexHistorySearch({
      query: "chat",
      results: [result("provider-duplicate", "Untitled Codex chat"), result("provider-history-only", "Older provider chat")],
      indexedSessions: 2,
      truncated: false,
    }, records, localThreadRefs)
    expect(search.results.map(row => row.threadRef)).toEqual(["provider-history-only"])
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
