import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AGENT_TOKEN_PREFIX,
  type AgentRegistrationStore,
} from './agent-registration'
import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
} from './autopilot-work-request'
import {
  type AutopilotWorkExecutor,
  type AutopilotWorkExecutionCloseoutRecord,
  type AutopilotWorkOrderRecord,
  type AutopilotWorkStore,
  makeAutopilotWorkRoutes,
  recordAutopilotWorkerCloseoutFromPylon,
} from './autopilot-work-routes'
import type {
  PylonApiAssignmentRecord,
  PylonApiEventRecord,
  PylonApiRegistrationRecord,
  PylonApiStore,
} from './pylon-api'
import { PylonApiStoreError } from './pylon-api'
import { makePylonApiRoutes } from './pylon-api-routes'

class MemoryAutopilotWorkStore implements AutopilotWorkStore {
  readonly records = new Map<string, AutopilotWorkOrderRecord>()
  readonly recordsByOwnerIdempotency = new Map<string, AutopilotWorkOrderRecord>()

  createWorkOrder = async (record: AutopilotWorkOrderRecord) => {
    const key = `${record.ownerUserId}:${record.idempotencyKeyHash}`
    const existing = this.recordsByOwnerIdempotency.get(key)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.records.set(record.workOrderRef, record)
    this.recordsByOwnerIdempotency.set(key, record)

    return { idempotent: false, record }
  }

  readWorkOrder = async (workOrderRef: string) =>
    this.records.get(workOrderRef)

  recordPylonAssignmentDispatch = async (input: Readonly<{
    ownerUserId: string
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    const updated = {
      ...existing,
      state: 'queued_or_running' as const,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }

  recordExecutionCloseout = async (input: Readonly<{
    executionCloseout: AutopilotWorkExecutionCloseoutRecord
    ownerUserId: string
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    const updated = {
      ...existing,
      executionCloseout: input.executionCloseout,
      state: 'delivered' as const,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }

  recordBuyerPaymentProof = async (input: Readonly<{
    buyerPaymentProofRef: string
    ownerUserId: string
    updatedAt: string
    workOrderRef: string
  }>) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    const updated = {
      ...existing,
      buyerPaymentProofRef: input.buyerPaymentProofRef,
      state: 'paid_ready' as const,
      updatedAt: input.updatedAt,
    }
    const key = `${existing.ownerUserId}:${existing.idempotencyKeyHash}`

    this.records.set(existing.workOrderRef, updated)
    this.recordsByOwnerIdempotency.set(key, updated)

    return updated
  }

  readWorkOrderByIdempotency = async (
    ownerUserId: string,
    idempotencyKeyHash: string,
  ) => this.recordsByOwnerIdempotency.get(`${ownerUserId}:${idempotencyKeyHash}`)
}

class MemoryPylonApiStore implements PylonApiStore {
  readonly assignments = new Map<string, PylonApiAssignmentRecord>()
  readonly assignmentsByIdempotency = new Map<string, PylonApiAssignmentRecord>()
  readonly events = new Map<string, PylonApiEventRecord>()
  readonly eventsByIdempotency = new Map<string, PylonApiEventRecord>()
  readonly registrations = new Map<string, PylonApiRegistrationRecord>()

  constructor(registrations: ReadonlyArray<PylonApiRegistrationRecord>) {
    registrations.forEach(registration => {
      this.registrations.set(registration.pylonRef, registration)
    })
  }

  createAssignment = async (record: PylonApiAssignmentRecord) => {
    const existing = this.assignmentsByIdempotency.get(
      record.idempotencyKeyHash,
    )

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.assignments.set(record.assignmentRef, record)
    this.assignmentsByIdempotency.set(record.idempotencyKeyHash, record)

    return { idempotent: false, record }
  }

  createEvent = async (record: PylonApiEventRecord) => {
    const existing = this.eventsByIdempotency.get(record.idempotencyKeyHash)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.events.set(record.eventRef, record)
    this.eventsByIdempotency.set(record.idempotencyKeyHash, record)

    return { idempotent: false, record }
  }

  listAssignmentsForPylon = async (pylonRef: string, limit: number) =>
    Array.from(this.assignments.values())
      .filter(assignment => assignment.pylonRef === pylonRef)
      .slice(0, limit)

  listEventsForPylon = async (pylonRef: string, limit: number) =>
    Array.from(this.events.values())
      .filter(event => event.pylonRef === pylonRef)
      .slice(0, limit)

  listEventsForAssignment = async (assignmentRef: string, limit: number) =>
    Array.from(this.events.values())
      .filter(event => event.assignmentRef === assignmentRef)
      .slice(0, limit)

  listRegistrations = async (limit: number) =>
    Array.from(this.registrations.values()).slice(0, limit)

  readAssignment = async (assignmentRef: string) =>
    this.assignments.get(assignmentRef)

  readAssignmentByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.assignmentsByIdempotency.get(idempotencyKeyHash)

  readEventByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.eventsByIdempotency.get(idempotencyKeyHash)

  readRegistration = async (pylonRef: string) =>
    this.registrations.get(pylonRef)

  updateAssignment = async (record: PylonApiAssignmentRecord) => {
    this.assignments.set(record.assignmentRef, record)
    this.assignmentsByIdempotency.set(record.idempotencyKeyHash, record)

    return record
  }

  upsertRegistration = async (record: PylonApiRegistrationRecord) => {
    const existing = this.registrations.get(record.pylonRef)

    if (
      existing !== undefined &&
      existing.ownerAgentUserId !== record.ownerAgentUserId
    ) {
      throw new PylonApiStoreError({
        kind: 'conflict',
        reason: 'Pylon ref is already owned by another registered agent.',
      })
    }

    const next =
      existing === undefined
        ? record
        : {
            ...record,
            createdAt: existing.createdAt,
            id: existing.id,
          }

    this.registrations.set(record.pylonRef, next)

    return next
  }
}

const agentToken = `${AGENT_TOKEN_PREFIX}autopilot-work-test`

const agentStoreForScopes = (
  scopes: ReadonlyArray<string> = [
    'customer_orders.read',
    'customer_orders.write',
  ],
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: 'agent_credential_autopilot_work_test',
      profileMetadataJson: JSON.stringify({
        customerOrderGrants: [
          {
            expiresAt: null,
            ownerUserId: 'github:autopilot-owner',
            scopes,
            status: 'active',
          },
        ],
      }),
      tokenPrefix: `${AGENT_TOKEN_PREFIX}autopilot`,
      user: {
        avatarUrl: null,
        createdAt: '2026-06-09T17:30:00.000Z',
        displayName: 'Autopilot Work Agent',
        id: 'agent_user_autopilot_work',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: '2026-06-09T17:30:00.000Z',
      },
    }),
  touchAgentCredential: () => Promise.resolve(),
})

