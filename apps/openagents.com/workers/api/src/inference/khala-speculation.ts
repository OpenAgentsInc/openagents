// Khala speculative-decoding telemetry + dynamic-disablement policy
// (book P1-8 / #6091).
//
// THE PRINCIPLE (book Ch.5 "Speculative Decoding", in our own words)
// ------------------------------------------------------------------
// Speculative decoding speeds up DECODE by letting a cheap drafter guess the
// next few tokens and letting the expensive target model VERIFY them in a single
// parallel forward pass. Accepted drafts give several tokens for the cost of one
// verification step; a rejection still makes forward progress of one true token.
//
// It is NOT a universal win. The verification pass costs spare compute, so the
// trick only pays off when there IS spare compute to spend — i.e. at LOW batch,
// where decode is the bottleneck and the machine is not already saturated. At
// HIGH batch (or under compute pressure) the extra verification work competes
// with real throughput and speculation becomes a LOSS. So speculation must be
// MEASURED (acceptance rate) and DYNAMICALLY DISABLED when the batch/pressure
// signal says it will not profit — and the mode must be DISCLOSED in the receipt,
// because a request served with speculation is a different serving product than
// one served by plain autoregressive decode.
//
// Code generation is a strong FIT: generated code repeats syntax and reuses
// prompt context, so cheap n-gram / lookahead drafting (no separate draft model)
// hits a high acceptance rate. EAGLE (learned hidden-state drafting) is flagged
// as a LATER Psionic / learned-serving lane: it needs target-model hidden-state
// data + training, so it is named here but never claimed as a built mode.
//
// THIS MODULE owns ONLY the typed speculation telemetry metadata, the canonical
// honest shapes, the builder, and the bounded `decideSpeculation` policy. It does
// NOT run a draft model (there is no draft model / real engine in the Worker);
// the REAL speculative decode is compute/owner-gated and lives behind a future
// serving engine. This module is the typed, public-safe, receipt-bearing record
// of "which speculation mode (if any) was active, how well it accepted, and why
// the policy enabled or disabled it" — pure, framework-agnostic, no Worker/D1.
//
// HONESTY CONTRACT (mirrors the telemetry schema's `not_measured` discipline):
//   - `none` means we KNOW no speculation ran (plain autoregressive decode).
//   - `not_measured` means we do NOT know whether speculation ran / how well it
//     accepted (e.g. a managed provider that speculates behind its API without
//     disclosing acceptance). A typed, first-class sentinel — never a fabricated
//     acceptance number and never a missing field. "We did not measure the
//     acceptance rate" is a real value, distinct from "the acceptance rate was 0".
//   - An acceptance rate is ONLY a real number when an actual draft/verify pass
//     produced accepted + proposed counts. Absent counts => the sentinel.
//
// PUBLIC-SAFE (INVARIANTS: no secret/private leakage): the metadata carries only
// neutral classifiers (the speculation mode, whether it was active, a coarse
// acceptance rate + draft-token counts, the decision + reason, and the workload/
// temperature CONTEXT that keyed the decision). No prompt, draft text, account,
// key, price, or weights material. The acceptance rate is an aggregate ratio, not
// any token content.
import { Schema as S } from 'effect'

// The honest "no measurement exists" sentinel. Defined LOCALLY (equal to the
// telemetry module's `NOT_MEASURED`) so this module — which the telemetry schema
// imports — carries no back-import to telemetry and there is no import cycle.
export const NOT_MEASURED = 'not_measured' as const

// A scalar that is EITHER a finite number OR the honest sentinel.
const MeasuredNumber = S.Union([S.Number, S.Literal(NOT_MEASURED)])
export type MeasuredNumber = typeof MeasuredNumber.Type

const isFiniteNonNegative = (value: number): boolean =>
  Number.isFinite(value) && value >= 0

// ---------------------------------------------------------------------------
// Speculation mode (book Ch.5: the drafting strategy).
// ---------------------------------------------------------------------------

