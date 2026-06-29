import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { evaluateDurableCheckpointSeal } from './training-durable-checkpoint-seal'
import {
  type PylonJoinLifecycleEventRecord,
  type PylonJoinLifecycleReasonCode,
  type PylonJoinLifecycleRecord,
  type PylonJoinLifecycleState,
  transitionPylonJoinLifecycleRecord,
} from './pylon-join-lifecycle'
import {
  TrainingPublicSafePylonRef,
  TrainingPublicSafeRef,
  type TrainingRunRecord,
  type TrainingWindowRecord,
} from './training-run-window-authority'

// Joiner bootstrap dispatcher rules (Pluralis roadmap P1.2/P1.3,
// openagents issues #4850 and #4851, master tracking issue #4855,
// rail #4673). Two rules, both pure functions over authority records
// with timestamps passed in:
//
// 1. Snapshot-lags-live: a joining device bootstraps from the LAST
//    DURABLE SEAL — a sealed window whose checkpoint digest is durably
//    stored on the seal record — never from any in-flight state. The
//    grant pins the seal's checkpoint digest and the joiner's
//    acceptance must echo it before any work is assigned.
// 2. Join-blocking window: while a merge/seal operation is in flight
//    (the run-level seal barrier is up), bootstrap grants and
//    join-lifecycle transitions are QUEUED with a typed reason code,
//    not rejected; once the barrier clears, the same request proceeds
//    against the new last durable seal.

export const TrainingWindowBootstrapGrantRequest = S.Struct({
  joinerRef: TrainingPublicSafePylonRef,
  receiptRefs: S.optionalKey(S.Array(TrainingPublicSafeRef)),
})
export type TrainingWindowBootstrapGrantRequest =
  typeof TrainingWindowBootstrapGrantRequest.Type

export const TrainingWindowBootstrapAcceptance = S.Struct({
  checkpointDigestRef: TrainingPublicSafeRef,
  grantRef: TrainingPublicSafeRef,
  joinerRef: TrainingPublicSafePylonRef,
})
export type TrainingWindowBootstrapAcceptance =
  typeof TrainingWindowBootstrapAcceptance.Type

// Queued bootstrap grants reuse the join-lifecycle queue-visibility
// reason code so a deferred join is observable through the same closed
// taxonomy the ladder already publishes.
export const TrainingBootstrapQueuedReasonCode =
  'join_lifecycle.public.join_deferred_seal_in_flight' as const

export const TrainingBootstrapRefusalReasonCodes = [
  'training.bootstrap.public.no_durable_seal',
] as const
export type TrainingBootstrapRefusalReasonCode =
  (typeof TrainingBootstrapRefusalReasonCodes)[number]

export const TrainingBootstrapAcceptanceRejectionReasonCodes = [
  'training.bootstrap.public.checkpoint_digest_echo_mismatch',
  'training.bootstrap.public.grant_ref_mismatch',
  'training.bootstrap.public.joiner_ref_mismatch',
] as const
export type TrainingBootstrapAcceptanceRejectionReasonCode =
  (typeof TrainingBootstrapAcceptanceRejectionReasonCodes)[number]

export type TrainingWindowBootstrapGrant = Readonly<{
  checkpointDigestRef: string
  grantRef: string
  joinerReceiptRefs: ReadonlyArray<string>
  joinerRef: string
  sealReceiptRefs: ReadonlyArray<string>
  sealedAtDisplay: string
  sealedWindowRef: string
  trainingRunRef: string
}>

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

export type TrainingWindowBootstrapOutcome =
  | Readonly<{
      grant: TrainingWindowBootstrapGrant
      kind: 'granted'
    }>
  | Readonly<{
      joinerRef: string
      kind: 'queued'
      reasonCode: typeof TrainingBootstrapQueuedReasonCode
      trainingRunRef: string
    }>
  | Readonly<{
      joinerRef: string
      kind: 'refused'
      reason: string
      reasonCode: TrainingBootstrapRefusalReasonCode
      trainingRunRef: string
    }>

/**
 * The last durable seal of a run: the most recently sealed window
 * (sealed or already reconciled) whose seal record carries a durably
 * stored checkpoint digest. A sealed window without a checkpoint
 * digest is not bootstrap material — the digest is what the joiner
 * verifies against, so a digestless seal proves nothing to a joiner.
 */
export const selectLastDurableSealWindow = (
  windows: ReadonlyArray<TrainingWindowRecord>,
): TrainingWindowRecord | undefined =>
  windows
    .filter(window => {
      const durableCheckpointSeal = window.sealMetadata?.durableCheckpointSeal

      return (
        (window.state === 'sealed' || window.state === 'reconciled') &&
        window.sealedAt !== null &&
        durableCheckpointSeal !== undefined &&
        window.sealMetadata?.checkpointDigestRef ===
          durableCheckpointSeal.checkpointDigestRef &&
        durableCheckpointSeal.windowRef === window.windowRef &&
        evaluateDurableCheckpointSeal(durableCheckpointSeal).durable
      )
    })
    .sort(
      (left, right) =>
        right.sealedAt!.localeCompare(left.sealedAt!) ||
        right.windowRef.localeCompare(left.windowRef),
    )[0]

/**
 * Dispatcher decision for one joiner bootstrap request. The authority
 * never hands a joiner anything but the last durable seal: barrier up
 * means queued (typed, replayable, not an error), no durable seal
 * means a typed refusal, and a grant pins the seal's checkpoint digest
 * for the acceptance echo.
 */
