/**
 * Presence/compute receipt tier contract (openagents issue #4854,
 * Pluralis roadmap P2.3, master tracking issue #4855).
 *
 * Pluralis splits incentives into presence points (Sync Phase 1) and
 * compute points (Phase 2), with the one-token-many-GPUs rule: many
 * processes under one token aggregate to ONE identity score. Their
 * points are unverified leaderboard decoration; our presence receipts
 * settle money, so presence pay must be
 *
 * 1. BOUNDED — a capped availability floor. Floors keep a fleet alive
 *    and enrolled, not rich (buildout plan §3.5).
 * 2. VERIFIED — liveness/qualification probes (the #4681
 *    device-benchmark instrument) are themselves the evidence; a
 *    presence accrual without probe-evidence refs is refused, typed.
 * 3. SYBIL-PRICED — presence accrues per IDENTITY, never per process.
 *    Two devices under one contributor identity share one cap, so
 *    splitting a machine into N processes buys nothing.
 *
 * Compute receipts remain what they are: verified work closeouts at
 * the class rate. Shadow-window (warmup-state) work classifies as
 * presence-tier by construction, exactly as Pluralis sync-phase
 * samples do not count toward `target_batch_size`.
 *
 * Cap unit: SATS PER IDENTITY PER UTC DAY. Windows vary in length and
 * one device spans many per day, so a per-window cap would price
 * presence by window-scheduling luck; the §3.5 floor is a daily
 * availability concept ("alive, enrolled, and warm between
 * assignments"), and a per-identity-per-day cap makes the Sybil budget
 * auditable arithmetic: max presence spend = cap × identities × days.
 *
 * This module is the contract/policy layer ABOVE the payment
 * authority. It never dispatches payment and never reads a clock;
 * accrual day keys and all evidence are passed in by callers. The
 * payment-authority receipt shape is consumed read-only (see
 * `settledSatsFromPaymentAuthorityReceipt` in training-leaderboards);
 * real settlement wiring is the hardware/settlement-gated remainder
 * recorded in `docs/2026-06-12-presence-compute-receipt-split.md`.
 */

import { Schema as S } from 'effect'

import type { PylonJoinLifecycleState } from './pylon-join-lifecycle'

export const PresenceComputeReceiptTierSchemaVersion =
  'openagents.training.presence_compute_receipt_tiers.v1'

export const ReceiptTiers = ['compute_tier', 'presence_tier'] as const
export type ReceiptTier = (typeof ReceiptTiers)[number]

// The work kinds the tier classifier understands. Liveness and
// qualification probes are the #4681 instrument; shadow_window_work is
// the P1.1 warmup analogue of Pluralis Sync Phase 2 (verified but
// unmerged); verified_closeout is the only kind that can ever reach
// compute tier.
export const PresenceComputeWorkKinds = [
  'liveness_probe',
  'qualification_probe',
  'shadow_window_work',
  'verified_closeout',
] as const
export type PresenceComputeWorkKind = (typeof PresenceComputeWorkKinds)[number]

// Funnel-compatible reason-code taxonomy, following the
// `join_lifecycle.public.*` / `device_admission.public.*` convention:
// platform-issued, closed-shape, projection-safe constants.
export const ReceiptTierReasonCodes = [
  'receipt_tier.public.compute_merged_verified_closeout',
  'receipt_tier.public.presence_cap_truncated',
  'receipt_tier.public.presence_liveness_probe',
  'receipt_tier.public.presence_probe_evidence_missing',
  'receipt_tier.public.presence_qualification_probe',
  'receipt_tier.public.presence_shadow_window_work',
  'receipt_tier.public.presence_unmerged_work_not_active',
] as const
export type ReceiptTierReasonCode = (typeof ReceiptTierReasonCodes)[number]

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
// Doc source refs follow the existing roadmap-anchor convention
// (`docs/training/...md#p2.3`), so they additionally allow `#`.
const PublicSafeDocRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/#-]*$/),
)
const UtcDayKey = NonEmptyTrimmedString.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}$/),
)
const PositiveIntSats = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 1, maximum: 100_000_000 }),
)

// Bounded lists: an aggregation call is a bounded batch, never an
// unbounded stream, and per-entry evidence lists stay small enough to
// project publicly.
export const MAX_PRESENCE_ACCRUAL_ENTRIES_PER_AGGREGATION = 500
export const MAX_PROBE_EVIDENCE_REFS_PER_ENTRY = 32

export const PresenceTierPolicy = S.Struct({
  capSatsPerIdentityPerDay: PositiveIntSats,
  policyRef: PublicSafeRef,
  // Literal true: a presence policy that waives probe evidence is
  // unrepresentable in this contract.
  probeEvidenceRequired: S.Literal(true),
  sourceRefs: S.Array(PublicSafeDocRef),
})
export type PresenceTierPolicy = typeof PresenceTierPolicy.Type

