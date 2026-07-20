/**
 * Session usage ledger contract (#8712 Lane C).
 *
 * Main owns an in-memory per-session token ledger fed from (a) local Claude
 * turn completions (exact SDK result usage) and (b) Codex sub-agent child
 * completions (exact `turn.completed` usage from `codex exec --json`). The
 * renderer reads it two ways, following the provider-accounts pattern:
 *
 * - `openagents:usage-ledger:snapshot` (invoke, no args): the full typed
 *   snapshot, aggregated per account.
 * - `openagents:usage-ledger:event` (webContents.send): the same snapshot
 *   pushed whenever the ledger changes.
 *
 * Evidence labeling: these numbers are the SESSION LEDGER (what this desktop
 * process itself dispatched and measured), distinct from the Fleet view's
 * per-account "probe" numbers (a pylon `accounts usage --refresh` provider
 * call). The two are never merged into one unlabeled figure.
 *
 * Codex rows carry `requestedModel`/`requestedEffort` as SPAWN-CONFIG TRUTH:
 * `codex exec --json` does not echo model/effort back, so the ledger records
 * exactly what was requested at spawn (see codex-child-contract.ts).
 *
 * `reconnectRequired` on a row is PROBE/CHILD EVIDENCE superseding the
 * registry's presence-based "ready": it flips when a spawned child failed
 * with a revoked refresh token for that account, and only an owner reconnect
 * (a fresh registry credential) clears it for a new session.
 */
import { Exit, Schema } from "@effect-native/core/effect"

export const UsageLedgerSnapshotChannel = "openagents:usage-ledger:snapshot" as const
export const UsageLedgerEventChannel = "openagents:usage-ledger:event" as const

export const usageLedgerProviders = ["claude_agent", "codex"] as const
export type UsageLedgerProvider = (typeof usageLedgerProviders)[number]

export const UsageLedgerRowSchema = Schema.Struct({
  accountRef: Schema.String,
  provider: Schema.Literals(usageLedgerProviders),
  /** Spawn-config truth (never a stream echo); null when unknown. */
  requestedModel: Schema.NullOr(Schema.String),
  /** Completed top-level Claude turns attributed to this account. */
  turns: Schema.Number,
  /** Completed Codex sub-agent children attributed to this account. */
  children: Schema.Number,
  inputTokens: Schema.Number,
  cachedInputTokens: Schema.Number,
  outputTokens: Schema.Number,
  reasoningTokens: Schema.Number,
  totalTokens: Schema.Number,
  /** Typed reconnect evidence superseding presence-based "ready". */
  reconnectRequired: Schema.Boolean,
  updatedAt: Schema.String,
})
export type UsageLedgerRow = typeof UsageLedgerRowSchema.Type

export const UsageLedgerSnapshotSchema = Schema.Struct({
  ok: Schema.Literal(true),
  generatedAt: Schema.String,
  /** Evidence label rendered next to every projected number. */
  evidence: Schema.Literal("session ledger"),
  rows: Schema.Array(UsageLedgerRowSchema),
})
export type UsageLedgerSnapshot = typeof UsageLedgerSnapshotSchema.Type

export const emptyUsageLedgerSnapshot = (generatedAt: string): UsageLedgerSnapshot => ({
  ok: true,
  generatedAt,
  evidence: "session ledger",
  rows: [],
})

export const decodeUsageLedgerSnapshot = (value: unknown): UsageLedgerSnapshot | null => {
  const decoded = Schema.decodeUnknownExit(UsageLedgerSnapshotSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** Exact usage split recorded per completion (all fields already truncated). */
export type UsageLedgerUsageInput = Readonly<{
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningTokens: number
  totalTokens: number
}>