// The drafting strategy that proposed tokens for the target model to verify.
//
//   - `none`        — no speculation ran; plain autoregressive decode (a real,
//                     known value, NOT the unknown sentinel).
//   - `n_gram`      — n-gram speculation: reuse repeated n-grams already seen in
//                     the current generation/context as the draft. No draft model.
//                     Strong fit for code (syntax + prompt-context repetition).
//   - `lookahead`   — lookahead decoding: maintain an n-gram table over the KV
//                     cache and propose matching continuations. No draft model.
//                     Same code-repetition fit, slightly different machinery.
//   - `eagle`       — EAGLE-style learned hidden-state drafting. FLAGGED AS A
//                     LATER Psionic / learned-serving lane: it requires target-
//                     model hidden-state data + a trained draft head, so it is a
//                     named-but-unbuilt mode here. The policy never SELECTS it (it
//                     is not a built mode); it exists so a future learned lane has
//                     a stable receipt vocabulary.
//   - `not_measured`— the honest sentinel: a managed lane may speculate behind its
//                     API without telling us the mode.
//
// A closed union so a new mode is added deliberately (and the policy stays
// exhaustive).
export const KhalaSpeculationMode = S.Literals([
  'none',
  'n_gram',
  'lookahead',
  'eagle',
  'not_measured',
])
export type KhalaSpeculationMode = typeof KhalaSpeculationMode.Type

// Whether a mode is a DRAFT-FREE mode (no separate draft model / no training).
// These are the two modes the Worker-side policy can profitably select TODAY for
// code workloads; `eagle` is learned (Psionic lane), `none`/`not_measured` are
// not drafting modes.
export const isDraftFreeMode = (mode: KhalaSpeculationMode): boolean =>
  mode === 'n_gram' || mode === 'lookahead'

// Whether a mode is a LEARNED mode that needs target hidden-state data + training
// (the EAGLE / Psionic lane). The policy never selects a learned mode; this flags
// it for the doc + receipt.
export const isLearnedMode = (mode: KhalaSpeculationMode): boolean =>
  mode === 'eagle'

// ---------------------------------------------------------------------------
// The speculation telemetry metadata (the typed receipt/telemetry fields).
// ---------------------------------------------------------------------------

// The speculation metadata recorded on the telemetry record / receipt. Every
// field is a concrete value or the honest sentinel. This is the typed answer to
// "was speculation active for this request, in which mode, how well did the
// drafter's proposals get accepted, and what acceptance/draft counts back that
// up?"
export const KhalaSpeculationMetadata = S.Struct({
  schemaVersion: S.Literal('openagents.khala.speculation.v1'),
  // The drafting mode (or `none` / `not_measured`).
  mode: KhalaSpeculationMode,
  // Whether speculation was ACTIVE for this request. `false` for `none`/`eagle`
  // (eagle is unbuilt) and for any request the policy disabled. A request can be
  // in mode `n_gram` with `active:false` if it was disabled by pressure — but the
  // builder keeps these consistent (an inactive request reports mode `none` unless
  // a managed lane disclosed an active mode we did not select).
  active: S.Boolean,
  // The acceptance RATE in [0, 1]: accepted draft tokens / proposed draft tokens
  // over the request. `not_measured` when no draft/verify pass produced counts
  // (the honest sentinel — never a fabricated rate, never defaulted to 0).
  acceptanceRate: MeasuredNumber,
  // The raw counts behind the rate (book Ch.5: proposed K, accepted ≤ K). Each is
  // a measured count or the sentinel. The rate is DERIVED from these; they are
  // recorded so the rate is auditable and never a bare unverifiable number.
  draftTokensProposed: MeasuredNumber,
  draftTokensAccepted: MeasuredNumber,
})
export type KhalaSpeculationMetadata = typeof KhalaSpeculationMetadata.Type

// The canonical "no speculation" metadata: we KNOW plain autoregressive decode
// ran. Distinct from `UNKNOWN_SPECULATION` (we do not know). Acceptance is
// `not_measured` because there were no drafts to accept (NOT a measured 0 — a 0
// would falsely imply a drafter ran and accepted nothing).
export const NO_SPECULATION: KhalaSpeculationMetadata = {
  schemaVersion: 'openagents.khala.speculation.v1',
  mode: 'none',
  active: false,
  acceptanceRate: NOT_MEASURED,
  draftTokensProposed: NOT_MEASURED,
  draftTokensAccepted: NOT_MEASURED,
}

// The canonical honest-unknown metadata: we do NOT know whether/how this lane
// speculated (a managed provider that does not disclose). Distinct from
// `NO_SPECULATION` (we know none ran).
export const UNKNOWN_SPECULATION: KhalaSpeculationMetadata = {
  schemaVersion: 'openagents.khala.speculation.v1',
  mode: 'not_measured',
  active: false,
  acceptanceRate: NOT_MEASURED,
  draftTokensProposed: NOT_MEASURED,
  draftTokensAccepted: NOT_MEASURED,
}

