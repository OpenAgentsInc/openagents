import { Schema as S } from 'effect'

import {
  type PylonJoinLifecycleEventRecord,
  type PylonJoinLifecycleRecord,
  type PylonJoinLifecycleState,
  transitionPylonJoinLifecycleRecord,
} from './pylon-join-lifecycle'
import type { TrainingRunRecord } from './training-run-window-authority'
import {
  type TrainingVerificationClass,
  type TrainingVerificationRegistration,
  type TrainingVerificationStalenessPolicy,
  defaultTrainingVerificationRegistry,
  trainingVerificationRegistrationFor,
} from './training-verification'

// Staleness-priced acceptance (Pluralis roadmap P2.2, openagents issue
// #4853, master tracking issue #4855). P0.2 made `steps_behind` a
// contract field on the window-seal record; this module makes it
// load-bearing at acceptance time. Every contribution carries
// `stepsBehind`, each verification class carries a staleness dimension
// (inherit-run-default or a tightening per-class override), and a
// contribution beyond the effective threshold routes to `sync_reentry`
// — re-ramped through the shadow window (P1.1, psionic#1125) — rather
// than being rejected (wasting a willing device) or merged (importing
// divergence). The decision union deliberately has no reject arm:
// staleness alone can never produce a bare rejection, by type. This is
// the AsyncPP lesson (Ajanthan et al., ICML 2025, arXiv:2505.01099)
// applied to dispatch rather than to the optimizer: measure the delay,
// respond to it, never pretend it is zero.

// Matches the TrainingWindowSealStepsBehind bound in the run/window
// authority so a steps_behind value that sealed cleanly always decides
// cleanly here.
export const MaxTrainingStalenessStepsBehind = 1_000_000

// The sync_reentry routing reuses the join-lifecycle ladder's existing
// closed reason-code taxonomy (P0.1, #4848) instead of inventing a
// parallel staleness vocabulary.
export const TrainingStalenessReentryReasonCode =
  'join_lifecycle.public.beyond_max_allowed_stale' as const
export const TrainingStalenessReentryStartedReasonCode =
  'join_lifecycle.public.sync_reentry_started' as const

export type TrainingStalenessThresholdSource = 'class_override' | 'run_default'

export class TrainingStalenessAcceptanceError extends S.TaggedErrorClass<TrainingStalenessAcceptanceError>()(
  'TrainingStalenessAcceptanceError',
  {
    kind: S.Literals(['validation_error']),
    reason: S.String,
  },
) {}

// Typed routing event: an over-stale contribution produces a structured
// fact about why it was re-ramped, never a bare rejection record.
export type TrainingStalenessRoutingEvent = Readonly<{
  contributionRef: string
  decidedAtIso: string
  effectiveMaxStepsBehind: number
  eventRef: string
  kind: 'training.staleness.public.sync_reentry_routed'
  reasonCode: typeof TrainingStalenessReentryReasonCode
  stepsBehind: number
  thresholdSource: TrainingStalenessThresholdSource
  trainingRunRef: string
  verificationClass: TrainingVerificationClass
}>

export type TrainingStalenessAcceptanceDecision =
  | Readonly<{
      contributionRef: string
      effectiveMaxStepsBehind: number
      outcome: 'accept'
      stepsBehind: number
      thresholdSource: TrainingStalenessThresholdSource
      trainingRunRef: string
      verificationClass: TrainingVerificationClass
    }>
  | Readonly<{
      contributionRef: string
      effectiveMaxStepsBehind: number
      event: TrainingStalenessRoutingEvent
      outcome: 'sync_reentry'
      reasonCode: typeof TrainingStalenessReentryReasonCode
      stepsBehind: number
      thresholdSource: TrainingStalenessThresholdSource
      trainingRunRef: string
      verificationClass: TrainingVerificationClass
    }>

const requireBoundedStepsBehind = (value: number, label: string): number => {
  if (
    !Number.isInteger(value) ||
    value < 0 ||
    value > MaxTrainingStalenessStepsBehind
  ) {
    throw new TrainingStalenessAcceptanceError({
      kind: 'validation_error',
      reason: `${label} must be an integer between 0 and ${MaxTrainingStalenessStepsBehind}.`,
    })
  }

  return value
}

const requireRunMaxAllowedStale = (value: number): number => {
  if (
    !Number.isInteger(value) ||
    value < 1 ||
    value > MaxTrainingStalenessStepsBehind
  ) {
    throw new TrainingStalenessAcceptanceError({
      kind: 'validation_error',
      reason: `run maxAllowedStale must be an integer between 1 and ${MaxTrainingStalenessStepsBehind}.`,
    })
  }

  return value
}

/**
 * Resolves the effective per-contribution staleness threshold for one
 * verification class against the run contract. A class without a
 * policy (or with `inherit_run_default`) uses the run's
 * `maxAllowedStale`. A `max_steps_behind_override` may only TIGHTEN:
 * the run-level contract value is a ceiling, so an override looser
 * than the run default clamps to the run default and reports
 * `run_default` as the threshold source — a class can never import
 * more divergence than the run contract allows.
 */
export const effectiveMaxStepsBehindFor = (
  policy: TrainingVerificationStalenessPolicy | undefined,
  runMaxAllowedStale: number,
): Readonly<{
  effectiveMaxStepsBehind: number
  thresholdSource: TrainingStalenessThresholdSource
}> => {
  const runCeiling = requireRunMaxAllowedStale(runMaxAllowedStale)

  if (policy === undefined || policy.kind === 'inherit_run_default') {
    return {
      effectiveMaxStepsBehind: runCeiling,
      thresholdSource: 'run_default',
    }
  }

  const override = requireBoundedStepsBehind(
    policy.maxStepsBehind,
    'class stalenessPolicy maxStepsBehind',
  )

  return override < runCeiling
    ? { effectiveMaxStepsBehind: override, thresholdSource: 'class_override' }
    : { effectiveMaxStepsBehind: runCeiling, thresholdSource: 'run_default' }
}