const pylonRegistration = (
  override: Partial<PylonApiRegistrationRecord> = {},
): PylonApiRegistrationRecord => ({
  capabilityRefs: [
    'capability.pylon.assignment_ready',
    'capability.pylon.local_codex',
  ],
  clientProtocolVersion: '0.2.5',
  clientVersion: '0.2.5',
  createdAt: '2026-06-09T17:25:00.000Z',
  displayName: 'Requester Pylon',
  id: 'pylon_registration_1',
  latestCapacityRefs: ['capacity.pylon.assignment_ready'],
  latestHeartbeatAt: '2026-06-09T17:29:30.000Z',
  latestHeartbeatStatus: 'ready',
  latestHealthRefs: ['health.pylon.ready'],
  latestLoadRefs: ['load.pylon.available'],
  latestResourceMode: 'balanced',
  ownerAgentCredentialId: 'agent_credential_autopilot_work_test',
  ownerAgentTokenPrefix: 'oa_agent',
  ownerAgentUserId: 'agent_user_autopilot_work',
  publicProjectionJson: '{}',
  pylonRef: 'pylon.local.docs_agent',
  resourceMode: 'balanced',
  status: 'active',
  updatedAt: '2026-06-09T17:29:30.000Z',
  walletReady: true,
  walletRef: 'wallet_ref.pylon.local.docs_agent',
  ...override,
})

const route = async (
  store: MemoryAutopilotWorkStore,
  path: string,
  options: Readonly<{
    body?: unknown
    headers?: HeadersInit
    idempotencyKey?: string
    executeReadyWork?: AutopilotWorkExecutor
    method?: string
    pylonApiStore?: PylonApiStore
    pylonRegistrations?: ReadonlyArray<PylonApiRegistrationRecord>
    pylonStoreRegistrations?: ReadonlyArray<PylonApiRegistrationRecord>
    scopes?: ReadonlyArray<string>
    token?: string
  }> = {},
) => {
  let counter = 0
  const maybePylonApiStore = options.pylonApiStore
  const dependencies = {
    agentStore: () => agentStoreForScopes(options.scopes),
    makeId: () => `autopilot_work_order.test_${++counter}`,
    makeStore: () => store,
    nowIso: () => '2026-06-09T17:30:00.000Z',
    ...(options.executeReadyWork === undefined
      ? {}
      : {
          executeReadyWork: (
            _env: Record<string, unknown>,
            input: Parameters<AutopilotWorkExecutor>[0],
          ) => options.executeReadyWork?.(input) ?? Promise.resolve(undefined),
        }),
    ...(options.pylonRegistrations === undefined
      ? {}
      : {
          pylonRegistrations: () =>
            Promise.resolve(options.pylonRegistrations ?? []),
        }),
    ...(options.pylonStoreRegistrations === undefined
      ? {}
      : {
          makePylonApiStore: () => ({
            listRegistrations: () =>
              Promise.resolve(options.pylonStoreRegistrations ?? []),
          }),
        }),
    ...(maybePylonApiStore === undefined
      ? {}
      : { makePylonApiStore: () => maybePylonApiStore }),
  }
  const routes = makeAutopilotWorkRoutes<Record<string, unknown>>(
    dependencies,
  )
  const body = options.body === undefined
    ? {}
    : { body: JSON.stringify(options.body) }
  const request = new Request(`https://openagents.com${path}`, {
    ...body,
    headers: {
      ...options.headers,
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.idempotencyKey === undefined
        ? {}
        : { 'Idempotency-Key': options.idempotencyKey }),
      ...(options.token === undefined
        ? { authorization: `Bearer ${agentToken}` }
        : options.token === ''
          ? {}
          : { authorization: `Bearer ${options.token}` }),
    },
    method: options.method ?? (options.body === undefined ? 'GET' : 'POST'),
  })
  const response = routes.routeAutopilotWorkRequest(request, {})

  if (response === undefined) {
    throw new Error(`No Autopilot work route matched ${path}`)
  }

  return Effect.runPromise(response)
}

const pylonRoute = async (
  store: PylonApiStore,
  path: string,
  options: Readonly<{
    body?: unknown
    headers?: HeadersInit
    idempotencyKey?: string
    method?: string
    recordAutopilotWorkerCloseout?: Parameters<
      typeof makePylonApiRoutes<Record<string, unknown>>
    >[0]['recordAutopilotWorkerCloseout']
    token?: string
  }> = {},
) => {
  const routes = makePylonApiRoutes<Record<string, unknown>>({
    agentStore: () => agentStoreForScopes(),
    makeStore: () => store,
    nowIso: () => '2026-06-09T17:30:30.000Z',
    ...(options.recordAutopilotWorkerCloseout === undefined
      ? {}
      : {
          recordAutopilotWorkerCloseout:
            options.recordAutopilotWorkerCloseout,
        }),
  })
  const body = options.body === undefined
    ? {}
    : { body: JSON.stringify(options.body) }
  const request = new Request(`https://openagents.com${path}`, {
    ...body,
    headers: {
      ...options.headers,
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.idempotencyKey === undefined
        ? {}
        : { 'Idempotency-Key': options.idempotencyKey }),
      ...(options.token === undefined
        ? { authorization: `Bearer ${agentToken}` }
        : options.token === ''
          ? {}
          : { authorization: `Bearer ${options.token}` }),
    },
    method: options.method ?? (options.body === undefined ? 'GET' : 'POST'),
  })
  const response = routes.routePylonApiRequest(request, {})

  if (response === undefined) {
    throw new Error(`No Pylon API route matched ${path}`)
  }

  return Effect.runPromise(response)
}