export const decideTrainingWindowBootstrapGrant = (
  input: Readonly<{
    joinerReceiptRefs?: ReadonlyArray<string> | undefined
    joinerRef: string
    makeId: () => string
    requestedAtIso: string
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): TrainingWindowBootstrapOutcome => {
  if (input.run.sealInFlightAt !== null) {
    return {
      joinerRef: input.joinerRef,
      kind: 'queued',
      reasonCode: TrainingBootstrapQueuedReasonCode,
      trainingRunRef: input.run.trainingRunRef,
    }
  }

  const seal = selectLastDurableSealWindow(
    input.windows.filter(
      window => window.trainingRunRef === input.run.trainingRunRef,
    ),
  )

  if (seal === undefined) {
    return {
      joinerRef: input.joinerRef,
      kind: 'refused',
      reason:
        'No durable seal exists for this run yet. A joiner bootstraps only from a sealed window whose checkpoint digest is durably stored, never from in-flight state.',
      reasonCode: 'training.bootstrap.public.no_durable_seal',
      trainingRunRef: input.run.trainingRunRef,
    }
  }

  return {
    grant: {
      checkpointDigestRef: seal.sealMetadata!.checkpointDigestRef!,
      grantRef: `training.bootstrap.grant.${input.makeId()}`,
      joinerReceiptRefs: uniqueRefs(input.joinerReceiptRefs ?? []),
      joinerRef: input.joinerRef,
      sealReceiptRefs: seal.receiptRefs,
      sealedAtDisplay: friendlyBlueprintMissionBriefingTime(
        seal.sealedAt!,
        input.requestedAtIso,
      ),
      sealedWindowRef: seal.windowRef,
      trainingRunRef: seal.trainingRunRef,
    },
    kind: 'granted',
  }
}

export type TrainingWindowBootstrapAcceptanceOutcome =
  | Readonly<{
      checkpointDigestRef: string
      grantRef: string
      kind: 'accepted'
      sealedWindowRef: string
    }>
  | Readonly<{
      kind: 'rejected'
      reason: string
      reasonCode: TrainingBootstrapAcceptanceRejectionReasonCode
    }>

/**
 * Contract-level digest-echo validation: a joiner's acceptance must
 * name the exact grant and echo the seal's checkpoint digest before
 * any work is assigned. The live joining-device exercise of this
 * contract is hardware-gated and not claimed here.
 */
export const validateTrainingWindowBootstrapAcceptance = (
  grant: TrainingWindowBootstrapGrant,
  acceptance: TrainingWindowBootstrapAcceptance,
): TrainingWindowBootstrapAcceptanceOutcome => {
  if (acceptance.grantRef !== grant.grantRef) {
    return {
      kind: 'rejected',
      reason: 'Acceptance must name the exact bootstrap grant ref it answers.',
      reasonCode: 'training.bootstrap.public.grant_ref_mismatch',
    }
  }

  if (acceptance.joinerRef !== grant.joinerRef) {
    return {
      kind: 'rejected',
      reason: 'Acceptance joiner ref must match the granted joiner ref.',
      reasonCode: 'training.bootstrap.public.joiner_ref_mismatch',
    }
  }

  if (acceptance.checkpointDigestRef !== grant.checkpointDigestRef) {
    return {
      kind: 'rejected',
      reason:
        "Acceptance must echo the granted seal's checkpoint digest exactly; a joiner that cannot echo the digest has not verified the durable seal it claims to bootstrap from.",
      reasonCode: 'training.bootstrap.public.checkpoint_digest_echo_mismatch',
    }
  }

  return {
    checkpointDigestRef: grant.checkpointDigestRef,
    grantRef: grant.grantRef,
    kind: 'accepted',
    sealedWindowRef: grant.sealedWindowRef,
  }
}

export type PylonJoinLifecycleSealBarrierOutcome =
  | Readonly<{
      event: PylonJoinLifecycleEventRecord
      kind: 'applied'
      record: PylonJoinLifecycleRecord
    }>
  | Readonly<{
      capacityRef: string
      kind: 'queued'
      reasonCode: typeof TrainingBootstrapQueuedReasonCode
    }>

/**
 * Join-lifecycle transitions obey the same merge barrier as bootstrap
 * grants: while a seal is in flight, the transition is queued with the
 * typed deferral reason code instead of being applied or rejected. The
 * caller replays the identical transition input once the barrier
 * clears.
 */
export const applyPylonJoinLifecycleTransitionUnderSealBarrier = (
  input: Readonly<{
    eventId: string
    nowIso: string
    reasonCode: PylonJoinLifecycleReasonCode
    record: PylonJoinLifecycleRecord
    receiptRef: string
    sealInFlight: boolean
    toState: PylonJoinLifecycleState
  }>,
): PylonJoinLifecycleSealBarrierOutcome => {
  if (input.sealInFlight) {
    return {
      capacityRef: input.record.capacityRef,
      kind: 'queued',
      reasonCode: TrainingBootstrapQueuedReasonCode,
    }
  }

  const applied = transitionPylonJoinLifecycleRecord({
    eventId: input.eventId,
    nowIso: input.nowIso,
    reasonCode: input.reasonCode,
    receiptRef: input.receiptRef,
    record: input.record,
    toState: input.toState,
  })

  return {
    event: applied.event,
    kind: 'applied',
    record: applied.record,
  }
}