/**
 * Acceptance-time staleness decision for one contribution. Boundary
 * decision (documented, tested): a contribution at EXACTLY the
 * effective threshold is ACCEPTED — the contract field is "beyond
 * max_allowed_stale" (strictly greater), matching the run-authority
 * comment "more than this many steps behind" and the Pluralis node0
 * `max_allowed_stale` reading. Over the threshold the outcome is
 * `sync_reentry` with a typed routing event; there is no reject arm
 * for staleness alone.
 */
export const decideTrainingStalenessAcceptance = (
  input: Readonly<{
    contributionRef: string
    decidedAtIso: string
    makeEventId: () => string
    registry?: ReadonlyMap<
      TrainingVerificationClass,
      TrainingVerificationRegistration
    >
    run: TrainingRunRecord
    stepsBehind: number
    verificationClass: TrainingVerificationClass
  }>,
): TrainingStalenessAcceptanceDecision => {
  const stepsBehind = requireBoundedStepsBehind(
    input.stepsBehind,
    'contribution stepsBehind',
  )
  const registry = input.registry ?? defaultTrainingVerificationRegistry
  const registration = trainingVerificationRegistrationFor(
    registry,
    input.verificationClass,
  )
  const { effectiveMaxStepsBehind, thresholdSource } =
    effectiveMaxStepsBehindFor(
      registration.stalenessPolicy,
      input.run.maxAllowedStale,
    )

  if (stepsBehind <= effectiveMaxStepsBehind) {
    return {
      contributionRef: input.contributionRef,
      effectiveMaxStepsBehind,
      outcome: 'accept',
      stepsBehind,
      thresholdSource,
      trainingRunRef: input.run.trainingRunRef,
      verificationClass: input.verificationClass,
    }
  }

  return {
    contributionRef: input.contributionRef,
    effectiveMaxStepsBehind,
    event: {
      contributionRef: input.contributionRef,
      decidedAtIso: input.decidedAtIso,
      effectiveMaxStepsBehind,
      eventRef: `training.staleness.reentry.${input.makeEventId()}`,
      kind: 'training.staleness.public.sync_reentry_routed',
      reasonCode: TrainingStalenessReentryReasonCode,
      stepsBehind,
      thresholdSource,
      trainingRunRef: input.run.trainingRunRef,
      verificationClass: input.verificationClass,
    },
    outcome: 'sync_reentry',
    reasonCode: TrainingStalenessReentryReasonCode,
    stepsBehind,
    thresholdSource,
    trainingRunRef: input.run.trainingRunRef,
    verificationClass: input.verificationClass,
  }
}

export type TrainingStalenessReentryRoutingOutcome =
  | Readonly<{
      events: ReadonlyArray<PylonJoinLifecycleEventRecord>
      kind: 'routed'
      record: PylonJoinLifecycleRecord
    }>
  | Readonly<{
      capacityRef: string
      kind: 'already_in_sync_reentry'
    }>
  | Readonly<{
      capacityRef: string
      kind: 'not_routable'
      reason: string
      state: PylonJoinLifecycleState
    }>

/**
 * Composes a `sync_reentry` staleness decision with the join-lifecycle
 * ladder (P0.1, #4848). An active contributor walks the back edge in
 * two reason-coded transitions (active -> lagged -> sync_reentry); a
 * contributor already lagged takes only the second edge; a contributor
 * already in sync_reentry needs no routing (idempotent outcome, not an
 * error). States with no ladder edge toward sync_reentry (registered,
 * qualified, state_synced, warmup) report a typed `not_routable`
 * outcome — only a device that has been on the live ladder can have
 * produced a stale contribution.
 */
export const routeTrainingStalenessSyncReentry = (
  input: Readonly<{
    decision: Extract<
      TrainingStalenessAcceptanceDecision,
      { outcome: 'sync_reentry' }
    >
    makeEventId: () => string
    nowIso: string
    receiptRef: string
    record: PylonJoinLifecycleRecord
  }>,
): TrainingStalenessReentryRoutingOutcome => {
  if (input.record.state === 'sync_reentry') {
    return {
      capacityRef: input.record.capacityRef,
      kind: 'already_in_sync_reentry',
    }
  }

  if (input.record.state !== 'active' && input.record.state !== 'lagged') {
    return {
      capacityRef: input.record.capacityRef,
      kind: 'not_routable',
      reason: `The join-lifecycle ladder has no edge from ${input.record.state} toward sync_reentry; only an active or lagged contributor can be re-ramped.`,
      state: input.record.state,
    }
  }

  const lagged =
    input.record.state === 'active'
      ? transitionPylonJoinLifecycleRecord({
          eventId: input.makeEventId(),
          nowIso: input.nowIso,
          reasonCode: TrainingStalenessReentryReasonCode,
          receiptRef: input.receiptRef,
          record: input.record,
          toState: 'lagged',
        })
      : undefined

  const reentry = transitionPylonJoinLifecycleRecord({
    eventId: input.makeEventId(),
    nowIso: input.nowIso,
    reasonCode: TrainingStalenessReentryStartedReasonCode,
    receiptRef: input.receiptRef,
    record: lagged?.record ?? input.record,
    toState: 'sync_reentry',
  })

  return {
    events:
      lagged === undefined
        ? [reentry.event]
        : [lagged.event, reentry.event],
    kind: 'routed',
    record: reentry.record,
  }
}
