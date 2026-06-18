import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type TrainingTraceContributionRecord,
  type TrainingTraceContributionStore,
  TrainingTraceContributionStoreError,
  buildTrainingTraceContributionRecord,
} from './tassadar-trace-contribution-authority'
import {
  type TassadarValidatorCandidate,
  decideTracePairing,
  runTassadarTracePairing,
  runTassadarTracePairingScheduled,
} from './tassadar-trace-pairing'
import {
  type TrainingVerificationChallengeRecord,
  buildTrainingVerificationChallengeRecord,
  verifyExactTraceReplay,
} from './training-verification'

/**
 * #5053 (epic #5051) worker -> validator pairing orchestration (Artanis-first,
 * design §4.3 option B). A pending worker trace contribution (#5052) is paired
 * with a DISTINCT validator device, the existing exact_trace_replay challenge is
 * built, and the pairing recorded. Self-pairs are rejected; a paired
 * contribution cannot be re-paired (no double-pair). The orchestration is OFF by
 * default behind the TASSADAR_TRACE_PAIRING flag.
 */

const RUN_REF = 'run.tassadar.executor.20260615'
const WINDOW_REF = 'training.window.tassadar.executor.20260615.w1'
const LEASE_REF = 'lease.tassadar.executor.1'
const WORKER_PYLON = 'pylon.worker'
const WORKER_DEVICE = 'device.worker.1'
const VALIDATOR_DEVICE = 'device.validator.1'
const TRACE_DIGEST = 'digest.trace.abc123'

const pendingContribution = (
  overrides: Partial<TrainingTraceContributionRecord> = {},
): TrainingTraceContributionRecord => {
  const base = buildTrainingTraceContributionRecord({
    leaseRef: LEASE_REF,
    makeId: () => 'contribution-id-1',
    nowIso: '2026-06-15T00:00:00.000Z',
    pylonRef: WORKER_PYLON,
    request: {
      assignmentRef: 'assignment.tassadar.1',
      pylonDeviceRef: WORKER_DEVICE,
      sampledWindow: { endStep: 32, startStep: 0 },
      sampledWindowRef: 'window.sampled.1',
      traceCommitmentDigestRef: TRACE_DIGEST,
      workerReceiptRef: 'receipt.worker.1',
      workloadFamily: 'article_closeout',
    },
    trainingRunRef: RUN_REF,
    windowRef: WINDOW_REF,
  })

  return { ...base, ...overrides }
}

type MemoryStore = TrainingTraceContributionStore &
  Readonly<{ _records: Map<string, TrainingTraceContributionRecord> }>

const makeStore = (
  initial: ReadonlyArray<TrainingTraceContributionRecord>,
): MemoryStore => {
  const records = new Map(initial.map(r => [r.contributionRef, r]))

  return {
    _records: records,
    listPendingContributions: async ({ limit, trainingRunRef }) =>
      [...records.values()]
        .filter(
          record =>
            record.state === 'pending' &&
            (trainingRunRef === undefined ||
              record.trainingRunRef === trainingRunRef),
        )
        .sort((a, b) => a.submittedAt.localeCompare(b.submittedAt))
        .slice(0, Math.max(1, limit)),
    pairValidatorVerdict: async input => {
      const found = records.get(input.contributionRef)

      if (found === undefined || found.state !== 'pending') {
        throw new TrainingTraceContributionStoreError({
          kind: 'conflict',
          reason: 'Worker trace contribution is not pending.',
        })
      }

      const paired: TrainingTraceContributionRecord = {
        ...found,
        publicProjectionJson: input.publicProjectionJson,
        replayDigestRef: input.replayDigestRef,
        state: 'paired',
        updatedAt: input.updatedAt,
        validatorDeviceRef: input.validatorDeviceRef,
        verificationChallengeRef: input.verificationChallengeRef,
      }
      records.set(found.contributionRef, paired)

      return paired
    },
    readMostRecentPylonRefByDeviceRef: async pylonDeviceRef =>
      [...records.values()]
        .filter(record => record.pylonDeviceRef === pylonDeviceRef)
        .sort((a, b) => b.submittedAt.localeCompare(a.submittedAt))[0]
        ?.pylonRef,
    readWorkerContribution: async (leaseRef, workloadFamily) =>
      [...records.values()].find(
        record =>
          record.leaseRef === leaseRef &&
          record.workloadFamily === workloadFamily,
      ),
    recordWorkerContribution: async record => {
      const existing = records.get(record.contributionRef)

      if (existing !== undefined) {
        return existing
      }

      records.set(record.contributionRef, record)

      return record
    },
  }
}