// ---------------------------------------------------------------------------
// Decoders.
// ---------------------------------------------------------------------------

export const decodeKhalaSpeculationMetadata = S.decodeUnknownOption(
  KhalaSpeculationMetadata,
)

// ---------------------------------------------------------------------------
// Builder — assemble metadata honestly from a draft/verify pass (or none).
// ---------------------------------------------------------------------------

// The raw, possibly-partial signals a serving lane CAN disclose about its
// speculation. Everything optional; absence collapses to the honest sentinel —
// never a guess.
export type KhalaSpeculationInput = Readonly<{
  // The mode the lane ran (or `none`). Absent => `not_measured`.
  mode?: KhalaSpeculationMode | undefined
  // Whether speculation was active for this request. Absent => derived from mode
  // (a real drafting mode with counts => active; otherwise false).
  active?: boolean | undefined
  // The raw draft-token counts from the draft/verify pass. Both required for a
  // measured acceptance rate; either absent => the rate is the sentinel.
  draftTokensProposed?: number | undefined
  draftTokensAccepted?: number | undefined
}>

// Derive the acceptance rate from accepted / proposed. `not_measured` unless BOTH
// counts are finite + non-negative AND proposed > 0 (a rate over zero proposals
// is undefined, NOT 0). Accepted is clamped to ≤ proposed so a malformed input
// never yields a rate above 1. PURE.
const deriveAcceptanceRate = (
  proposed: number | undefined,
  accepted: number | undefined,
): MeasuredNumber => {
  if (
    proposed === undefined ||
    accepted === undefined ||
    !isFiniteNonNegative(proposed) ||
    !isFiniteNonNegative(accepted) ||
    proposed === 0
  ) {
    return NOT_MEASURED
  }
  const clampedAccepted = Math.min(accepted, proposed)
  return clampedAccepted / proposed
}

// Build the speculation metadata from raw lane signals. PURE: same input => same
// metadata. Absent mode yields the honest-unknown shape; an explicit `none`
// yields the no-speculation shape; a real drafting mode records the disclosed
// counts + the derived acceptance rate.
export const buildKhalaSpeculationMetadata = (
  input: KhalaSpeculationInput,
): KhalaSpeculationMetadata => {
  const mode: KhalaSpeculationMode = input.mode ?? 'not_measured'

  if (mode === 'none') {
    return NO_SPECULATION
  }
  if (mode === 'not_measured') {
    // Honest unknown: a managed lane that may speculate but disclosed no mode. We
    // never invent counts/rate for it.
    return UNKNOWN_SPECULATION
  }

  const acceptanceRate = deriveAcceptanceRate(
    input.draftTokensProposed,
    input.draftTokensAccepted,
  )
  const proposed =
    input.draftTokensProposed !== undefined &&
    isFiniteNonNegative(input.draftTokensProposed)
      ? input.draftTokensProposed
      : NOT_MEASURED
  const acceptedRaw =
    input.draftTokensAccepted !== undefined &&
    isFiniteNonNegative(input.draftTokensAccepted)
      ? input.draftTokensAccepted
      : NOT_MEASURED
  // Keep accepted ≤ proposed in the recorded counts too (consistency with the
  // clamped rate) so a malformed disclosure never records accepted > proposed.
  const accepted =
    typeof acceptedRaw === 'number' && typeof proposed === 'number'
      ? Math.min(acceptedRaw, proposed)
      : acceptedRaw

  // `active` defaults from the mode: a real drafting mode is active unless the
  // caller said otherwise. (A disabled drafting lane should record mode `none`;
  // a disclosed-but-inactive managed mode can set `active:false` explicitly.)
  const active = input.active ?? true

  return {
    schemaVersion: 'openagents.khala.speculation.v1',
    mode,
    active,
    acceptanceRate,
    draftTokensProposed: proposed,
    draftTokensAccepted: accepted,
  }
}

// ---------------------------------------------------------------------------
// The dynamic-disablement POLICY (`decideSpeculation`).
// ---------------------------------------------------------------------------
//
// The book's core operating rule: speculation profits ONLY when there is spare
// compute to spend on verification — i.e. at LOW batch / LOW compute pressure.
// As the batch grows (or the machine is otherwise saturated), the verification
// pass competes with real throughput and speculation becomes a loss, so it must
// be DISABLED. This is a bounded, typed decision over an explicit pressure
// signal — NOT ad-hoc per-request string matching.

