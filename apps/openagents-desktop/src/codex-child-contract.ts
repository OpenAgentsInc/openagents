/**
 * Codex sub-agent (child) contract (#8712 Lane C — Fable delegates to Codex).
 *
 * One bounded `codex exec --json` child per call, pinned to model
 * `gpt-5.6-sol` at reasoning effort `medium`. IMPORTANT LIMITATION (receipted
 * 2026-07-11 from codex-rs exec_events): the `codex exec --json` event stream
 * does NOT echo the effective model or reasoning effort back (there is no
 * `session_configured` event in exec JSON mode), so `requestedModel` /
 * `requestedEffort` in results and ledger rows are SPAWN-CONFIG TRUTH — the
 * exact `-m` / `-c model_reasoning_effort=` arguments this runtime passed —
 * not a provider echo. Per-axis model assertion on this surface is therefore
 * at spawn config; results must always be labeled that way.
 *
 * All summaries crossing out of this module are public-safe: bounded, with
 * the child scratch workspace path replaced by `<child-workspace>` and the
 * user's home prefix dropped. Failures are typed values, never throws.
 */

export const CODEX_CHILD_MODEL = "gpt-5.6-sol" as const
export const CODEX_CHILD_REASONING_EFFORT = "medium" as const
/**
 * `codex exec` has NO timeout flag (codex-cli 0.144.1, receipted): the bound
 * is host-side — a wall-clock timer that SIGTERMs the child.
 */
export const CODEX_CHILD_TIMEOUT_MS = 240_000
export const CODEX_CHILD_SUMMARY_LIMIT = 400
export const CODEX_CHILD_TEXT_LIMIT = 32_000

export type CodexChildFailureReason =
  /**
   * The selected account's refresh token is revoked (auth.json presence is
   * NOT credential validity — registry "ready" is presence-only). The runtime
   * rotates to the next registered Codex home on this reason; when every
   * registered account fails this way the child call fails typed with this
   * reason naming the reconnect need. Never silently skipped.
   */
  | "account_reconnect_required"
  /** No Codex account is registered in the pylon account registry at all. */
  | "no_codex_account"
  /** Host-side wall-clock bound reached; the child was SIGTERMed. */
  | "child_timeout"
  /** The child exited non-zero / errored without a revoked-credential marker,
   * or completed without any agent_message text. */
  | "child_failed"

export type CodexChildUsage = Readonly<{
  inputTokens: number
  cachedInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
  /**
   * Mirrors apps/pylon/src/codex-agent-executor.ts usage accounting:
   * total = input + output + reasoning (cached input is reported separately,
   * never double-counted into the total).
   */
  totalTokens: number
}>

export const codexChildUsageFromTurnCompleted = (value: unknown): CodexChildUsage | null => {
  if (value === null || typeof value !== "object") return null
  const usage = (value as { usage?: unknown }).usage
  if (usage === null || typeof usage !== "object") return null
  const finite = (candidate: unknown): number =>
    typeof candidate === "number" && Number.isFinite(candidate) && candidate > 0
      ? Math.trunc(candidate)
      : 0
  const record = usage as Record<string, unknown>
  const inputTokens = finite(record.input_tokens)
  const cachedInputTokens = finite(record.cached_input_tokens)
  const outputTokens = finite(record.output_tokens)
  const reasoningOutputTokens = finite(record.reasoning_output_tokens)
  return {
    inputTokens,
    cachedInputTokens,
    outputTokens,
    reasoningOutputTokens,
    totalTokens: inputTokens + outputTokens + reasoningOutputTokens,
  }
}

/**
 * Revoked-credential detection, receipted live 2026-07-11 against every
 * currently registered codex home: `codex exec` fails in ~4s with exit 1,
 * the error message "Your access token could not be refreshed because your
 * refresh token was revoked", and stderr markers `token_invalidated` /
 * `refresh_token_invalidated`. Any of those markers classifies the attempt
 * as `account_reconnect_required` (typed, rotatable) — never a generic
 * child failure.
 */
export const isCodexReconnectRequiredText = (text: string): boolean => {
  const lowered = text.toLowerCase()
  return lowered.includes("refresh_token_invalidated") ||
    lowered.includes("token_invalidated") ||
    lowered.includes("refresh token was revoked") ||
    (lowered.includes("access token could not be refreshed") && lowered.includes("revoked"))
}

/** Streamed to the caller while ONE child runs (already public-safe). */
export type CodexChildStreamEvent =
  | Readonly<{ kind: "attempt_started"; accountRef: string }>
  | Readonly<{ kind: "item"; itemType: string; summary: string }>
  | Readonly<{
      kind: "account_reconnect_required"
      accountRef: string
      detail: string
    }>

export type CodexChildSuccess = Readonly<{
  ok: true
  text: string
  usage: CodexChildUsage | null
  threadId: string | null
  accountRef: string
  /** Spawn-config truth (see module docstring), never a stream echo. */
  requestedModel: typeof CODEX_CHILD_MODEL
  requestedEffort: typeof CODEX_CHILD_REASONING_EFFORT
  durationMs: number
}>

export type CodexChildFailure = Readonly<{
  ok: false
  reason: CodexChildFailureReason
  detail: string
  accountRef: string | null
  durationMs: number
}>

export type CodexChildResult = CodexChildSuccess | CodexChildFailure

export type CodexChildRunInput = Readonly<{
  /** Caller-scoped child tag; also names the per-child scratch dir. */
  childRef: string
  task: string
  context?: string
  onEvent?: (event: CodexChildStreamEvent) => void
}>
