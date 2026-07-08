// Per-user model configuration (MM-F1, #8484, epic #8467 mobile-only MVP).
//
// THE GAP this closes: the model catalog + supply lanes exist (Fireworks,
// Hydralisk, OpenRouter, Vertex Claude, Vertex Gemini — model-catalog.ts,
// pricing.ts, model-serving-policy.ts), but WHICH lane backs a request is an
// OPERATOR knob (`KHALA_BACKING_MODEL`, `resolveKhalaBackingModel`), not a
// per-user choice. This module is the server-side store + pure resolution
// logic for "the user picked model X"; `/api/mobile/model-preference` (GET
// read, PUT write — both mobile-bearer-authorized) is the route surface.
//
// SCOPE NOTE (read before wiring a consumer): this module intentionally does
// NOT alter `chat-completions-routes.ts`'s public gateway behavior. That
// route enforces a documented, deliberate invariant — "Public model selection
// intentionally collapses to [the single Khala virtual model id]... must not
// be exposed as public choices" (see `DEFAULT_CHAT_MODEL` / `isKhalaModel`
// there) — so a non-Khala `model` value from an ordinary API caller is
// rejected today unless it resolves through the fine-tuned-model seam. Mobile
// per-user model choice ("Gemini or our coding-agent pool") is a NEW,
// intentionally more expansive capability the mobile-only MVP pivot adds; it
// should be threaded through a privileged, mobile/coding-turn-specific
// dispatch path (the org-cloud coding executor, #8473/#8474, Lane 0), not by
// silently relaxing the public gateway's existing collapse-to-Khala
// enforcement. This module exposes the read/write store and a pure
// `resolveModelPreference` decision; the ORG-CLOUD EXECUTOR LANE SELECTION
// is the consumer that honors it for coding turns (posted as a comment on
// #8473 once this lands — see the epic's dependency note).
//
// AVAILABILITY. "Available" means the model's supply lane is actually armed
// in this Worker's env (`resolveSupplyLaneArming`), not merely present in the
// static pricing table — so a picker never offers (and a write never
// persists) a model this deployment cannot actually serve.
//
// TYPED FALLBACK, NEVER SILENT. `resolveModelPreference` always reports which
// case applied (`none` / `no_preference_set` / `preference_unavailable` /
// `default_unavailable`) alongside the id it actually resolved to (or `null`
// when nothing is servable) — a caller can always tell whether it is running
// the user's real choice, a default, or nothing at all.

import {
  KHALA_MODEL_ID,
  MODEL_PRICING_TABLE,
  normalizeKhalaModelId,
  type SupplyLane,
} from './pricing'
import type { SupplyLaneArming } from './model-serving-policy'

// The bare `gemini` alias (see model-router.ts's `isGeminiModel`/`ModelClass`
// and inference-free-allowance.ts's `FREE_ELIGIBLE_MODEL_IDS`) is the
// canonical, already-understood routing id for the first-party Vertex Gemini
// lane — distinct from the specific dated catalog row (`gemini-3.5-flash`).
// It is the default preference per the issue ("Gemini is the default for the
// coding lane").
export const DEFAULT_MODEL_PREFERENCE_ID = 'gemini' as const
export const DEFAULT_EXECUTION_TARGET_ID = DEFAULT_MODEL_PREFERENCE_ID
export const AUTO_EXECUTION_TARGET_ID = 'auto' as const
export const KHALA_EXECUTION_TARGET_ID = 'khala' as const

// ----------------------------------------------------------------------------
// Availability: only lanes actually armed in THIS deployment
// ----------------------------------------------------------------------------

const isLaneArmedForModel = (
  modelId: string,
  lane: SupplyLane,
  arming: SupplyLaneArming,
): boolean => {
  if (lane !== 'hydralisk') {
    return arming[lane] === true
  }
  const hydraliskModels = arming.hydraliskModels
  if (hydraliskModels === undefined) {
    return arming.hydralisk === true
  }
  // Raw Hydralisk model ids (GLM-5.2 REAP, GPT-OSS 20B/120B) check their OWN
  // specific armed flag; everything else on the hydralisk lane (e.g. the
  // Khala alias, which routes over whichever Hydralisk backing is live) uses
  // the blanket lane boolean — the SAME derivation
  // `resolveSupplyLaneArming` itself uses for `arming.hydralisk`.
  const specific = hydraliskModels[modelId as keyof typeof hydraliskModels]
  return specific ?? arming.hydralisk === true
}

