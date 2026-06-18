import { Effect } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  type TrainingVerificationChallengeEventRecord,
  type TrainingVerificationChallengeRecord,
  type TrainingVerificationClass,
  type TrainingVerificationStore,
} from './training-verification'
import { makeTrainingVerificationRoutes } from './training-verification-routes'

const jsonRequest = (
  path: string,
  body: Record<string, unknown>,
  init: RequestInit = {},
): Request =>
  new Request(`https://openagents.test${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json', ...(init.headers ?? {}) },
    method: 'POST',
    ...init,
  })

const runRoute = async (
  route: Effect.Effect<Response> | undefined,
): Promise<Response> => {
  expect(route).toBeDefined()

  return Effect.runPromise(route!)
}

const makeMemoryStore = (): TrainingVerificationStore => {
  const challenges = new Map<string, TrainingVerificationChallengeRecord>()
  const events: Array<TrainingVerificationChallengeEventRecord> = []

  return {
    createChallenge: async (challenge, event) => {
      challenges.set(challenge.challengeRef, challenge)
      events.push(event)

      return challenge
    },
    leaseChallenge: async (challenge, event) => {
      challenges.set(challenge.challengeRef, challenge)
      events.push(event)

      return challenge
    },
    listLeaseCandidates: async (_nowIso, _limit, verificationClass) =>
      [...challenges.values()].filter(
        challenge =>
          ['Queued', 'Retrying'].includes(challenge.state) &&
          (verificationClass === undefined ||
            challenge.verificationClass === verificationClass),
      ),
    readChallenge: async challengeRef => challenges.get(challengeRef),
    transitionChallenge: async (challenge, event) => {
      challenges.set(challenge.challengeRef, challenge)
      events.push(event)

      return challenge
    },
  }
}

describe('training verification routes', () => {
  it('creates, leases, finalizes, and reads a verified challenge', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingVerificationRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const created = await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest('/api/training/verification/challenges', {
          commitmentRefs: ['commitment.training.output'],
          contributionRef: 'contribution.training.1',
          homeworkKind: 'admin_dispatched_homework',
          payload: {
            contributionRefs: ['contribution.training.1'],
            expectedDigestRef: 'digest.output.abc',
            recomputedDigestRef: 'digest.output.abc',
          },
          trainingRunRef: 'training.run.4674',
          verificationClass: 'deterministic_recompute',
        }),
        {},
      ),
    )
    const createdBody = await created.json() as {
      challenge: { challengeRef: string; state: string }
    }

    expect(created.status).toBe(200)
    expect(createdBody.challenge.state).toBe('Queued')

    const claimed = await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest('/api/training/verification/challenges/claim', {
          validatorRef: 'validator.training.1',
          verificationClass: 'deterministic_recompute' satisfies TrainingVerificationClass,
        }),
        {},
      ),
    )
    await expect(claimed.json()).resolves.toMatchObject({
      challenge: {
        challengeRef: createdBody.challenge.challengeRef,
        leasedToRef: 'validator.training.1',
        state: 'Leased',
      },
    })

    const finalized = await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest(
          `/api/training/verification/challenges/${encodeURIComponent(createdBody.challenge.challengeRef)}/finalize`,
          { receiptRefs: ['receipt.training.verified'] },
        ),
        {},
      ),
    )
    await expect(finalized.json()).resolves.toMatchObject({
      challenge: {
        failureCodes: [],
        state: 'Verified',
        verificationClass: 'deterministic_recompute',
      },
    })

    const read = await runRoute(
      routes.routeTrainingVerificationRequest(
        new Request(
          `https://openagents.test/api/training/verification/challenges/${encodeURIComponent(createdBody.challenge.challengeRef)}`,
        ),
        {},
      ),
    )
    await expect(read.json()).resolves.toMatchObject({
      challenge: { state: 'Verified' },
    })
  })

  it('supports retry and explicit timeout lifecycle routes', async () => {
    const store = makeMemoryStore()
    let counter = 0
    const routes = makeTrainingVerificationRoutes({
      makeId: () => String(++counter).padStart(4, '0'),
      makeStore: () => store,
      nowIso: () => '2026-06-10T10:00:00.000Z',
      requireAdminApiToken: async () => true,
    })

    const created = await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest('/api/training/verification/challenges', {
          homeworkKind: 'admin_dispatched_homework',
          maxAttempts: 2,
          payload: {
            contributionRefs: ['contribution.training.1'],
            expectedDigestRef: 'digest.output.abc',
            recomputedDigestRef: 'digest.output.wrong',
          },
          trainingRunRef: 'training.run.4674',
          verificationClass: 'deterministic_recompute',
        }),
        {},
      ),
    )
    const createdBody = await created.json() as {
      challenge: { challengeRef: string }
    }

    await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest('/api/training/verification/challenges/claim', {
          validatorRef: 'validator.training.1',
        }),
        {},
      ),
    )

    const retried = await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest(
          `/api/training/verification/challenges/${encodeURIComponent(createdBody.challenge.challengeRef)}/retry`,
          { failureCodes: ['LeaseExpired'] },
        ),
        {},
      ),
    )
    await expect(retried.json()).resolves.toMatchObject({
      challenge: { failureCodes: ['LeaseExpired'], state: 'Retrying' },
    })

    const timedOut = await runRoute(
      routes.routeTrainingVerificationRequest(
        jsonRequest(
          `/api/training/verification/challenges/${encodeURIComponent(createdBody.challenge.challengeRef)}/timeout`,
          {},
        ),
        {},
      ),
    )
    await expect(timedOut.json()).resolves.toMatchObject({
      challenge: {
        failureCodes: ['LeaseExpired', 'RetryBudgetExhausted'],
        state: 'TimedOut',
      },
    })
  })

  // #5403 gap 3: a skeptic must be able to dereference ONE worker->validator
  // replay pair directly, with no admin auth, and the public projection must
  // carry generatedAt + the staleness contract (projection-staleness
  // invariant) while never leaking seeds, payloads, payment hashes, or raw
  // traces.
  it('serves a single verification challenge under the public read path (no admin, staleness-declared)', async () => {
    const store = makeMemoryStore()
    const challengeRef = 'training.verification.challenge.5403.public'
    const verified: TrainingVerificationChallengeRecord = {
      challengeRef,
      commitmentRefs: ['commitment.training.output'],
      contributionRef: 'contribution.tassadar.5403',
      createdAt: '2026-06-16T10:00:00.000Z',
      failureCodes: [],
      homeworkKind: 'admin_dispatched_homework',
      id: 'challenge-5403-public',
      leaseExpiresAt: null,
      leaseRef: 'training.lease.5403',
      leasedToRef: 'pylon.public.validator_5403',
      maxAttempts: 3,
      payloadJson: JSON.stringify({
        // These MUST NOT appear in the public projection.
        rawTrace: 'SECRET-RAW-TRACE-bytes',
        seedPhrase: 'abandon abandon abandon',
        // Public-safe refs/digests the projection exposes.
        pylonDeviceRef: 'pylon.public.worker_5403',
        replayDigestRef: 'sha256:replaydigest5403',
        traceCommitmentDigestRef: 'sha256:commitmentdigest5403',
        validatorDeviceRef: 'pylon.public.validator_5403',
      }),
      publicProjectionJson: '{}',
      rejectedAt: null,
      samplingPolicy: 'per_contribution',
      state: 'Verified',
      timedOutAt: null,
      trainingRunRef: 'run.tassadar.executor.20260615',
      updatedAt: '2026-06-16T10:03:00.000Z',
      verdictRefs: ['verdict.training.exact_trace_replay.verified.5403'],
      verificationClass: 'exact_trace_replay',
      verifiedAt: '2026-06-16T10:03:00.000Z',
      windowRef: 'training.window.tassadar.executor.20260615.w1',
    }
    await store.createChallenge(verified, {
      challengeRef,
      createdAt: '2026-06-16T10:00:00.000Z',
      failureCodes: [],
      id: 'event-5403',
      receiptRefs: [],
      stateFrom: null,
      stateTo: 'Verified',
      transitionKind: 'seed',
      validatorRef: 'pylon.public.validator_5403',
    })

    // No requireAdminApiToken provided => public read must still succeed.
    const routes = makeTrainingVerificationRoutes({
      makeStore: () => store,
      nowIso: () => '2026-06-16T12:00:00.000Z',
    })

    const response = await runRoute(
      routes.routeTrainingVerificationRequest(
        new Request(
          `https://openagents.test/api/public/training/verification-challenges/${encodeURIComponent(challengeRef)}`,
        ),
        {},
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      challenge: {
        challengeRef: string
        exactTraceCommitmentDigestRef: string | null
        exactTraceReplayDigestRef: string | null
        leasedToRef: string | null
        state: string
        verdictRefs: ReadonlyArray<string>
        verificationClass: string
      }
      generatedAt: string
      schemaVersion: string
      staleness: { composition: string; maxStalenessSeconds: number }
    }

    expect(body.schemaVersion).toBe(
      'openagents.public_training_verification_challenge.v1',
    )
    expect(body.generatedAt).toBe('2026-06-16T12:00:00.000Z')
    expect(body.staleness.composition).toBe('live_at_read')
    expect(body.staleness.maxStalenessSeconds).toBe(0)

    // The single worker->validator replay pair is dereferenceable.
    expect(body.challenge.challengeRef).toBe(challengeRef)
    expect(body.challenge.state).toBe('Verified')
    expect(body.challenge.verificationClass).toBe('exact_trace_replay')
    expect(body.challenge.leasedToRef).toBe('pylon.public.validator_5403')
    expect(body.challenge.verdictRefs).toEqual([
      'verdict.training.exact_trace_replay.verified.5403',
    ])
    expect(body.challenge.exactTraceCommitmentDigestRef).toBe(
      'sha256:commitmentdigest5403',
    )
    expect(body.challenge.exactTraceReplayDigestRef).toBe(
      'sha256:replaydigest5403',
    )

    // Public-safe: never leak seeds or raw traces from the payload.
    const serialized = JSON.stringify(body)
    expect(serialized).not.toMatch(/SECRET-RAW-TRACE/i)
    expect(serialized).not.toMatch(/abandon abandon/i)
    expect(serialized).not.toMatch(/seedPhrase|rawTrace/i)
  })

  it('returns 404 for the public verification challenge read when not found (#5403)', async () => {
    const store = makeMemoryStore()
    const routes = makeTrainingVerificationRoutes({
      makeStore: () => store,
      nowIso: () => '2026-06-16T12:00:00.000Z',
    })

    const response = await runRoute(
      routes.routeTrainingVerificationRequest(
        new Request(
          'https://openagents.test/api/public/training/verification-challenges/does.not.exist',
        ),
        {},
      ),
    )

    expect(response.status).toBe(404)
  })
})
