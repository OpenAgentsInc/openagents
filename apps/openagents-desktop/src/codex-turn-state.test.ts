import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "vite-plus/test"
import type { CodexAppServerLease } from "./codex-app-server-supervisor.ts"
import { codexTurnEffectByMethod, makeCodexTurnState } from "./codex-turn-state.ts"

describe("generated Codex turn/item effects", () => {
  test("replays the complete generated notification and current item fixture corpus", () => {
    const notifications = JSON.parse(readFileSync(new URL("../../../packages/codex-app-server-protocol/fixtures/current-source-notifications.json", import.meta.url), "utf8")) as Record<string, unknown>
    const items = JSON.parse(readFileSync(new URL("../../../packages/codex-app-server-protocol/fixtures/current-source-thread-items.json", import.meta.url), "utf8")) as Array<Record<string, unknown>>
    const machine = makeCodexTurnState()
    for (const [method, params] of Object.entries(notifications)) machine.apply({ generation: 1, message: { method, params } })
    items.forEach((item, index) => {
      const value = { ...item, id: `fixture-item-${index}` }
      machine.apply({ generation: 1, message: { method: "item/started", params: { threadId: "fixture-thread", turnId: "fixture-turn", item: value } } })
      machine.apply({ generation: 1, message: { method: "item/completed", params: { threadId: "fixture-thread", turnId: "fixture-turn", item: value } } })
    })
    expect(items.every((_, index) => machine.snapshot().items[`fixture-item-${index}`] !== undefined)).toBe(true)
    expect(Object.keys(codexTurnEffectByMethod).length).toBeGreaterThan(60)
    expect(Object.keys(codexTurnEffectByMethod).every(method => method in notifications)).toBe(true)
  })

  test("classifies the generated notification inventory and terminal state wins every race", () => {
    expect(codexTurnEffectByMethod["item/agentMessage/delta"]).toBe("item_delta")
    expect(codexTurnEffectByMethod["item/commandExecution/outputDelta"]).toBe("item_delta")
    const machine = makeCodexTurnState()
    machine.bindStartedTurn("thread-1", "turn-1")
    machine.apply({ generation: 1, message: { method: "item/started", params: { threadId: "thread-1", turnId: "turn-1", item: { id: "item-1", type: "agentMessage", status: "inProgress" } } } })
    machine.apply({ generation: 1, message: { method: "item/agentMessage/delta", params: { threadId: "thread-1", turnId: "turn-1", itemId: "item-1", delta: "hi" } } })
    machine.apply({ generation: 1, message: { method: "item/completed", params: { threadId: "thread-1", turnId: "turn-1", item: { id: "item-1", type: "agentMessage", status: "completed", text: "hi" } } } })
    machine.apply({ generation: 1, message: { method: "turn/completed", params: { threadId: "thread-1", turn: { id: "turn-1", status: "completed" } } } })
    machine.apply({ generation: 1, message: { method: "turn/started", params: { threadId: "thread-1", turn: { id: "turn-1" } } } })
    expect(machine.snapshot()).toMatchObject({ activeTurnId: null, terminalTurnIds: ["turn-1"], items: { "item-1": { status: "completed", deltas: [{ method: "item/agentMessage/delta" }] } } })
  })

  test("steer is CAS-bound to an active regular turn and receipts contain hashes only", () => {
    const root = mkdtempSync(join(tmpdir(), "oa-steer-")); const path = join(root, "receipts.json")
    try {
      const machine = makeCodexTurnState({ receiptPath: path })
      machine.bindStartedTurn("thread-1", "turn-1")
      const accepted = machine.authorizeSteer("thread-1", "turn-1")
      expect(accepted.accepted).toBe(true)
      machine.settleSteer("thread-1", "turn-1", accepted.clientUserMessageId, true)
      expect(machine.authorizeSteer("thread-1", "wrong").accepted).toBe(false)
      expect(machine.admitInterrupt("thread-1", "turn-1")).toBe(true)
      expect(machine.snapshot().activeTurnId).toBe("turn-1")
      machine.apply({ generation: 1, message: { method: "turn/completed", params: { threadId: "thread-1", turn: { id: "turn-1", status: "interrupted" } } } })
      expect(machine.snapshot().activeTurnId).toBeNull()
      const disk = readFileSync(path, "utf8")
      expect(disk).not.toContain("thread-1"); expect(disk).not.toContain("turn-1"); expect(disk).not.toContain(accepted.clientUserMessageId)
    } finally { rmSync(root, { recursive: true, force: true }) }
  })

  test("supports inline and detached review identities", async () => {
    const machine = makeCodexTurnState()
    const lease = { request: async (_method: string, params: unknown) => (params as { delivery: string }).delivery === "detached" ? { turn: { id: "review-turn-2" }, reviewThreadId: "review-thread-2" } : { turn: { id: "review-turn-1" } } } as unknown as CodexAppServerLease
    expect(await machine.startReview(lease, { threadId: "thread-1", delivery: "inline", target: { type: "uncommittedChanges" } })).toEqual({ turnId: "review-turn-1", reviewThreadId: "thread-1" })
    expect(await machine.startReview(lease, { threadId: "thread-1", delivery: "detached", target: { type: "baseBranch", branch: "main" } })).toEqual({ turnId: "review-turn-2", reviewThreadId: "review-thread-2" })
    expect(machine.snapshot()).toMatchObject({ activeTurnId: "review-turn-2", activeKind: "review", threadId: "review-thread-2" })
  })
})