// The compute-pressure signal the policy decides over. These are the only inputs
// the decision reads; they come from the (fixture/observed) serving state, not
// from request content. All numeric inputs are honest: an unknown signal is
// `not_measured` and the policy treats unknown pressure conservatively.
export type KhalaSpeculationPressureSignal = Readonly<{
  // The current decode BATCH size (concurrent sequences sharing the decode step).
  // The headline signal: speculation helps at LOW batch. `not_measured` => the
  // policy cannot confirm low batch and conservatively does NOT enable.
  batchSize: MeasuredNumber
  // A normalized compute-pressure scalar in [0, 1] (e.g. GPU/decode-step
  // utilization): 0 = idle (lots of spare compute to verify drafts), 1 = fully
  // saturated (no spare compute). `not_measured` => treated conservatively.
  computePressure: MeasuredNumber
}>

// The policy CONFIG — the bounded thresholds. Documented + tunable, never
// hand-coded at the call site.
export type KhalaSpeculationPolicyConfig = Readonly<{
  // The maximum batch size at/below which speculation is allowed to profit. Above
  // this, the verification work competes with throughput => disable. (Book: a
  // small batch — single/low concurrency — is the sweet spot.)
  maxProfitableBatchSize: number
  // The maximum normalized compute pressure at/below which speculation may profit.
  // Above this, there is no spare compute to spend verifying drafts => disable.
  maxProfitableComputePressure: number
}>

// The default policy config. Conservative + documented: speculation is enabled
// only at genuinely low concurrency (batch ≤ 4) and well below saturation
// (pressure ≤ 0.6). These are the book's "low-batch sweet spot" turned into
// bounded numbers; tune from observed acceptance/throughput telemetry.
export const DEFAULT_SPECULATION_POLICY: KhalaSpeculationPolicyConfig = {
  maxProfitableBatchSize: 4,
  maxProfitableComputePressure: 0.6,
}

// The reason the policy reached its decision — a closed, public-safe vocabulary
// (never free-form intent text). Each value names a concrete cause so a receipt
// reader knows WHY speculation was on or off.
export const KhalaSpeculationDecisionReason = S.Literals([
  // Enabled: batch + pressure are both in the profitable low range AND the
  // workload is a fit (a drafting mode was requested).
  'enabled_low_batch',
  // Disabled: batch size is above the profitable threshold (high concurrency).
  'disabled_high_batch',
  // Disabled: compute pressure is above the profitable threshold (saturated).
  'disabled_high_pressure',
  // Disabled: the pressure signal is unknown (`not_measured`), so the policy
  // cannot confirm a low-batch sweet spot and conservatively declines.
  'disabled_pressure_unknown',
  // Disabled: the requested mode is not a draft-free mode the Worker can run
  // today (e.g. `eagle`/learned, or `none`/`not_measured`).
  'disabled_mode_unavailable',
  // Disabled: speculation was not requested for this lane at all.
  'disabled_not_requested',
])
export type KhalaSpeculationDecisionReason =
  typeof KhalaSpeculationDecisionReason.Type

// The policy DECISION: enable or not, the selected mode (the requested drafting
// mode when enabled, else `none`), and the typed reason.
export const KhalaSpeculationDecision = S.Struct({
  schemaVersion: S.Literal('openagents.khala.speculation-decision.v1'),
  enabled: S.Boolean,
  // The mode the policy SELECTED. When enabled, the requested drafting mode; when
  // disabled, `none` (we know speculation did not run because we turned it off).
  selectedMode: KhalaSpeculationMode,
  reason: KhalaSpeculationDecisionReason,
})
export type KhalaSpeculationDecision = typeof KhalaSpeculationDecision.Type

// The inputs to the decision. `requestedMode` is the mode the lane WOULD run if
// the policy allows it (a drafting mode for a code workload, or `none`).
export type KhalaSpeculationDecisionInput = Readonly<{
  requestedMode: KhalaSpeculationMode
  signal: KhalaSpeculationPressureSignal
  // Optional override of the bounded thresholds; defaults to
  // `DEFAULT_SPECULATION_POLICY`.
  policy?: KhalaSpeculationPolicyConfig | undefined
}>