export const PresenceAccrualEntry = S.Struct({
  accrualDayUtc: UtcDayKey,
  amountSats: PositiveIntSats,
  deviceRef: PublicSafeRef,
  identityRef: PublicSafeRef,
  probeEvidenceRefs: S.Array(PublicSafeRef),
})
export type PresenceAccrualEntry = typeof PresenceAccrualEntry.Type

export type ReceiptTierClassification = Readonly<{
  joinLifecycleState: PylonJoinLifecycleState
  reasonCode: ReceiptTierReasonCode
  tier: ReceiptTier
  workKind: PresenceComputeWorkKind
}>

// One capped accrual per (identity, UTC day). Device refs aggregate
// under the identity: this record is the Sybil-pricing boundary.
export type PresenceIdentityAccrual = Readonly<{
  accrualDayUtc: string
  accruedSats: number
  capSatsPerIdentityPerDay: number
  deviceRefs: ReadonlyArray<string>
  identityRef: string
  probeEvidenceRefs: ReadonlyArray<string>
  requestedSats: number
}>

export type PresenceCapTruncationEvent = Readonly<{
  accrualDayUtc: string
  identityRef: string
  reasonCode: 'receipt_tier.public.presence_cap_truncated'
  requestedSats: number
  truncatedSats: number
}>

export type PresenceAccrualRefusal = Readonly<{
  accrualDayUtc: string
  deviceRef: string
  identityRef: string
  reason: string
  reasonCode: 'receipt_tier.public.presence_probe_evidence_missing'
}>

export type PresenceAccrualAggregation = Readonly<{
  accruals: ReadonlyArray<PresenceIdentityAccrual>
  refusals: ReadonlyArray<PresenceAccrualRefusal>
  schemaVersion: typeof PresenceComputeReceiptTierSchemaVersion
  truncationEvents: ReadonlyArray<PresenceCapTruncationEvent>
}>

export type PresenceComputeReceiptTierContract = Readonly<{
  contractDefinitionOnly: true
  livePayoutClaim: false
  policy: PresenceTierPolicy
  policyRefs: ReadonlyArray<string>
  schemaVersion: typeof PresenceComputeReceiptTierSchemaVersion
  sourceRefs: ReadonlyArray<string>
  tiers: ReadonlyArray<ReceiptTier>
}>

export class ReceiptTierValidationError extends Error {
  readonly _tag = 'ReceiptTierValidationError'
}

export class ReceiptTierUnsafeError extends Error {
  readonly _tag = 'ReceiptTierUnsafeError'
}

// Same posture as the join-lifecycle and admission-gate guards:
// identity, device, and evidence refs get a substring scan for private
// host, wallet, payment, payout, secret, or raw timestamp material
// before any record carrying them is considered projectable.
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const safeDocRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,260}$/
const unsafeRefPattern =
  /(@|access[_-]?token|bearer|cookie|email|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mnemonic|oauth|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination)|preimage|private[_-]?key|secret|seed[_-]?phrase|serial[_-]?number|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

// Platform-issued taxonomy constants are a closed shape; strip them
// before the substring scan so the closed set cannot trip its own
// scanner (the dark_capacity.public.wallet_not_ready lesson from the
// live funnel 500 of 2026-06-11).
const platformIssuedReasonPattern = /receipt_tier\.public\.[a-z0-9_]+/g

const assertSafeRefAgainst = (
  pattern: RegExp,
  label: string,
  ref: string,
): void => {
  const scrubbed = ref.replaceAll(
    platformIssuedReasonPattern,
    'receipt_tier.public.reason',
  )

  if (
    !pattern.test(ref) ||
    unsafeRefPattern.test(scrubbed) ||
    isoTimestampPattern.test(ref)
  ) {
    throw new ReceiptTierUnsafeError(
      `${label} contains private host, wallet, payment, payout target, secret, or raw timestamp material.`,
    )
  }
}

const assertSafeRef = (label: string, ref: string): void =>
  assertSafeRefAgainst(safeRefPattern, label, ref)

const assertSafeDocRef = (label: string, ref: string): void =>
  assertSafeRefAgainst(safeDocRefPattern, label, ref)

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

/**
 * Classifies one unit of work into a receipt tier from its work kind,
 * its join-lifecycle state, and its verification outcome refs.
 *
 * The compute tier is reachable ONLY by a verified closeout from an
 * ACTIVE device: verified_closeout work in any pre-active or back-edge
 * ladder state is shadow-window or unmerged work and pays
 * presence-tier by construction, exactly as Pluralis sync-phase
 * samples do not count toward target_batch_size. A compute claim
 * without verification outcome refs is not classifiable at all —
 * unverified work has no tier in a network that settles money.
 */