// The model ids this deployment can actually serve right now, derived from
// the pricing table filtered to armed lanes, plus the `gemini` alias when the
// Vertex Gemini lane is armed (its own catalog row is the dated
// `gemini-3.5-flash` id; exposing the stable alias avoids the picker/store
// needing to track dated ids as the underlying Gemini version changes).
export const resolveAvailableModelIds = (
  arming: SupplyLaneArming,
): ReadonlyArray<string> => {
  const ids = new Set<string>()
  if (arming['vertex-gemini'] === true) {
    ids.add(DEFAULT_MODEL_PREFERENCE_ID)
  }
  for (const row of MODEL_PRICING_TABLE) {
    if (row.lane === 'vertex-gemini') {
      // Covered by the stable `gemini` alias above; avoid offering both the
      // alias and the dated catalog id for the same lane.
      continue
    }
    if (isLaneArmedForModel(row.model, row.lane, arming)) {
      ids.add(row.model)
    }
  }
  return [...ids]
}

// Normalize a caller-supplied model id the same way the rest of the gateway
// does (the `khala` slug collapses to the canonical `openagents/khala`
// id; everything else lowercases/trims). Bounded string normalization, not an
// intent parser.
export const normalizeModelPreferenceId = (modelId: string): string =>
  normalizeKhalaModelId(modelId)

export const normalizeExecutionTargetId = (targetId: string): string => {
  const normalized = targetId.trim()
  if (normalized.toLowerCase().startsWith('codex:')) {
    return `codex:${normalized.slice('codex:'.length)}`
  }
  if (normalized.toLowerCase().startsWith('claude:')) {
    return `claude:${normalized.slice('claude:'.length)}`
  }
  return normalizeModelPreferenceId(normalized)
}

export const isModelIdAvailable = (
  modelId: string,
  availableModelIds: ReadonlyArray<string>,
): boolean => {
  const normalized = normalizeModelPreferenceId(modelId)
  return availableModelIds.some(
    id => normalizeModelPreferenceId(id) === normalized,
  )
}

export const isCodexExecutionTargetId = (targetId: string): boolean =>
  /^codex:[A-Za-z0-9_.:-]{3,128}$/.test(normalizeExecutionTargetId(targetId))

export const isClaudeExecutionTargetId = (targetId: string): boolean =>
  /^claude:[A-Za-z0-9_.:-]{3,128}$/.test(normalizeExecutionTargetId(targetId))