const disabled = (
  reason: KhalaSpeculationDecisionReason,
): KhalaSpeculationDecision => ({
  schemaVersion: 'openagents.khala.speculation-decision.v1',
  enabled: false,
  selectedMode: 'none',
  reason,
})

// Decide whether speculation should run for a request, given the requested mode
// and the (fixture/observed) compute-pressure signal. PURE + bounded:
//
//   1. If no drafting mode was requested (`none`/`not_measured`)         => off.
//   2. If a learned/unavailable mode was requested (`eagle`)             => off
//      (the Worker has no draft model / learned head — that is the Psionic lane).
//   3. If the pressure signal is unknown                                 => off
//      (cannot confirm the low-batch sweet spot — be conservative).
//   4. If batch size is above the profitable threshold                   => off.
//   5. If compute pressure is above the profitable threshold             => off.
//   6. Otherwise (a draft-free mode at low batch + low pressure)         => ON.
//
// This is the book's "disable speculation when large batches make verification
// hurt throughput" turned into a typed, auditable decision.
export const decideSpeculation = (
  input: KhalaSpeculationDecisionInput,
): KhalaSpeculationDecision => {
  const policy = input.policy ?? DEFAULT_SPECULATION_POLICY
  const { requestedMode, signal } = input

  // (1) No drafting requested.
  if (requestedMode === 'none' || requestedMode === 'not_measured') {
    return disabled('disabled_not_requested')
  }
  // (2) Learned/unavailable mode — the Worker cannot run it (Psionic lane).
  if (!isDraftFreeMode(requestedMode)) {
    return disabled('disabled_mode_unavailable')
  }
  // (3) Unknown pressure => conservative decline.
  if (signal.batchSize === NOT_MEASURED || signal.computePressure === NOT_MEASURED) {
    return disabled('disabled_pressure_unknown')
  }
  // (4) High batch => the verification work competes with throughput.
  if (signal.batchSize > policy.maxProfitableBatchSize) {
    return disabled('disabled_high_batch')
  }
  // (5) High compute pressure => no spare compute to verify drafts.
  if (signal.computePressure > policy.maxProfitableComputePressure) {
    return disabled('disabled_high_pressure')
  }
  // (6) Low batch + low pressure + draft-free mode => enable.
  return {
    schemaVersion: 'openagents.khala.speculation-decision.v1',
    enabled: true,
    selectedMode: requestedMode,
    reason: 'enabled_low_batch',
  }
}

// ---------------------------------------------------------------------------
// Real speculative-decoding lane preflight — owner/engine/evidence-gated.
// ---------------------------------------------------------------------------

export type RealSpeculationLaneEvidence = Readonly<{
  schemaVersion: 'openagents.khala.real-speculation-evidence.v1'
  evidenceRef: string
  ownerApprovalRef: string
  workloadRef: string
  workload: string
  model: string
  route: string
  temperature: number
  mode: KhalaSpeculationMode
  engineRef: string
  engineKind: 'draft_free_engine' | 'draft_model'
  signal: KhalaSpeculationPressureSignal
  draftTokensProposed: number
  draftTokensAccepted: number
  acceptanceEvidenceRef: string
  latencyEvidenceRef: string
  publicSafeEvidenceRefs: ReadonlyArray<string>
}>

export type RealSpeculationLaneBlocker =
  | 'owner_confirmation_missing'
  | 'owner_approval_ref_missing'
  | 'real_speculation_evidence_missing'
  | 'evidence_ref_missing'
  | 'workload_context_missing'
  | 'model_context_missing'
  | 'route_context_missing'
  | 'temperature_not_measured'
  | 'mode_not_real_speculation'
  | 'engine_ref_missing'
  | 'draft_counts_not_measured'
  | 'accepted_exceeds_proposed'
  | 'acceptance_evidence_ref_missing'
  | 'latency_evidence_ref_missing'
  | 'public_safe_evidence_refs_missing'
  | 'policy_disabled'

export type RealSpeculationLanePreflightInput = Readonly<{
  ownerConfirmed: boolean
  evidence?: RealSpeculationLaneEvidence | undefined
  policy?: KhalaSpeculationPolicyConfig | undefined
}>

