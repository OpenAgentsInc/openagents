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
})
