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

import { CODEX_CHILD_REASONING_EFFORT } from "./codex-child-contract.ts"
import {
  CodexModelSchema,
  CodexReasoningEffortSchema,
  FableLocalInterruptRequestSchema,
  FableLocalStartRequestSchema,
  LocalModelSchema,
  isCodexModel,
  type CodexModel,
  type CodexReasoningEffort,
  type FableLocalFailureReason,
} from "./fable-local-contract.ts"

export const CodexLocalAvailabilityChannel = "openagents:codex-local:availability" as const
export const CodexLocalStartChannel = "openagents:codex-local:start" as const
export const CodexLocalInterruptChannel = "openagents:codex-local:interrupt" as const
export const CodexLocalSteerTurnChannel = "openagents:codex-local:steer-turn" as const
export const CodexLocalQueueFollowupChannel = "openagents:codex-local:queue-followup" as const
export const CodexLocalQueueListChannel = "openagents:codex-local:queue-list" as const
export const CodexLocalQueueEditChannel = "openagents:codex-local:queue-edit" as const
export const CodexLocalQueueCancelChannel = "openagents:codex-local:queue-cancel" as const
export const CodexLocalEventChannel = "openagents:codex-local:event" as const
/**
 * Full Auto (#8853): main-owned durable per-thread toggle. Setting this is
 * what makes the loop survive a restart -- main persists the fact and
 * re-evaluates it at both turn completion and app startup, instead of the
 * renderer deciding to continue from in-memory state that a reload destroys.
 */
export const CodexLocalFullAutoSetChannel = "openagents:codex-local:full-auto:set" as const
export const CodexLocalFullAutoGetChannel = "openagents:codex-local:full-auto:get" as const
/**
 * FA-H4 (#8877): coarse, typed, main-owned in-flight state for background
 * (main-initiated) continuations. The ProductSpec deliberately cuts live
 * token streaming; this broadcast makes only "something is running right
 * now" (plus its terminal outcome) a rendered fact, carrying the running
 * turn ref so the renderer's stop control can target the ACTUAL background
 * turn instead of silently no-opping.
 */
export const CodexLocalFullAutoStateChannel = "openagents:codex-local:full-auto:state" as const
/** FA-H4 (#8877): thread-scoped interrupt for the background continuation
 * turn — the renderer never learned the background turnRef before the state
 * broadcast existed, so this channel resolves it main-side by threadRef. */
export const CodexLocalFullAutoInterruptChannel = "openagents:codex-local:full-auto:interrupt" as const
/** Exact installed Codex compatibility identity; thread handoff remains disabled
 * unless a separately verified official-app continuity proof cites this ref. */
export const CODEX_LOCAL_RUNTIME_COMPATIBILITY_REF = "codex.compat.0.144.1" as const

/**
 * Startup fallback only. Once the installed app-server answers `model/list`,
 * its visible default and catalog become the exact model authority. Delegate
 * children retain their separately pinned model.
 */
export const CODEX_LOCAL_MODEL = "gpt-5.6-sol" as const
export const CODEX_LOCAL_REASONING_EFFORT = CODEX_CHILD_REASONING_EFFORT

/** The renderer-facing model caption value ("(requested)" = spawn-config
 * truth, never a provider echo — the exec stream has none). */
export const codexLocalRequestedModelLabel = (model: string = CODEX_LOCAL_MODEL): string => `${model} (requested)`

/** Effective-model caption trace line for codex turns ("Codex · gpt-5.6-sol
 * (requested)") — the lane-branded sibling of fableLocalModelNoteText. */
export const codexLocalModelNoteText = (model: string): string => `Codex · ${model}`

export const CodexLocalModelOptionSchema = Schema.Struct({
  id: CodexModelSchema,
  displayName: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  isDefault: Schema.Boolean,
  defaultReasoningEffort: CodexReasoningEffortSchema,
  supportedReasoningEfforts: Schema.Array(CodexReasoningEffortSchema).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(6),
  ),
})
export type CodexLocalModelOption = typeof CodexLocalModelOptionSchema.Type

export const CodexLocalAvailabilitySchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    /** The health-ordered first VERIFIED account ref. */
    accountRef: Schema.String,
    /** How many registered accounts passed the session probe. */
    verifiedCount: Schema.Number,
    /** Main-owned visible `model/list` projection from the installed Codex. */
    models: Schema.optional(Schema.Array(CodexLocalModelOptionSchema).check(
      Schema.isMinLength(1),
      Schema.isMaxLength(16),
    )),
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    /**
     * `quota_exhausted` is a usage/credit budget. `rate_limited` is transient
     * throttling without a quota marker. Reconnect fixes neither, and the two
     * states keep their distinct owner actions.
     */
    reason: Schema.Literals(["no_codex_account", "no_verified_account", "policy_denied", "quota_exhausted", "rate_limited"]),
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    reason: Schema.Literal("invalid_config"),
    detail: Schema.String,
  }),
])
export type CodexLocalAvailability = typeof CodexLocalAvailabilitySchema.Type

export const CodexQueueMutationSchema = Schema.Struct({
  queueRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  expectedRevision: Schema.Number,
  message: Schema.optional(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(20_000))),
})
export const decodeCodexQueueMutation = (value: unknown): typeof CodexQueueMutationSchema.Type | null => {
  const decoded = Schema.decodeUnknownExit(CodexQueueMutationSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const decodeCodexLocalAvailability = (value: unknown): CodexLocalAvailability | null => {
  const decoded = Schema.decodeUnknownExit(CodexLocalAvailabilitySchema)(value)
  if (!Exit.isSuccess(decoded)) return null
  if (decoded.value.state !== "available" || decoded.value.models === undefined) return decoded.value
  const ids = decoded.value.models.map(model => model.id)
  if (new Set(ids).size !== ids.length) return null
  if (decoded.value.models.some(model =>
    new Set(model.supportedReasoningEfforts).size !== model.supportedReasoningEfforts.length ||
    !model.supportedReasoningEfforts.includes(model.defaultReasoningEffort))) return null
  return decoded.value
}

/** Start/interrupt/steer/queue requests reuse the frozen fable-local shapes. */
export const CodexLocalStartRequestSchema = FableLocalStartRequestSchema
export const CodexLocalInterruptRequestSchema = FableLocalInterruptRequestSchema

/** Full Auto (#8853) toggle set/get requests, bounded to one thread ref. */
export const CodexLocalFullAutoSetRequestSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  enabled: Schema.Boolean,
})
export type CodexLocalFullAutoSetRequest = typeof CodexLocalFullAutoSetRequestSchema.Type
export const decodeCodexLocalFullAutoSetRequest = (value: unknown): CodexLocalFullAutoSetRequest | null => {
  const decoded = Schema.decodeUnknownExit(CodexLocalFullAutoSetRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}
/**
 * FA-H6 (#8879): revalidate a durably-stored Full Auto execution profile
 * against the LIVE contract enums before a continuation replays it. The
 * registry stores plain bounded strings (so an enum change can never
 * corrupt-fail the whole registry file); this narrows each field back to the
 * typed value a `FableLocalStartRequest` accepts, dropping any field that no
 * longer decodes (that field then falls back to lane defaults, exactly the
 * pre-binding behavior).
 */
export const decodeCodexLocalContinuationProfile = (
  profile: Readonly<{ accountRef?: string; model?: string; reasoningEffort?: string }> | undefined,
  allowedModels?: ReadonlyArray<string>,
): Readonly<{ accountRef: string | null; model: CodexModel | null; reasoningEffort: CodexReasoningEffort | null }> => {
  const decodedModel = Schema.decodeUnknownExit(LocalModelSchema)(profile?.model)
  const decodedEffort = Schema.decodeUnknownExit(CodexReasoningEffortSchema)(profile?.reasoningEffort)
  return {
    accountRef: profile?.accountRef !== undefined && profile.accountRef.length >= 1 && profile.accountRef.length <= 80
      ? profile.accountRef
      : null,
    model: Exit.isSuccess(decodedModel) && isCodexModel(decodedModel.value) &&
        (allowedModels === undefined || allowedModels.includes(decodedModel.value))
      ? decodedModel.value
      : null,
    reasoningEffort: Exit.isSuccess(decodedEffort) ? decodedEffort.value : null,
  }
}

export const CodexLocalFullAutoGetRequestSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type CodexLocalFullAutoGetRequest = typeof CodexLocalFullAutoGetRequestSchema.Type
export const decodeCodexLocalFullAutoGetRequest = (value: unknown): CodexLocalFullAutoGetRequest | null => {
  const decoded = Schema.decodeUnknownExit(CodexLocalFullAutoGetRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/**
 * FA-H4 (#8877): the coarse per-thread live state a background continuation
 * moves through. `turn_running` carries the leased turn ref (interruptible);
 * terminal states keep the last outcome until the next transition. `blocked`
 * covers the typed disables (workspace_mismatch / workspace_unbound /
 * failure-limit) with `detail` carrying the blockedReason — never a new
 * silent-dormancy state.
 */
export const CodexLocalFullAutoLiveStateSchema = Schema.Literals([
  "idle",
  "turn_running",
  "turn_completed",
  "turn_failed",
  "cap_reached",
  "blocked",
])
export type CodexLocalFullAutoLiveState = typeof CodexLocalFullAutoLiveStateSchema.Type

/** Bounded to the registry's blockedReason limit; the contract keeps its own
 * literal so the renderer-safe module never imports the node:fs registry. */
export const CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT = 300

export const CodexLocalFullAutoStateSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  state: CodexLocalFullAutoLiveStateSchema,
  turnRef: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))),
  detail: Schema.optional(Schema.String.check(
    Schema.isMinLength(1),
    Schema.isMaxLength(CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT),
  )),
})
export type CodexLocalFullAutoState = typeof CodexLocalFullAutoStateSchema.Type
export const decodeCodexLocalFullAutoState = (value: unknown): CodexLocalFullAutoState | null => {
  const decoded = Schema.decodeUnknownExit(CodexLocalFullAutoStateSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

/** FA-H4 (#8877): { threadRef } — main resolves the running background turn
 * ref itself; the renderer never supplies a turn identity to interrupt. */
export const CodexLocalFullAutoInterruptRequestSchema = Schema.Struct({
  threadRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
})
export type CodexLocalFullAutoInterruptRequest = typeof CodexLocalFullAutoInterruptRequestSchema.Type
export const decodeCodexLocalFullAutoInterruptRequest = (
  value: unknown,
): CodexLocalFullAutoInterruptRequest | null => {
  const decoded = Schema.decodeUnknownExit(CodexLocalFullAutoInterruptRequestSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

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
export const CODEX_CHIP_REASON_QUOTA_EXHAUSTED =
  "Codex — usage quota exhausted · wait for reset or add credits"
export const CODEX_CHIP_REASON_POLICY_DENIED =
  "Codex — blocked by the active policy · review the task policy"
export const CODEX_CHIP_REASON_INVALID_CONFIG =
  "Codex — configuration error"

/**
 * Pure chip projection from typed availability (unit-tested lifecycle
 * evidence: boot-probe→verified→enabled; none verified→disabled with the
 * reconnect reason; probe still pending→disabled "verifying").
 */
export const codexHarnessLaneFromAvailability = (
  availability: CodexLocalAvailability | null,
): Readonly<{
  available: boolean
  reason: string | null
  diagnostic?: Readonly<{ kind: "invalid_config"; detail: string }>
}> => {
  if (availability === null) return { available: false, reason: CODEX_CHIP_REASON_VERIFYING }
  if (availability.state === "available") return { available: true, reason: null }
  if (availability.reason === "invalid_config") {
    return {
      available: false,
      reason: CODEX_CHIP_REASON_INVALID_CONFIG,
      diagnostic: { kind: "invalid_config", detail: availability.detail },
    }
  }
  return {
    available: false,
    reason: availability.reason === "policy_denied"
      ? CODEX_CHIP_REASON_POLICY_DENIED
      : availability.reason === "quota_exhausted"
        ? CODEX_CHIP_REASON_QUOTA_EXHAUSTED
        : availability.reason === "rate_limited"
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
    case "incompatible_workflow":
      return `The ProductSpec Codex workflow is incompatible${suffix}. No ambient skill or other lane was substituted.`
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
