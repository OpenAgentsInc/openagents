import { describe, expect, it } from 'vitest'

import {
  type PylonJoinLifecycleReasonCode,
  type PylonJoinLifecycleRecord,
  type PylonJoinLifecycleState,
  buildPylonJoinLifecycleRecord,
  transitionPylonJoinLifecycleRecord,
} from './pylon-join-lifecycle'
import type { TrainingRunRecord } from './training-run-window-authority'
import {
  TrainingStalenessAcceptanceError,
  decideTrainingStalenessAcceptance,
  effectiveMaxStepsBehindFor,
  routeTrainingStalenessSyncReentry,
} from './training-staleness-acceptance'
import {
  type TrainingVerificationRegistration,
  defaultTrainingVerificationRegistry,
} from './training-verification'

const nowIso = '2026-06-12T10:00:00.000Z'

const makeRun = (
  overrides: Partial<TrainingRunRecord> = {},
): TrainingRunRecord => ({
  createdAt: nowIso,
  id: 'run-1',
  manifest: null,
  maxAllowedStale: 5,
  promiseRef: 'promise.training.4673',
  publicProjectionJson: '{}',
  receiptRefs: [],
  sealInFlightAt: null,
  sealPublicationCadenceWindows: 1,
  sourceRefs: [],
  state: 'active',
  trainingRunRef: 'training.run.4673',
  updatedAt: nowIso,
  ...overrides,
})

const sequencedEventIds = (): (() => string) => {
  const ids = ['0001', '0002', '0003', '0004']

  return () => ids.shift() ?? 'overflow'
}

const decide = (
  stepsBehind: number,
  overrides: Partial<
    Parameters<typeof decideTrainingStalenessAcceptance>[0]
  > = {},
) =>
  decideTrainingStalenessAcceptance({
    contributionRef: 'training.contribution.42',
    decidedAtIso: nowIso,
    makeEventId: () => '0001',
    run: makeRun(),
    stepsBehind,
    verificationClass: 'deterministic_recompute',
    ...overrides,
  })

type LadderStep = Readonly<{
  reasonCode: PylonJoinLifecycleReasonCode
  toState: PylonJoinLifecycleState
}>

const forwardLadderSteps: ReadonlyArray<LadderStep> = [
  {
    reasonCode: 'join_lifecycle.public.qualification_gate_passed',
    toState: 'qualified',
  },
  {
    reasonCode: 'join_lifecycle.public.durable_seal_digest_synced',
    toState: 'state_synced',
  },
  {
    reasonCode: 'join_lifecycle.public.warmup_started',
    toState: 'warmup',
  },
  {
    reasonCode: 'join_lifecycle.public.shadow_work_verified',
    toState: 'active',
  },
]

const laggedLadderStep: LadderStep = {
  reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
  toState: 'lagged',
}

const recordAtState = (
  target: 'active' | 'lagged',
): PylonJoinLifecycleRecord => {
  const steps =
    target === 'lagged'
      ? [...forwardLadderSteps, laggedLadderStep]
      : forwardLadderSteps

  return steps.reduce(
    (record, step, index) =>
      transitionPylonJoinLifecycleRecord({
        eventId: `setup-${index}`,
        nowIso,
        reasonCode: step.reasonCode,
        receiptRef: `receipt.join.setup.${index}`,
        record,
        toState: step.toState,
      }).record,
    buildPylonJoinLifecycleRecord({ capacityRef: 'capacity.pylon.1', nowIso }),
  )
}

const syncReentryDecision = () => {
  const decision = decide(6)

  if (decision.outcome !== 'sync_reentry') {
    throw new Error('expected a sync_reentry decision fixture')
  }

  return decision
}

describe('decideTrainingStalenessAcceptance', () => {
  it('accepts a fresh contribution under the run default', () => {
    expect(decide(3)).toEqual({
      contributionRef: 'training.contribution.42',
      effectiveMaxStepsBehind: 5,
      outcome: 'accept',
      stepsBehind: 3,
      thresholdSource: 'run_default',
      trainingRunRef: 'training.run.4673',
      verificationClass: 'deterministic_recompute',
    })
  })

  it('accepts the boundary value of exactly maxAllowedStale (beyond means strictly greater)', () => {
    expect(decide(5)).toMatchObject({
      effectiveMaxStepsBehind: 5,
      outcome: 'accept',
      stepsBehind: 5,
    })
  })

  it('routes an over-stale contribution to sync_reentry with a typed event, never a bare reject', () => {
    const decision = decide(6)

    expect(decision).toMatchObject({
      effectiveMaxStepsBehind: 5,
      outcome: 'sync_reentry',
      reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
      stepsBehind: 6,
      thresholdSource: 'run_default',
    })
    expect(decision.outcome === 'sync_reentry' && decision.event).toEqual({
      contributionRef: 'training.contribution.42',
      decidedAtIso: nowIso,
      effectiveMaxStepsBehind: 5,
      eventRef: 'training.staleness.reentry.0001',
      kind: 'training.staleness.public.sync_reentry_routed',
      reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
      stepsBehind: 6,
      thresholdSource: 'run_default',
      trainingRunRef: 'training.run.4673',
      verificationClass: 'deterministic_recompute',
    })
  })

  it('lets a tighter class-level override beat the run default', () => {
    const decision = decide(3, {
      verificationClass: 'statistical_cross_check',
    })

    expect(decision).toMatchObject({
      effectiveMaxStepsBehind: 2,
      outcome: 'sync_reentry',
      reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
      stepsBehind: 3,
      thresholdSource: 'class_override',
      verificationClass: 'statistical_cross_check',
    })
  })

  it('clamps a looser class override to the run contract ceiling', () => {
    const base = defaultTrainingVerificationRegistry.get(
      'deterministic_recompute',
    )!
    const loosened: TrainingVerificationRegistration = {
      ...base,
      stalenessPolicy: {
        kind: 'max_steps_behind_override',
        maxStepsBehind: 50,
      },
    }
    const registry = new Map(defaultTrainingVerificationRegistry)
    registry.set('deterministic_recompute', loosened)

    expect(decide(7, { registry })).toMatchObject({
      effectiveMaxStepsBehind: 5,
      outcome: 'sync_reentry',
      thresholdSource: 'run_default',
    })
  })

  it('rejects non-integer and negative stepsBehind with a typed validation error', () => {
    expect(() => decide(1.5)).toThrow(TrainingStalenessAcceptanceError)
    expect(() => decide(-1)).toThrow(TrainingStalenessAcceptanceError)
  })
})