export const classifyReceiptTier = (
  input: Readonly<{
    joinLifecycleState: PylonJoinLifecycleState
    verificationOutcomeRefs: ReadonlyArray<string>
    workKind: PresenceComputeWorkKind
  }>,
): ReceiptTierClassification => {
  for (const outcomeRef of input.verificationOutcomeRefs) {
    assertSafeRef('Receipt tier verification outcome ref', outcomeRef)
  }

  const presence = (
    reasonCode: ReceiptTierReasonCode,
  ): ReceiptTierClassification => ({
    joinLifecycleState: input.joinLifecycleState,
    reasonCode,
    tier: 'presence_tier',
    workKind: input.workKind,
  })

  switch (input.workKind) {
    case 'liveness_probe':
      return presence('receipt_tier.public.presence_liveness_probe')
    case 'qualification_probe':
      return presence('receipt_tier.public.presence_qualification_probe')
    case 'shadow_window_work':
      return presence('receipt_tier.public.presence_shadow_window_work')
    case 'verified_closeout': {
      if (input.verificationOutcomeRefs.length === 0) {
        throw new ReceiptTierValidationError(
          'A verified closeout without verification outcome refs has no receipt tier; unverified work is not payable on any tier.',
        )
      }

      if (input.joinLifecycleState === 'active') {
        return {
          joinLifecycleState: input.joinLifecycleState,
          reasonCode: 'receipt_tier.public.compute_merged_verified_closeout',
          tier: 'compute_tier',
          workKind: input.workKind,
        }
      }

      // Warmup-state work is the shadow window: verified but unmerged.
      if (input.joinLifecycleState === 'warmup') {
        return presence('receipt_tier.public.presence_shadow_window_work')
      }

      return presence('receipt_tier.public.presence_unmerged_work_not_active')
    }
  }
}

const assertAdmissiblePresencePolicy = (policy: PresenceTierPolicy): void => {
  assertSafeRef('Presence tier policy ref', policy.policyRef)

  for (const sourceRef of policy.sourceRefs) {
    assertSafeDocRef('Presence tier policy source ref', sourceRef)
  }

  if (
    !Number.isInteger(policy.capSatsPerIdentityPerDay) ||
    policy.capSatsPerIdentityPerDay <= 0
  ) {
    throw new ReceiptTierValidationError(
      'Presence tier policy requires a positive integer sats cap per identity per day.',
    )
  }
}

const assertAdmissiblePresenceAccrualEntry = (
  entry: PresenceAccrualEntry,
): void => {
  assertSafeRef('Presence accrual identity ref', entry.identityRef)
  assertSafeRef('Presence accrual device ref', entry.deviceRef)

  for (const evidenceRef of entry.probeEvidenceRefs) {
    assertSafeRef('Presence accrual probe evidence ref', evidenceRef)
  }

  if (entry.probeEvidenceRefs.length > MAX_PROBE_EVIDENCE_REFS_PER_ENTRY) {
    throw new ReceiptTierValidationError(
      `Presence accrual entries carry at most ${MAX_PROBE_EVIDENCE_REFS_PER_ENTRY} probe evidence refs.`,
    )
  }

  if (!Number.isInteger(entry.amountSats) || entry.amountSats <= 0) {
    throw new ReceiptTierValidationError(
      'Presence accrual entries require a positive integer sats amount.',
    )
  }

  if (!/^\d{4}-\d{2}-\d{2}$/.test(entry.accrualDayUtc)) {
    throw new ReceiptTierValidationError(
      'Presence accrual entries require a YYYY-MM-DD UTC accrual day key passed in by the caller.',
    )
  }
}

/**
 * Aggregates presence accrual entries per (identity, UTC day) and
 * applies the cap to the IDENTITY, not the device or process.
 *
 * - Entries without probe-evidence refs are refused with a typed
 *   refusal record and never accrue (presence without liveness
 *   evidence is not payable).
 * - Multiple device/process refs under one identity aggregate into one
 *   accrual; the cap applies to their sum, so a Sybil split across
 *   processes buys nothing.
 * - Accrual beyond the cap truncates to the cap and emits a typed
 *   truncation event carrying the requested and truncated amounts.
 */
