/**
 * Main-process session usage ledger (#8712 Lane C). In-memory, per desktop
 * session, aggregated per (provider, accountRef). Fed by main.ts from the
 * typed Fable/Codex-child completion events; read through the snapshot
 * channel and pushed through the event channel (see usage-ledger-contract).
 *
 * This module never imports `electron` (unit-testable under `bun test`).
 */
import {
  emptyUsageLedgerSnapshot,
  type UsageLedgerProvider,
  type UsageLedgerRow,
  type UsageLedgerSnapshot,
  type UsageLedgerUsageInput,
} from "./usage-ledger-contract.ts"

export type UsageLedgerRecordInput = Readonly<{
  provider: UsageLedgerProvider
  accountRef: string
  /** Spawn-config truth for the lane (e.g. gpt-5.6-sol / claude-fable-5). */
  requestedModel: string | null
  kind: "turn" | "child"
  usage: UsageLedgerUsageInput | null
}>

export type UsageLedger = Readonly<{
  record: (input: UsageLedgerRecordInput) => void
  markReconnectRequired: (input: Readonly<{
    provider: UsageLedgerProvider
    accountRef: string
  }>) => void
  snapshot: () => UsageLedgerSnapshot
  subscribe: (listener: (snapshot: UsageLedgerSnapshot) => void) => () => void
  dispose: () => void
}>

const finite = (value: number): number =>
  Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0

export const makeUsageLedger = (now: () => Date = () => new Date()): UsageLedger => {
  const rows = new Map<string, UsageLedgerRow>()
  const listeners = new Set<(snapshot: UsageLedgerSnapshot) => void>()
  let disposed = false

  const keyFor = (provider: UsageLedgerProvider, accountRef: string): string =>
    `${provider}:${accountRef}`

  const emptyRow = (provider: UsageLedgerProvider, accountRef: string): UsageLedgerRow => ({
    accountRef,
    provider,
    requestedModel: null,
    turns: 0,
    children: 0,
    inputTokens: 0,
    cachedInputTokens: 0,
    outputTokens: 0,
    reasoningTokens: 0,
    totalTokens: 0,
    reconnectRequired: false,
    updatedAt: now().toISOString(),
  })

  const snapshot = (): UsageLedgerSnapshot => ({
    ...emptyUsageLedgerSnapshot(now().toISOString()),
    rows: [...rows.values()].sort((left, right) =>
      left.provider.localeCompare(right.provider) ||
      left.accountRef.localeCompare(right.accountRef)),
  })

  const publish = (): void => {
    if (disposed) return
    const current = snapshot()
    for (const listener of [...listeners]) listener(current)
  }

  const upsert = (
    provider: UsageLedgerProvider,
    accountRef: string,
    mutate: (row: UsageLedgerRow) => UsageLedgerRow,
  ): void => {
    if (disposed || accountRef.trim() === "") return
    const key = keyFor(provider, accountRef)
    const previous = rows.get(key) ?? emptyRow(provider, accountRef)
    rows.set(key, { ...mutate(previous), updatedAt: now().toISOString() })
    publish()
  }

  return {
    record: input =>
      upsert(input.provider, input.accountRef, row => ({
        ...row,
        requestedModel: input.requestedModel ?? row.requestedModel,
        turns: row.turns + (input.kind === "turn" ? 1 : 0),
        children: row.children + (input.kind === "child" ? 1 : 0),
        inputTokens: row.inputTokens + finite(input.usage?.inputTokens ?? 0),
        cachedInputTokens: row.cachedInputTokens + finite(input.usage?.cachedInputTokens ?? 0),
        outputTokens: row.outputTokens + finite(input.usage?.outputTokens ?? 0),
        reasoningTokens: row.reasoningTokens + finite(input.usage?.reasoningTokens ?? 0),
        totalTokens: row.totalTokens + finite(input.usage?.totalTokens ?? 0),
      })),
    markReconnectRequired: input =>
      upsert(input.provider, input.accountRef, row => ({ ...row, reconnectRequired: true })),
    snapshot,
    subscribe: listener => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    dispose: () => {
      disposed = true
      listeners.clear()
      rows.clear()
    },
  }
}
