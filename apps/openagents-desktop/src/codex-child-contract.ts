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
 * Owner-local danger profile (owner statement 2026-07-11, verbatim:
 * "disallowing bash is retarded, give them full tools full permissions
 * etc"): children run with full access, mirroring the repo's owner-local
 * executor invariant (Khala->Pylon runbook: sandbox danger-full-access,
 * approval never). This is an OWNER-LOCAL invariant, never a public wire
 * field, and never applies to untrusted labor/provider work.
 */
export const CODEX_CHILD_SANDBOX = "danger-full-access" as const
/**
 * `codex exec` has NO timeout flag (codex-cli 0.144.1, receipted): the bound
 * is host-side — a wall-clock timer that SIGTERMs the child.
 */
export const CODEX_CHILD_TIMEOUT_MS = 240_000
export const CODEX_CHILD_SUMMARY_LIMIT = 400
export const CODEX_CHILD_TEXT_LIMIT = 32_000

export type CodexChildFailureReason =
  /**
   * The selected account's credentials were rejected (auth.json presence is
   * NOT credential validity — registry "ready" is presence-only). The runtime
   * rotates to the next candidate Codex home on this reason; when every
   * registered account fails this way the child call fails typed with this
   * reason naming the reconnect need. Never silently skipped.
   */
  | "account_reconnect_required"
  /** No Codex account is registered in the pylon account registry at all. */
  | "no_codex_account"
  /** Host-side wall-clock bound reached; the child was SIGTERMed. */
  | "child_timeout"
  /**
   * A POST-content failure (the child already produced an agent_message or
   * consumed usage), or every candidate account was exhausted by pre-content
   * failures that were not all auth-class. Pre-content failures on a single
   * account rotate (typed `pre_content_failure_rotated` stream event) rather
   * than landing here directly — children are ephemeral, so pre-content
   * rotation loses nothing and is bounded by the registry size.
   */
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
 * Auth-class (reconnect-required) failure detection. BROADENED 2026-07-11
 * after a live miss: the first receipted variant was the LONG message "Your
 * access token could not be refreshed because your refresh token was
 * revoked" (+ stderr `token_invalidated` / `refresh_token_invalidated`), but
 * a live child then failed with the SHORT variant "Your access token could
 * not be refreshed. Please log out and sign in again." which carries NONE of
 * the original markers — so no rotation happened and the child burned on a
 * broken account while a known-good home sat idle. The classifier is now a
 * case-insensitive any-of over the marker list below; a match classifies the
 * attempt as `account_reconnect_required` (typed, rotatable, health-marked)
 * — never a generic child failure.
 */
export const CODEX_RECONNECT_MARKERS = [
  "refresh token",
  "access token could not be refreshed",
  "sign in again",
  "token_invalidated",
  "revoked",
  "401",
  "unauthorized",
] as const

export const isCodexReconnectRequiredText = (text: string): boolean => {
  const lowered = text.toLowerCase()
  return CODEX_RECONNECT_MARKERS.some(marker => lowered.includes(marker))
}

/**
 * Quota exhaustion and transient rate-limit classification (EP250 signature
 * corpus). Neither state means the credential is broken, but they remain
 * distinct because an exhausted usage/credit budget and temporary throttling
 * have different owner actions. Auth-class markers win over both.
 */
export const CODEX_QUOTA_EXHAUSTION_MARKERS = [
  "usage limit",
  "quota",
  "purchase more credits",
] as const

export const isCodexQuotaExhaustionText = (text: string): boolean => {
  if (isCodexReconnectRequiredText(text)) return false
  const lowered = text.toLowerCase()
  return CODEX_QUOTA_EXHAUSTION_MARKERS.some(marker => lowered.includes(marker))
}

export const CODEX_RATE_LIMIT_MARKERS = [
  "429",
  "rate limit",
  "rate-limit",
  "too many requests",
] as const

export const isCodexRateLimitText = (text: string): boolean => {
  if (isCodexReconnectRequiredText(text) || isCodexQuotaExhaustionText(text)) return false
  const lowered = text.toLowerCase()
  return CODEX_RATE_LIMIT_MARKERS.some(marker => lowered.includes(marker))
}

/** The three-way signature classification the corpus table asserts per row. */
export type CodexFailureClass = "auth" | "quota_exhausted" | "rate_limit" | "generic"

export const classifyCodexFailureText = (text: string): CodexFailureClass =>
  isCodexReconnectRequiredText(text)
    ? "auth"
    : isCodexQuotaExhaustionText(text)
      ? "quota_exhausted"
      : isCodexRateLimitText(text)
        ? "rate_limit"
        : "generic"

/** Streamed to the caller while ONE child runs (already public-safe). */
export type CodexChildStreamEvent =
  | Readonly<{ kind: "attempt_started"; accountRef: string }>
  | Readonly<{ kind: "item"; itemType: string; summary: string }>
  | Readonly<{
      kind: "account_reconnect_required"
      accountRef: string
      detail: string
    }>
  /**
   * A NON-auth pre-content failure (no agent_message completed, zero usage)
   * was rotated past — typed and visible, never silent. Auth-class failures
   * use `account_reconnect_required` above; this covers the rest (children
   * are ephemeral, so pre-content rotation is safe and loses nothing).
   */
  | Readonly<{
      kind: "pre_content_failure_rotated"
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