export const aggregatePresenceAccruals = (
  input: Readonly<{
    entries: ReadonlyArray<PresenceAccrualEntry>
    policy: PresenceTierPolicy
  }>,
): PresenceAccrualAggregation => {
  assertAdmissiblePresencePolicy(input.policy)

  if (input.entries.length > MAX_PRESENCE_ACCRUAL_ENTRIES_PER_AGGREGATION) {
    throw new ReceiptTierValidationError(
      `Presence accrual aggregation accepts at most ${MAX_PRESENCE_ACCRUAL_ENTRIES_PER_AGGREGATION} entries per call.`,
    )
  }

  const refusals: Array<PresenceAccrualRefusal> = []
  const admitted = new Map<string, Array<PresenceAccrualEntry>>()

  for (const entry of input.entries) {
    assertAdmissiblePresenceAccrualEntry(entry)

    if (entry.probeEvidenceRefs.length === 0) {
      refusals.push({
        accrualDayUtc: entry.accrualDayUtc,
        deviceRef: entry.deviceRef,
        identityRef: entry.identityRef,
        reason:
          'Presence accrual requires liveness or qualification probe evidence refs; presence without probe evidence is not payable.',
        reasonCode: 'receipt_tier.public.presence_probe_evidence_missing',
      })
      continue
    }

    const key = `${entry.identityRef} ${entry.accrualDayUtc}`
    const bucket = admitted.get(key) ?? []
    bucket.push(entry)
    admitted.set(key, bucket)
  }

  const accruals: Array<PresenceIdentityAccrual> = []
  const truncationEvents: Array<PresenceCapTruncationEvent> = []
  const cap = input.policy.capSatsPerIdentityPerDay

  for (const bucket of [...admitted.values()]) {
    const first = bucket[0]!
    const requestedSats = bucket.reduce(
      (total, entry) => total + entry.amountSats,
      0,
    )
    const accruedSats = Math.min(requestedSats, cap)

    if (requestedSats > cap) {
      truncationEvents.push({
        accrualDayUtc: first.accrualDayUtc,
        identityRef: first.identityRef,
        reasonCode: 'receipt_tier.public.presence_cap_truncated',
        requestedSats,
        truncatedSats: requestedSats - cap,
      })
    }

    accruals.push({
      accrualDayUtc: first.accrualDayUtc,
      accruedSats,
      capSatsPerIdentityPerDay: cap,
      deviceRefs: uniqueRefs(bucket.map(entry => entry.deviceRef)),
      identityRef: first.identityRef,
      probeEvidenceRefs: uniqueRefs(
        bucket.flatMap(entry => entry.probeEvidenceRefs),
      ),
      requestedSats,
    })
  }

  accruals.sort(
    (left, right) =>
      left.identityRef.localeCompare(right.identityRef) ||
      left.accrualDayUtc.localeCompare(right.accrualDayUtc),
  )

  return {
    accruals,
    refusals,
    schemaVersion: PresenceComputeReceiptTierSchemaVersion,
    truncationEvents,
  }
}

const deepFreeze = <T>(value: T): T => {
  if (typeof value === 'object' && value !== null && !Object.isFrozen(value)) {
    Object.freeze(value)
    Object.values(value).forEach(deepFreeze)
  }

  return value
}

// Seeded contract policy. This is a DEFINITION demonstrating the
// capped-floor shape, not live payout policy: no presence receipt has
// settled against it, and the cap value is a contract example until
// the settlement-gated wiring lands. 1000 sats/identity/day is floor
// money by design — it keeps a device enrolled and probed, and is two
// to three orders of magnitude below a productive compute day.
export const CONTRACT_PRESENCE_TIER_POLICY: PresenceTierPolicy = deepFreeze({
  capSatsPerIdentityPerDay: 1000,
  policyRef: 'policy.receipt_tier.contract.presence_floor_cap.v1',
  probeEvidenceRequired: true,
  sourceRefs: [
    'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p2.3',
    'docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md#3.5',
    'issue.github.openagents.4854',
  ],
})

/**
 * The versioned, frozen, JSON-able receipt-tier contract. Like the
 * device-admission gate contract, `contractDefinitionOnly` and
 * `livePayoutClaim: false` make the non-claim explicit in the exported
 * payload itself: this is the policy layer above the payment
 * authority, and no live presence settlement is claimed by exporting
 * it.
 */
export const exportPresenceComputeReceiptTierContract = (
  policy: PresenceTierPolicy = CONTRACT_PRESENCE_TIER_POLICY,
): PresenceComputeReceiptTierContract => {
  assertAdmissiblePresencePolicy(policy)

  return deepFreeze({
    contractDefinitionOnly: true,
    livePayoutClaim: false,
    policy,
    policyRefs: [
      'policy.public.receipt_tier.compute_tier_is_verified_closeouts_at_class_rate',
      'policy.public.receipt_tier.presence_cap_applies_to_identity_not_process',
      'policy.public.receipt_tier.presence_requires_probe_evidence',
      'policy.public.receipt_tier.shadow_window_work_pays_presence_tier',
    ],
    schemaVersion: PresenceComputeReceiptTierSchemaVersion,
    sourceRefs: [
      'docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md#p2.3',
      'issue.github.openagents.4854',
    ],
    tiers: ReceiptTiers,
  })
}
