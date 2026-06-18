import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import type {
  AgentRegistrationStore,
  AgentUserRecord,
} from './agent-registration'
import {
  type TrainingTraceContributionRecord,
  type TrainingTraceContributionStore,
  TrainingTraceContributionStoreError,
  buildTrainingTraceContributionRecord,
  pairedContributionProjectionJson,
} from './tassadar-trace-contribution-authority'
import { makeTassadarTraceContributionRoutes } from './tassadar-trace-contribution-routes'
import {
  type TrainingAuthorityStore,
  type TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import {
  type TrainingVerificationChallengeRecord,
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
} from './training-verification'

/**
 * #5052 (epic #5051) agent-gated worker -> validator executor-trace completion
 * routes. These exercise lease-ownership, device-distinctness, idempotency, and
 * the digest-match -> Verified / mismatch -> Rejected verdict that the existing
 * exact_trace_replay challenge computes.
 */

const WORKER_USER = 'agent-worker'
const VALIDATOR_USER = 'agent-validator'
const WORKER_PYLON = 'pylon.worker'
const VALIDATOR_PYLON = 'pylon.validator'
const LEASE_REF = 'lease.tassadar.executor.1'
const RUN_REF = 'run.tassadar.executor.20260615'
const WINDOW_REF = 'training.window.tassadar.executor.20260615.w1'

const userFor = (id: string): AgentUserRecord => ({
  avatarUrl: null,
  createdAt: '2026-06-15T00:00:00.000Z',
  displayName: id,
  id,
  kind: 'agent',
  primaryEmail: null,
  status: 'active',
  updatedAt: '2026-06-15T00:00:00.000Z',
})

// Authenticates the caller as `userId`. requireAgent calls
// authenticateProgrammaticAgent, which hashes the bearer token and looks it up
// via findAgentByTokenHash; binding the store to the expected userId (as the
// existing pylon-api tests do) yields a deterministic per-request session. When
// `userId` is undefined the store resolves no agent (unauthenticated path).
const makeAgentStore = (
  userId: string | undefined,
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve(
      userId === undefined
        ? undefined
        : {
            credentialId: `cred-${userId}`,
            profileMetadataJson: '{}',
            tokenPrefix: 'oa_agent_',
            user: userFor(userId),
          },
    ),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

const leaseRecord = (
  overrides: Partial<TrainingWindowLeaseRecord> = {},
): TrainingWindowLeaseRecord => ({
  claimedAt: '2026-06-15T00:00:00.000Z',
  id: 'lease-1',
  leaseExpiresAt: '2026-06-15T01:00:00.000Z',
  leaseRef: LEASE_REF,
  publicProjectionJson: '{}',
  pylonRef: WORKER_PYLON,
  receiptRefs: [],
  state: 'active',
  trainingRunRef: RUN_REF,
  windowRef: WINDOW_REF,
  ...overrides,
})

type MemoryAuthorityStore = TrainingAuthorityStore &
  Readonly<{ _leases: Map<string, TrainingWindowLeaseRecord> }>

const notImplemented = async (): Promise<never> => {
  throw new Error('not implemented in authority stub')
}

const makeAuthorityStore = (
  leases: ReadonlyArray<TrainingWindowLeaseRecord>,
): MemoryAuthorityStore => {
  const leaseMap = new Map(leases.map(lease => [lease.leaseRef, lease]))

  return {
    _leases: leaseMap,
    attachRunEvidence: notImplemented,
    beginRunSealBarrier: notImplemented,
    claimLease: notImplemented,
    clearRunSealBarrier: notImplemented,
    listClaimableWindows: async () => [],
    listRuns: async () => [],
    listVerificationChallengesForRun: async () => [],
    listWindowLeasesForRun: async () => [],
    listWindowsForRun: async () => [],
    planRun: notImplemented,
    planWindow: notImplemented,
    readRun: notImplemented,
    readWindow: notImplemented,
    readWindowLease: async leaseRef => leaseMap.get(leaseRef),
    transitionRun: notImplemented,
    transitionWindow: notImplemented,
  }
}

type MemoryContributionStore = TrainingTraceContributionStore &
  Readonly<{ _records: Map<string, TrainingTraceContributionRecord> }>

const makeContributionStore = (): MemoryContributionStore => {
  const records = new Map<string, TrainingTraceContributionRecord>()
  const key = (leaseRef: string, workloadFamily: string) =>
    `${leaseRef}::${workloadFamily}`

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
        .slice(0, Math.max(1, limit)),
    readMostRecentPylonRefByDeviceRef: async pylonDeviceRef =>
      [...records.values()]
        .filter(record => record.pylonDeviceRef === pylonDeviceRef)
        .sort((a, b) => b.submittedAt.localeCompare(a.submittedAt))[0]
        ?.pylonRef,
    readWorkerContribution: async (leaseRef, workloadFamily) =>
      records.get(key(leaseRef, workloadFamily)),
    recordWorkerContribution: async record => {
      const existing = records.get(key(record.leaseRef, record.workloadFamily))

      if (existing !== undefined) {
        return existing
      }

      records.set(key(record.leaseRef, record.workloadFamily), record)

      return record
    },
    pairValidatorVerdict: async input => {
      const found = [...records.values()].find(
        record => record.contributionRef === input.contributionRef,
      )

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
      records.set(key(found.leaseRef, found.workloadFamily), paired)

      return paired
    },
  }
}

type Harness = Readonly<{
  authority: MemoryAuthorityStore
  contributions: MemoryContributionStore
  challenges: Array<TrainingVerificationChallengeRecord>
  route: (
    path: string,
    options: Readonly<{
      body?: Record<string, unknown>
      tokenUserId?: string
      method?: string
    }>,
  ) => Promise<Response>
}>

type AutoStreamHook = (
  env: Readonly<Record<string, unknown>>,
  input: Readonly<{
    challenge: TrainingVerificationChallengeRecord
    lease: TrainingWindowLeaseRecord
    validatorContributorRef: string
  }>,
) => Effect.Effect<void, unknown>

// resolvePylonOwnerUserId binds each Pylon to its owning agent user so the
// lease-ownership check can compare the lease pylon_ref to the session user.
const PYLON_OWNERS = new Map<string, string>([
  [WORKER_PYLON, WORKER_USER],
  [VALIDATOR_PYLON, VALIDATOR_USER],
])

const makeHarness = (
  leases: ReadonlyArray<TrainingWindowLeaseRecord> = [leaseRecord()],
  options: Readonly<{
    onVerifiedExactTraceReplayPair?: AutoStreamHook
  }> = {},
): Harness => {
  const authority = makeAuthorityStore(leases)
  const contributions = makeContributionStore()
  const challenges: Array<TrainingVerificationChallengeRecord> = []
  let counter = 0
  // The agent store dependency is invoked per request; it resolves the caller
  // bound to the token user of the in-flight request (set by `route`). An
  // undefined token user resolves no session (unauthenticated).
  let currentTokenUser: string | undefined
  const routes = makeTassadarTraceContributionRoutes({
    agentStore: () => makeAgentStore(currentTokenUser),
    createVerificationChallenge: async (_env, input) => {
      // Use the real builder + lease + finalize path so the route returns the
      // same Verified/Rejected outcome production records for validate --auto.
      const built = buildTrainingVerificationChallengeRecord({
        makeId: () => `challenge-${++counter}`,
        nowIso: '2026-06-15T00:05:00.000Z',
        request: input.request,
      })
      const leased = leaseTrainingVerificationChallengeRecord({
        challenge: built.challenge,
        eventId: `lease-${++counter}`,
        nowIso: '2026-06-15T00:05:01.000Z',
        request: {
          leaseSeconds: 60,
          validatorRef: input.validatorDeviceRef,
        },
      })
      const verdict = await runTrainingVerificationClass({
        challenge: leased.challenge,
      })
      const finalized = finalizeTrainingVerificationChallengeRecord({
        challenge: leased.challenge,
        eventId: `finalize-${++counter}`,
        nowIso: '2026-06-15T00:05:02.000Z',
        request: { receiptRefs: [] },
        validatorRef: input.validatorDeviceRef,
        verdict,
      })
      challenges.push(finalized.challenge)

      return finalized.challenge
    },
    makeContributionStore: () => contributions,
    makeId: () => `id-${++counter}`,
    makeStore: () => authority,
    nowIso: () => '2026-06-15T00:05:00.000Z',
    ...(options.onVerifiedExactTraceReplayPair === undefined
      ? {}
      : {
          onVerifiedExactTraceReplayPair:
            options.onVerifiedExactTraceReplayPair,
        }),
    resolvePylonOwnerUserId: async (_env, pylonRef) =>
      PYLON_OWNERS.get(pylonRef),
  })

  return {
    authority,
    challenges,
    contributions,
    route: (path, options) => {
      currentTokenUser = options.tokenUserId
      const init: RequestInit = {
        headers: {
          ...(options.body === undefined
            ? {}
            : { 'content-type': 'application/json' }),
          ...(options.tokenUserId === undefined
            ? {}
            : { authorization: `Bearer oa_agent_${options.tokenUserId}` }),
        },
        method: options.method ?? 'POST',
      }

      if (options.body !== undefined) {
        init.body = JSON.stringify(options.body)
      }

      const request = new Request(`https://openagents.test${path}`, init)
      const response = routes.routeTassadarTraceContributionRequest(request, {})

      if (response === undefined) {
        throw new Error(`No route matched ${path}`)
      }

      return Effect.runPromise(response)
    },
  }
}

const submissionBody = (
  overrides: Record<string, unknown> = {},
): Record<string, unknown> => ({
  assignmentRef: 'assignment.tassadar.1',
  pylonDeviceRef: 'device.worker.1',
  sampledWindow: { endStep: 32, startStep: 0 },
  sampledWindowRef: 'window.sampled.1',
  traceCommitmentDigestRef: 'digest.trace.abc',
  workerReceiptRef: 'receipt.worker.1',
  workloadFamily: 'article_closeout',
  ...overrides,
})

const submitPath = `/api/training/leases/${LEASE_REF}/trace-submission`
const verdictPath = `/api/training/leases/${LEASE_REF}/replay-verdict`

describe('tassadar trace contribution routes (#5052)', () => {
  it('§4.1 records a pending worker contribution for an owned lease', async () => {
    const harness = makeHarness()
    const response = await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: WORKER_USER,
    })
    const body = (await response.json()) as {
      contribution: { state: string; pylonRef: string; workloadFamily: string }
    }

    expect(response.status).toBe(200)
    expect(body.contribution.state).toBe('pending')
    expect(body.contribution.pylonRef).toBe(WORKER_PYLON)
    expect(body.contribution.workloadFamily).toBe('article_closeout')
    expect(harness.contributions._records.size).toBe(1)
  })

  it('§4.1 requires an agent bearer token', async () => {
    const harness = makeHarness()
    const response = await harness.route(submitPath, {
      body: submissionBody(),
    })

    expect(response.status).toBe(401)
    expect(harness.contributions._records.size).toBe(0)
  })

  it('§4.1 rejects a lease owned by another registered Pylon with a clear hint', async () => {
    const harness = makeHarness()
    // The validator agent owns a DIFFERENT registered Pylon, not the worker
    // Pylon's lease: ownerUserId is defined but != session.user.id.
    const response = await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: VALIDATOR_USER,
    })
    const body = (await response.json()) as { error: string; reason: string }

    expect(response.status).toBe(403)
    // Typed tag is unchanged — only the human-facing reason is clearer.
    expect(body.error).toBe('training_authority_forbidden')
    expect(body.reason).toContain('registered to a different agent identity')
    expect(body.reason).toContain(WORKER_PYLON)
    expect(harness.contributions._records.size).toBe(0)
  })

  it('§4.1 tells an UNREGISTERED Pylon to run `pylon presence register` first', async () => {
    // The contributor footgun: a fresh node claimed a lease (public, no owner
    // binding) but never ran `pylon presence register`, so the lease pylon_ref
    // resolves to NO owning agent identity. The agent is a legitimate, fully
    // authenticated session; the only thing missing is the Pylon registration.
    const harness = makeHarness([leaseRecord({ pylonRef: 'pylon.unregistered' })])
    const response = await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: WORKER_USER,
    })
    const body = (await response.json()) as { error: string; reason: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('training_authority_forbidden')
    expect(body.reason).toContain('not registered to your agent identity')
    expect(body.reason).toContain('pylon presence register')
    expect(body.reason).toContain('pylon.unregistered')
    expect(harness.contributions._records.size).toBe(0)
  })

  it('§4.1 404s an unknown lease', async () => {
    const harness = makeHarness([])
    const response = await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: WORKER_USER,
    })

    expect(response.status).toBe(404)
  })

  it('§4.1 is idempotent by lease + workload', async () => {
    const harness = makeHarness()
    const first = await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: WORKER_USER,
    })
    const second = await harness.route(submitPath, {
      body: submissionBody({ workerReceiptRef: 'receipt.worker.retry' }),
      tokenUserId: WORKER_USER,
    })
    const firstBody = (await first.json()) as {
      contribution: { contributionRef: string }
    }
    const secondBody = (await second.json()) as {
      contribution: { contributionRef: string }
    }

    expect(first.status).toBe(200)
    expect(second.status).toBe(200)
    expect(harness.contributions._records.size).toBe(1)
    expect(secondBody.contribution.contributionRef).toBe(
      firstBody.contribution.contributionRef,
    )
    // The retried submission keeps the original worker receipt ref.
    const stored = [...harness.contributions._records.values()][0]!
    expect(stored.workerReceiptRef).toBe('receipt.worker.1')
  })

  it('§4.2 rejects self-validation (validator device == worker device)', async () => {
    const harness = makeHarness()
    await harness.route(submitPath, {
      body: submissionBody({ pylonDeviceRef: 'device.shared' }),
      tokenUserId: WORKER_USER,
    })
    const response = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.trace.abc',
        validatorDeviceRef: 'device.shared',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: WORKER_USER,
    })
    const body = (await response.json()) as { error: string }

    expect(response.status).toBe(403)
    expect(body.error).toBe('training_trace_contribution_forbidden')
    expect(harness.challenges.length).toBe(0)
    // The contribution stays pending; no verdict was created.
    const stored = [...harness.contributions._records.values()][0]!
    expect(stored.state).toBe('pending')
  })

  it('§4.2 digest match -> exact_trace_replay challenge finalizes Verified', async () => {
    const harness = makeHarness()
    await harness.route(submitPath, {
      body: submissionBody({ traceCommitmentDigestRef: 'digest.match' }),
      tokenUserId: WORKER_USER,
    })
    const response = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.match',
        validatorDeviceRef: 'device.validator.1',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: VALIDATOR_USER,
    })
    const body = (await response.json()) as {
      challenge: { challengeRef: string; state: string }
      contribution: { state: string; verificationChallengeRef: string }
    }

    expect(response.status).toBe(200)
    expect(harness.challenges.length).toBe(1)
    expect(harness.challenges[0]!.verificationClass).toBe('exact_trace_replay')
    // Matching worker/validator digests -> Verified (feeds verifiedWorkCount).
    expect(harness.challenges[0]!.state).toBe('Verified')
    expect(body.challenge.state).toBe('Verified')
    expect(body.contribution.state).toBe('paired')
    expect(body.contribution.verificationChallengeRef).toBe(
      harness.challenges[0]!.challengeRef,
    )
  })

  it('§4.2 keeps Verified verdicts fail-soft when auto-streaming fails', async () => {
    const autoStreamCalls: Array<string> = []
    const harness = makeHarness([leaseRecord()], {
      onVerifiedExactTraceReplayPair: (_env, input) => {
        autoStreamCalls.push(input.validatorContributorRef)

        return Effect.fail({ _tag: 'AutoStreamUnavailable' as const })
      },
    })
    await harness.route(submitPath, {
      body: submissionBody({ traceCommitmentDigestRef: 'digest.match' }),
      tokenUserId: WORKER_USER,
    })
    const response = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.match',
        validatorDeviceRef: 'device.validator.1',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: VALIDATOR_USER,
    })
    const body = (await response.json()) as {
      challenge: { state: string }
      contribution: { state: string }
    }

    expect(response.status).toBe(200)
    expect(body.challenge.state).toBe('Verified')
    expect(body.contribution.state).toBe('paired')
    expect(autoStreamCalls).toEqual(['device.validator.1'])
  })

  it('§4.2 digest mismatch -> exact_trace_replay challenge finalizes Rejected', async () => {
    const harness = makeHarness()
    await harness.route(submitPath, {
      body: submissionBody({ traceCommitmentDigestRef: 'digest.worker' }),
      tokenUserId: WORKER_USER,
    })
    const response = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.validator.different',
        validatorDeviceRef: 'device.validator.1',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: VALIDATOR_USER,
    })

    expect(response.status).toBe(200)
    expect(harness.challenges.length).toBe(1)
    // Differing digests -> Rejected (feeds rejectedWorkCount).
    expect(harness.challenges[0]!.state).toBe('Rejected')
  })

  it('§4.2 404s when no pending worker contribution exists', async () => {
    const harness = makeHarness()
    const response = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.any',
        validatorDeviceRef: 'device.validator.1',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: WORKER_USER,
    })

    expect(response.status).toBe(404)
    expect(harness.challenges.length).toBe(0)
  })

  it('§4.2 conflicts when the contribution is already paired', async () => {
    const harness = makeHarness()
    await harness.route(submitPath, {
      body: submissionBody({ traceCommitmentDigestRef: 'digest.match' }),
      tokenUserId: WORKER_USER,
    })
    await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.match',
        validatorDeviceRef: 'device.validator.1',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: VALIDATOR_USER,
    })
    const second = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.match',
        validatorDeviceRef: 'device.validator.2',
        workloadFamily: 'article_closeout',
      },
      tokenUserId: VALIDATOR_USER,
    })

    expect(second.status).toBe(409)
    expect(harness.challenges.length).toBe(1)
  })

  it('§4.2 requires an agent bearer token', async () => {
    const harness = makeHarness()
    const response = await harness.route(verdictPath, {
      body: {
        replayDigestRef: 'digest.any',
        validatorDeviceRef: 'device.validator.1',
        workloadFamily: 'article_closeout',
      },
    })

    expect(response.status).toBe(401)
  })

  it('authority builder + projection produce stable contribution refs', () => {
    const record = buildTrainingTraceContributionRecord({
      leaseRef: LEASE_REF,
      makeId: () => 'id-1',
      nowIso: '2026-06-15T00:05:00.000Z',
      pylonRef: WORKER_PYLON,
      request: {
        assignmentRef: 'assignment.tassadar.1',
        pylonDeviceRef: 'device.worker.1',
        sampledWindow: { endStep: 32, startStep: 0 },
        sampledWindowRef: 'window.sampled.1',
        traceCommitmentDigestRef: 'digest.trace.abc',
        workerReceiptRef: 'receipt.worker.1',
        workloadFamily: 'article_closeout',
      },
      trainingRunRef: RUN_REF,
      windowRef: WINDOW_REF,
    })

    expect(record.contributionRef).toBe(
      `contribution.tassadar_executor_trace.${LEASE_REF}.article_closeout`,
    )
    expect(record.state).toBe('pending')

    const projection = JSON.parse(
      pairedContributionProjectionJson(record, {
        replayDigestRef: 'digest.replay',
        validatorDeviceRef: 'device.validator.1',
        verificationChallengeRef: 'challenge-1',
      }),
    ) as { state: string; verificationChallengeRef: string }

    expect(projection.state).toBe('paired')
    expect(projection.verificationChallengeRef).toBe('challenge-1')
  })
})

