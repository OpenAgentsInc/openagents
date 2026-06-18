import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
} from './agent-registration'
import {
  type AutopilotContinuationDispatchResult,
  type AutopilotContinuationEventRecord,
  type AutopilotContinuationPolicyRecord,
  type AutopilotContinuationRunCandidate,
  type AutopilotContinuationStore,
  AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY,
  AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN,
  defaultAutopilotContinuationPolicy,
  runAutopilotContinuationSweep,
} from './autopilot-continuation-policy'
import { makeAutopilotContinuationPolicyRoutes } from './autopilot-continuation-policy-routes'

const nowIso = '2026-06-11T07:00:00.000Z'
const agentToken = `${AGENT_TOKEN_PREFIX}continuation-policy-test`

class MemoryContinuationStore implements AutopilotContinuationStore {
  readonly events = new Map<string, AutopilotContinuationEventRecord>()
  readonly policies = new Map<string, AutopilotContinuationPolicyRecord>()

  claimContinuationAttempt = async (
    record: AutopilotContinuationEventRecord,
  ) => {
    const key = `${record.runId}:${record.attempt}`

    if (
      [...this.events.values()].some(
        event => `${event.runId}:${event.attempt}` === key,
      )
    ) {
      return { claimed: false }
    }

    this.events.set(record.id, record)

    return { claimed: true }
  }

  countAttemptsForRun = async (runId: string) =>
    [...this.events.values()].filter(event => event.runId === runId).length

  countAttemptsForUserSince = async (userId: string, sinceIso: string) =>
    [...this.events.values()].filter(
      event => event.userId === userId && event.createdAt >= sinceIso,
    ).length

  listEnabledPolicies = async (limit: number) =>
    [...this.policies.values()].filter(policy => policy.enabled).slice(0, limit)

  listEventsForUserSince = async (
    userId: string,
    sinceIso: string,
    limit: number,
  ) =>
    [...this.events.values()]
      .filter(event => event.userId === userId && event.createdAt >= sinceIso)
      .slice(0, limit)

  markContinuationAttemptFailed = async (id: string, reasonRef: string) => {
    const existing = this.events.get(id)

    if (existing !== undefined) {
      this.events.set(id, {
        ...existing,
        decision: 'failed',
        reasonRef,
      })
    }
  }

  readPolicy = async (userId: string) => this.policies.get(userId)

  upsertPolicy = async (record: AutopilotContinuationPolicyRecord) => {
    this.policies.set(record.userId, record)

    return record
  }
}

const agentStoreForOwner = (
  ownerUserId = 'github:continuation-owner',
  scopes: ReadonlyArray<string> = [
    'customer_orders.read',
    'customer_orders.write',
  ],
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: 'agent_credential_continuation_test',
      profileMetadataJson: JSON.stringify({
        customerOrderGrants: [
          {
            expiresAt: null,
            ownerUserId,
            scopes,
            status: 'active',
          },
        ],
      }),
      tokenPrefix: AGENT_TOKEN_PREFIX,
      user: {
        avatarUrl: null,
        createdAt: nowIso,
        displayName: 'Continuation Agent',
        id: 'agent_user_continuation_test',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: nowIso,
      },
    }),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

const policyRoute = async (
  store: MemoryContinuationStore,
  options: Readonly<{
    body?: unknown
    method?: string
    sessionUserId?: string
    token?: string
  }> = {},
) => {
  const routes = makeAutopilotContinuationPolicyRoutes<
    Record<string, unknown>
  >({
    agentStore: () => agentStoreForOwner(),
    makeStore: () => store,
    nowIso: () => nowIso,
    requireBrowserSession: () =>
      Promise.resolve(
        options.sessionUserId === undefined
          ? undefined
          : { user: { userId: options.sessionUserId } },
      ),
  })
  const request = new Request(
    'https://openagents.com/api/autopilot/continuation-policy',
    {
      ...(options.body === undefined
        ? {}
        : { body: JSON.stringify(options.body) }),
      headers: {
        ...(options.body === undefined
          ? {}
          : { 'content-type': 'application/json' }),
        ...(options.token === undefined
          ? {}
          : { authorization: `Bearer ${options.token}` }),
      },
      method: options.method ?? (options.body === undefined ? 'GET' : 'PUT'),
    },
  )

  return Effect.runPromise(
    routes.routeAutopilotContinuationPolicyRequest(
      request,
      {},
      {} as ExecutionContext,
    ),
  )
}

const policyJson = async (response: Response) =>
  response.json() as Promise<Readonly<{
    error?: string
    generatedAt?: string
    policy?: Readonly<{
      budgetGateRefs: ReadonlyArray<string>
      enabled: boolean
      maxContinuationsPerDay: number
      maxContinuationsPerRun: number
      spendAuthority: boolean
      updatedAt: string | null
    }>
    reason?: string
  }>>