const challengeCollector = () => {
  const challenges: Array<TrainingVerificationChallengeRecord> = []
  let counter = 0

  return {
    challenges,
    createVerificationChallenge: async (
      request: Parameters<
        typeof buildTrainingVerificationChallengeRecord
      >[0]['request'],
    ) => {
      const built = buildTrainingVerificationChallengeRecord({
        makeId: () => `challenge-${++counter}`,
        nowIso: '2026-06-15T00:05:00.000Z',
        request,
      })
      challenges.push(built.challenge)

      return built.challenge
    },
  }
}

const distinctValidator: TassadarValidatorCandidate = {
  replayDigestRef: TRACE_DIGEST,
  validatorDeviceRef: VALIDATOR_DEVICE,
}

describe('decideTracePairing (#5053 pure pairing)', () => {
  it('pairs a pending contribution with a distinct validator and builds the exact_trace_replay challenge', () => {
    const decision = decideTracePairing(pendingContribution(), [
      distinctValidator,
    ])

    expect(decision.kind).toBe('pairable')
    if (decision.kind !== 'pairable') {
      throw new Error('expected pairable decision')
    }
    expect(decision.validatorDeviceRef).toBe(VALIDATOR_DEVICE)
    expect(decision.pylonDeviceRef).toBe(WORKER_DEVICE)
    expect(decision.challengeRequest.verificationClass).toBe(
      'exact_trace_replay',
    )
    expect(decision.challengeRequest.trainingRunRef).toBe(RUN_REF)
    expect(decision.challengeRequest.windowRef).toBe(WINDOW_REF)
    expect(decision.challengeRequest.payload.validatorDeviceRef).toBe(
      VALIDATOR_DEVICE,
    )
    expect(decision.challengeRequest.payload.pylonDeviceRef).toBe(WORKER_DEVICE)
  })

  it('rejects a self-pair (validator device == worker device)', () => {
    const decision = decideTracePairing(pendingContribution(), [
      { replayDigestRef: TRACE_DIGEST, validatorDeviceRef: WORKER_DEVICE },
    ])

    expect(decision.kind).toBe('rejected')
    if (decision.kind !== 'rejected') {
      throw new Error('expected rejected decision')
    }
    expect(decision.reason).toBe('self_pair_device_not_distinct')
  })

  it('skips a distinct validator candidate that equals the worker and uses the next distinct one', () => {
    const decision = decideTracePairing(pendingContribution(), [
      { replayDigestRef: TRACE_DIGEST, validatorDeviceRef: WORKER_DEVICE },
      distinctValidator,
    ])

    expect(decision.kind).toBe('pairable')
    if (decision.kind !== 'pairable') {
      throw new Error('expected pairable decision')
    }
    expect(decision.validatorDeviceRef).toBe(VALIDATOR_DEVICE)
  })

  it('rejects when no validator candidate exists', () => {
    const decision = decideTracePairing(pendingContribution(), [])

    expect(decision.kind).toBe('rejected')
    if (decision.kind !== 'rejected') {
      throw new Error('expected rejected decision')
    }
    expect(decision.reason).toBe('no_distinct_validator_candidate')
  })

  it('rejects an already-paired contribution', () => {
    const decision = decideTracePairing(
      pendingContribution({ state: 'paired' }),
      [distinctValidator],
    )

    expect(decision.kind).toBe('rejected')
    if (decision.kind !== 'rejected') {
      throw new Error('expected rejected decision')
    }
    expect(decision.reason).toBe('contribution_not_pending')
  })
})