const discoverPath = (query: string): string =>
  `/api/training/contributions/next-unpaired${query}`

describe('tassadar validator auto-discovery (#5121)', () => {
  it('returns the oldest pending contribution from a distinct worker device', async () => {
    const harness = makeHarness()
    await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: WORKER_USER,
    })

    const response = await harness.route(
      discoverPath('?validatorDeviceRef=device.validator.1'),
      { method: 'GET', tokenUserId: VALIDATOR_USER },
    )
    const body = (await response.json()) as {
      contribution: {
        leaseRef: string
        workloadFamily: string
        workerPylonDeviceRef: string
      } | null
    }

    expect(response.status).toBe(200)
    expect(body.contribution?.leaseRef).toBe(LEASE_REF)
    expect(body.contribution?.workloadFamily).toBe('article_closeout')
    expect(body.contribution?.workerPylonDeviceRef).toBe('device.worker.1')
  })

  it('skips a contribution whose worker device is the asking validator (no self-validate)', async () => {
    const harness = makeHarness()
    await harness.route(submitPath, {
      body: submissionBody(),
      tokenUserId: WORKER_USER,
    })

    // The only pending contribution is from device.worker.1; asking as that same
    // device must not hand it back to itself.
    const response = await harness.route(
      discoverPath('?validatorDeviceRef=device.worker.1'),
      { method: 'GET', tokenUserId: WORKER_USER },
    )
    const body = (await response.json()) as { contribution: unknown }

    expect(response.status).toBe(200)
    expect(body.contribution).toBeNull()
  })

  it('returns null when nothing is pending', async () => {
    const harness = makeHarness()
    const response = await harness.route(
      discoverPath('?validatorDeviceRef=device.validator.1'),
      { method: 'GET', tokenUserId: VALIDATOR_USER },
    )
    const body = (await response.json()) as { contribution: unknown }

    expect(response.status).toBe(200)
    expect(body.contribution).toBeNull()
  })

  it('requires the validatorDeviceRef query parameter', async () => {
    const harness = makeHarness()
    const response = await harness.route(discoverPath(''), {
      method: 'GET',
      tokenUserId: VALIDATOR_USER,
    })

    expect(response.status).toBe(400)
  })

  it('requires an agent bearer token', async () => {
    const harness = makeHarness()
    const response = await harness.route(
      discoverPath('?validatorDeviceRef=device.validator.1'),
      { method: 'GET' },
    )

    expect(response.status).toBe(401)
  })

  it('rejects a non-GET method', async () => {
    const harness = makeHarness()
    const response = await harness.route(
      discoverPath('?validatorDeviceRef=device.validator.1'),
      { method: 'POST', tokenUserId: VALIDATOR_USER },
    )

    expect(response.status).toBe(405)
  })
})
