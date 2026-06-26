import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Option, Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  publicScannerSafeRef,
  publicScannerSafeRefs,
} from './public-ref-scanner-safety'
import { PylonResourceMode } from './pylon-resource-mode-setup'
import { isoTimestampAfterIso } from './runtime-primitives'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  // Pylon heartbeat refs include bounded metric suffixes such as
  // `capacity.coding.codex.ready=1`; the value still stays public-safe and
  // scanner-guarded below, but the request schema must admit the modeled form.
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/=-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))
const NonNegativeInteger = S.Int.check(S.isGreaterThanOrEqualTo(0))
const PylonRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(120),
  S.isPattern(/^[a-z0-9][a-z0-9_.:-]*$/),
)
const PylonDisplayName = NonEmptyTrimmedString.check(S.isMaxLength(120))
const PylonEventStatus = NonEmptyTrimmedString.check(S.isMaxLength(80))
const PylonClientVersion = NonEmptyTrimmedString.check(
  S.isMaxLength(80),
  S.isPattern(
    /^(?:(?:pylon-v)|(?:openagents\.pylon@))?\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.:-]+)?$/,
  ),
)
// #4864 provider discovery fields. A provider Pylon that goes online
// already announces this same Nostr pubkey publicly: the NIP-90 provider
// loop publishes NIP-89 handler info signed by this key on the market
// relays it listens on. Carrying the pubkey, the relay refs, and the
// declared NIP-90 lane refs into /api/pylons adds discoverability, not
// exposure — a stranger buyer can map a relay bid (event.pubkey) to
// registered capacity using public data only. The privacy boundary stays
// pubkey + relay refs + lane refs: nothing wallet-adjacent, no payment
// material, no credential-source detail.
const ProviderNostrPubkeyHex = NonEmptyTrimmedString.check(
  S.isPattern(/^[0-9a-f]{64}$/),
)
const ProviderNostrNpub = NonEmptyTrimmedString.check(
  S.isPattern(/^npub1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{58}$/),
)
// The relay ref is a value carried from the Pylon (what its provider loop
// actually listens on), never a worker-side constant, so the #4863
// relay-domain cutover cannot strand this field.
const ProviderMarketRelayRef = NonEmptyTrimmedString.check(
  S.isMaxLength(200),
  S.isPattern(/^wss?:\/\/[a-z0-9.-]+(?::\d{1,5})?(?:\/[A-Za-z0-9._/-]*)?$/),
)

export type NormalizedPylonClientVersion = Readonly<{
  label: 'openagents.pylon@' | 'plain' | 'pylon-v'
  major: number
  minor: number
  patch: number
}>

const pylonClientVersionPattern =
  /^(?:(pylon-v|openagents\.pylon@))?(\d+)\.(\d+)\.(\d+)(?:[-+][A-Za-z0-9.:-]+)?$/

export const parsePylonClientVersion = (
  value: string,
): NormalizedPylonClientVersion | null => {
  const match = pylonClientVersionPattern.exec(value.trim())

  if (match === null) {
    return null
  }

  return {
    label: (match[1] ?? 'plain') as NormalizedPylonClientVersion['label'],
    major: Number.parseInt(match[2] ?? '0', 10),
    minor: Number.parseInt(match[3] ?? '0', 10),
    patch: Number.parseInt(match[4] ?? '0', 10),
  }
}

export const pylonClientVersionMeetsMinimum = (
  value: string | null | undefined,
  minimum: string,
): boolean => {
  if (value === null || value === undefined) {
    return false
  }

  const parsed = parsePylonClientVersion(value)
  const parsedMinimum = parsePylonClientVersion(minimum)

  if (parsed === null || parsedMinimum === null) {
    return false
  }

  return (
    parsed.major > parsedMinimum.major ||
    (parsed.major === parsedMinimum.major &&
      (parsed.minor > parsedMinimum.minor ||
        (parsed.minor === parsedMinimum.minor &&
          parsed.patch >= parsedMinimum.patch)))
  )
}

export const PylonApiRegistrationStatus = S.Literals([
  'active',
  'blocked',
  'retired',
])
export type PylonApiRegistrationStatus = typeof PylonApiRegistrationStatus.Type

export const PylonApiEventKind = S.Literals([
  'artifact_proof_metadata',
  'assignment_acceptance',
  'assignment_progress',
  'heartbeat',
  'payment_receipt',
  'payout_target_admission',
  'registration',
  'settlement_status',
  'wallet_readiness',
  'worker_closeout',
])
export type PylonApiEventKind = typeof PylonApiEventKind.Type

export const PylonApiAssignmentJobKind = S.Literals([
  'artifact_review',
  'claude_agent_task',
  'codex_agent_task',
  'cs336_a1_homework',
  'cs336_a2_device_benchmark',
  'cs336_a3_scaling_sweep',
  'cs336_a5_alignment',
  'healthcheck_echo',
  'inference',
  'tassadar_executor_trace',
  'validation',
])
export type PylonApiAssignmentJobKind = typeof PylonApiAssignmentJobKind.Type

export const PylonApiAssignmentPaymentMode = S.Literals([
  'unpaid_smoke',
  'operator_credit',
  'payable_pending_settlement',
  'settled_bitcoin',
  'rejected_no_pay',
])
export type PylonApiAssignmentPaymentMode =
  typeof PylonApiAssignmentPaymentMode.Type

export const PylonApiAssignmentState = S.Literals([
  'accepted',
  'accepted_work',
  'blocked',
  'cancelled',
  'closeout_submitted',
  'offered',
  'proof_submitted',
  'rejected',
  'running',
  'stale',
])
export type PylonApiAssignmentState = typeof PylonApiAssignmentState.Type

export const PylonApiRegistrationRequest = S.Struct({
  capabilityRefs: PublicSafeRefs,
  clientProtocolVersion: S.optionalKey(PylonClientVersion),
  clientVersion: S.optionalKey(PylonClientVersion),
  displayName: S.optionalKey(PylonDisplayName),
  providerMarketRelayRefs: S.optionalKey(S.Array(ProviderMarketRelayRef)),
  providerNip90LaneRefs: PublicSafeRefs,
  providerNostrNpub: S.optionalKey(ProviderNostrNpub),
  providerNostrPubkey: S.optionalKey(ProviderNostrPubkeyHex),
  pylonRef: S.optionalKey(PylonRef),
  resourceMode: S.optionalKey(PylonResourceMode),
  statusRefs: PublicSafeRefs,
  walletRef: S.optionalKey(PublicSafeRef),
})
export type PylonApiRegistrationRequest =
  typeof PylonApiRegistrationRequest.Type

export const PylonApiHeartbeatRequest = S.Struct({
  capacityRefs: PublicSafeRefs,
  clientProtocolVersion: S.optionalKey(PylonClientVersion),
  clientVersion: S.optionalKey(PylonClientVersion),
  healthRefs: PublicSafeRefs,
  loadRefs: PublicSafeRefs,
  providerMarketRelayRefs: S.optionalKey(S.Array(ProviderMarketRelayRef)),
  providerNip90LaneRefs: PublicSafeRefs,
  providerNostrNpub: S.optionalKey(ProviderNostrNpub),
  providerNostrPubkey: S.optionalKey(ProviderNostrPubkeyHex),
  resourceMode: S.optionalKey(PylonResourceMode),
  status: S.optionalKey(PylonEventStatus),
  // Live receive-readiness published by the heartbeat (#5151): an online,
  // receive-ready node now updates registration.walletReady (and thus public
  // walletReadyNow) without needing a separate wallet-readiness event. Omitted
  // when the node could not probe its wallet, leaving the last known value.
  walletReady: S.optionalKey(S.Boolean),
})
export type PylonApiHeartbeatRequest = typeof PylonApiHeartbeatRequest.Type

export const PylonApiWalletReadinessRequest = S.Struct({
  balanceRefs: PublicSafeRefs,
  liquidityRefs: PublicSafeRefs,
  readinessRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
  walletReady: S.Boolean,
  walletRef: S.optionalKey(PublicSafeRef),
})
export type PylonApiWalletReadinessRequest =
  typeof PylonApiWalletReadinessRequest.Type