export const resolveAvailableExecutionTargetIds = (
  input: Readonly<{
    availableModelIds: ReadonlyArray<string>
    codexAccountRefHashes?: ReadonlyArray<string>
    claudeAccountRefHashes?: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => {
  const ids = new Set<string>()
  if (isModelIdAvailable(DEFAULT_EXECUTION_TARGET_ID, input.availableModelIds)) {
    ids.add(DEFAULT_EXECUTION_TARGET_ID)
    ids.add(AUTO_EXECUTION_TARGET_ID)
  }
  if (isModelIdAvailable(KHALA_MODEL_ID, input.availableModelIds)) {
    ids.add(KHALA_EXECUTION_TARGET_ID)
  }
  for (const accountRefHash of input.codexAccountRefHashes ?? []) {
    const trimmed = accountRefHash.trim()
    if (trimmed !== '') ids.add(`codex:${trimmed}`)
  }
  for (const accountRefHash of input.claudeAccountRefHashes ?? []) {
    const trimmed = accountRefHash.trim()
    if (trimmed !== '') ids.add(`claude:${trimmed}`)
  }
  return [...ids]
}

export const isExecutionTargetIdAvailable = (
  targetId: string,
  availableTargetIds: ReadonlyArray<string>,
): boolean => {
  const normalized = normalizeExecutionTargetId(targetId)
  if (isCodexExecutionTargetId(normalized)) {
    return availableTargetIds.some(id => normalizeExecutionTargetId(id) === normalized)
  }
  if (isClaudeExecutionTargetId(normalized)) {
    return availableTargetIds.some(id => normalizeExecutionTargetId(id) === normalized)
  }
  return availableTargetIds.some(
    id => normalizeExecutionTargetId(id) === normalized,
  )
}

// ----------------------------------------------------------------------------
// Pure resolution: preference -> effective model, with a typed, never-silent
// fallback.
// ----------------------------------------------------------------------------

export type ModelPreferenceFallback =
  | 'none'
  | 'no_preference_set'
  | 'preference_unavailable'
  | 'default_unavailable'

export type ModelPreferenceResolution = Readonly<{
  // The model id that should actually be used. `null` only in the extreme
  // case where NEITHER the user's stored preference NOR the compiled default
  // is currently servable (e.g. a total supply-lane outage) — callers must
  // treat `null` as "nothing servable", never silently pick something else.
  effectiveModelId: string | null
  // The user's raw stored preference, or `null` if they never set one.
  preferredModelId: string | null
  // True only when `effectiveModelId` is actually the user's own preference.
  usedPreference: boolean
  fallback: ModelPreferenceFallback
}>

export const resolveModelPreference = (
  input: Readonly<{
    storedModelId: string | null
    availableModelIds: ReadonlyArray<string>
  }>,
): ModelPreferenceResolution => {
  const defaultAvailable = isModelIdAvailable(
    DEFAULT_MODEL_PREFERENCE_ID,
    input.availableModelIds,
  )

  if (input.storedModelId === null) {
    return {
      effectiveModelId: defaultAvailable ? DEFAULT_MODEL_PREFERENCE_ID : null,
      fallback: defaultAvailable ? 'no_preference_set' : 'default_unavailable',
      preferredModelId: null,
      usedPreference: false,
    }
  }

  if (isModelIdAvailable(input.storedModelId, input.availableModelIds)) {
    return {
      effectiveModelId: input.storedModelId,
      fallback: 'none',
      preferredModelId: input.storedModelId,
      usedPreference: true,
    }
  }

  return {
    effectiveModelId: defaultAvailable ? DEFAULT_MODEL_PREFERENCE_ID : null,
    fallback: defaultAvailable ? 'preference_unavailable' : 'default_unavailable',
    preferredModelId: input.storedModelId,
    usedPreference: false,
  }
}

export const resolveExecutionTargetPreference = (
  input: Readonly<{
    storedTargetId: string | null
    availableTargetIds: ReadonlyArray<string>
  }>,
): ModelPreferenceResolution => {
  const defaultAvailable = isExecutionTargetIdAvailable(
    DEFAULT_EXECUTION_TARGET_ID,
    input.availableTargetIds,
  )

  if (input.storedTargetId === null) {
    return {
      effectiveModelId: defaultAvailable ? DEFAULT_EXECUTION_TARGET_ID : null,
      fallback: defaultAvailable ? 'no_preference_set' : 'default_unavailable',
      preferredModelId: null,
      usedPreference: false,
    }
  }

  const normalized = normalizeExecutionTargetId(input.storedTargetId)
  if (isExecutionTargetIdAvailable(normalized, input.availableTargetIds)) {
    return {
      effectiveModelId: normalized,
      fallback: 'none',
      preferredModelId: normalized,
      usedPreference: true,
    }
  }

  return {
    effectiveModelId: defaultAvailable ? DEFAULT_EXECUTION_TARGET_ID : null,
    fallback: defaultAvailable ? 'preference_unavailable' : 'default_unavailable',
    preferredModelId: normalized,
    usedPreference: false,
  }
}

// ----------------------------------------------------------------------------
// CX-4 (#8548): quota-aware `auto`, typed and never silent.
//
// v1 is DELIBERATELY dumb per the multi-harness analysis doc §6
// (`docs/fable/2026-07-08-multi-harness-parallelization-effect-native-analysis.md`):
// first READY candidate in a FIXED preference order wins; every candidate
// skipped along the way emits its own typed event naming which target was
// skipped, why, and what it fell through to. No cost/affinity/role scoring —
// that's explicitly deferred until per-harness economics are measured. This
// contract is intentionally generic over "what counts as ready" (a plain
// boolean per candidate) so it works whether the readiness signal is today's
// coarse account `health` projection or tomorrow's real quota/cooldown
// telemetry (CX-7) — callers just build a different `candidates` list, the
// resolution + event shape never changes. Reusable as-is for MH-8.
// ----------------------------------------------------------------------------

export type AutoExecutionTargetSkipReason =
  | 'account_exhausted'
  | 'account_rate_limited'
  | 'account_requires_reauth'
  | 'account_unavailable'

export type AutoExecutionTargetCandidate = Readonly<{
  targetId: string
  ready: boolean
  // Only meaningful when `ready` is false; a not-ready candidate with no
  // reason falls back to `account_unavailable`.
  reason?: AutoExecutionTargetSkipReason | undefined
}>

export type AutoExecutionTargetFallbackEvent = Readonly<{
  type: AutoExecutionTargetSkipReason
  targetId: string
  nextTargetId: string | null
}>

export type AutoExecutionTargetResolution = Readonly<{
  // The concrete, dispatchable target `auto` resolves to right now, or
  // `null` only when every candidate was skipped AND no fallback target was
  // servable either (mirrors `ModelPreferenceResolution.effectiveModelId`'s
  // "nothing servable" law).
  effectiveTargetId: string | null
  // True whenever the resolution did NOT land on the first candidate in the
  // preference order — i.e. at least one skip happened.
  usedFallback: boolean
  // One typed event per skipped candidate, in order. NEVER empty when
  // `usedFallback` is true — every skip is named, never a silent swap.
  events: ReadonlyArray<AutoExecutionTargetFallbackEvent>
}>

/**
 * Pure, typed `auto` policy: walk `candidates` in the given fixed order,
 * return the first `ready` one, and emit one `AutoExecutionTargetFallbackEvent`
 * for every not-ready candidate skipped along the way. Falls through to
 * `fallbackTargetId` (e.g. the compiled default `gemini`/`khala` lane) when
 * NO candidate is ready — that fallback itself is also reported as an event
 * chain, never a silent substitution.
 */
export const resolveAutoExecutionTarget = (
  input: Readonly<{
    candidates: ReadonlyArray<AutoExecutionTargetCandidate>
    fallbackTargetId: string | null
  }>,
): AutoExecutionTargetResolution => {
  const events: AutoExecutionTargetFallbackEvent[] = []

  for (let i = 0; i < input.candidates.length; i++) {
    const candidate = input.candidates[i]
    if (candidate === undefined) continue

    if (candidate.ready) {
      return { effectiveTargetId: candidate.targetId, events, usedFallback: events.length > 0 }
    }

    const next = input.candidates[i + 1]?.targetId ?? input.fallbackTargetId
    events.push({
      nextTargetId: next,
      targetId: candidate.targetId,
      type: candidate.reason ?? 'account_unavailable',
    })
  }

  return {
    effectiveTargetId: input.fallbackTargetId,
    events,
    usedFallback: true,
  }
}

// ----------------------------------------------------------------------------
// D1 read/write (one mutable row per user)
// ----------------------------------------------------------------------------

type ModelPreferenceRow = Readonly<{
  model_id: string
  updated_at: string
}>

export const readUserModelPreference = async (
  db: D1Database,
  userId: string,
): Promise<Readonly<{ modelId: string; updatedAt: string }> | null> => {
  const row = await db
    .prepare(
      `SELECT model_id, updated_at FROM user_model_preferences
        WHERE user_id = ? LIMIT 1`,
    )
    .bind(userId)
    .first<ModelPreferenceRow>()
  return row === null
    ? null
    : { modelId: row.model_id, updatedAt: row.updated_at }
}

export const writeUserModelPreference = async (
  db: D1Database,
  input: Readonly<{ userId: string; modelId: string; nowIso: string }>,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO user_model_preferences (user_id, model_id, created_at, updated_at)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(user_id) DO UPDATE SET
         model_id = excluded.model_id,
         updated_at = excluded.updated_at`,
    )
    .bind(input.userId, input.modelId, input.nowIso, input.nowIso)
    .run()
}

// Re-exported for consumers (e.g. the coding-executor lane selection, #8473)
// that only need to know the public Khala model id without importing pricing.ts
// directly.
export { KHALA_MODEL_ID }