describe('effectiveMaxStepsBehindFor', () => {
  it('inherits the run default when the class declares no policy', () => {
    expect(effectiveMaxStepsBehindFor(undefined, 5)).toEqual({
      effectiveMaxStepsBehind: 5,
      thresholdSource: 'run_default',
    })
    expect(
      effectiveMaxStepsBehindFor({ kind: 'inherit_run_default' }, 5),
    ).toEqual({
      effectiveMaxStepsBehind: 5,
      thresholdSource: 'run_default',
    })
  })

  it('applies a tightening override and clamps a loosening one', () => {
    expect(
      effectiveMaxStepsBehindFor(
        { kind: 'max_steps_behind_override', maxStepsBehind: 2 },
        5,
      ),
    ).toEqual({
      effectiveMaxStepsBehind: 2,
      thresholdSource: 'class_override',
    })
    expect(
      effectiveMaxStepsBehindFor(
        { kind: 'max_steps_behind_override', maxStepsBehind: 50 },
        5,
      ),
    ).toEqual({
      effectiveMaxStepsBehind: 5,
      thresholdSource: 'run_default',
    })
  })
})

describe('routeTrainingStalenessSyncReentry', () => {
  it('walks an active contributor through lagged into sync_reentry with reason-coded events', () => {
    const outcome = routeTrainingStalenessSyncReentry({
      decision: syncReentryDecision(),
      makeEventId: sequencedEventIds(),
      nowIso,
      receiptRef: 'receipt.training.staleness.42',
      record: recordAtState('active'),
    })

    expect(outcome).toMatchObject({
      kind: 'routed',
      record: { state: 'sync_reentry' },
    })
    expect(outcome.kind === 'routed' && outcome.events).toMatchObject([
      {
        fromState: 'active',
        reasonCode: 'join_lifecycle.public.beyond_max_allowed_stale',
        receiptRef: 'receipt.training.staleness.42',
        toState: 'lagged',
      },
      {
        fromState: 'lagged',
        reasonCode: 'join_lifecycle.public.sync_reentry_started',
        receiptRef: 'receipt.training.staleness.42',
        toState: 'sync_reentry',
      },
    ])
  })

  it('takes only the second edge for a contributor already lagged', () => {
    const outcome = routeTrainingStalenessSyncReentry({
      decision: syncReentryDecision(),
      makeEventId: sequencedEventIds(),
      nowIso,
      receiptRef: 'receipt.training.staleness.42',
      record: recordAtState('lagged'),
    })

    expect(outcome).toMatchObject({
      kind: 'routed',
      record: { state: 'sync_reentry' },
    })
    expect(outcome.kind === 'routed' && outcome.events).toMatchObject([
      {
        fromState: 'lagged',
        reasonCode: 'join_lifecycle.public.sync_reentry_started',
        toState: 'sync_reentry',
      },
    ])
  })

  it('reports an idempotent outcome for a contributor already in sync_reentry', () => {
    const lagged = recordAtState('lagged')
    const reentry = transitionPylonJoinLifecycleRecord({
      eventId: 'setup-reentry',
      nowIso,
      reasonCode: 'join_lifecycle.public.sync_reentry_started',
      receiptRef: 'receipt.join.setup.reentry',
      record: lagged,
      toState: 'sync_reentry',
    }).record

    expect(
      routeTrainingStalenessSyncReentry({
        decision: syncReentryDecision(),
        makeEventId: sequencedEventIds(),
        nowIso,
        receiptRef: 'receipt.training.staleness.42',
        record: reentry,
      }),
    ).toEqual({
      capacityRef: 'capacity.pylon.1',
      kind: 'already_in_sync_reentry',
    })
  })

  it('reports a typed not_routable outcome for ladder states with no reentry edge', () => {
    expect(
      routeTrainingStalenessSyncReentry({
        decision: syncReentryDecision(),
        makeEventId: sequencedEventIds(),
        nowIso,
        receiptRef: 'receipt.training.staleness.42',
        record: buildPylonJoinLifecycleRecord({
          capacityRef: 'capacity.pylon.1',
          nowIso,
        }),
      }),
    ).toMatchObject({
      kind: 'not_routable',
      state: 'registered',
    })
  })
})
