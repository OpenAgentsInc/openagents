import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  compilerInput,
  evaluationInputFor,
  genesisState,
  releaseGate,
  stateAfter,
} from './blueprint/services/test-forensic-prompt-fixtures'
import {
  compileForensicPromptCandidates,
  promoteForensicPrompt,
  validateForensicPromptEvaluation,
} from './blueprint/services/forensic-prompt-compiler'
import { ForensicPromptGovernanceError } from './blueprint/repositories/forensic-prompt-governance'
import type {
  ForensicPromptActiveTransition,
  ForensicPromptGovernanceState,
} from './blueprint/schemas/forensic-prompt-optimization'
import {
  BLUEPRINT_FORENSIC_PROMPT_GOVERNANCE_PATH,
  makeBlueprintForensicPromptGovernanceRoutes,
} from './blueprint-forensic-prompt-governance-routes'

const OWNER = 'owner.forensic.operator'
const NOW = '2026-08-03T20:30:00.000Z'

type Appended = Readonly<{
  ownerRef: string
  expectedRevision: number
  transition: ForensicPromptActiveTransition
}>

/**
 * An in-memory stand-in for the durable store. It records exactly what the
 * route handed it, so a test can check the compare-and-set revision the route
 * chose rather than only the response body.
 */
const makeFakeStore = (
  initial: ForensicPromptGovernanceState,
  options: Readonly<{
    appendError?: ForensicPromptGovernanceError
    readError?: ForensicPromptGovernanceError
  }> = {},
) => {
  let state = initial
  const appended: Array<Appended> = []

  return {
    appended,
    get state() {
      return state
    },
    store: {
      append: (
        ownerRef: string,
        transition: unknown,
        expectedRevision: number,
      ) => {
        if (options.appendError !== undefined) {
          return Effect.fail(options.appendError)
        }

        const decided = transition as ForensicPromptActiveTransition
        appended.push({ ownerRef, expectedRevision, transition: decided })
        state = {
          activePromptDigest: decided.activePromptDigest,
          history: [...state.history, decided],
          ownerRef,
          revision: state.revision + 1,
          schema: state.schema,
        }

        return Effect.succeed(state)
      },
      read: (ownerRef: string) =>
        options.readError === undefined
          ? Effect.succeed({ ...state, ownerRef })
          : Effect.fail(options.readError),
    },
  }
}

const makeRoutes = (
  fake: ReturnType<typeof makeFakeStore>,
  authorized = true,
) =>
  makeBlueprintForensicPromptGovernanceRoutes<Record<string, never>>({
    nowIso: () => NOW,
    requireAdminApiToken: () => Promise.resolve(authorized),
    store: () => fake.store,
  })

const url = (query = '') =>
  `https://openagents.com${BLUEPRINT_FORENSIC_PROMPT_GOVERNANCE_PATH}${query}`

const call = async (
  routes: ReturnType<typeof makeRoutes>,
  request: Request,
): Promise<Readonly<{ status: number; body: Record<string, unknown> }>> => {
  const response = await Effect.runPromise(routes.handle(request, {}))

  return {
    status: response.status,
    body: (await response.json()) as Record<string, unknown>,
  }
}

const post = (body: unknown) =>
  new Request(url(), { method: 'POST', body: JSON.stringify(body) })

/** A candidate, evaluation, and passing release gate that the gate admits. */
const admittedPromotionPayload = () => {
  const candidate = compileForensicPromptCandidates(compilerInput())
    .candidates[0]!
  const evaluation = validateForensicPromptEvaluation(
    evaluationInputFor(candidate),
  )

  return {
    candidate,
    decision: 'promote' as const,
    evaluation,
    evaluationRef: evaluation.evaluationRef,
    operatorDecisionRef: 'operator-decision.forensic.route.1',
    operatorIdentityRef: 'identity.operator.release',
    ownerRef: OWNER,
    releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
    transitionRef: 'transition.forensic.route.1',
  }
}

