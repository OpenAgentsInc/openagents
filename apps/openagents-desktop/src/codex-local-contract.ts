/**
 * Codex local lane contract (EP250 #8712 — "yeah i need codex and claude
 * both first class").
 *
 * In local (not-signed-in) mode the composer's "Codex" chip runs a REAL
 * `codex exec --json` turn on this machine against the pylon account
 * registry's isolated Codex homes — never the default `~/.codex`, never the
 * cloud gateway, never another provider (the no-silent-substitution law).
 *
 * The lane deliberately REUSES the frozen fable-local event envelope
 * (`FableLocalEventSchema` / `FableLocalEventEnvelopeSchema`): codex turns
 * stream the same typed kinds (text_delta, tool_use/tool_result, reasoning,
 * turn_completed, turn_failed …) over their OWN channels below, so the
 * existing renderer transcript cards render codex turns identically with no
 * new components. This was the smaller diff vs a parallel event schema.
 *
 * MODEL TRUTH: `codex exec --json` does NOT echo the effective model back
 * (no session_configured event in exec JSON mode — receipted in
 * ./codex-child-contract.ts). Every projected model string on this lane is
 * SPAWN-CONFIG TRUTH and is labeled "(requested)".
 *
 * CHIP EVIDENCE RULE (EP250): the Codex chip lights only on a PROBE-VERIFIED
 * account — a real minimal `codex exec` turn that produced content this
 * session (see ./codex-preflight.ts). Registry auth.json presence is NOT
 * validity (receipted: `codex login status` reports "Logged in" for a
 * revoked-token home).
 */
import { Exit, Schema } from "@effect-native/core/effect"

import { CODEX_CHILD_MODEL, CODEX_CHILD_REASONING_EFFORT } from "./codex-child-contract.ts"
import {
  FableLocalInterruptRequestSchema,
  FableLocalStartRequestSchema,
  type FableLocalFailureReason,
} from "./fable-local-contract.ts"

export const CodexLocalAvailabilityChannel = "openagents:codex-local:availability" as const
export const CodexLocalStartChannel = "openagents:codex-local:start" as const
export const CodexLocalInterruptChannel = "openagents:codex-local:interrupt" as const
export const CodexLocalSteerTurnChannel = "openagents:codex-local:steer-turn" as const
export const CodexLocalQueueFollowupChannel = "openagents:codex-local:queue-followup" as const
export const CodexLocalEventChannel = "openagents:codex-local:event" as const
/** Exact packaged Codex compatibility identity; thread handoff remains disabled
 * unless a separately verified official-app continuity proof cites this ref. */
export const CODEX_LOCAL_RUNTIME_COMPATIBILITY_REF = "codex.compat.0.144.1" as const

/** The lane's requested model/effort — spawn-config truth, shared with the
 * delegate children so "Codex" means ONE pinned model everywhere. */
export const CODEX_LOCAL_MODEL = CODEX_CHILD_MODEL
export const CODEX_LOCAL_REASONING_EFFORT = CODEX_CHILD_REASONING_EFFORT

/** The renderer-facing model caption value ("(requested)" = spawn-config
 * truth, never a provider echo — the exec stream has none). */
export const codexLocalRequestedModelLabel = (model: string = CODEX_LOCAL_MODEL): string => `${model} (requested)`

/** Effective-model caption trace line for codex turns ("Codex · gpt-5.6-sol
 * (requested)") — the lane-branded sibling of fableLocalModelNoteText. */
export const codexLocalModelNoteText = (model: string): string => `Codex · ${model}`

export const CodexLocalAvailabilitySchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    /** The health-ordered first VERIFIED account ref. */
    accountRef: Schema.String,
    /** How many registered accounts passed the session probe. */
    verifiedCount: Schema.Number,
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    /**
     * "rate_limited": zero verified accounts but at least one probe hit a
     * quota/429 (live verbatim 2026-07-11: "You've hit your usage limit.
     * … try again at 8:31 PM."). A reconnect will NOT fix quota, so the
     * chip reason must not send the owner to Settings for it.
     */
    reason: Schema.Literals(["no_codex_account", "no_verified_account", "rate_limited"]),
  }),
])
export type CodexLocalAvailability = typeof CodexLocalAvailabilitySchema.Type

export const decodeCodexLocalAvailability = (value: unknown): CodexLocalAvailability | null => {
  const decoded = Schema.decodeUnknownExit(CodexLocalAvailabilitySchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** Start/interrupt/steer/queue requests reuse the frozen fable-local shapes. */
export const CodexLocalStartRequestSchema = FableLocalStartRequestSchema
export const CodexLocalInterruptRequestSchema = FableLocalInterruptRequestSchema

/**
 * Composer chip reason strings (EP250 verified-evidence rule). The chrome
 * lane renders whatever reason the state carries (accessible label / hover
 * popover) — these are the single source for the codex chip's copy.
 */
export const CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT =
  "Codex — no verified account · Reconnect in Settings"
export const CODEX_CHIP_REASON_VERIFYING = "Codex — verifying accounts…"
/** Quota is not a credential problem: reconnecting will not fix it. */
export const CODEX_CHIP_REASON_RATE_LIMITED =
  "Codex — accounts rate-limited · retry later or connect another account"

/**
 * Pure chip projection from typed availability (unit-tested lifecycle
 * evidence: boot-probe→verified→enabled; none verified→disabled with the
 * reconnect reason; probe still pending→disabled "verifying").
 */
export const codexHarnessLaneFromAvailability = (
  availability: CodexLocalAvailability | null,
): Readonly<{ available: boolean; reason: string | null }> => {
  if (availability === null) return { available: false, reason: CODEX_CHIP_REASON_VERIFYING }
  if (availability.state === "available") return { available: true, reason: null }
  return {
    available: false,
    reason: availability.reason === "rate_limited"
      ? CODEX_CHIP_REASON_RATE_LIMITED
      : CODEX_CHIP_REASON_NO_VERIFIED_ACCOUNT,
  }
}

/**
 * Renderer-facing copy for a typed codex-local failure — lane-branded, no
 * provider text leaks, and ALWAYS states that no other lane was substituted.
 */
export const codexLocalFailureMessage = (
  reason: FableLocalFailureReason,
  detail: string,
): string => {
  const suffix = detail.trim() === "" ? "" : ` (${detail.trim()})`
  switch (reason) {
    case "no_codex_account":
      return "Codex is unavailable: no Codex account is registered on this machine. No message was routed to any other lane."
    case "account_reconnect_required":
      return `Codex is unavailable: every registered Codex account needs reconnect${suffix}. Reconnect in Settings — no message was routed to any other lane.`
    case "interrupted":
      return "The local Codex turn was interrupted."
    case "timeout":
      return "The local Codex turn timed out."
    case "budget_exceeded":
      return "The local Codex turn hit its turn budget before finishing."
    case "session_failed":
      return `The local Codex turn failed${suffix}. No message was routed to any other lane.`
    // These reasons belong to the fable lane and never originate here; the
    // switch stays exhaustive over the shared reason set.
    case "no_claude_account":
    case "sdk_unavailable":
    case "model_substituted":
      return `The local Codex turn failed${suffix}.`
  }
}