describe('runTassadarTracePairing (#5053 orchestration tick)', () => {
  it('pairs the pending contribution, records the verdict, and Verified on a matching replay digest', async () => {
    const store = makeStore([pendingContribution()])
    const collector = challengeCollector()

    const outcome = await runTassadarTracePairing({
      createVerificationChallenge: collector.createVerificationChallenge,
      nowIso: '2026-06-15T00:05:00.000Z',
      resolveValidatorCandidates: async () => [distinctValidator],
      store,
    })

    expect(outcome.state).toBe('paired')
    expect(outcome.validatorDeviceRef).toBe(VALIDATOR_DEVICE)
    expect(outcome.verificationChallengeRef).not.toBeNull()
    expect(store._records.get(outcome.contributionRef!)?.state).toBe('paired')

    // Replay is the trust anchor: a matching worker/validator digest -> Verified.
    const verdict = verifyExactTraceReplay({
      challenge: collector.challenges[0]!,
      payload: collector.challenges[0]!.payloadJson
        ? (JSON.parse(collector.challenges[0]!.payloadJson) as Record<
            string,
            unknown
          >)
        : {},
    })
    expect(verdict.state).toBe('Verified')
  })

  it('Rejected when the validator replay digest does not match the worker commitment', async () => {
    const store = makeStore([pendingContribution()])
    const collector = challengeCollector()

    await runTassadarTracePairing({
      createVerificationChallenge: collector.createVerificationChallenge,
      nowIso: '2026-06-15T00:05:00.000Z',
      resolveValidatorCandidates: async () => [
        {
          replayDigestRef: 'digest.trace.MISMATCH',
          validatorDeviceRef: VALIDATOR_DEVICE,
        },
      ],
      store,
    })

    const verdict = verifyExactTraceReplay({
      challenge: collector.challenges[0]!,
      payload: JSON.parse(collector.challenges[0]!.payloadJson!) as Record<
        string,
        unknown
      >,
    })
    expect(verdict.state).toBe('Rejected')
  })

  it('does not double-pair: a second tick over the same paired contribution finds no pending work', async () => {
    const store = makeStore([pendingContribution()])
    const collector = challengeCollector()

    const first = await runTassadarTracePairing({
      createVerificationChallenge: collector.createVerificationChallenge,
      nowIso: '2026-06-15T00:05:00.000Z',
      resolveValidatorCandidates: async () => [distinctValidator],
      store,
    })
    const second = await runTassadarTracePairing({
      createVerificationChallenge: collector.createVerificationChallenge,
      nowIso: '2026-06-15T00:06:00.000Z',
      resolveValidatorCandidates: async () => [distinctValidator],
      store,
    })

    expect(first.state).toBe('paired')
    expect(second.state).toBe('no_pending')
    // Only one challenge ever created for the contribution.
    expect(collector.challenges).toHaveLength(1)
  })

  it('does not pair when only the worker device is offered as validator (self-pair guard)', async () => {
    const store = makeStore([pendingContribution()])
    const collector = challengeCollector()

    const outcome = await runTassadarTracePairing({
      createVerificationChallenge: collector.createVerificationChallenge,
      nowIso: '2026-06-15T00:05:00.000Z',
      resolveValidatorCandidates: async () => [
        { replayDigestRef: TRACE_DIGEST, validatorDeviceRef: WORKER_DEVICE },
      ],
      store,
    })

    expect(outcome.state).toBe('no_distinct_validator')
    expect(outcome.reason).toBe('self_pair_device_not_distinct')
    expect(collector.challenges).toHaveLength(0)
    expect(store._records.get(pendingContribution().contributionRef)?.state).toBe(
      'pending',
    )
  })

  it('returns no_pending when there are no pending contributions', async () => {
    const store = makeStore([pendingContribution({ state: 'paired' })])
    const collector = challengeCollector()

    const outcome = await runTassadarTracePairing({
      createVerificationChallenge: collector.createVerificationChallenge,
      nowIso: '2026-06-15T00:05:00.000Z',
      resolveValidatorCandidates: async () => [distinctValidator],
      store,
    })

    expect(outcome.state).toBe('no_pending')
  })
})

describe('runTassadarTracePairingScheduled (#5053 flag gate)', () => {
  it('is INERT when disabled: never reads the store, never creates a challenge', async () => {
    let storeTouched = false
    let challengeTouched = false
    const store = makeStore([pendingContribution()])
    const guardedStore: TrainingTraceContributionStore = {
      ...store,
      listPendingContributions: async input => {
        storeTouched = true
        return store.listPendingContributions(input)
      },
    }

    const outcome = await Effect.runPromise(
      runTassadarTracePairingScheduled({
        createVerificationChallenge: async () => {
          challengeTouched = true
          throw new Error('should not be called while disabled')
        },
        enabled: false,
        nowIso: '2026-06-15T00:05:00.000Z',
        resolveValidatorCandidates: async () => [distinctValidator],
        store: guardedStore,
      }),
    )

    expect(outcome.state).toBe('skipped')
    expect(outcome.reason).toBe('trace_pairing_disabled')
    expect(storeTouched).toBe(false)
    expect(challengeTouched).toBe(false)
  })

  it('pairs when explicitly enabled (the #5061 dry-run path)', async () => {
    const store = makeStore([pendingContribution()])
    const collector = challengeCollector()

    const outcome = await Effect.runPromise(
      runTassadarTracePairingScheduled({
        createVerificationChallenge: collector.createVerificationChallenge,
        enabled: true,
        nowIso: '2026-06-15T00:05:00.000Z',
        resolveValidatorCandidates: async () => [distinctValidator],
        store,
      }),
    )

    expect(outcome.state).toBe('paired')
    expect(collector.challenges).toHaveLength(1)
  })

  it('catches store/challenge errors into a skipped outcome (tick never crashes)', async () => {
    const store = makeStore([pendingContribution()])

    const outcome = await Effect.runPromise(
      runTassadarTracePairingScheduled({
        createVerificationChallenge: async () => {
          throw new TrainingTraceContributionStoreError({
            kind: 'storage_error',
            reason: 'challenge store unavailable',
          })
        },
        enabled: true,
        nowIso: '2026-06-15T00:05:00.000Z',
        resolveValidatorCandidates: async () => [distinctValidator],
        store,
      }),
    )

    expect(outcome.state).toBe('skipped')
    expect(outcome.reason).toContain('pairing_error')
  })
})