const responseJson = async (response: Response) =>
  response.json() as Promise<Readonly<{
    error?: string
    events?: ReadonlyArray<Readonly<{
      eventKind: string
      publicSafe: boolean
      sequence: number
      taskRefs: ReadonlyArray<string>
      workOrderRef: string
    }>>
    nextAfter?: number
    work?: Readonly<{
      accessRequirements?: ReadonlyArray<Readonly<{
        accessRequestRef: string
        grantAction: string
        kind: string
        ownerActionRef: string
        reasonRef: string
        requiredBeforeLaunch: boolean
        status: string
        taskRef: string
      }>>
      assignmentIntents?: ReadonlyArray<Readonly<{
        accessState: string
        assignmentIntentRef: string
        assignmentKind: string
        deployAuthority: boolean
        paymentState: string
        placementState: string
        plannerReasonRefs: ReadonlyArray<string>
        plannerState: string
        readyForAssignment: boolean
        spendAuthority: boolean
        taskRef: string
        workerPayoutEligible: boolean
        workOrderRef: string
      }>>
      buyerPaymentProofRef?: string | null
      accessRequestRefs?: ReadonlyArray<string>
      executionCloseout?: Readonly<{
        acceptedWorkAuthority: boolean
        assignmentRefs: ReadonlyArray<string>
        closeoutRefs: ReadonlyArray<string>
        forumAutoPublishAllowed: boolean
        proofRefs: ReadonlyArray<string>
        publicSafe: boolean
        resultRefs: ReadonlyArray<string>
        runnerKind: string
        workerPayoutAuthority: boolean
      }> | null
      fallbackLeaseIntents?: ReadonlyArray<Readonly<{
        assignmentRef: string
        fallbackLaneRef: string
        forumAutoPublishAllowed: boolean
        paymentMode: string
        requiredCapabilityRefs: ReadonlyArray<string>
        runnerKind: string
        spendCapRefs: ReadonlyArray<string>
        taskRef: string
        workerPayoutAuthority: boolean
      }>>
      funding?: Readonly<{
        buyerFundingState: string
        buyerPaymentProofRef: string | null
        fundedAmountCents: number
        quoteRef: string
        settlementBlockedReasonRef: string
        settlementEligible: boolean
        workerPayoutEligible: boolean
      }>
      idempotent: boolean
      nextAction?: Readonly<{
        callerActionRefs: ReadonlyArray<string>
        reasonRefs: ReadonlyArray<string>
        retryAfterSeconds: number | null
        state: string
      }>
      paymentChallenge?: Readonly<{
        amountCents: number
        challengeRef: string
        kind: string
        quoteRef: string
        status: string
      }> | null
      paymentChallengeRef: string | null
      placementDecision?: Readonly<{
        availabilityState: string
        callerActionRefs: ReadonlyArray<string>
        fallbackRunnerKind: string | null
        pylonCandidates: ReadonlyArray<Readonly<{
          assignmentReady: boolean
          heartbeatFresh: boolean
          localExecutionReady: boolean
          ownerLinked: boolean
          pylonRef: string
          selected: boolean
          versionCompatible: boolean
          walletReady: boolean
        }>>
        reasonRefs: ReadonlyArray<string>
        refusalReasonRefs: ReadonlyArray<string>
        retryAfterSeconds: number | null
        selectedPylonRef: string | null
        selectedRunnerKind: string | null
        source: string
      }>
      placementPolicy?: Readonly<{
        allowedRunnerKinds: ReadonlyArray<string>
        auditable: boolean
        disallowedRunnerKinds: ReadonlyArray<string>
        localOnlyAllowed: boolean
        placementPolicyRef: string
        preferredRunnerKinds: ReadonlyArray<string>
        privacyTier: string
        promptKeywordRouting: boolean
        publicTraceAllowed: boolean
        reasonRefs: ReadonlyArray<string>
        requiresSecretBroker: boolean
      }>
      pylonAssignmentIntents?: ReadonlyArray<Readonly<{
        assignmentRef: string
        forumAutoPublishAllowed: boolean
        paymentMode: string
        pylonRef: string
        requiredCapabilityRefs: ReadonlyArray<string>
        spendCapRefs: ReadonlyArray<string>
        taskRef: string
      }>>
      quote?: Readonly<{
        amountCents: number
        paymentRequired: boolean
        quoteRef: string
      }>
      repositoryAuthorities?: ReadonlyArray<Readonly<{
        deployAuthority: boolean
        fullName: string
        pullRequestAuthority: string
        readAuthority: string
        spendAuthority: boolean
        taskRef: string
        writeAuthority: string
      }>>
      state: string
      taskRefs: ReadonlyArray<string>
      tasks?: ReadonlyArray<Readonly<{
        acceptanceCriteriaRefs: ReadonlyArray<string>
        accessRequirements: ReadonlyArray<Readonly<{
          accessRequestRef: string
          grantAction: string
          kind: string
          ownerActionRef: string
          reasonRef: string
          requiredBeforeLaunch: boolean
          status: string
          taskRef: string
        }>>
        accessState: string
        kind: string
        lifecycleState: string
        paymentState: string
        placementState: string
        repository: Readonly<{
          branch: string
          fullName: string
          provider: string
          visibility: string
        }> | null
        taskRef: string
      }>>
      workOrderRef: string
    }>
    assignment?: Readonly<{
      assignmentRef: string
      leaseState: string
      state: string
    }>
    assignments?: ReadonlyArray<Readonly<{
      assignmentRef: string
      leaseState: string
      state: string
      taskRefs: ReadonlyArray<string>
    }>>
  }>>

