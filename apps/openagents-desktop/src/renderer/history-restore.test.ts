import { describe, expect, test } from "bun:test"
import type { CodexHistoryCatalog } from "../codex-history-contract.ts"
import { restorableHistoryThreadRef } from "./history-restore.ts"

const agent = (threadRef: string, parentThreadRef: string | null) => ({
  threadRef,
  parentThreadRef,
  title: threadRef,
  status: "completed" as const,
  createdAt: "2026-07-11T00:00:00.000Z",
  updatedAt: "2026-07-11T00:00:00.000Z",
  depth: parentThreadRef === null ? 0 : 1,
  descendantCount: parentThreadRef === null ? 1 : 0,
  model: null,
  role: null,
  nickname: null,
  agentPath: null,
  sourceVersion: null,
  reasoning: null,
  source: "codex" as const,
})

describe("Desktop history restoration", () => {
  test("restores a child when its canonical root is in the visible catalog window", () => {
    const root = agent("root", null)
    const child = agent("child", "root")
    const catalog: CodexHistoryCatalog = { roots: [root], agents: [root, child] }
    expect(restorableHistoryThreadRef(catalog, "child", 40)).toBe("child")
  })

  test("refuses missing, orphaned, cyclic, and off-window selections", () => {
    const root = agent("root", null)
    const later = agent("later", null)
    const orphan = agent("orphan", "missing")
    const cycleA = agent("cycle-a", "cycle-b")
    const cycleB = agent("cycle-b", "cycle-a")
    const catalog: CodexHistoryCatalog = {
      roots: [root, later],
      agents: [root, later, orphan, cycleA, cycleB],
    }
    expect(restorableHistoryThreadRef(catalog, "missing", 40)).toBeNull()
    expect(restorableHistoryThreadRef(catalog, "orphan", 40)).toBeNull()
    expect(restorableHistoryThreadRef(catalog, "cycle-a", 40)).toBeNull()
    expect(restorableHistoryThreadRef(catalog, "later", 1)).toBeNull()
  })
})