describe('Autopilot continuation policy routes (M6)', () => {
  test('reads the disabled default policy with budget gates declared', async () => {
    const store = new MemoryContinuationStore()
    const response = await policyRoute(store, {
      sessionUserId: 'github:continuation-owner',
    })
    const json = await policyJson(response)

    expect(response.status).toBe(200)
    expect(json.policy?.enabled).toBe(false)
    expect(json.policy?.maxContinuationsPerRun).toBe(
      AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_RUN,
    )
    expect(json.policy?.maxContinuationsPerDay).toBe(
      AUTOPILOT_CONTINUATION_DEFAULT_MAX_PER_DAY,
    )
    expect(json.policy?.spendAuthority).toBe(false)
    expect(json.policy?.budgetGateRefs).toContain(
      'budget_gate.billing.minimum_run_credits',
    )
    expect(json.generatedAt).toBe(nowIso)
  })

  test('browser owners can enable continuation with bounded counters', async () => {
    const store = new MemoryContinuationStore()
    const response = await policyRoute(store, {
      body: { enabled: true, maxContinuationsPerRun: 3 },
      sessionUserId: 'github:continuation-owner',
    })
    const json = await policyJson(response)

    expect(response.status).toBe(200)
    expect(json.policy?.enabled).toBe(true)
    expect(json.policy?.maxContinuationsPerRun).toBe(3)
    expect(store.policies.get('github:continuation-owner')?.enabled).toBe(true)
  })

  test('registered agents can set the policy through the same path (A1 parity)', async () => {
    const store = new MemoryContinuationStore()
    const response = await policyRoute(store, {
      body: { enabled: true, maxContinuationsPerDay: 5 },
      token: agentToken,
    })
    const json = await policyJson(response)

    expect(response.status).toBe(200)
    expect(json.policy?.enabled).toBe(true)
    expect(json.policy?.maxContinuationsPerDay).toBe(5)
    expect(store.policies.get('github:continuation-owner')?.enabled).toBe(true)

    const read = await policyRoute(store, { token: agentToken })

    expect((await policyJson(read)).policy?.enabled).toBe(true)
  })

  test('rejects out-of-bounds counters', async () => {
    const store = new MemoryContinuationStore()
    const response = await policyRoute(store, {
      body: { enabled: true, maxContinuationsPerRun: 99 },
      sessionUserId: 'github:continuation-owner',
    })
    const json = await policyJson(response)

    expect(response.status).toBe(400)
    expect(json.error).toBe('autopilot_continuation_policy_validation_error')
    expect(store.policies.size).toBe(0)
  })

  test('requires a browser session or agent token', async () => {
    const store = new MemoryContinuationStore()
    const response = await policyRoute(store)

    expect(response.status).toBe(401)
  })
})

type SweepHarnessOptions = Readonly<{
  billing?: AutopilotContinuationDispatchResult
  candidates: ReadonlyArray<AutopilotContinuationRunCandidate>
  followUpResult?: AutopilotContinuationDispatchResult
  goalResult?: AutopilotContinuationDispatchResult
  policy?: Partial<AutopilotContinuationPolicyRecord>
}>

const sweepHarness = (options: SweepHarnessOptions) => {
  const store = new MemoryContinuationStore()
  const followUps: Array<string> = []
  const goalContinuations: Array<string> = []
  let idCounter = 0

  store.policies.set('github:continuation-owner', {
    ...defaultAutopilotContinuationPolicy('github:continuation-owner', nowIso),
    enabled: true,
    ...options.policy,
  })

  const run = () =>
    Effect.runPromise(
      runAutopilotContinuationSweep({
        billingAllowsContinuation: async () =>
          options.billing ?? { ok: true, reasonRef: 'continuation.billing_ok' },
        dispatchFollowUpTurn: async candidate => {
          followUps.push(candidate.runId)

          return (
            options.followUpResult ?? {
              ok: true,
              reasonRef: 'continuation.dispatched.follow_up_turn',
            }
          )
        },
        dispatchGoalContinuation: async candidate => {
          goalContinuations.push(candidate.runId)

          return (
            options.goalResult ?? {
              ok: true,
              reasonRef: 'continuation.dispatched.goal_continuation',
            }
          )
        },
        listStoppedRunsForUser: async () => options.candidates,
        makeId: () => `continuation_event_${++idCounter}`,
        nowIso,
        store,
      }),
    )

  return { followUps, goalContinuations, run, store }
}

const failedRun = (
  runId: string,
  goalId: string | null = 'goal_1',
): AutopilotContinuationRunCandidate => ({
  goalId,
  runId,
  status: 'failed',
  updatedAt: '2026-06-11T03:00:00.000Z',
  userId: 'github:continuation-owner',
})