describe('blueprint forensic prompt governance routes', () => {
  test('refuses an unsupported method and an unauthenticated caller', async () => {
    const fake = makeFakeStore(genesisState())
    const notAllowed = await call(
      makeRoutes(fake),
      new Request(url(), { method: 'PUT' }),
    )
    expect(notAllowed.status).toBe(405)

    const unauthenticated = await call(
      makeRoutes(fake, false),
      new Request(url(`?ownerRef=${OWNER}`)),
    )
    expect(unauthenticated.status).toBe(401)
    expect(fake.appended).toHaveLength(0)
  })

  test('reads the durable pointer and its append-only history', async () => {
    const payload = admittedPromotionPayload()
    const activation = promoteForensicPrompt({
      candidate: payload.candidate,
      currentState: genesisState(),
      decidedAt: NOW,
      evaluation: payload.evaluation,
      evaluationRef: payload.evaluationRef,
      operatorDecisionRef: payload.operatorDecisionRef,
      operatorIdentityRef: payload.operatorIdentityRef,
      releaseGate: payload.releaseGate,
      transitionRef: payload.transitionRef,
    })
    const fake = makeFakeStore(stateAfter(activation))

    const missingOwner = await call(makeRoutes(fake), new Request(url()))
    expect(missingOwner.status).toBe(400)

    const read = await call(
      makeRoutes(fake),
      new Request(url(`?ownerRef=${OWNER}`)),
    )
    expect(read.status).toBe(200)
    expect(read.body['activePromptDigest']).toBe(
      payload.candidate.candidateDigest,
    )
    expect(read.body['revision']).toBe(1)
    expect(read.body['history']).toHaveLength(1)
  })

  test('promotes through the gate, sets the clock, and appends against the revision it read', async () => {
    const fake = makeFakeStore(genesisState())
    const payload = admittedPromotionPayload()
    const response = await call(
      makeRoutes(fake),
      post({
        ...payload,
        // A caller cannot backdate a decision or hand in its own transition.
        decidedAt: '2020-01-01T00:00:00.000Z',
        transition: { transitionDigest: 'sha256:'.padEnd(71, '0') },
      }),
    )

    expect(response.status).toBe(200)
    expect(fake.appended).toHaveLength(1)
    expect(fake.appended[0]?.expectedRevision).toBe(0)
    expect(fake.appended[0]?.ownerRef).toBe(OWNER)
    expect(fake.appended[0]?.transition.decidedAt).toBe(NOW)
    expect(fake.appended[0]?.transition.transitionType).toBe('activate')
    expect(fake.appended[0]?.transition.activePromptDigest).toBe(
      payload.candidate.candidateDigest,
    )
    expect(response.body['revision']).toBe(1)
  })

  test('surfaces the gate refusal and stores nothing when the producer promotes itself', async () => {
    const fake = makeFakeStore(genesisState())
    const payload = admittedPromotionPayload()
    const response = await call(
      makeRoutes(fake),
      post({
        ...payload,
        operatorIdentityRef:
          payload.candidate.blueprintModuleVersion.provenance.createdByRef,
      }),
    )

    expect(response.status).toBe(422)
    expect(response.body['error']).toBe('refused')
    expect(String(response.body['message'])).toMatch(
      /cannot evaluate or promote themselves/,
    )
    expect(fake.appended).toHaveLength(0)
  })

  test('rejects a malformed decision and a body that is not a JSON object', async () => {
    const fake = makeFakeStore(genesisState())
    const routes = makeRoutes(fake)

    const badDecision = await call(
      routes,
      post({ decision: 'activate', ownerRef: OWNER }),
    )
    expect(badDecision.status).toBe(400)

    const notJson = await call(
      routes,
      new Request(url(), { method: 'POST', body: 'not json' }),
    )
    expect(notJson.status).toBe(400)

    const noOwner = await call(routes, post({ decision: 'promote' }))
    expect(noOwner.status).toBe(400)
    expect(fake.appended).toHaveLength(0)
  })

  test('reverts the activation in force and refuses its producer on rollback', async () => {
    const payload = admittedPromotionPayload()
    const activation = promoteForensicPrompt({
      candidate: payload.candidate,
      currentState: genesisState(),
      decidedAt: NOW,
      evaluation: payload.evaluation,
      evaluationRef: payload.evaluationRef,
      operatorDecisionRef: payload.operatorDecisionRef,
      operatorIdentityRef: payload.operatorIdentityRef,
      releaseGate: payload.releaseGate,
      transitionRef: payload.transitionRef,
    })

    const producerAttempt = makeFakeStore(stateAfter(activation))
    const denied = await call(
      makeRoutes(producerAttempt),
      post({
        decision: 'rollback',
        operatorDecisionRef: 'operator-decision.forensic.rollback.1',
        operatorIdentityRef: activation.candidateProducerRef,
        ownerRef: OWNER,
        transitionRef: 'transition.forensic.rollback.1',
      }),
    )
    expect(denied.status).toBe(422)
    expect(producerAttempt.appended).toHaveLength(0)

    const fake = makeFakeStore(stateAfter(activation))
    const reverted = await call(
      makeRoutes(fake),
      post({
        decision: 'rollback',
        operatorDecisionRef: 'operator-decision.forensic.rollback.1',
        operatorIdentityRef: 'identity.operator.recovery',
        ownerRef: OWNER,
        transitionRef: 'transition.forensic.rollback.1',
      }),
    )

    expect(reverted.status).toBe(200)
    expect(fake.appended[0]?.expectedRevision).toBe(1)
    expect(fake.appended[0]?.transition.transitionType).toBe('rollback')
    expect(reverted.body['activePromptDigest']).toBe(null)
  })

  test('maps a stale-read conflict to 409 and unavailable storage to 503', async () => {
    const conflicted = makeFakeStore(genesisState(), {
      appendError: new ForensicPromptGovernanceError({
        code: 'conflict',
        message: 'forensic prompt active pointer changed',
        retryable: false,
      }),
    })
    const conflict = await call(
      makeRoutes(conflicted),
      post(admittedPromotionPayload()),
    )
    expect(conflict.status).toBe(409)
    expect(conflict.body['error']).toBe('conflict')

    const unavailable = makeFakeStore(genesisState(), {
      readError: new ForensicPromptGovernanceError({
        code: 'storage_unavailable',
        message: 'forensic prompt governance storage is unavailable',
        retryable: true,
      }),
    })
    const down = await call(
      makeRoutes(unavailable),
      new Request(url(`?ownerRef=${OWNER}`)),
    )
    expect(down.status).toBe(503)

    const driftRead = makeFakeStore(genesisState(), {
      readError: new ForensicPromptGovernanceError({
        code: 'invalid_transition',
        message: 'forensic prompt transition failed immutable validation',
        retryable: false,
      }),
    })
    const drift = await call(
      makeRoutes(driftRead),
      new Request(url(`?ownerRef=${OWNER}`)),
    )
    // Storage drift on read is a server integrity fault, not a caller error.
    expect(drift.status).toBe(500)
  })
})