export type RealSpeculationLanePreflight = Readonly<{
  schemaVersion: 'openagents.khala.real-speculation-preflight.v1'
  eligible: boolean
  decision: KhalaSpeculationDecision
  metadata: KhalaSpeculationMetadata
  blockers: ReadonlyArray<RealSpeculationLaneBlocker>
  evidenceRef: string | null
  publicSafeEvidenceRefs: ReadonlyArray<string>
}>

const nonBlank = (value: string): boolean => value.trim().length > 0

const evidenceBlockers = (
  input: RealSpeculationLanePreflightInput,
): ReadonlyArray<RealSpeculationLaneBlocker> => {
  const blockers: Array<RealSpeculationLaneBlocker> = []
  if (!input.ownerConfirmed) {
    blockers.push('owner_confirmation_missing')
  }

  const evidence = input.evidence
  if (evidence === undefined) {
    return [...blockers, 'real_speculation_evidence_missing']
  }

  if (!nonBlank(evidence.evidenceRef)) {
    blockers.push('evidence_ref_missing')
  }
  if (!nonBlank(evidence.ownerApprovalRef)) {
    blockers.push('owner_approval_ref_missing')
  }
  if (!nonBlank(evidence.workloadRef) || !nonBlank(evidence.workload)) {
    blockers.push('workload_context_missing')
  }
  if (!nonBlank(evidence.model)) {
    blockers.push('model_context_missing')
  }
  if (!nonBlank(evidence.route)) {
    blockers.push('route_context_missing')
  }
  if (!Number.isFinite(evidence.temperature)) {
    blockers.push('temperature_not_measured')
  }
  if (!isDraftFreeMode(evidence.mode)) {
    blockers.push('mode_not_real_speculation')
  }
  if (!nonBlank(evidence.engineRef)) {
    blockers.push('engine_ref_missing')
  }
  if (
    !isFiniteNonNegative(evidence.draftTokensProposed) ||
    !isFiniteNonNegative(evidence.draftTokensAccepted) ||
    evidence.draftTokensProposed <= 0
  ) {
    blockers.push('draft_counts_not_measured')
  } else if (evidence.draftTokensAccepted > evidence.draftTokensProposed) {
    blockers.push('accepted_exceeds_proposed')
  }
  if (!nonBlank(evidence.acceptanceEvidenceRef)) {
    blockers.push('acceptance_evidence_ref_missing')
  }
  if (!nonBlank(evidence.latencyEvidenceRef)) {
    blockers.push('latency_evidence_ref_missing')
  }
  if (
    evidence.publicSafeEvidenceRefs.length === 0 ||
    evidence.publicSafeEvidenceRefs.some(ref => !nonBlank(ref))
  ) {
    blockers.push('public_safe_evidence_refs_missing')
  }
  return blockers
}

// Preflight a real speculative-decoding lane. This does not execute a draft
// model or engine. It only validates that supplied real measurement evidence is
// owner-armed, public-safe, count-backed, and still inside the low-batch policy
// window before it can be projected into Khala receipt metadata.
export const preflightRealSpeculationLane = (
  input: RealSpeculationLanePreflightInput,
): RealSpeculationLanePreflight => {
  const blockers = evidenceBlockers(input)
  const evidence = input.evidence
  const decision =
    evidence === undefined
      ? disabled('disabled_pressure_unknown')
      : decideSpeculation({
          requestedMode: evidence.mode,
          signal: evidence.signal,
          policy: input.policy,
        })

  const policyBlockers: ReadonlyArray<RealSpeculationLaneBlocker> =
    decision.enabled ? [] : ['policy_disabled']
  const allBlockers = [...blockers, ...policyBlockers]
  const eligible = allBlockers.length === 0 && evidence !== undefined
  const metadata =
    eligible && evidence !== undefined
      ? buildKhalaSpeculationMetadata({
          mode: evidence.mode,
          active: true,
          draftTokensProposed: evidence.draftTokensProposed,
          draftTokensAccepted: evidence.draftTokensAccepted,
        })
      : decision.reason === 'disabled_high_batch' ||
          decision.reason === 'disabled_high_pressure'
        ? NO_SPECULATION
        : UNKNOWN_SPECULATION

  return {
    schemaVersion: 'openagents.khala.real-speculation-preflight.v1',
    eligible,
    decision,
    metadata,
    blockers: allBlockers,
    evidenceRef: eligible && evidence !== undefined ? evidence.evidenceRef : null,
    publicSafeEvidenceRefs:
      eligible && evidence !== undefined ? evidence.publicSafeEvidenceRefs : [],
  }
}