export const PylonApiPayoutTargetAdmissionRequest = S.Struct({
  admissionRefs: PublicSafeRefs,
  payoutTargetRef: PublicSafeRef,
  policyRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiPayoutTargetAdmissionRequest =
  typeof PylonApiPayoutTargetAdmissionRequest.Type

// #5252: raw Spark address registration. The raw `spark1…` is PAYMENT MATERIAL.
// It rides ONLY this authenticated request body, is stored in the private
// operator store keyed to the agent's pylonRef, and is NEVER projected, logged,
// or persisted into a public event. The redacted `payout.spark.<digest>` ref is
// the only public projection. `RawSparkAddress` is deliberately NOT a
// `PublicSafeRef`: it bounds the raw value's shape (a Spark bech32 address)
// without ever flowing into a projection.
const RawSparkAddress = NonEmptyTrimmedString.check(
  S.isMaxLength(200),
  S.isPattern(/^spark1[0-9a-z]{20,}$/i),
)
const SparkPayoutTargetRef = PublicSafeRef.check(
  S.isPattern(/^payout\.spark\.[0-9a-f]{8,}$/i),
)

export const PylonApiSparkPayoutTargetRegisterRequest = S.Struct({
  payoutTargetRef: SparkPayoutTargetRef,
  policyRefs: PublicSafeRefs,
  // PRIVATE: stored operator-only, never projected. Decoded here so the raw
  // address never reaches a route handler as an unvalidated string.
  rawSparkAddress: RawSparkAddress,
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiSparkPayoutTargetRegisterRequest =
  typeof PylonApiSparkPayoutTargetRegisterRequest.Type

// Private operator-only store for raw Spark payout targets (#5252). The raw
// `spark1…` lives ONLY here, keyed by pylonRef and bound to the owning agent.
// Public surfaces resolve only the redacted `payout.spark.<digest>` ref.
export type PylonSparkPayoutTargetRecord = Readonly<{
  pylonRef: string
  ownerAgentUserId: string
  payoutTargetRef: string
  // Raw payment material; never enters a projection.
  rawSparkAddress: string
  createdAt: string
  updatedAt: string
}>

export type PylonSparkPayoutTargetStore = Readonly<{
  // Idempotent upsert keyed by pylonRef. Re-registering the same address for the
  // same pylon is a no-op update, not a duplicate.
  upsert: (
    record: PylonSparkPayoutTargetRecord,
  ) => Promise<PylonSparkPayoutTargetRecord>
  read: (pylonRef: string) => Promise<PylonSparkPayoutTargetRecord | undefined>
  // Owner-scoped read (#5252 backing index): the most recently updated Spark
  // payout target registered by an owning agent across ANY of its pylons. The
  // settlement resolver uses this as a canonical fallback when a contribution's
  // lease pylonRef (e.g. a device-ref) differs from the pylonRef the target was
  // registered under, so a contributor whose registered Pylon projects
  // `sparkPayoutTargetReady: true` actually resolves a destination. Bound to the
  // owning agent only; it never crosses agent ownership.
  readByOwner: (
    ownerAgentUserId: string,
  ) => Promise<PylonSparkPayoutTargetRecord | undefined>
}>

export const makeD1PylonSparkPayoutTargetStore = (
  db: D1Database,
): PylonSparkPayoutTargetStore => ({
  upsert: async record => {
    await db
      .prepare(
        `INSERT INTO pylon_spark_payout_targets
           (pylon_ref, owner_agent_user_id, payout_target_ref, raw_spark_address,
            created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(pylon_ref) DO UPDATE SET
           owner_agent_user_id = excluded.owner_agent_user_id,
           payout_target_ref = excluded.payout_target_ref,
           raw_spark_address = excluded.raw_spark_address,
           updated_at = excluded.updated_at`,
      )
      .bind(
        record.pylonRef,
        record.ownerAgentUserId,
        record.payoutTargetRef,
        record.rawSparkAddress,
        record.createdAt,
        record.updatedAt,
      )
      .run()

    return record
  },

  read: async pylonRef => {
    const row = await db
      .prepare(
        `SELECT pylon_ref, owner_agent_user_id, payout_target_ref,
                raw_spark_address, created_at, updated_at
           FROM pylon_spark_payout_targets
          WHERE pylon_ref = ?
          LIMIT 1`,
      )
      .bind(pylonRef)
      .first<{
        pylon_ref: string
        owner_agent_user_id: string
        payout_target_ref: string
        raw_spark_address: string
        created_at: string
        updated_at: string
      }>()

    if (row === null) {
      return undefined
    }

    return {
      pylonRef: row.pylon_ref,
      ownerAgentUserId: row.owner_agent_user_id,
      payoutTargetRef: row.payout_target_ref,
      rawSparkAddress: row.raw_spark_address,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    }
  },

  readByOwner: async ownerAgentUserId => {
    const row = await db
      .prepare(
        `SELECT pylon_ref, owner_agent_user_id, payout_target_ref,
                raw_spark_address, created_at, updated_at
           FROM pylon_spark_payout_targets
          WHERE owner_agent_user_id = ?
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(ownerAgentUserId)
      .first<{
        pylon_ref: string
        owner_agent_user_id: string
        payout_target_ref: string
        raw_spark_address: string
        created_at: string
        updated_at: string
      }>()

    if (row === null) {
      return undefined
    }

    return {
      pylonRef: row.pylon_ref,
      ownerAgentUserId: row.owner_agent_user_id,
      payoutTargetRef: row.payout_target_ref,
      rawSparkAddress: row.raw_spark_address,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    }
  },
})

/**
 * Resolve a recipient's registered raw Spark payout destination (#5252).
 * Returns the recipient's OWN registered raw `spark1…` (never projected) so the
 * settlement payout authority can pay it natively over Spark, or `undefined`
 * when the recipient has no registered Spark target (or the read fails). The
 * caller fails closed on `undefined` — no vetted target, no native send. The
 * raw address returned here is payment material and must never be logged or
 * projected.
 */
export const resolveSparkPayoutDestination = async (
  store: PylonSparkPayoutTargetStore,
  contributorRef: string,
  // Optional owner-resolver (#5252): when the contribution's lease pylonRef is
  // not the exact pylonRef the target was registered under, fall back to the
  // owning agent's canonical registered Spark target. Bound to the same owning
  // agent only; absent resolver preserves prior exact-key behavior.
  resolveOwnerAgentUserId?: (pylonRef: string) => Promise<string | undefined>,
): Promise<string | undefined> => {
  const rawFromRecord = (
    record: PylonSparkPayoutTargetRecord | undefined,
  ): string | undefined => {
    const raw = record?.rawSparkAddress.trim()

    return raw !== undefined && raw !== '' ? raw : undefined
  }

  try {
    // 1) Exact pylonRef match (prior behavior — the common case).
    const direct = rawFromRecord(await store.read(contributorRef))

    if (direct !== undefined) {
      return direct
    }

    // 2) Owner-scoped canonical fallback. The target may be registered under a
    //    different pylonRef owned by the SAME agent (e.g. the lease used a
    //    device-ref). Resolve the owner, then use the owner's registered target.
    //    Fail-closed: any missing owner / missing target / read error -> no send.
    if (resolveOwnerAgentUserId === undefined) {
      return undefined
    }

    const ownerAgentUserId = await resolveOwnerAgentUserId(contributorRef)

    if (ownerAgentUserId === undefined || ownerAgentUserId.trim() === '') {
      return undefined
    }

    return rawFromRecord(await store.readByOwner(ownerAgentUserId))
  } catch {
    return undefined
  }
}

// Public-safe Spark payout-target readiness (#5306). `ready` reflects whether a
// node has auto-registered (#5305) a Spark payout target for this pylonRef; the
// `ref` is the redacted `payout.spark.<digest>` ref the store already holds —
// NEVER the raw `spark1…` address. This is the onboarding backstop: readiness is
// recomputed from the live private store on every register/heartbeat/read, so a
// missing target self-heals to `ready` the moment the node next comes online and
// registers, with no manual step. It fails closed: a missing target, an empty or
// malformed stored ref, or any store read error all yield `ready: false` and a
// `null` ref. The server never fabricates a target — the raw address must come
// from the node and stays node-owned + private-store-only.
export type PylonSparkPayoutTargetReadiness = Readonly<{
  ready: boolean
  ref: string | null
}>

export const SPARK_PAYOUT_TARGET_NOT_READY: PylonSparkPayoutTargetReadiness = {
  ready: false,
  ref: null,
}

const isSparkPayoutTargetRef = S.is(SparkPayoutTargetRef)

export const resolveSparkPayoutTargetReadiness = async (
  store: PylonSparkPayoutTargetStore,
  pylonRef: string,
): Promise<PylonSparkPayoutTargetReadiness> => {
  try {
    const record = await store.read(pylonRef)

    if (record === undefined) {
      return SPARK_PAYOUT_TARGET_NOT_READY
    }

    const ref = record.payoutTargetRef.trim()
    const rawRegistered = record.rawSparkAddress.trim() !== ''

    // Fail closed unless a node-registered raw address exists AND the stored
    // digest ref is the expected redacted shape. The ref is public-safe (a
    // digest, not the address), but re-checking the shape keeps a malformed or
    // tampered row from ever projecting as ready.
    if (!rawRegistered || !isSparkPayoutTargetRef(ref)) {
      return SPARK_PAYOUT_TARGET_NOT_READY
    }

    return { ready: true, ref }
  } catch {
    return SPARK_PAYOUT_TARGET_NOT_READY
  }
}

export const PylonApiAssignmentAcceptanceRequest = S.Struct({
  acceptanceRefs: PublicSafeRefs,
  accepted: S.Boolean,
  resourceMode: S.optionalKey(PylonResourceMode),
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiAssignmentAcceptanceRequest =
  typeof PylonApiAssignmentAcceptanceRequest.Type

export const PylonApiAssignmentProgressRequest = S.Struct({
  artifactRefs: PublicSafeRefs,
  blockerRefs: PublicSafeRefs,
  progressPercent: S.optionalKey(S.Number),
  progressRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiAssignmentProgressRequest =
  typeof PylonApiAssignmentProgressRequest.Type

export const PylonApiArtifactProofMetadataRequest = S.Struct({
  artifactRefs: PublicSafeRefs,
  proofRefs: PublicSafeRefs,
  storageRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiArtifactProofMetadataRequest =
  typeof PylonApiArtifactProofMetadataRequest.Type

export const PylonApiAssignmentWorkerCloseoutRequest = S.Struct({
  artifactRefs: PublicSafeRefs,
  authorityReceiptRefs: PublicSafeRefs,
  blockerRefs: PublicSafeRefs,
  buildRefs: PublicSafeRefs,
  changeCaptureRefs: PublicSafeRefs,
  changeCaptureStatus: S.optionalKey(
    S.Literals(['blocked', 'review_ready', 'stale']),
  ),
  closeoutRefs: PublicSafeRefs,
  deliveryReadinessFreshness: S.optionalKey(S.Literals(['fresh', 'stale'])),
  deliveryReadinessRefs: PublicSafeRefs,
  deliveryReadinessStatus: S.optionalKey(
    S.Literals(['blocked', 'ready', 'scoped_exception']),
  ),
  fileCount: S.optionalKey(NonNegativeInteger),
  addedLineCount: S.optionalKey(NonNegativeInteger),
  patchDigestRef: S.optionalKey(S.NullOr(PublicSafeRef)),
  previewRefs: PublicSafeRefs,
  proofRefs: PublicSafeRefs,
  removedLineCount: S.optionalKey(NonNegativeInteger),
  resultRefs: PublicSafeRefs,
  reviewCaveatRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
  summaryRefs: PublicSafeRefs,
  testRefs: PublicSafeRefs,
  verificationRefs: PublicSafeRefs,
  worktreeIdentityStatus: S.optionalKey(
    S.Literals(['blocked', 'ready', 'stale']),
  ),
  writebackRequired: S.optionalKey(S.Boolean),
})
export type PylonApiAssignmentWorkerCloseoutRequest =
  typeof PylonApiAssignmentWorkerCloseoutRequest.Type

export const PylonApiPaymentReceiptRequest = S.Struct({
  paymentProofRefs: PublicSafeRefs,
  receiptRefs: PublicSafeRefs,
  settlementRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiPaymentReceiptRequest =
  typeof PylonApiPaymentReceiptRequest.Type

export const PylonApiSettlementStatusRequest = S.Struct({
  settlementRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
  treasuryReceiptRefs: PublicSafeRefs,
})
export type PylonApiSettlementStatusRequest =
  typeof PylonApiSettlementStatusRequest.Type

export const PylonApiCreateAssignmentRequest = S.Struct({
  acceptanceCriteriaRefs: PublicSafeRefs,
  assignmentRef: S.optionalKey(PublicSafeRef),
  campaignPaused: S.optionalKey(S.Boolean),
  campaignPolicyRefs: PublicSafeRefs,
  codingAssignment: S.optionalKey(S.Record(S.String, S.Unknown)),
  campaignRef: S.optionalKey(PublicSafeRef),
  closeoutPathRefs: PublicSafeRefs,
  forumAutoPublishAllowed: S.optionalKey(S.Boolean),
  idempotencyRefs: PublicSafeRefs,
  jobKind: PylonApiAssignmentJobKind,
  leaseSeconds: S.optionalKey(S.Number),
  noDuplicateAssignmentRefs: PublicSafeRefs,
  noForumAutoPublishRefs: PublicSafeRefs,
  operatorPauseRefs: PublicSafeRefs,
  paymentMode: S.optionalKey(PylonApiAssignmentPaymentMode),
  pylonRef: PylonRef,
  requiredCapabilityRefs: PublicSafeRefs,
  resultExpectationRefs: PublicSafeRefs,
  rollbackRefs: PublicSafeRefs,
  selectionPolicyRefs: PublicSafeRefs,
  spendCapRefs: PublicSafeRefs,
  taskRefs: PublicSafeRefs,
})
export type PylonApiCreateAssignmentRequest =
  typeof PylonApiCreateAssignmentRequest.Type

export const PylonApiAssignmentCloseoutRequest = S.Struct({
  accepted: S.Boolean,
  acceptedWorkRefs: PublicSafeRefs,
  closeoutRefs: PublicSafeRefs,
  rejectionRefs: PublicSafeRefs,
  status: S.optionalKey(PylonEventStatus),
})
export type PylonApiAssignmentCloseoutRequest =
  typeof PylonApiAssignmentCloseoutRequest.Type

export type PylonApiRegistrationRecord = Readonly<{
  capabilityRefs: ReadonlyArray<string>
  clientProtocolVersion: string | null
  clientVersion: string | null
  createdAt: string
  displayName: string
  id: string
  latestHeartbeatAt: string | null
  latestHeartbeatStatus: string | null
  latestCapacityRefs: ReadonlyArray<string>
  latestHealthRefs: ReadonlyArray<string>
  latestLoadRefs: ReadonlyArray<string>
  latestResourceMode: typeof PylonResourceMode.Type | null
  ownerAgentCredentialId: string
  ownerAgentTokenPrefix: string
  ownerAgentUserId: string
  providerMarketRelayRefs: ReadonlyArray<string>
  providerNip90LaneRefs: ReadonlyArray<string>
  providerNostrNpub: string | null
  providerNostrPubkey: string | null
  publicProjectionJson: string
  pylonRef: string
  resourceMode: typeof PylonResourceMode.Type
  status: PylonApiRegistrationStatus
  updatedAt: string
  walletReady: boolean
  walletRef: string | null
}>

export type PylonApiEventRecord = Readonly<{
  assignmentRef: string | null
  createdAt: string
  eventBody: Record<string, unknown>
  eventKind: PylonApiEventKind
  eventRef: string
  id: string
  idempotencyKeyHash: string
  ownerAgentUserId: string
  publicProjectionJson: string
  pylonRef: string
  status: string
}>

export type PylonApiAssignmentRecord = Readonly<{
  acceptanceCriteriaRefs: ReadonlyArray<string>
  acceptedWorkRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  assignmentRef: string
  closeoutRefs: ReadonlyArray<string>
  codingAssignment: Record<string, unknown> | null
  createdAt: string
  id: string
  idempotencyKeyHash: string
  jobKind: PylonApiAssignmentJobKind
  leaseExpiresAt: string
  ownerAgentUserId: string
  proofRefs: ReadonlyArray<string>
  publicProjectionJson: string
  pylonRef: string
  rejectionRefs: ReadonlyArray<string>
  resultExpectationRefs: ReadonlyArray<string>
  state: PylonApiAssignmentState
  taskRefs: ReadonlyArray<string>
  updatedAt: string
}>

export type PylonApiRegistrationProjection = Readonly<{
  capabilityRefs: ReadonlyArray<string>
  codingCapacity: ReadonlyArray<PylonCodingServiceCapacityProjection>
  clientProtocolVersion: string | null
  clientVersion: string | null
  createdAtDisplay: string
  displayName: string
  latestCapacityRefs: ReadonlyArray<string>
  latestHeartbeatDisplay: string | null
  latestHeartbeatStatus: string | null
  latestHealthRefs: ReadonlyArray<string>
  latestLoadRefs: ReadonlyArray<string>
  latestResourceMode: typeof PylonResourceMode.Type | null
  ownerAgentRef: string
  providerMarketRelayRefs: ReadonlyArray<string>
  providerNip90LaneRefs: ReadonlyArray<string>
  providerNostrNpub: string | null
  providerNostrPubkey: string | null
  pylonRef: string
  resourceMode: typeof PylonResourceMode.Type
  // #5306: public-safe onboarding readiness for the node's native Spark payout
  // target. `true` once a node auto-registers (#5305); `false` is a visible,
  // self-healing gap, never a fabricated target. `sparkPayoutTargetRef` is the
  // redacted `payout.spark.<digest>` ref when present — never the raw address.
  sparkPayoutTargetReady: boolean
  sparkPayoutTargetRef: string | null
  status: PylonApiRegistrationStatus
  updatedAtDisplay: string
  walletReady: boolean
  walletRef: string | null
}>

export type PylonCodingServiceCapacityProjection = Readonly<{
  available: number
  busy: number
  queued: number
  ready: number
  service: 'claude' | 'codex'
}>

export type PylonApiEventProjection = Readonly<{
  assignmentRef: string | null
  createdAtDisplay: string
  eventKind: PylonApiEventKind
  eventRef: string
  pylonRef: string
  status: string
}>

export type PylonApiAssignmentProjection = Readonly<{
  acceptanceCriteriaRefs: ReadonlyArray<string>
  acceptedWorkRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  assignmentRef: string
  closeoutRefs: ReadonlyArray<string>
  codingAssignment: Record<string, unknown> | null
  createdAtDisplay: string
  jobKind: PylonApiAssignmentJobKind
  leaseExpiresInSeconds: number
  leaseState: 'active' | 'expired' | 'terminal'
  proofRefs: ReadonlyArray<string>
  pylonRef: string
  rejectionRefs: ReadonlyArray<string>
  resultExpectationRefs: ReadonlyArray<string>
  state: PylonApiAssignmentState
  taskRefs: ReadonlyArray<string>
  updatedAtDisplay: string
}>

export type PylonProviderJobLifecycleStage =
  | 'accepted'
  | 'accepted_work'
  | 'artifact_submitted'
  | 'closeout_submitted'
  | 'offered'
  | 'running'

export type PylonApiProviderJobLifecycleRecord = Readonly<{
  acceptedWorkRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  assignmentRef: string
  closeoutRefs: ReadonlyArray<string>
  createdAt: string
  id: string
  jobKind: PylonApiAssignmentJobKind
  ownerAgentUserId: string
  proofRefs: ReadonlyArray<string>
  publicProjectionJson: string
  pylonRef: string
  stage: PylonProviderJobLifecycleStage
  taskRefs: ReadonlyArray<string>
  updatedAt: string
}>

export type PylonApiStore = Readonly<{
  createAssignment: (
    record: PylonApiAssignmentRecord,
  ) => Promise<
    Readonly<{ idempotent: boolean; record: PylonApiAssignmentRecord }>
  >
  createEvent: (
    record: PylonApiEventRecord,
  ) => Promise<Readonly<{ idempotent: boolean; record: PylonApiEventRecord }>>
  listAssignmentsForPylon: (
    pylonRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiAssignmentRecord>>
  listAssignmentsForPylons?: (
    pylonRefs: ReadonlyArray<string>,
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiAssignmentRecord>>
  listEventsForPylon: (
    pylonRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiEventRecord>>
  listEventsForAssignment: (
    assignmentRef: string,
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiEventRecord>>
  listRegistrations: (
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiRegistrationRecord>>
  listRegistrationsForOwnerAgentUserIds?: (
    ownerAgentUserIds: ReadonlyArray<string>,
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiRegistrationRecord>>
  listProviderJobLifecycleForPylons: (
    pylonRefs: ReadonlyArray<string>,
    limit: number,
  ) => Promise<ReadonlyArray<PylonApiProviderJobLifecycleRecord>>
  readEventByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<PylonApiEventRecord | undefined>
  readAssignment: (
    assignmentRef: string,
  ) => Promise<PylonApiAssignmentRecord | undefined>
  readAssignmentByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<PylonApiAssignmentRecord | undefined>
  readRegistration: (
    pylonRef: string,
  ) => Promise<PylonApiRegistrationRecord | undefined>
  updateAssignment: (
    record: PylonApiAssignmentRecord,
  ) => Promise<PylonApiAssignmentRecord>
  updateAssignmentIfState: (
    record: PylonApiAssignmentRecord,
    expectedState: PylonApiAssignmentState,
  ) => Promise<PylonApiAssignmentRecord | undefined>
  upsertProviderJobLifecycle: (
    record: PylonApiProviderJobLifecycleRecord,
  ) => Promise<PylonApiProviderJobLifecycleRecord>
  upsertRegistration: (
    record: PylonApiRegistrationRecord,
  ) => Promise<PylonApiRegistrationRecord>
}>

export class PylonApiStoreError extends S.TaggedErrorClass<PylonApiStoreError>()(
  'PylonApiStoreError',
  {
    kind: S.Literals([
      'conflict',
      'forbidden',
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

type PylonApiRegistrationRow = Readonly<{
  capability_refs_json: string
  client_protocol_version?: string | null
  client_version?: string | null
  created_at: string
  display_name: string
  id: string
  latest_heartbeat_at: string | null
  latest_heartbeat_status?: string | null
  latest_capacity_refs_json?: string | null
  latest_health_refs_json?: string | null
  latest_load_refs_json?: string | null
  latest_resource_mode?: typeof PylonResourceMode.Type | null
  owner_agent_credential_id: string
  owner_agent_token_prefix: string
  owner_agent_user_id: string
  provider_market_relay_refs_json?: string | null
  provider_nip90_lane_refs_json?: string | null
  provider_nostr_npub?: string | null
  provider_nostr_pubkey?: string | null
  public_projection_json: string
  pylon_ref: string
  resource_mode: typeof PylonResourceMode.Type
  status: PylonApiRegistrationStatus
  updated_at: string
  wallet_ready: number
  wallet_ref: string | null
}>

type PylonApiEventRow = Readonly<{
  assignment_ref: string | null
  created_at: string
  event_body_json: string
  event_kind: PylonApiEventKind
  event_ref: string
  id: string
  idempotency_key_hash: string
  owner_agent_user_id: string
  public_projection_json: string
  pylon_ref: string
  status: string
}>

type PylonApiAssignmentRow = Readonly<{
  acceptance_criteria_refs_json: string
  accepted_work_refs_json: string
  artifact_refs_json: string
  assignment_ref: string
  closeout_refs_json: string
  coding_assignment_json?: string | null
  created_at: string
  id: string
  idempotency_key_hash: string
  job_kind: PylonApiAssignmentJobKind
  lease_expires_at: string
  owner_agent_user_id: string
  proof_refs_json: string
  public_projection_json: string
  pylon_ref: string
  rejection_refs_json: string
  result_expectation_refs_json: string
  state: PylonApiAssignmentState
  task_refs_json: string
  updated_at: string
}>

type PylonProviderJobLifecycleRow = Readonly<{
  accepted_work_refs_json: string
  artifact_refs_json: string
  assignment_ref: string
  closeout_refs_json: string
  created_at: string
  id: string
  job_kind: PylonApiAssignmentJobKind
  owner_agent_user_id: string
  proof_refs_json: string
  public_projection_json: string
  pylon_ref: string
  stage: PylonProviderJobLifecycleStage
  task_refs_json: string
  updated_at: string
}>

const unsafePylonApiMaterialPattern =
  /([A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|\/Users\/|\/home\/|access[_-]?token|auth\.json|balance[._-]?sats|bearer|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof=)|payout[_-]?(address|destination|private|raw)|preimage|private[_-]?(artifact|channel|key|output|proof|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(artifact|auth|backup|balance|channel|invoice|liquidity|output|payload|payment|payout|prompt|proof|provider|runner|run[_-]?log|state|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|"token"\s*:|token[_-]?(credential|grant|payload|secret|value)|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const exactBalanceRefPattern = /balance\.mdk_agent_wallet\.\d+\b/i
// #5252: a raw Spark address/invoice is PAYMENT MATERIAL and must never appear
// in a public Pylon payload. The redacted `payout.spark.<digest>` ref does not
// match (it has no `spark1…`/`spark1q…` body), so this rejects only the raw
// address forms while leaving the digest ref safe.
const rawSparkPaymentMaterialPattern = /\bspark1[0-9a-z]{20,}\b/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const safeFalseTracePolicyJsonReplacements: ReadonlyArray<
  readonly [RegExp, string]
> = [
  [/"rawPromptAllowed":false/g, '"promptMaterialDenied":true'],
  [/"rawProviderPayloadAllowed":false/g, '"modelRequestBodyDenied":true'],
  [/"rawRunnerLogAllowed":false/g, '"runnerLogDenied":true'],
  [/"rawSourceArchiveAllowed":false/g, '"sourceArchiveDenied":true'],
]

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

// Relay refs keep the Pylon-declared listening order: the first entry is
// the canonical market relay the provider loop announces on (#4864).
const uniqueOrderedRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => [
  ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
]

const d1MultiValueSelectChunkSize = 80

const chunkRefsForD1Select = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<ReadonlyArray<string>> => {
  const chunks: Array<ReadonlyArray<string>> = []

  for (
    let index = 0;
    index < refs.length;
    index += d1MultiValueSelectChunkSize
  ) {
    chunks.push(refs.slice(index, index + d1MultiValueSelectChunkSize))
  }

  return chunks
}

const mergeRowsByUpdatedAtDesc = <Row extends Readonly<{ updated_at: string }>>(
  rows: ReadonlyArray<Row>,
  limit: number,
): ReadonlyArray<Row> =>
  [...rows]
    .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
    .slice(0, Math.max(1, Math.trunc(limit)))

export const pylonApiPayloadHasPrivateMaterial = (value: unknown): boolean => {
  const json = safeFalseTracePolicyJsonReplacements.reduce(
    (current, [pattern, replacement]) => current.replace(pattern, replacement),
    JSON.stringify(value),
  )

  return (
    containsProviderSecretMaterial(json) ||
    unsafePylonApiMaterialPattern.test(json) ||
    exactBalanceRefPattern.test(json) ||
    rawSparkPaymentMaterialPattern.test(json) ||
    rawTimestampPattern.test(json)
  )
}

export const assertPylonApiPayloadSafe = (
  label: string,
  value: unknown,
): void => {
  if (pylonApiPayloadHasPrivateMaterial(value)) {
    throw new PylonApiStoreError({
      kind: 'validation_error',
      reason: `${label} contains wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, private channel state, provider secrets, raw logs, private paths, customer data, or raw timestamps.`,
    })
  }
}

export const pylonApiStoreErrorFromUnknown = (
  error: unknown,
): PylonApiStoreError =>
  error instanceof PylonApiStoreError
    ? error
    : new PylonApiStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

const codingServiceCapacityRefPattern =
  /^capacity\.coding\.(codex|claude)\.(ready|available)=(\d+)$/
const codingServiceLoadRefPattern =
  /^load\.coding\.(codex|claude)\.(busy|queued)=(\d+)$/

const boundedRefInteger = (value: string): number => {
  const parsed = Number.parseInt(value, 10)

  return Number.isSafeInteger(parsed) && parsed >= 0
    ? Math.min(parsed, 1_000_000)
    : 0
}

export const pylonCodingServiceCapacityProjection = (
  record: PylonApiRegistrationRecord,
): ReadonlyArray<PylonCodingServiceCapacityProjection> => {
  const byService = new Map<
    PylonCodingServiceCapacityProjection['service'],
    { available: number; busy: number; queued: number; ready: number }
  >()
  const serviceSlot = (
    service: PylonCodingServiceCapacityProjection['service'],
  ) => {
    const existing = byService.get(service)
    if (existing !== undefined) {
      return existing
    }
    const next = { available: 0, busy: 0, queued: 0, ready: 0 }
    byService.set(service, next)
    return next
  }

  for (const ref of record.latestCapacityRefs) {
    const match = codingServiceCapacityRefPattern.exec(ref)
    if (match === null) {
      continue
    }

    const slot = serviceSlot(
      match[1] as PylonCodingServiceCapacityProjection['service'],
    )
    slot[match[2] as 'available' | 'ready'] = boundedRefInteger(match[3]!)
  }

  for (const ref of record.latestLoadRefs) {
    const match = codingServiceLoadRefPattern.exec(ref)
    if (match === null) {
      continue
    }

    const slot = serviceSlot(
      match[1] as PylonCodingServiceCapacityProjection['service'],
    )
    slot[match[2] as 'busy' | 'queued'] = boundedRefInteger(match[3]!)
  }

  return [...byService.entries()]
    .map(([service, slot]) => ({
      available: slot.available,
      busy: slot.busy,
      queued: slot.queued,
      ready: slot.ready,
      service,
    }))
    .sort((left, right) => left.service.localeCompare(right.service))
}

export const publicPylonApiRegistrationProjection = (
  record: PylonApiRegistrationRecord,
  nowIso: string,
  // #5306: Spark payout-target readiness, resolved from the private operator
  // store keyed by pylonRef. Defaults fail-closed to not-ready so any caller
  // that has not (or could not) read the store projects a visible gap rather
  // than a fabricated target. The raw `spark1…` never reaches this function.
  sparkPayoutTargetReadiness: PylonSparkPayoutTargetReadiness = SPARK_PAYOUT_TARGET_NOT_READY,
): PylonApiRegistrationProjection => ({
  capabilityRefs: publicScannerSafeRefs(
    'capability.public.pylon',
    record.capabilityRefs,
  ),
  codingCapacity: pylonCodingServiceCapacityProjection(record),
  clientProtocolVersion: record.clientProtocolVersion,
  clientVersion: record.clientVersion,
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.createdAt,
    nowIso,
  ),
  displayName: record.displayName,
  latestCapacityRefs: publicScannerSafeRefs(
    'capacity.public.pylon',
    record.latestCapacityRefs,
  ),
  latestHeartbeatDisplay:
    record.latestHeartbeatAt === null
      ? null
      : friendlyBlueprintMissionBriefingTime(record.latestHeartbeatAt, nowIso),
  latestHeartbeatStatus: record.latestHeartbeatStatus,
  latestHealthRefs: publicScannerSafeRefs(
    'health.public.pylon',
    record.latestHealthRefs,
  ),
  latestLoadRefs: publicScannerSafeRefs(
    'load.public.pylon',
    record.latestLoadRefs,
  ),
  latestResourceMode: record.latestResourceMode,
  ownerAgentRef: `agent:${record.ownerAgentUserId}`,
  // NOTE: the provider Nostr pubkey and npub are deliberately projected
  // verbatim and never routed through publicScannerSafeRef. A 64-char hex
  // pubkey matches the long-base64url raw-id scanner shape, and aliasing
  // it would defeat the stranger-buyer mapping this field exists for: the
  // same hex pubkey appears as event.pubkey on the provider's public
  // NIP-89/NIP-90 relay traffic, so this is an allowlisted public
  // identity, not a raw id leak. Intake validation pins the exact shapes
  // (hex64 / npub bech32 / ws(s) relay URL).
  providerMarketRelayRefs: record.providerMarketRelayRefs,
  providerNip90LaneRefs: publicScannerSafeRefs(
    'lane.public.pylon_nip90',
    record.providerNip90LaneRefs,
  ),
  providerNostrNpub: record.providerNostrNpub,
  providerNostrPubkey: record.providerNostrPubkey,
  pylonRef: record.pylonRef,
  resourceMode: record.resourceMode,
  sparkPayoutTargetReady: sparkPayoutTargetReadiness.ready,
  sparkPayoutTargetRef: sparkPayoutTargetReadiness.ref,
  status: record.status,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.updatedAt,
    nowIso,
  ),
  walletReady: record.walletReady,
  walletRef:
    record.walletRef === null
      ? null
      : publicScannerSafeRef('wallet.public.pylon', record.walletRef),
})

export const publicPylonApiEventProjection = (
  record: PylonApiEventRecord,
  nowIso: string,
): PylonApiEventProjection => ({
  assignmentRef: record.assignmentRef,
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.createdAt,
    nowIso,
  ),
  eventKind: record.eventKind,
  eventRef: record.eventRef,
  pylonRef: record.pylonRef,
  status: record.status,
})

const terminalAssignmentStates: ReadonlyArray<PylonApiAssignmentState> = [
  'accepted_work',
  'cancelled',
  'rejected',
]

const assignmentLeaseState = (
  record: PylonApiAssignmentRecord,
  nowIso: string,
): PylonApiAssignmentProjection['leaseState'] => {
  if (terminalAssignmentStates.includes(record.state)) {
    return 'terminal'
  }

  return Date.parse(record.leaseExpiresAt) <= Date.parse(nowIso)
    ? 'expired'
    : 'active'
}

const assignmentLeaseExpiresInSeconds = (
  record: PylonApiAssignmentRecord,
  nowIso: string,
): number =>
  Math.max(
    0,
    Math.floor((Date.parse(record.leaseExpiresAt) - Date.parse(nowIso)) / 1000),
  )

export const publicPylonApiAssignmentProjection = (
  record: PylonApiAssignmentRecord,
  nowIso: string,
): PylonApiAssignmentProjection => ({
  acceptanceCriteriaRefs: publicScannerSafeRefs(
    'acceptance_criteria.public.pylon_assignment',
    record.acceptanceCriteriaRefs,
  ),
  acceptedWorkRefs: publicScannerSafeRefs(
    'accepted_work.public.pylon_assignment',
    record.acceptedWorkRefs,
  ),
  artifactRefs: publicScannerSafeRefs(
    'artifact.public.pylon_assignment',
    record.artifactRefs,
  ),
  assignmentRef: record.assignmentRef,
  closeoutRefs: publicScannerSafeRefs(
    'closeout.public.pylon_assignment',
    record.closeoutRefs,
  ),
  codingAssignment: record.codingAssignment,
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.createdAt,
    nowIso,
  ),
  jobKind: record.jobKind,
  leaseExpiresInSeconds: assignmentLeaseExpiresInSeconds(record, nowIso),
  leaseState: assignmentLeaseState(record, nowIso),
  proofRefs: publicScannerSafeRefs(
    'proof.public.pylon_assignment',
    record.proofRefs,
  ),
  pylonRef: record.pylonRef,
  rejectionRefs: publicScannerSafeRefs(
    'rejection.public.pylon_assignment',
    record.rejectionRefs,
  ),
  resultExpectationRefs: publicScannerSafeRefs(
    'result_expectation.public.pylon_assignment',
    record.resultExpectationRefs,
  ),
  state:
    assignmentLeaseState(record, nowIso) === 'expired' ? 'stale' : record.state,
  taskRefs: publicScannerSafeRefs(
    'task.public.pylon_assignment',
    record.taskRefs,
  ),
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.updatedAt,
    nowIso,
  ),
})

export const buildPylonApiRegistrationRecord = (
  input: Readonly<{
    credentialId: string
    displayName: string
    makeId: () => string
    nowIso: string
    ownerAgentTokenPrefix: string
    ownerAgentUserId: string
    request: PylonApiRegistrationRequest
  }>,
): PylonApiRegistrationRecord => {
  assertPylonApiPayloadSafe('pylon registration request', input.request)

  const pylonRef =
    input.request.pylonRef ??
    `pylon.${input.ownerAgentUserId.replaceAll(/[^a-z0-9]+/gi, '-').toLowerCase()}`
  const record: PylonApiRegistrationRecord = {
    capabilityRefs: uniqueRefs(input.request.capabilityRefs),
    clientProtocolVersion: input.request.clientProtocolVersion ?? null,
    clientVersion: input.request.clientVersion ?? null,
    createdAt: input.nowIso,
    displayName: input.request.displayName ?? input.displayName,
    id: `pylon_api_registration_${input.makeId()}`,
    latestHeartbeatAt: null,
    latestHeartbeatStatus: null,
    latestCapacityRefs: [],
    latestHealthRefs: [],
    latestLoadRefs: [],
    latestResourceMode: null,
    ownerAgentCredentialId: input.credentialId,
    ownerAgentTokenPrefix: input.ownerAgentTokenPrefix,
    ownerAgentUserId: input.ownerAgentUserId,
    providerMarketRelayRefs: uniqueOrderedRefs(
      input.request.providerMarketRelayRefs,
    ),
    providerNip90LaneRefs: uniqueRefs(input.request.providerNip90LaneRefs),
    providerNostrNpub: input.request.providerNostrNpub ?? null,
    providerNostrPubkey: input.request.providerNostrPubkey ?? null,
    publicProjectionJson: '{}',
    pylonRef,
    resourceMode: input.request.resourceMode ?? 'background_20',
    status: 'active',
    updatedAt: input.nowIso,
    walletReady: false,
    walletRef: input.request.walletRef ?? null,
  }
  const projection = publicPylonApiRegistrationProjection(record, input.nowIso)

  return {
    ...record,
    publicProjectionJson: JSON.stringify(projection),
  }
}

export const buildPylonApiEventRecord = (
  input: Readonly<{
    assignmentRef?: string | undefined
    body: Record<string, unknown>
    eventKind: PylonApiEventKind
    idempotencyKeyHash: string
    makeId: () => string
    nowIso: string
    ownerAgentUserId: string
    pylonRef: string
    status: string
  }>,
): PylonApiEventRecord => {
  assertPylonApiPayloadSafe(`pylon ${input.eventKind} request`, input.body)

  const eventRef = `pylon_event.${input.eventKind}.${input.makeId()}`
  const record: PylonApiEventRecord = {
    assignmentRef: input.assignmentRef ?? null,
    createdAt: input.nowIso,
    eventBody: input.body,
    eventKind: input.eventKind,
    eventRef,
    id: `pylon_api_event_${input.makeId()}`,
    idempotencyKeyHash: input.idempotencyKeyHash,
    ownerAgentUserId: input.ownerAgentUserId,
    publicProjectionJson: '{}',
    pylonRef: input.pylonRef,
    status: input.status,
  }
  const projection = publicPylonApiEventProjection(record, input.nowIso)

  return {
    ...record,
    publicProjectionJson: JSON.stringify(projection),
  }
}

const leaseSecondsForRequest = (
  request: PylonApiCreateAssignmentRequest,
): number => {
  const leaseSeconds = request.leaseSeconds ?? 15 * 60

  if (
    !Number.isFinite(leaseSeconds) ||
    leaseSeconds < 60 ||
    leaseSeconds > 86_400
  ) {
    throw new PylonApiStoreError({
      kind: 'validation_error',
      reason: 'leaseSeconds must be between 60 and 86400.',
    })
  }

  return Math.floor(leaseSeconds)
}

export const buildPylonApiAssignmentRecord = (
  input: Readonly<{
    idempotencyKeyHash: string
    makeId: () => string
    nowIso: string
    ownerAgentUserId: string
    request: PylonApiCreateAssignmentRequest
  }>,
): PylonApiAssignmentRecord => {
  assertPylonApiPayloadSafe('pylon assignment request', input.request)

  const id = input.makeId()
  const record: PylonApiAssignmentRecord = {
    acceptanceCriteriaRefs: uniqueRefs(input.request.acceptanceCriteriaRefs),
    acceptedWorkRefs: [],
    artifactRefs: [],
    assignmentRef:
      input.request.assignmentRef ?? `assignment.public.pylon_api.${id}`,
    closeoutRefs: [],
    codingAssignment: input.request.codingAssignment ?? null,
    createdAt: input.nowIso,
    id: `pylon_api_assignment_${input.makeId()}`,
    idempotencyKeyHash: input.idempotencyKeyHash,
    jobKind: input.request.jobKind,
    leaseExpiresAt: isoTimestampAfterIso(
      input.nowIso,
      leaseSecondsForRequest(input.request) * 1000,
    ),
    ownerAgentUserId: input.ownerAgentUserId,
    proofRefs: [],
    publicProjectionJson: '{}',
    pylonRef: input.request.pylonRef,
    rejectionRefs: [],
    resultExpectationRefs: uniqueRefs(input.request.resultExpectationRefs),
    state: 'offered',
    taskRefs: uniqueRefs(input.request.taskRefs),
    updatedAt: input.nowIso,
  }
  const projection = publicPylonApiAssignmentProjection(record, input.nowIso)

  return {
    ...record,
    publicProjectionJson: JSON.stringify(projection),
  }
}

const stringRefsFromEventBody = (
  body: Record<string, unknown>,
  key: string,
  fallback: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const value = body[key]

  return typeof value === 'object' && Array.isArray(value)
    ? value.filter((ref): ref is string => typeof ref === 'string')
    : fallback
}

export const nextAssignmentForEvent = (
  assignment: PylonApiAssignmentRecord,
  event: PylonApiEventRecord,
  nowIso: string,
): PylonApiAssignmentRecord => {
  const body = event.eventBody
  const next: PylonApiAssignmentRecord =
    event.eventKind === 'assignment_acceptance'
      ? {
          ...assignment,
          state: body.accepted === false ? 'rejected' : 'accepted',
          updatedAt: nowIso,
        }
      : event.eventKind === 'assignment_progress'
        ? {
            ...assignment,
            state: event.status === 'blocked' ? 'blocked' : 'running',
            updatedAt: nowIso,
          }
        : event.eventKind === 'artifact_proof_metadata'
          ? {
              ...assignment,
              artifactRefs: uniqueRefs(
                stringRefsFromEventBody(
                  body,
                  'artifactRefs',
                  assignment.artifactRefs,
                ),
              ),
              proofRefs: uniqueRefs(
                stringRefsFromEventBody(
                  body,
                  'proofRefs',
                  assignment.proofRefs,
                ),
              ),
              state: 'proof_submitted',
              updatedAt: nowIso,
            }
          : event.eventKind === 'worker_closeout'
            ? {
                ...assignment,
                artifactRefs: uniqueRefs(
                  stringRefsFromEventBody(
                    body,
                    'artifactRefs',
                    assignment.artifactRefs,
                  ),
                ),
                closeoutRefs: uniqueRefs(
                  stringRefsFromEventBody(
                    body,
                    'closeoutRefs',
                    assignment.closeoutRefs,
                  ),
                ),
                proofRefs: uniqueRefs(
                  stringRefsFromEventBody(
                    body,
                    'proofRefs',
                    assignment.proofRefs,
                  ),
                ),
                state: 'closeout_submitted',
                updatedAt: nowIso,
              }
            : {
                ...assignment,
                updatedAt: nowIso,
              }
  const projection = publicPylonApiAssignmentProjection(next, nowIso)

  return {
    ...next,
    publicProjectionJson: JSON.stringify(projection),
  }
}

export const closeoutPylonApiAssignmentRecord = (
  input: Readonly<{
    assignment: PylonApiAssignmentRecord
    nowIso: string
    request: PylonApiAssignmentCloseoutRequest
  }>,
): PylonApiAssignmentRecord => {
  assertPylonApiPayloadSafe('pylon assignment closeout request', input.request)

  const acceptedWorkRefs = uniqueRefs(input.request.acceptedWorkRefs)
  const rejectionRefs = uniqueRefs(input.request.rejectionRefs)
  const closeoutRefs = uniqueRefs(input.request.closeoutRefs)

  if (input.request.accepted && acceptedWorkRefs.length === 0) {
    throw new PylonApiStoreError({
      kind: 'validation_error',
      reason: 'acceptedWorkRefs are required for accepted work closeout.',
    })
  }

  if (!input.request.accepted && rejectionRefs.length === 0) {
    throw new PylonApiStoreError({
      kind: 'validation_error',
      reason: 'rejectionRefs are required for rejected work closeout.',
    })
  }

  if (
    input.request.accepted &&
    input.assignment.artifactRefs.length === 0 &&
    input.assignment.proofRefs.length === 0
  ) {
    throw new PylonApiStoreError({
      kind: 'conflict',
      reason:
        'accepted work closeout requires retained artifact or proof refs.',
    })
  }

  const next: PylonApiAssignmentRecord = {
    ...input.assignment,
    acceptedWorkRefs: input.request.accepted ? acceptedWorkRefs : [],
    closeoutRefs,
    rejectionRefs: input.request.accepted ? [] : rejectionRefs,
    state: input.request.accepted ? 'accepted_work' : 'rejected',
    updatedAt: input.nowIso,
  }

  return {
    ...next,
    publicProjectionJson: JSON.stringify(
      publicPylonApiAssignmentProjection(next, input.nowIso),
    ),
  }
}

export const nextRegistrationForEvent = (
  registration: PylonApiRegistrationRecord,
  event: PylonApiEventRecord,
  nowIso: string,
): PylonApiRegistrationRecord => {
  const body = event.eventBody
  const clientVersion =
    typeof body.clientVersion === 'string'
      ? body.clientVersion
      : registration.clientVersion
  const clientProtocolVersion =
    typeof body.clientProtocolVersion === 'string'
      ? body.clientProtocolVersion
      : registration.clientProtocolVersion
  const walletRef =
    typeof body.walletRef === 'string' ? body.walletRef : registration.walletRef
  const resourceMode = Option.getOrElse(
    typeof body.resourceMode === 'string'
      ? S.decodeUnknownOption(PylonResourceMode)(body.resourceMode)
      : Option.none<typeof PylonResourceMode.Type>(),
    () => registration.resourceMode,
  )
  const walletReady =
    typeof body.walletReady === 'boolean'
      ? body.walletReady
      : registration.walletReady
  const providerNostrPubkey =
    typeof body.providerNostrPubkey === 'string'
      ? body.providerNostrPubkey
      : registration.providerNostrPubkey
  const providerNostrNpub =
    typeof body.providerNostrNpub === 'string'
      ? body.providerNostrNpub
      : registration.providerNostrNpub
  const providerMarketRelayRefs = Array.isArray(body.providerMarketRelayRefs)
    ? uniqueOrderedRefs(
        body.providerMarketRelayRefs.filter(
          (ref): ref is string => typeof ref === 'string',
        ),
      )
    : registration.providerMarketRelayRefs
  const providerNip90LaneRefs = Array.isArray(body.providerNip90LaneRefs)
    ? uniqueRefs(
        body.providerNip90LaneRefs.filter(
          (ref): ref is string => typeof ref === 'string',
        ),
      )
    : registration.providerNip90LaneRefs
  const isHeartbeat = event.eventKind === 'heartbeat'
  const next: PylonApiRegistrationRecord = {
    ...registration,
    clientProtocolVersion,
    clientVersion,
    latestHeartbeatAt: isHeartbeat
      ? event.createdAt
      : registration.latestHeartbeatAt,
    latestHeartbeatStatus: isHeartbeat
      ? event.status
      : registration.latestHeartbeatStatus,
    latestCapacityRefs:
      isHeartbeat && Array.isArray(body.capacityRefs)
        ? uniqueRefs(
            body.capacityRefs.filter(
              (ref): ref is string => typeof ref === 'string',
            ),
          )
        : registration.latestCapacityRefs,
    latestHealthRefs:
      isHeartbeat && Array.isArray(body.healthRefs)
        ? uniqueRefs(
            body.healthRefs.filter(
              (ref): ref is string => typeof ref === 'string',
            ),
          )
        : registration.latestHealthRefs,
    latestLoadRefs:
      isHeartbeat && Array.isArray(body.loadRefs)
        ? uniqueRefs(
            body.loadRefs.filter(
              (ref): ref is string => typeof ref === 'string',
            ),
          )
        : registration.latestLoadRefs,
    latestResourceMode: isHeartbeat
      ? resourceMode
      : registration.latestResourceMode,
    providerMarketRelayRefs,
    providerNip90LaneRefs,
    providerNostrNpub,
    providerNostrPubkey,
    resourceMode,
    updatedAt: nowIso,
    walletReady,
    walletRef,
  }

  return {
    ...next,
    publicProjectionJson: JSON.stringify(
      publicPylonApiRegistrationProjection(next, nowIso),
    ),
  }
}

const rowToRegistration = (
  row: PylonApiRegistrationRow,
): PylonApiRegistrationRecord => ({
  capabilityRefs: parseJsonStringArray(row.capability_refs_json),
  clientProtocolVersion: row.client_protocol_version ?? null,
  clientVersion: row.client_version ?? null,
  createdAt: row.created_at,
  displayName: row.display_name,
  id: row.id,
  latestHeartbeatAt: row.latest_heartbeat_at,
  latestHeartbeatStatus: row.latest_heartbeat_status ?? null,
  latestCapacityRefs: parseJsonStringArray(
    row.latest_capacity_refs_json ?? '[]',
  ),
  latestHealthRefs: parseJsonStringArray(row.latest_health_refs_json ?? '[]'),
  latestLoadRefs: parseJsonStringArray(row.latest_load_refs_json ?? '[]'),
  latestResourceMode: row.latest_resource_mode ?? null,
  ownerAgentCredentialId: row.owner_agent_credential_id,
  ownerAgentTokenPrefix: row.owner_agent_token_prefix,
  ownerAgentUserId: row.owner_agent_user_id,
  providerMarketRelayRefs: parseJsonStringArray(
    row.provider_market_relay_refs_json ?? '[]',
  ),
  providerNip90LaneRefs: parseJsonStringArray(
    row.provider_nip90_lane_refs_json ?? '[]',
  ),
  providerNostrNpub: row.provider_nostr_npub ?? null,
  providerNostrPubkey: row.provider_nostr_pubkey ?? null,
  publicProjectionJson: row.public_projection_json,
  pylonRef: row.pylon_ref,
  resourceMode: row.resource_mode,
  status: row.status,
  updatedAt: row.updated_at,
  walletReady: row.wallet_ready === 1,
  walletRef: row.wallet_ref,
})

const rowToEvent = (row: PylonApiEventRow): PylonApiEventRecord => ({
  assignmentRef: row.assignment_ref,
  createdAt: row.created_at,
  eventBody: parseJsonRecord(row.event_body_json) ?? {},
  eventKind: row.event_kind,
  eventRef: row.event_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  ownerAgentUserId: row.owner_agent_user_id,
  publicProjectionJson: row.public_projection_json,
  pylonRef: row.pylon_ref,
  status: row.status,
})

const rowToAssignment = (
  row: PylonApiAssignmentRow,
): PylonApiAssignmentRecord => ({
  acceptanceCriteriaRefs: parseJsonStringArray(
    row.acceptance_criteria_refs_json,
  ),
  acceptedWorkRefs: parseJsonStringArray(row.accepted_work_refs_json),
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  assignmentRef: row.assignment_ref,
  closeoutRefs: parseJsonStringArray(row.closeout_refs_json),
  codingAssignment: parseJsonRecord(row.coding_assignment_json ?? '') ?? null,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  jobKind: row.job_kind,
  leaseExpiresAt: row.lease_expires_at,
  ownerAgentUserId: row.owner_agent_user_id,
  proofRefs: parseJsonStringArray(row.proof_refs_json),
  publicProjectionJson: row.public_projection_json,
  pylonRef: row.pylon_ref,
  rejectionRefs: parseJsonStringArray(row.rejection_refs_json),
  resultExpectationRefs: parseJsonStringArray(row.result_expectation_refs_json),
  state: row.state,
  taskRefs: parseJsonStringArray(row.task_refs_json),
  updatedAt: row.updated_at,
})

const lifecycleStageForAssignment = (
  assignment: PylonApiAssignmentRecord,
): PylonProviderJobLifecycleStage =>
  assignment.state === 'accepted_work'
    ? 'accepted_work'
    : assignment.state === 'closeout_submitted'
      ? 'closeout_submitted'
      : assignment.artifactRefs.length > 0 || assignment.proofRefs.length > 0
        ? 'artifact_submitted'
        : assignment.state === 'running'
          ? 'running'
          : assignment.state === 'accepted'
            ? 'accepted'
            : 'offered'

const publicProviderJobLifecycleProjection = (
  record: PylonApiProviderJobLifecycleRecord,
): Record<string, unknown> => ({
  assignmentRef: record.assignmentRef,
  stage: record.stage,
})

export const providerJobLifecycleRecordFromAssignment = (
  assignment: PylonApiAssignmentRecord,
): PylonApiProviderJobLifecycleRecord => {
  const record: PylonApiProviderJobLifecycleRecord = {
    acceptedWorkRefs: assignment.acceptedWorkRefs,
    artifactRefs: assignment.artifactRefs,
    assignmentRef: assignment.assignmentRef,
    closeoutRefs: assignment.closeoutRefs,
    createdAt: assignment.createdAt,
    id: `pylon_provider_job_lifecycle_${assignment.id}`,
    jobKind: assignment.jobKind,
    ownerAgentUserId: assignment.ownerAgentUserId,
    proofRefs: assignment.proofRefs,
    publicProjectionJson: '{}',
    pylonRef: assignment.pylonRef,
    stage: lifecycleStageForAssignment(assignment),
    taskRefs: assignment.taskRefs,
    updatedAt: assignment.updatedAt,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicProviderJobLifecycleProjection(record),
    ),
  }
}

const rowToProviderJobLifecycle = (
  row: PylonProviderJobLifecycleRow,
): PylonApiProviderJobLifecycleRecord => ({
  acceptedWorkRefs: parseJsonStringArray(row.accepted_work_refs_json),
  artifactRefs: parseJsonStringArray(row.artifact_refs_json),
  assignmentRef: row.assignment_ref,
  closeoutRefs: parseJsonStringArray(row.closeout_refs_json),
  createdAt: row.created_at,
  id: row.id,
  jobKind: row.job_kind,
  ownerAgentUserId: row.owner_agent_user_id,
  proofRefs: parseJsonStringArray(row.proof_refs_json),
  publicProjectionJson: row.public_projection_json,
  pylonRef: row.pylon_ref,
  stage: row.stage,
  taskRefs: parseJsonStringArray(row.task_refs_json),
  updatedAt: row.updated_at,
})

const insertAssignmentStatement = (
  db: D1Database,
  record: PylonApiAssignmentRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO pylon_api_assignments
        (id, assignment_ref, pylon_ref, owner_agent_user_id,
         idempotency_key_hash, job_kind, state, lease_expires_at,
         task_refs_json, acceptance_criteria_refs_json,
         result_expectation_refs_json, artifact_refs_json, proof_refs_json,
         accepted_work_refs_json, rejection_refs_json, closeout_refs_json,
         coding_assignment_json, public_projection_json, created_at,
         updated_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      record.id,
      record.assignmentRef,
      record.pylonRef,
      record.ownerAgentUserId,
      record.idempotencyKeyHash,
      record.jobKind,
      record.state,
      record.leaseExpiresAt,
      JSON.stringify(record.taskRefs),
      JSON.stringify(record.acceptanceCriteriaRefs),
      JSON.stringify(record.resultExpectationRefs),
      JSON.stringify(record.artifactRefs),
      JSON.stringify(record.proofRefs),
      JSON.stringify(record.acceptedWorkRefs),
      JSON.stringify(record.rejectionRefs),
      JSON.stringify(record.closeoutRefs),
      record.codingAssignment === null
        ? null
        : JSON.stringify(record.codingAssignment),
      record.publicProjectionJson,
      record.createdAt,
      record.updatedAt,
    )

const upsertProviderJobLifecycleStatement = (
  db: D1Database,
  record: PylonApiProviderJobLifecycleRecord,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO pylon_provider_job_lifecycle
        (id, pylon_ref, assignment_ref, owner_agent_user_id, job_kind, stage,
         task_refs_json, artifact_refs_json, proof_refs_json, closeout_refs_json,
         accepted_work_refs_json, public_projection_json, created_at,
         updated_at, archived_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)
       ON CONFLICT(assignment_ref) DO UPDATE SET
         pylon_ref = excluded.pylon_ref,
         owner_agent_user_id = excluded.owner_agent_user_id,
         job_kind = excluded.job_kind,
         stage = excluded.stage,
         task_refs_json = excluded.task_refs_json,
         artifact_refs_json = excluded.artifact_refs_json,
         proof_refs_json = excluded.proof_refs_json,
         closeout_refs_json = excluded.closeout_refs_json,
         accepted_work_refs_json = excluded.accepted_work_refs_json,
         public_projection_json = excluded.public_projection_json,
         updated_at = excluded.updated_at,
         archived_at = NULL`,
    )
    .bind(
      record.id,
      record.pylonRef,
      record.assignmentRef,
      record.ownerAgentUserId,
      record.jobKind,
      record.stage,
      JSON.stringify(record.taskRefs),
      JSON.stringify(record.artifactRefs),
      JSON.stringify(record.proofRefs),
      JSON.stringify(record.closeoutRefs),
      JSON.stringify(record.acceptedWorkRefs),
      record.publicProjectionJson,
      record.createdAt,
      record.updatedAt,
    )

export const makeD1PylonApiStore = (db: D1Database): PylonApiStore => ({
  createAssignment: async record => {
    const existing = await makeD1PylonApiStore(
      db,
    ).readAssignmentByIdempotencyKeyHash(record.idempotencyKeyHash)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    await db.batch([
      insertAssignmentStatement(db, record),
      upsertProviderJobLifecycleStatement(
        db,
        providerJobLifecycleRecordFromAssignment(record),
      ),
    ])

    return { idempotent: false, record }
  },

  createEvent: async record => {
    const existing = await makeD1PylonApiStore(
      db,
    ).readEventByIdempotencyKeyHash(record.idempotencyKeyHash)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    await db
      .prepare(
        `INSERT INTO pylon_api_events
          (id, event_ref, pylon_ref, owner_agent_user_id, idempotency_key_hash,
           event_kind, assignment_ref, status, event_body_json,
           public_projection_json, created_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        record.id,
        record.eventRef,
        record.pylonRef,
        record.ownerAgentUserId,
        record.idempotencyKeyHash,
        record.eventKind,
        record.assignmentRef,
        record.status,
        JSON.stringify(record.eventBody),
        record.publicProjectionJson,
        record.createdAt,
      )
      .run()

    return { idempotent: false, record }
  },

  listAssignmentsForPylon: async (pylonRef, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM pylon_api_assignments
          WHERE pylon_ref = ?
            AND archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(pylonRef, limit)
      .all<PylonApiAssignmentRow>()

    return (result.results ?? []).map(rowToAssignment)
  },

  listAssignmentsForPylons: async (pylonRefs, limit) => {
    if (pylonRefs.length === 0) {
      return []
    }

    const rows: PylonApiAssignmentRow[] = []

    for (const pylonRefChunk of chunkRefsForD1Select(
      uniqueOrderedRefs(pylonRefs),
    )) {
      const placeholders = pylonRefChunk.map(() => '?').join(', ')
      const result = await db
        .prepare(
          `SELECT *
             FROM pylon_api_assignments
            WHERE pylon_ref IN (${placeholders})
              AND archived_at IS NULL
            ORDER BY updated_at DESC
            LIMIT ?`,
        )
        .bind(...pylonRefChunk, limit)
        .all<PylonApiAssignmentRow>()

      rows.push(...(result.results ?? []))
    }

    return mergeRowsByUpdatedAtDesc(rows, limit).map(rowToAssignment)
  },

  listEventsForPylon: async (pylonRef, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM pylon_api_events
          WHERE pylon_ref = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(pylonRef, limit)
      .all<PylonApiEventRow>()

    return (result.results ?? []).map(rowToEvent)
  },

  listEventsForAssignment: async (assignmentRef, limit) => {
    const result = await db
      .prepare(
        `SELECT *
           FROM pylon_api_events
          WHERE assignment_ref = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(assignmentRef, limit)
      .all<PylonApiEventRow>()

    return (result.results ?? []).map(rowToEvent)
  },

  listRegistrations: async limit => {
    const result = await db
      .prepare(
        `SELECT *
           FROM pylon_api_registrations
          WHERE archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<PylonApiRegistrationRow>()

    return (result.results ?? []).map(rowToRegistration)
  },

  listRegistrationsForOwnerAgentUserIds: async (ownerAgentUserIds, limit) => {
    if (ownerAgentUserIds.length === 0) {
      return []
    }

    const boundedLimit = Math.max(1, Math.min(Math.trunc(limit), 200))
    const rows: PylonApiRegistrationRow[] = []

    for (const ownerIdChunk of chunkRefsForD1Select(
      uniqueOrderedRefs(ownerAgentUserIds).slice(0, 200),
    )) {
      const placeholders = ownerIdChunk.map(() => '?').join(', ')
      const result = await db
        .prepare(
          `SELECT *
             FROM pylon_api_registrations
            WHERE owner_agent_user_id IN (${placeholders})
              AND archived_at IS NULL
            ORDER BY updated_at DESC
            LIMIT ?`,
        )
        .bind(...ownerIdChunk, boundedLimit)
        .all<PylonApiRegistrationRow>()

      rows.push(...(result.results ?? []))
    }

    return mergeRowsByUpdatedAtDesc(rows, boundedLimit).map(rowToRegistration)
  },

  listProviderJobLifecycleForPylons: async (pylonRefs, limit) => {
    if (pylonRefs.length === 0) {
      return []
    }

    const rows: PylonProviderJobLifecycleRow[] = []

    for (const pylonRefChunk of chunkRefsForD1Select(
      uniqueOrderedRefs(pylonRefs),
    )) {
      const placeholders = pylonRefChunk.map(() => '?').join(', ')
      const result = await db
        .prepare(
          `SELECT *
             FROM pylon_provider_job_lifecycle
            WHERE pylon_ref IN (${placeholders})
              AND archived_at IS NULL
            ORDER BY updated_at DESC
            LIMIT ?`,
        )
        .bind(...pylonRefChunk, limit)
        .all<PylonProviderJobLifecycleRow>()

      rows.push(...(result.results ?? []))
    }

    return mergeRowsByUpdatedAtDesc(rows, limit).map(rowToProviderJobLifecycle)
  },

  readEventByIdempotencyKeyHash: async idempotencyKeyHash => {
    const row = await db
      .prepare(
        `SELECT *
           FROM pylon_api_events
          WHERE idempotency_key_hash = ?
            AND archived_at IS NULL`,
      )
      .bind(idempotencyKeyHash)
      .first<PylonApiEventRow>()

    return row === null ? undefined : rowToEvent(row)
  },

  readAssignment: async assignmentRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM pylon_api_assignments
          WHERE assignment_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(assignmentRef)
      .first<PylonApiAssignmentRow>()

    return row === null ? undefined : rowToAssignment(row)
  },

  readAssignmentByIdempotencyKeyHash: async idempotencyKeyHash => {
    const row = await db
      .prepare(
        `SELECT *
           FROM pylon_api_assignments
          WHERE idempotency_key_hash = ?
            AND archived_at IS NULL`,
      )
      .bind(idempotencyKeyHash)
      .first<PylonApiAssignmentRow>()

    return row === null ? undefined : rowToAssignment(row)
  },

  readRegistration: async pylonRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM pylon_api_registrations
          WHERE pylon_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(pylonRef)
      .first<PylonApiRegistrationRow>()

    return row === null ? undefined : rowToRegistration(row)
  },

  updateAssignment: async record => {
    const publicProjectionJson = JSON.stringify(
      publicPylonApiAssignmentProjection(record, record.updatedAt),
    )

    const update = db
      .prepare(
        `UPDATE pylon_api_assignments
            SET state = ?,
                artifact_refs_json = ?,
                proof_refs_json = ?,
                accepted_work_refs_json = ?,
                rejection_refs_json = ?,
                closeout_refs_json = ?,
                coding_assignment_json = ?,
                public_projection_json = ?,
                updated_at = ?
          WHERE assignment_ref = ?
            AND pylon_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(
        record.state,
        JSON.stringify(record.artifactRefs),
        JSON.stringify(record.proofRefs),
        JSON.stringify(record.acceptedWorkRefs),
        JSON.stringify(record.rejectionRefs),
        JSON.stringify(record.closeoutRefs),
        record.codingAssignment === null
          ? null
          : JSON.stringify(record.codingAssignment),
        publicProjectionJson,
        record.updatedAt,
        record.assignmentRef,
        record.pylonRef,
      )
    const next = {
      ...record,
      publicProjectionJson,
    }

    await db.batch([
      update,
      upsertProviderJobLifecycleStatement(
        db,
        providerJobLifecycleRecordFromAssignment(next),
      ),
    ])

    return next
  },

  updateAssignmentIfState: async (record, expectedState) => {
    const publicProjectionJson = JSON.stringify(
      publicPylonApiAssignmentProjection(record, record.updatedAt),
    )

    const result = await db
      .prepare(
        `UPDATE pylon_api_assignments
            SET state = ?,
                artifact_refs_json = ?,
                proof_refs_json = ?,
                accepted_work_refs_json = ?,
                rejection_refs_json = ?,
                closeout_refs_json = ?,
                coding_assignment_json = ?,
                public_projection_json = ?,
                updated_at = ?
          WHERE assignment_ref = ?
            AND pylon_ref = ?
            AND state = ?
            AND archived_at IS NULL`,
      )
      .bind(
        record.state,
        JSON.stringify(record.artifactRefs),
        JSON.stringify(record.proofRefs),
        JSON.stringify(record.acceptedWorkRefs),
        JSON.stringify(record.rejectionRefs),
        JSON.stringify(record.closeoutRefs),
        record.codingAssignment === null
          ? null
          : JSON.stringify(record.codingAssignment),
        publicProjectionJson,
        record.updatedAt,
        record.assignmentRef,
        record.pylonRef,
        expectedState,
      )
      .run()

    if ((result.meta?.changes ?? 0) < 1) {
      return undefined
    }

    const next = {
      ...record,
      publicProjectionJson,
    }

    await upsertProviderJobLifecycleStatement(
      db,
      providerJobLifecycleRecordFromAssignment(next),
    ).run()

    return next
  },

  upsertProviderJobLifecycle: async record => {
    await upsertProviderJobLifecycleStatement(db, record).run()

    return record
  },

  upsertRegistration: async record => {
    const existing = await makeD1PylonApiStore(db).readRegistration(
      record.pylonRef,
    )

    if (
      existing !== undefined &&
      existing.ownerAgentUserId !== record.ownerAgentUserId
    ) {
      throw new PylonApiStoreError({
        kind: 'conflict',
        reason: 'Pylon ref is already owned by another registered agent.',
      })
    }

    if (existing === undefined) {
      await db
        .prepare(
          `INSERT INTO pylon_api_registrations
            (id, pylon_ref, owner_agent_user_id, owner_agent_credential_id,
             owner_agent_token_prefix, display_name, status, resource_mode,
             capability_refs_json, client_version, client_protocol_version,
             wallet_ref, wallet_ready, latest_heartbeat_at,
             latest_heartbeat_status, latest_resource_mode,
             latest_health_refs_json, latest_load_refs_json,
             latest_capacity_refs_json, provider_nostr_pubkey,
             provider_nostr_npub, provider_market_relay_refs_json,
             provider_nip90_lane_refs_json, public_projection_json, created_at,
             updated_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.pylonRef,
          record.ownerAgentUserId,
          record.ownerAgentCredentialId,
          record.ownerAgentTokenPrefix,
          record.displayName,
          record.status,
          record.resourceMode,
          JSON.stringify(record.capabilityRefs),
          record.clientVersion,
          record.clientProtocolVersion,
          record.walletRef,
          record.walletReady ? 1 : 0,
          record.latestHeartbeatAt,
          record.latestHeartbeatStatus,
          record.latestResourceMode,
          JSON.stringify(record.latestHealthRefs),
          JSON.stringify(record.latestLoadRefs),
          JSON.stringify(record.latestCapacityRefs),
          record.providerNostrPubkey,
          record.providerNostrNpub,
          JSON.stringify(record.providerMarketRelayRefs),
          JSON.stringify(record.providerNip90LaneRefs),
          record.publicProjectionJson,
          record.createdAt,
          record.updatedAt,
        )
        .run()

      return record
    }

    const next: PylonApiRegistrationRecord = {
      ...record,
      createdAt: existing.createdAt,
      id: existing.id,
    }
    const publicProjectionJson = JSON.stringify(
      publicPylonApiRegistrationProjection(next, record.updatedAt),
    )

    await db
      .prepare(
        `UPDATE pylon_api_registrations
            SET owner_agent_credential_id = ?,
                owner_agent_token_prefix = ?,
                display_name = ?,
                status = ?,
                resource_mode = ?,
                capability_refs_json = ?,
                client_version = ?,
                client_protocol_version = ?,
                wallet_ref = ?,
                wallet_ready = ?,
                latest_heartbeat_at = ?,
                latest_heartbeat_status = ?,
                latest_resource_mode = ?,
                latest_health_refs_json = ?,
                latest_load_refs_json = ?,
                latest_capacity_refs_json = ?,
                provider_nostr_pubkey = ?,
                provider_nostr_npub = ?,
                provider_market_relay_refs_json = ?,
                provider_nip90_lane_refs_json = ?,
                public_projection_json = ?,
                updated_at = ?
          WHERE pylon_ref = ?
            AND owner_agent_user_id = ?
            AND archived_at IS NULL`,
      )
      .bind(
        record.ownerAgentCredentialId,
        record.ownerAgentTokenPrefix,
        record.displayName,
        record.status,
        record.resourceMode,
        JSON.stringify(record.capabilityRefs),
        record.clientVersion,
        record.clientProtocolVersion,
        record.walletRef,
        record.walletReady ? 1 : 0,
        record.latestHeartbeatAt,
        record.latestHeartbeatStatus,
        record.latestResourceMode,
        JSON.stringify(record.latestHealthRefs),
        JSON.stringify(record.latestLoadRefs),
        JSON.stringify(record.latestCapacityRefs),
        record.providerNostrPubkey,
        record.providerNostrNpub,
        JSON.stringify(record.providerMarketRelayRefs),
        JSON.stringify(record.providerNip90LaneRefs),
        publicProjectionJson,
        record.updatedAt,
        record.pylonRef,
        record.ownerAgentUserId,
      )
      .run()

    return {
      ...next,
      publicProjectionJson,
    }
  },
})