describe('Autopilot work routes', () => {
  test('creates and recovers the same work projection with an idempotency key', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-create',
    })
    const replay = await route(store, '/api/autopilot/work', {
      body: {
        prompt: 'This malformed replay body should not replace the record.',
      },
      idempotencyKey: 'idem-autopilot-work-create',
    })
    const firstJson = await responseJson(first)
    const replayJson = await responseJson(replay)

    expect(first.status).toBe(202)
    expect(replay.status).toBe(200)
    expect(firstJson.work).toMatchObject({
      idempotent: false,
      placementPolicy: {
        allowedRunnerKinds: ['requester_pylon', 'openagents_shc'],
        auditable: true,
        disallowedRunnerKinds: [],
        localOnlyAllowed: false,
        placementPolicyRef:
          'placement_policy.autopilot_work_order.test_1',
        preferredRunnerKinds: ['requester_pylon'],
        privacyTier: 'public_beta',
        promptKeywordRouting: false,
        publicTraceAllowed: true,
        reasonRefs: [
          'placement.privacy.public_beta',
          'placement.local_only.not_allowed',
          'placement.public_trace.allowed',
          'placement.secret_broker.not_required',
        ],
        requiresSecretBroker: false,
      },
      state: 'accepted_free_slice',
      taskRefs: ['task.autopilot_coder.docs_contract'],
      workOrderRef: 'autopilot_work_order.test_1',
    })
    expect(replayJson.work).toEqual({
      ...firstJson.work,
      idempotent: true,
    })

    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)

    expect(detail.status).toBe(200)
    expect(detailJson.work).toEqual(firstJson.work)
  })

  test('requires idempotency on create', async () => {
    const response = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work',
      {
        body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      },
    )
    const body = await responseJson(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('autopilot_work_validation_error')
  })

  test('projects independent typed task records for batch requests', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
          taskRef: 'task.autopilot_coder.docs_contract',
        },
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [
            {
              kind: 'github_repo_write' as const,
              reasonRef: 'reason.repo_write_required',
            },
          ],
          acceptanceCriteriaRefs: ['acceptance.patch_tests_pass'],
          kind: 'test_repair' as const,
          taskRef: 'task.autopilot_coder.test_repair',
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-task-records',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work).toMatchObject({
      state: 'access_required',
      assignmentIntents: [
        {
          assignmentKind: 'repo_change',
          plannerReasonRefs: [
            'assignment.free_slice',
            'assignment.ready_for_assignment',
          ],
          plannerState: 'free_slice',
          readyForAssignment: true,
          taskRef: 'task.autopilot_coder.docs_contract',
        },
        {
          assignmentKind: 'test_repair',
          plannerReasonRefs: [
            'assignment.blocked.access_required',
            'access_request.task.autopilot_coder.test_repair.github_repo_write',
          ],
          plannerState: 'access_required',
          readyForAssignment: false,
          taskRef: 'task.autopilot_coder.test_repair',
        },
      ],
      taskRefs: [
        'task.autopilot_coder.docs_contract',
        'task.autopilot_coder.test_repair',
      ],
      tasks: [
        {
          acceptanceCriteriaRefs: [
            'acceptance.docs.updated',
            'acceptance.tests.contract',
          ],
          accessRequirements: [],
          accessState: 'satisfied',
          kind: 'code_change',
          lifecycleState: 'ready_for_assignment',
          paymentState: 'not_required',
          placementState: 'ready_for_assignment',
          taskRef: 'task.autopilot_coder.docs_contract',
        },
        {
          acceptanceCriteriaRefs: ['acceptance.patch_tests_pass'],
          accessRequirements: [
            {
              accessRequestRef:
                'access_request.task.autopilot_coder.test_repair.github_repo_write',
              grantAction: 'connect_github_repository',
              kind: 'github_repo_write',
              ownerActionRef:
                'owner_action.task.autopilot_coder.test_repair.github_repo_write',
              reasonRef: 'reason.repo_write_required',
              requiredBeforeLaunch: true,
              status: 'missing',
              taskRef: 'task.autopilot_coder.test_repair',
            },
          ],
          accessState: 'missing_required_access',
          kind: 'test_repair',
          lifecycleState: 'access_required',
          paymentState: 'not_required',
          placementState: 'blocked_on_access',
          taskRef: 'task.autopilot_coder.test_repair',
        },
      ],
    })
  })

  test('selects an online compatible requester Pylon before fallback', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-pylon-placement',
      pylonRegistrations: [
        pylonRegistration({
          ownerAgentUserId: 'other_agent',
          pylonRef: 'pylon.other',
        }),
        pylonRegistration(),
      ],
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.placementDecision).toMatchObject({
      fallbackRunnerKind: 'openagents_shc',
      reasonRefs: [
        'placement.selected.requester_pylon',
        'placement.pylon.preferred_before_fallback',
      ],
      selectedPylonRef: 'pylon.local.docs_agent',
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    })
    expect(body.work?.placementDecision?.pylonCandidates).toEqual([
      expect.objectContaining({
        ownerLinked: false,
        selected: false,
      }),
      expect.objectContaining({
        assignmentReady: true,
        heartbeatFresh: true,
        localExecutionReady: true,
        ownerLinked: true,
        selected: true,
        versionCompatible: true,
        walletReady: true,
      }),
    ])
    expect(body.work?.pylonAssignmentIntents).toEqual([
      expect.objectContaining({
        assignmentRef:
          'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract',
        forumAutoPublishAllowed: false,
        paymentMode: 'unpaid_smoke',
        pylonRef: 'pylon.local.docs_agent',
        requiredCapabilityRefs: [
          'capability.pylon.assignment_ready',
          'capability.pylon.local_codex',
          'capability.pylon.local_coding_agent',
        ],
        spendCapRefs: ['spend_cap.no_spend.autopilot_pylon_assignment'],
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
    ])
  })

  test('selects requester Pylon from the production Pylon store dependency', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-production-pylon-placement',
      pylonStoreRegistrations: [
        pylonRegistration({
          capabilityRefs: ['capability.pylon.assignment_ready'],
          pylonRef: 'pylon.missing_local_agent',
        }),
        pylonRegistration({
          pylonRef: 'pylon.production.docs_agent',
        }),
      ],
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.placementDecision).toMatchObject({
      fallbackRunnerKind: 'openagents_shc',
      selectedPylonRef: 'pylon.production.docs_agent',
      selectedRunnerKind: 'requester_pylon',
      source: 'requester_pylon',
    })
    expect(body.work?.placementDecision?.pylonCandidates).toEqual([
      expect.objectContaining({
        localExecutionReady: false,
        pylonRef: 'pylon.missing_local_agent',
        selected: false,
      }),
      expect.objectContaining({
        assignmentReady: true,
        heartbeatFresh: true,
        localExecutionReady: true,
        ownerLinked: true,
        pylonRef: 'pylon.production.docs_agent',
        selected: true,
        versionCompatible: true,
        walletReady: true,
      }),
    ])
    expect(body.work?.pylonAssignmentIntents).toEqual([
      expect.objectContaining({
        pylonRef: 'pylon.production.docs_agent',
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
    ])
  })

  test('creates one durable no-spend Pylon assignment lease for requester Pylon work', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])
    const create = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-pylon-lease',
      pylonApiStore,
    })
    const createJson = await responseJson(create)
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'
    const replay = await route(store, '/api/autopilot/work', {
      body: { ignored: 'idempotent replay does not create another lease' },
      idempotencyKey: 'idem-autopilot-work-pylon-lease',
      pylonApiStore,
    })
    const replayJson = await responseJson(replay)
    const poll = await pylonRoute(
      pylonApiStore,
      '/api/pylons/pylon.production.docs_agent/assignments',
    )
    const pollJson = await responseJson(poll)
    const accept = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
          accepted: true,
        },
        idempotencyKey: 'accept-autopilot-pylon-lease',
        method: 'POST',
      },
    )
    const acceptJson = await responseJson(accept)

    expect(create.status).toBe(202)
    expect(createJson.work).toMatchObject({
      assignmentIntents: [
        expect.objectContaining({
          plannerReasonRefs: ['assignment.queued_or_running'],
          plannerState: 'queued_or_running',
          readyForAssignment: false,
        }),
      ],
      placementDecision: {
        selectedPylonRef: 'pylon.production.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      pylonAssignmentIntents: [],
      state: 'queued_or_running',
    })
    expect(pylonApiStore.assignments.size).toBe(1)
    expect(replay.status).toBe(200)
    expect(replayJson.work?.state).toBe('queued_or_running')
    expect(pylonApiStore.assignments.size).toBe(1)
    expect(poll.status).toBe(200)
    expect(pollJson.assignments).toEqual([
      expect.objectContaining({
        assignmentRef,
        codingAssignment: expect.objectContaining({
          assignmentRef,
          budget: expect.objectContaining({
            paymentMode: 'unpaid_smoke',
            workerPayoutAuthority: false,
          }),
          closeoutSchema: expect.objectContaining({
            acceptedWorkAuthority: false,
          }),
          publicSafe: true,
          runnerKind: 'requester_pylon',
          schema: 'openagents.autopilot_coding_assignment.v1',
          tracePolicy: expect.objectContaining({
            rawPromptAllowed: false,
            rawProviderPayloadAllowed: false,
            rawRunnerLogAllowed: false,
            rawSourceArchiveAllowed: false,
          }),
        }),
        leaseState: 'active',
        state: 'offered',
        taskRefs: [
          'autopilot_work_order.test_1',
          'task.autopilot_coder.docs_contract',
        ],
      }),
    ])
    expect(accept.status).toBe(201)
    expect(acceptJson.assignment).toMatchObject({
      assignmentRef,
      leaseState: 'active',
      state: 'accepted',
    })
  })

  test('ingests Pylon worker closeout refs into delivered Autopilot work', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'
    const create = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-pylon-closeout',
      pylonApiStore,
    })

    expect(create.status).toBe(202)

    const accept = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
          accepted: true,
        },
        idempotencyKey: 'accept-autopilot-pylon-closeout',
        method: 'POST',
      },
    )
    const closeout = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
          blockerRefs: [],
          buildRefs: ['build.public.autopilot_docs.not_required'],
          closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
          previewRefs: ['preview.public.autopilot_docs.not_required'],
          proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
          resultRefs: ['result.public.autopilot_docs.delivered'],
          status: 'closeout_submitted',
          summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
          testRefs: ['test.public.autopilot_docs.not_required'],
        },
        idempotencyKey: 'worker-closeout-autopilot-pylon',
        method: 'POST',
        recordAutopilotWorkerCloseout: (_env, input) =>
          recordAutopilotWorkerCloseoutFromPylon(store, input),
      },
    )
    const delivered = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const events = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1/events',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const deliveredJson = await responseJson(delivered)
    const eventsJson = await responseJson(events)
    const closeoutJson = await responseJson(closeout)

    expect(accept.status).toBe(201)
    expect(closeout.status).toBe(201)
    expect(closeoutJson.assignment).toMatchObject({
      assignmentRef,
      acceptedWorkRefs: [],
      artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
      closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
      proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
      state: 'closeout_submitted',
    })
    expect(delivered.status).toBe(200)
    expect(deliveredJson.work).toMatchObject({
      executionCloseout: {
        acceptedWorkAuthority: false,
        artifactRefs: ['artifact.public.autopilot_docs.patch_summary'],
        blockerRefs: [],
        buildRefs: ['build.public.autopilot_docs.not_required'],
        closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
        forumAutoPublishAllowed: false,
        previewRefs: ['preview.public.autopilot_docs.not_required'],
        proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
        publicSafe: true,
        resultRefs: ['result.public.autopilot_docs.delivered'],
        runnerKind: 'requester_pylon',
        summaryRefs: ['summary.public.autopilot_docs.customer_safe'],
        testRefs: ['test.public.autopilot_docs.not_required'],
        workerPayoutAuthority: false,
      },
      nextAction: {
        reasonRefs: ['next_action.review_delivered_work'],
        state: 'delivered',
      },
      state: 'delivered',
    })
    expect(events.status).toBe(200)
    expect(eventsJson.events).toEqual([
      expect.objectContaining({ eventKind: 'queued' }),
      expect.objectContaining({ eventKind: 'delivered' }),
    ])
  })

  test('rejects unsafe Pylon worker closeout refs before Autopilot delivery persistence', async () => {
    const store = new MemoryAutopilotWorkStore()
    const pylonApiStore = new MemoryPylonApiStore([
      pylonRegistration({
        pylonRef: 'pylon.production.docs_agent',
      }),
    ])
    const assignmentRef =
      'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract'

    await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-unsafe-pylon-closeout',
      pylonApiStore,
    })
    await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.autopilot_pylon.accepted'],
          accepted: true,
        },
        idempotencyKey: 'accept-autopilot-pylon-unsafe-closeout',
        method: 'POST',
      },
    )

    const closeout = await pylonRoute(
      pylonApiStore,
      `/api/pylons/pylon.production.docs_agent/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public./Users/christopher/raw.patch'],
          closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
          proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
          resultRefs: ['result.public.autopilot_docs.delivered'],
          status: 'closeout_submitted',
        },
        idempotencyKey: 'worker-closeout-autopilot-pylon-unsafe',
        method: 'POST',
        recordAutopilotWorkerCloseout: (_env, input) =>
          recordAutopilotWorkerCloseoutFromPylon(store, input),
      },
    )
    const recovered = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        pylonApiStore,
      },
    )
    const recoveredJson = await responseJson(recovered)

    expect(closeout.status).toBe(400)
    expect(recovered.status).toBe(200)
    expect(recoveredJson.work).toMatchObject({
      executionCloseout: null,
      state: 'queued_or_running',
    })
  })

  test('returns actionable placement needs-input when no runner is available', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].placementPolicy,
        allowedRunnerKinds: ['requester_pylon'] as const,
        localOnlyAllowed: true,
        preferredRunnerKinds: ['requester_pylon'] as const,
        privacyTier: 'local_only' as const,
        publicTraceAllowed: false,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [],
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-placement-needs-input',
      pylonRegistrations: [],
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.funding?.buyerFundingState).toBe('not_required')
    expect(body.work?.placementDecision).toMatchObject({
      availabilityState: 'needs_input',
      callerActionRefs: [
        'caller.add_or_restart_pylon',
        'caller.relax_privacy_or_runner_policy',
      ],
      fallbackRunnerKind: null,
      refusalReasonRefs: [
        'placement.blocked.no_compatible_runner',
        'placement.blocked.local_only_without_eligible_pylon',
        'placement.blocked.no_pylon_candidates',
      ],
      retryAfterSeconds: null,
      selectedRunnerKind: null,
      source: 'none_available',
    })
    expect(body.work?.nextAction).toEqual({
      callerActionRefs: [
        'caller.add_or_restart_pylon',
        'caller.relax_privacy_or_runner_policy',
      ],
      reasonRefs: [
        'placement.blocked.no_compatible_runner',
        'placement.blocked.local_only_without_eligible_pylon',
        'placement.blocked.no_pylon_candidates',
      ],
      retryAfterSeconds: null,
      state: 'needs_input',
    })
    expect(body.work?.fallbackLeaseIntents).toEqual([])
    expect(body.work?.pylonAssignmentIntents).toEqual([])
  })

  test('allows public read-only repository tasks to proceed', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-public-read',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work).toMatchObject({
      accessRequirements: [],
      accessRequestRefs: [],
      funding: {
        buyerFundingState: 'not_required',
        fundedAmountCents: 0,
        settlementBlockedReasonRef: 'settlement.no_worker_payout_mode',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallengeRef: null,
      state: 'accepted_free_slice',
    })
    expect(body.work?.repositoryAuthorities).toEqual([
      expect.objectContaining({
        deployAuthority: false,
        fullName: 'OpenAgentsInc/openagents',
        pullRequestAuthority: 'not_requested',
        readAuthority: 'public_read_available',
        spendAuthority: false,
        taskRef: 'task.autopilot_coder.docs_contract',
        writeAuthority: 'not_requested',
      }),
    ])
  })

  test('returns the same deterministic quote across payment challenge proof retry and detail', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        quoteRef: null,
        quotedAmountCents: null,
      },
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-paid-quote',
    })
    const replay = await route(store, '/api/autopilot/work', {
      body: { ignored: 'idempotent replay does not replace stored request' },
      idempotencyKey: 'idem-autopilot-work-paid-quote',
    })
    const firstJson = await responseJson(first)
    const replayJson = await responseJson(replay)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      headers: {
        'X-OpenAgents-L402':
          'oa-l402-v1.autopilot_test:payment_proof.autopilot_work.test_1',
      },
      idempotencyKey: 'idem-autopilot-work-paid-quote',
    })
    const paidJson = await responseJson(paid)
    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)

    expect(first.status).toBe(402)
    expect(first.headers.get('www-authenticate')).toContain('L402')
    expect(replay.status).toBe(402)
    expect(firstJson.work).toMatchObject({
      assignmentIntents: [
        {
          assignmentKind: 'test_repair',
          plannerReasonRefs: ['assignment.blocked.payment_required'],
          plannerState: 'payment_required',
          readyForAssignment: false,
          taskRef: 'task.autopilot_coder.paid_test_repair',
        },
      ],
      funding: {
        buyerFundingState: 'payment_required',
        buyerPaymentProofRef: null,
        fundedAmountCents: 0,
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        settlementBlockedReasonRef: 'settlement.buyer_payment_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallenge: {
        amountCents: 6400,
        challengeRef:
          'challenge.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        kind: 'l402',
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        status: 'payment_required',
      },
      paymentChallengeRef:
        'challenge.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      quote: {
        amountCents: 6400,
        paymentRequired: true,
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      },
      state: 'payment_required',
    })
    expect(firstJson.work?.fallbackLeaseIntents).toEqual([])
    expect(replayJson.work?.quote).toEqual(firstJson.work?.quote)
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      assignmentIntents: [
        {
          assignmentKind: 'test_repair',
          plannerReasonRefs: [
            'assignment.paid_ready',
            'assignment.ready_for_assignment',
          ],
          plannerState: 'paid_ready',
          readyForAssignment: true,
          taskRef: 'task.autopilot_coder.paid_test_repair',
        },
      ],
      buyerPaymentProofRef: 'payment_proof.autopilot_work.test_1',
      funding: {
        buyerFundingState: 'funded',
        buyerPaymentProofRef: 'payment_proof.autopilot_work.test_1',
        fundedAmountCents: 6400,
        quoteRef:
          'quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallenge: {
        status: 'paid_ready',
      },
      quote: firstJson.work?.quote,
      state: 'paid_ready',
    })
    expect(paidJson.work?.fallbackLeaseIntents).toEqual([
      expect.objectContaining({
        assignmentRef:
          'fallback_assignment.autopilot_work_order.test_1.task.autopilot_coder.paid_test_repair',
        fallbackLaneRef: 'fallback_lane.openagents.shc',
        forumAutoPublishAllowed: false,
        paymentMode: 'buyer_funded',
        requiredCapabilityRefs: [
          'capability.fallback.assignment_ready',
          'capability.openagents.shc',
        ],
        runnerKind: 'openagents_shc',
        spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
        taskRef: 'task.autopilot_coder.paid_test_repair',
        workerPayoutAuthority: false,
      }),
    ])
    expect(detailJson.work?.quote).toEqual(firstJson.work?.quote)
    expect(detailJson.work?.buyerPaymentProofRef).toBe(
      'payment_proof.autopilot_work.test_1',
    )
    expect(detailJson.work?.funding).toEqual(paidJson.work?.funding)
    expect(detailJson.work?.paymentChallengeRef).toBe(
      firstJson.work?.paymentChallengeRef,
    )
  })

  test('projects a funded hosted Gemini fallback lease without execution authority', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      clientRequestRef: 'client.example.20260609.hosted_gemini_smoke',
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        maxSpendCents: 5000,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['hosted_gemini'] as const,
        preferredRunnerKinds: ['hosted_gemini'] as const,
        privacyTier: 'cloud_allowed' as const,
        publicTraceAllowed: true,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].tasks[0],
          acceptanceCriteriaRefs: [
            'acceptance.audit.updated_with_hosted_gemini_smoke_result',
          ],
          kind: 'research_and_patch' as const,
          objective:
            'Audit the red hosted Gemini product promise and return a public-safe documentation patch.',
          taskRef: 'task.product_promise_docs_hosted_gemini_smoke',
        },
      ],
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-smoke',
      pylonRegistrations: [],
    })
    const firstJson = await responseJson(first)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      headers: {
        'X-OpenAgents-L402':
          'oa-l402-v1.autopilot_test:payment_proof.autopilot_work.hosted_gemini_smoke',
      },
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-smoke',
      pylonRegistrations: [],
    })
    const paidJson = await responseJson(paid)

    expect(first.status).toBe(402)
    expect(firstJson.work).toMatchObject({
      fallbackLeaseIntents: [],
      funding: {
        buyerFundingState: 'payment_required',
        settlementBlockedReasonRef: 'settlement.buyer_payment_required',
        workerPayoutEligible: false,
      },
      placementDecision: {
        selectedRunnerKind: 'hosted_gemini',
        source: 'fallback',
      },
      quote: {
        amountCents: 3700,
        paymentRequired: true,
      },
      state: 'payment_required',
    })
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef:
        'payment_proof.autopilot_work.hosted_gemini_smoke',
      funding: {
        buyerFundingState: 'funded',
        fundedAmountCents: 3700,
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      nextAction: {
        callerActionRefs: [],
        state: 'ready',
      },
      placementDecision: {
        selectedRunnerKind: 'hosted_gemini',
        source: 'fallback',
      },
      state: 'paid_ready',
    })
    expect(paidJson.work?.fallbackLeaseIntents).toEqual([
      expect.objectContaining({
        assignmentRef:
          'fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_smoke',
        fallbackLaneRef: 'fallback_lane.openagents.hosted_gemini',
        forumAutoPublishAllowed: false,
        paymentMode: 'buyer_funded',
        requiredCapabilityRefs: [
          'capability.fallback.assignment_ready',
          'capability.openagents.hosted_gemini',
        ],
        runnerKind: 'hosted_gemini',
        spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
        taskRef: 'task.product_promise_docs_hosted_gemini_smoke',
        workerPayoutAuthority: false,
      }),
    ])
    expect(paidJson.work?.pylonAssignmentIntents).toEqual([])
    expect(paidJson.work?.executionCloseout).toBeNull()
  })

  test('delivers a paid hosted Gemini work order through the execution closeout bridge', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      clientRequestRef: 'client.example.20260609.hosted_gemini_closeout',
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        maxSpendCents: 5000,
        quoteRef: null,
        quotedAmountCents: null,
      },
      placementPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].placementPolicy,
        allowedRunnerKinds: ['hosted_gemini'] as const,
        preferredRunnerKinds: ['hosted_gemini'] as const,
        privacyTier: 'cloud_allowed' as const,
        publicTraceAllowed: true,
      },
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].tasks[0],
          acceptanceCriteriaRefs: [
            'acceptance.audit.updated_with_hosted_gemini_closeout',
          ],
          kind: 'research_and_patch' as const,
          objective:
            'Audit the hosted Gemini product promise and return a public-safe closeout.',
          taskRef: 'task.product_promise_docs_hosted_gemini_closeout',
        },
      ],
    }
    const executeReadyWork: AutopilotWorkExecutor = async ({ work }) => ({
      assignmentRefs: work.fallbackLeaseIntents.map(
        intent => intent.assignmentRef,
      ),
      closeoutRefs: work.fallbackLeaseIntents.flatMap(intent => [
        `closeout.${intent.assignmentRef}.public_safe_summary_delivered`,
        `closeout.${intent.assignmentRef}.tests_or_blocker_retained`,
      ]),
      proofRefs: work.fallbackLeaseIntents.map(
        intent => `proof.${intent.assignmentRef}.route_harness`,
      ),
      resultRefs: work.fallbackLeaseIntents.flatMap(
        intent => intent.resultExpectationRefs,
      ),
      runnerKind: 'hosted_gemini',
    })
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      executeReadyWork,
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-closeout',
      pylonRegistrations: [],
    })
    const firstJson = await responseJson(first)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      executeReadyWork,
      headers: {
        'X-OpenAgents-L402':
          'oa-l402-v1.autopilot_test:payment_proof.autopilot_work.hosted_gemini_closeout',
      },
      idempotencyKey: 'idem-autopilot-work-hosted-gemini-closeout',
      pylonRegistrations: [],
    })
    const paidJson = await responseJson(paid)
    const detail = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}`,
      { method: 'GET' },
    )
    const detailJson = await responseJson(detail)
    const events = await route(
      store,
      `/api/autopilot/work/${firstJson.work?.workOrderRef}/events`,
      { method: 'GET' },
    )
    const eventsJson = await responseJson(events)

    expect(first.status).toBe(402)
    expect(firstJson.work?.state).toBe('payment_required')
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef:
        'payment_proof.autopilot_work.hosted_gemini_closeout',
      executionCloseout: {
        acceptedWorkAuthority: false,
        assignmentRefs: [
          'fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout',
        ],
        closeoutRefs: [
          'closeout.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.public_safe_summary_delivered',
          'closeout.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.tests_or_blocker_retained',
        ],
        forumAutoPublishAllowed: false,
        proofRefs: [
          'proof.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.route_harness',
        ],
        publicSafe: true,
        resultRefs: [
          'result.fallback_assignment.autopilot_work_order.test_1.task.product_promise_docs_hosted_gemini_closeout.public_safe_closeout',
        ],
        runnerKind: 'hosted_gemini',
        workerPayoutAuthority: false,
      },
      funding: {
        buyerFundingState: 'funded',
        fundedAmountCents: 3700,
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      nextAction: {
        callerActionRefs: ['caller.review_autopilot_closeout'],
        reasonRefs: ['next_action.review_delivered_work'],
        retryAfterSeconds: null,
        state: 'delivered',
      },
      paymentChallenge: {
        status: 'paid_ready',
      },
      placementDecision: {
        selectedRunnerKind: 'hosted_gemini',
        source: 'fallback',
      },
      state: 'delivered',
      tasks: [
        {
          lifecycleState: 'delivered',
          placementState: 'delivered',
          taskRef: 'task.product_promise_docs_hosted_gemini_closeout',
        },
      ],
    })
    expect(paidJson.work?.assignmentIntents).toEqual([
      expect.objectContaining({
        plannerReasonRefs: ['assignment.delivered'],
        plannerState: 'delivered',
        readyForAssignment: false,
      }),
    ])
    expect(paidJson.work?.fallbackLeaseIntents).toEqual([])
    expect(detail.status).toBe(200)
    expect(detailJson.work?.executionCloseout).toEqual(
      paidJson.work?.executionCloseout,
    )
    expect(events.status).toBe(200)
    expect(eventsJson.events).toEqual([
      expect.objectContaining({
        eventKind: 'queued',
        publicSafe: true,
        sequence: 1,
      }),
      expect.objectContaining({
        eventKind: 'delivered',
        publicSafe: true,
        sequence: 2,
      }),
    ])
  })

  test('accepts an MDK checkout proof retry for payable work', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
      paymentPolicy: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1].paymentPolicy,
        buyerPaymentMode: 'mdk_checkout' as const,
        quoteRef: null,
        quotedAmountCents: null,
      },
    }
    const first = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-mdk-checkout',
    })
    const firstJson = await responseJson(first)
    const paid = await route(store, '/api/autopilot/work', {
      body: { ignored: 'paid retry does not replace stored request' },
      headers: {
        'X-OpenAgents-MDK-Checkout-Proof':
          'checkout_proof.autopilot_work.test_1',
      },
      idempotencyKey: 'idem-autopilot-work-mdk-checkout',
    })
    const paidJson = await responseJson(paid)

    expect(first.status).toBe(402)
    expect(first.headers.get('www-authenticate')).toBeNull()
    expect(firstJson.work?.paymentChallenge).toMatchObject({
      amountCents: 6400,
      checkoutIntentRef:
        'checkout_intent.quote.autopilot_work.client.example.20260609.002.6400.openagents.autopilot_work_quote.v1',
      kind: 'mdk_checkout',
      status: 'payment_required',
    })
    expect(firstJson.work?.funding).toMatchObject({
      buyerFundingState: 'payment_required',
      buyerPaymentProofRef: null,
      fundedAmountCents: 0,
      settlementBlockedReasonRef: 'settlement.buyer_payment_required',
      settlementEligible: false,
      workerPayoutEligible: false,
    })
    expect(paid.status).toBe(200)
    expect(paidJson.work).toMatchObject({
      buyerPaymentProofRef: 'checkout_proof.autopilot_work.test_1',
      funding: {
        buyerFundingState: 'funded',
        fundedAmountCents: 6400,
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
        settlementEligible: false,
        workerPayoutEligible: false,
      },
      paymentChallenge: {
        kind: 'mdk_checkout',
        status: 'paid_ready',
      },
      state: 'paid_ready',
    })
  })

  test('returns exact structured access requirements before launch', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [
            {
              kind: 'github_account_link',
              reasonRef: 'access.github.account_link',
            },
            {
              kind: 'repository_selection',
              reasonRef: 'access.repository.selection',
            },
            {
              kind: 'github_repo_write',
              reasonRef: 'access.github.repo_write',
            },
            {
              kind: 'pylon_enrollment',
              reasonRef: 'access.pylon.enrollment',
            },
            {
              kind: 'secret_broker',
              reasonRef: 'access.broker.required',
            },
            {
              kind: 'privacy_tier_confirmation',
              reasonRef: 'access.privacy.confirmation',
            },
            {
              kind: 'customer_review',
              reasonRef: 'access.customer.review',
            },
            {
              kind: 'operator_review',
              reasonRef: 'access.operator.review',
            },
          ],
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-access-required',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.state).toBe('access_required')
    expect(body.work?.accessRequirements).toEqual([
      expect.objectContaining({
        accessRequestRef:
          'access_request.task.autopilot_coder.docs_contract.github_account_link',
        grantAction: 'connect_github_account',
        kind: 'github_account_link',
        reasonRef: 'access.github.account_link',
        requiredBeforeLaunch: true,
        status: 'missing',
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
      expect.objectContaining({
        grantAction: 'select_repository',
        kind: 'repository_selection',
        reasonRef: 'access.repository.selection',
      }),
      expect.objectContaining({
        grantAction: 'connect_github_repository',
        kind: 'github_repo_write',
      }),
      expect.objectContaining({
        grantAction: 'enroll_pylon',
        kind: 'pylon_enrollment',
      }),
      expect.objectContaining({
        grantAction: 'configure_secret_broker',
        kind: 'secret_broker',
      }),
      expect.objectContaining({
        grantAction: 'confirm_privacy_tier',
        kind: 'privacy_tier_confirmation',
      }),
      expect.objectContaining({
        grantAction: 'customer_review',
        kind: 'customer_review',
      }),
      expect.objectContaining({
        grantAction: 'operator_review',
        kind: 'operator_review',
      }),
    ])
    expect(body.work?.paymentChallengeRef).toBeNull()
  })

  test('blocks branch and pull request work until owner approval', async () => {
    const store = new MemoryAutopilotWorkStore()
    const request = {
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          accessRequests: [
            {
              kind: 'github_branch_write',
              reasonRef: 'access.github.branch_write',
            },
            {
              kind: 'github_pull_request',
              reasonRef: 'access.github.pull_request',
            },
          ],
        },
      ],
    }
    const response = await route(store, '/api/autopilot/work', {
      body: request,
      idempotencyKey: 'idem-autopilot-work-branch-pr',
    })
    const body = await responseJson(response)

    expect(response.status).toBe(202)
    expect(body.work?.state).toBe('access_required')
    expect(body.work?.accessRequirements).toEqual([
      expect.objectContaining({
        grantAction: 'authorize_github_branch',
        kind: 'github_branch_write',
        requiredBeforeLaunch: true,
        status: 'missing',
      }),
      expect.objectContaining({
        grantAction: 'authorize_github_pull_request',
        kind: 'github_pull_request',
        requiredBeforeLaunch: true,
        status: 'missing',
      }),
    ])
    expect(body.work?.repositoryAuthorities).toEqual([
      expect.objectContaining({
        deployAuthority: false,
        pullRequestAuthority: 'owner_grant_required',
        readAuthority: 'public_read_available',
        spendAuthority: false,
        writeAuthority: 'owner_grant_required',
      }),
    ])
  })

  test('requires a registered agent grant for create and read', async () => {
    const create = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work',
      {
        body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        idempotencyKey: 'idem-autopilot-work-unauthorized',
        token: '',
      },
    )
    const read = await route(
      new MemoryAutopilotWorkStore(),
      '/api/autopilot/work/autopilot_work_order.test_1',
      {
        method: 'GET',
        token: '',
      },
    )

    expect(create.status).toBe(401)
    expect(read.status).toBe(401)
  })

  test('requires read scope for detail recovery', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-read-scope',
    })
    const createJson = await responseJson(create)
    const read = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}`,
      {
        method: 'GET',
        scopes: ['customer_orders.write'],
      },
    )

    expect(read.status).toBe(401)
  })

  test('returns pollable work events without internal operator logs', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [
              {
                kind: 'github_repo_write',
                reasonRef: 'access.github.repo_write',
              },
            ],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-events',
    })
    const createJson = await responseJson(create)
    const events = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}/events`,
      { method: 'GET' },
    )
    const eventsJson = await responseJson(events)

    expect(events.status).toBe(200)
    expect(eventsJson.nextAfter).toBe(2)
    expect(eventsJson.events).toEqual([
      expect.objectContaining({
        eventKind: 'queued',
        publicSafe: true,
        sequence: 1,
        taskRefs: ['task.autopilot_coder.docs_contract'],
        workOrderRef: 'autopilot_work_order.test_1',
      }),
      expect.objectContaining({
        eventKind: 'needs_access',
        publicSafe: true,
        sequence: 2,
        taskRefs: ['task.autopilot_coder.docs_contract'],
        workOrderRef: 'autopilot_work_order.test_1',
      }),
    ])
  })

  test('supports event cursors and server-sent event formatting', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: {
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            accessRequests: [
              {
                kind: 'github_repo_write',
                reasonRef: 'access.github.repo_write',
              },
            ],
          },
        ],
      },
      idempotencyKey: 'idem-autopilot-work-event-stream',
    })
    const createJson = await responseJson(create)
    const events = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}/events?after=1`,
      {
        headers: { accept: 'text/event-stream' },
        method: 'GET',
      },
    )
    const body = await events.text()

    expect(events.status).toBe(200)
    expect(events.headers.get('content-type')).toContain('text/event-stream')
    expect(body).toContain('id: 2')
    expect(body).toContain('event: needs_access')
    expect(body).not.toContain('id: 1')
  })

  test('requires read scope for work events', async () => {
    const store = new MemoryAutopilotWorkStore()
    const create = await route(store, '/api/autopilot/work', {
      body: OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      idempotencyKey: 'idem-autopilot-work-events-scope',
    })
    const createJson = await responseJson(create)
    const events = await route(
      store,
      `/api/autopilot/work/${createJson.work?.workOrderRef}/events`,
      {
        method: 'GET',
        scopes: ['customer_orders.write'],
      },
    )

    expect(events.status).toBe(401)
  })
})