const waitingRun = (runId: string): AutopilotContinuationRunCandidate => ({
  goalId: 'goal_1',
  runId,
  status: 'waiting_for_input',
  updatedAt: '2026-06-11T03:00:00.000Z',
  userId: 'github:continuation-owner',
})

describe('Autopilot continuation sweep (M6)', () => {
  test('continues a failed run through goal continuation and records the attempt', async () => {
    const harness = sweepHarness({ candidates: [failedRun('run_failed_1')] })
    const report = await harness.run()

    expect(report.continuedRunIds).toEqual(['run_failed_1'])
    expect(harness.goalContinuations).toEqual(['run_failed_1'])
    expect(harness.followUps).toEqual([])
    expect(
      [...harness.store.events.values()].map(event => ({
        attempt: event.attempt,
        decision: event.decision,
        mode: event.mode,
      })),
    ).toEqual([
      { attempt: 1, decision: 'dispatched', mode: 'goal_continuation' },
    ])
  })

  test('continues a waiting run through a follow-up turn', async () => {
    const harness = sweepHarness({ candidates: [waitingRun('run_waiting_1')] })
    const report = await harness.run()

    expect(report.continuedRunIds).toEqual(['run_waiting_1'])
    expect(harness.followUps).toEqual(['run_waiting_1'])
    expect(harness.goalContinuations).toEqual([])
  })

  test('skips goal continuation when the run has no durable goal', async () => {
    const harness = sweepHarness({
      candidates: [failedRun('run_no_goal', null)],
    })
    const report = await harness.run()

    expect(report.continuedRunIds).toEqual([])
    expect(report.skipped).toEqual([
      {
        reasonRef: 'continuation.skipped.run_goal_required',
        runId: 'run_no_goal',
      },
    ])
  })

  test('stops at the max-continuations-per-run counter', async () => {
    const harness = sweepHarness({
      candidates: [failedRun('run_capped')],
      policy: { maxContinuationsPerRun: 1 },
    })

    await harness.run()

    const second = await harness.run()

    expect(second.continuedRunIds).toEqual([])
    expect(second.skipped).toEqual([
      {
        reasonRef: 'continuation.skipped.max_per_run_reached',
        runId: 'run_capped',
      },
    ])
    expect(harness.goalContinuations).toEqual(['run_capped'])
  })

  test('stops at the max-continuations-per-day counter', async () => {
    const harness = sweepHarness({
      candidates: [failedRun('run_a'), failedRun('run_b')],
      policy: { maxContinuationsPerDay: 1 },
    })
    const report = await harness.run()

    expect(report.continuedRunIds).toEqual(['run_a'])
    expect(report.skipped).toEqual([
      {
        reasonRef: 'continuation.skipped.max_per_day_reached',
        runId: 'run_b',
      },
    ])
  })

  test('skips every candidate when billing blocks continuation', async () => {
    const harness = sweepHarness({
      billing: {
        ok: false,
        reasonRef: 'continuation.skipped.billing_blocked',
      },
      candidates: [failedRun('run_broke')],
    })
    const report = await harness.run()

    expect(report.continuedRunIds).toEqual([])
    expect(report.skipped).toEqual([
      {
        reasonRef: 'continuation.skipped.billing_blocked',
        runId: 'run_broke',
      },
    ])
    expect(harness.store.events.size).toBe(0)
  })

  test('marks the claimed attempt failed when dispatch fails', async () => {
    const harness = sweepHarness({
      candidates: [failedRun('run_dispatch_fail')],
      goalResult: {
        ok: false,
        reasonRef: 'continuation.failed.goal_continuation',
      },
    })
    const report = await harness.run()

    expect(report.continuedRunIds).toEqual([])
    expect(report.skipped).toEqual([
      {
        reasonRef: 'continuation.failed.goal_continuation',
        runId: 'run_dispatch_fail',
      },
    ])
    expect(
      [...harness.store.events.values()].map(event => event.decision),
    ).toEqual(['failed'])
  })

  test('does nothing when no policy is enabled', async () => {
    const store = new MemoryContinuationStore()
    const report = await Effect.runPromise(
      runAutopilotContinuationSweep({
        billingAllowsContinuation: async () => ({
          ok: true,
          reasonRef: 'continuation.billing_ok',
        }),
        dispatchFollowUpTurn: async () => ({
          ok: true,
          reasonRef: 'continuation.dispatched.follow_up_turn',
        }),
        dispatchGoalContinuation: async () => ({
          ok: true,
          reasonRef: 'continuation.dispatched.goal_continuation',
        }),
        listStoppedRunsForUser: async () => [failedRun('run_ignored')],
        nowIso,
        store,
      }),
    )

    expect(report.continuedRunIds).toEqual([])
    expect(report.skipped).toEqual([])
  })
})
