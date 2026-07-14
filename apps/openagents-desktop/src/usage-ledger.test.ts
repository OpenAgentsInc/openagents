/**
 * Session usage ledger tests (#8712 Lane C): exact per-account accumulation
 * for Fable turns and Codex children, the typed reconnect flag (probe/child
 * evidence superseding presence-based "ready"), deterministic snapshot
 * order, push notification, and disposal.
 */
import { describe, expect, test } from "vite-plus/test"

import { decodeUsageLedgerSnapshot } from "./usage-ledger-contract.ts"
import { makeUsageLedger } from "./usage-ledger.ts"

const fixedNow = () => new Date("2026-07-11T12:00:00.000Z")

describe("makeUsageLedger", () => {
  test("accumulates exact turn and child usage per (provider, account)", () => {
    const ledger = makeUsageLedger(fixedNow)
    ledger.record({
      provider: "claude_agent",
      accountRef: "claude-pylon-b",
      requestedModel: "claude-fable-5",
      kind: "turn",
      usage: { inputTokens: 10, cachedInputTokens: 2, outputTokens: 5, reasoningTokens: 0, totalTokens: 17 },
    })
    ledger.record({
      provider: "claude_agent",
      accountRef: "claude-pylon-b",
      requestedModel: "claude-fable-5",
      kind: "turn",
      usage: { inputTokens: 3, cachedInputTokens: 0, outputTokens: 4, reasoningTokens: 0, totalTokens: 7 },
    })
    ledger.record({
      provider: "codex",
      accountRef: "codex-2",
      requestedModel: "gpt-5.6-sol",
      kind: "child",
      usage: { inputTokens: 1200, cachedInputTokens: 900, outputTokens: 180, reasoningTokens: 60, totalTokens: 1440 },
    })
    const snapshot = ledger.snapshot()
    expect(snapshot.evidence).toBe("session ledger")
    expect(snapshot.rows).toEqual([
      {
        accountRef: "claude-pylon-b",
        provider: "claude_agent",
        requestedModel: "claude-fable-5",
        turns: 2,
        children: 0,
        inputTokens: 13,
        cachedInputTokens: 2,
        outputTokens: 9,
        reasoningTokens: 0,
        totalTokens: 24,
        reconnectRequired: false,
        updatedAt: "2026-07-11T12:00:00.000Z",
      },
      {
        accountRef: "codex-2",
        provider: "codex",
        requestedModel: "gpt-5.6-sol",
        turns: 0,
        children: 1,
        inputTokens: 1200,
        cachedInputTokens: 900,
        outputTokens: 180,
        reasoningTokens: 60,
        totalTokens: 1440,
        reconnectRequired: false,
        updatedAt: "2026-07-11T12:00:00.000Z",
      },
    ])
    // The wire snapshot decodes against its own schema (both sides agree).
    expect(decodeUsageLedgerSnapshot(JSON.parse(JSON.stringify(snapshot)))).toEqual(snapshot)
  })

  test("markReconnectRequired creates or flags the row without inventing usage", () => {
    const ledger = makeUsageLedger(fixedNow)
    ledger.markReconnectRequired({ provider: "codex", accountRef: "codex" })
    const row = ledger.snapshot().rows[0]!
    expect(row.accountRef).toBe("codex")
    expect(row.reconnectRequired).toBe(true)
    expect(row.totalTokens).toBe(0)
    expect(row.turns).toBe(0)
    expect(row.children).toBe(0)
    // Later real usage on the same account keeps the reconnect flag.
    ledger.record({
      provider: "codex",
      accountRef: "codex",
      requestedModel: "gpt-5.6-sol",
      kind: "child",
      usage: { inputTokens: 1, cachedInputTokens: 0, outputTokens: 1, reasoningTokens: 0, totalTokens: 2 },
    })
    expect(ledger.snapshot().rows[0]!.reconnectRequired).toBe(true)
    expect(ledger.snapshot().rows[0]!.totalTokens).toBe(2)
  })

  test("total-only recording is accepted (split unavailable, stated as zeros)", () => {
    const ledger = makeUsageLedger(fixedNow)
    ledger.record({
      provider: "claude_agent",
      accountRef: "claude-pylon-b",
      requestedModel: "claude-fable-5",
      kind: "turn",
      usage: { inputTokens: 0, cachedInputTokens: 0, outputTokens: 0, reasoningTokens: 0, totalTokens: 49 },
    })
    ledger.record({
      provider: "claude_agent",
      accountRef: "claude-pylon-b",
      requestedModel: "claude-fable-5",
      kind: "turn",
      usage: null,
    })
    const row = ledger.snapshot().rows[0]!
    expect(row.turns).toBe(2)
    expect(row.totalTokens).toBe(49)
  })

  test("subscribe pushes a fresh snapshot on every change; unsubscribe and dispose stop it", () => {
    const ledger = makeUsageLedger(fixedNow)
    const seen: number[] = []
    const unsubscribe = ledger.subscribe(snapshot => seen.push(snapshot.rows.length))
    ledger.markReconnectRequired({ provider: "codex", accountRef: "codex" })
    expect(seen).toEqual([1])
    unsubscribe()
    ledger.markReconnectRequired({ provider: "codex", accountRef: "codex-2" })
    expect(seen).toEqual([1])
    ledger.subscribe(snapshot => seen.push(snapshot.rows.length))
    ledger.dispose()
    ledger.record({
      provider: "codex",
      accountRef: "codex-3",
      requestedModel: "gpt-5.6-sol",
      kind: "child",
      usage: null,
    })
    expect(seen).toEqual([1])
    expect(ledger.snapshot().rows).toEqual([])
  })

  test("blank account refs are refused (no anonymous rows)", () => {
    const ledger = makeUsageLedger(fixedNow)
    ledger.record({ provider: "codex", accountRef: "  ", requestedModel: null, kind: "child", usage: null })
    expect(ledger.snapshot().rows).toEqual([])
  })
})
