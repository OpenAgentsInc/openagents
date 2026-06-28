import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { AGENT_TOKEN_PREFIX } from './agent-registration'
import type { AgentRegistrationStore } from './agent-registration'
import { makeAutopilotDecisionRoutes } from './autopilot-decision-routes'
import type { AutopilotDecisionCloseoutReceipt } from './autopilot-decision-closeout'
import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'
import {
  type AutopilotWorkExecutionCloseoutRecord,
  type AutopilotWorkOrderRecord,
  type AutopilotWorkReviewDecisionRecord,
  type AutopilotWorkStore,
  AutopilotWorkStoreError,
} from './autopilot-work-routes'

const agentToken = `${AGENT_TOKEN_PREFIX}autopilot-decision-test`
const fixtureNowIso = '2026-06-11T17:30:00.000Z'
const ownerUserId = 'github:autopilot-owner'

class MemoryAutopilotWorkStore implements AutopilotWorkStore {
  readonly closeoutReceipts =
    new Map<string, AutopilotDecisionCloseoutReceipt>()
  readonly records = new Map<string, AutopilotWorkOrderRecord>()

  createWorkOrder = async (record: AutopilotWorkOrderRecord) => {
    this.records.set(record.workOrderRef, record)

    return { idempotent: false, record }
  }

  readWorkOrder = async (workOrderRef: string) => this.records.get(workOrderRef)

  readWorkOrderByIdempotency = async () => undefined

  listWorkOrdersForOwner = async (
    input: Readonly<{ limit: number; ownerUserId: string }>,
  ) =>
    [...this.records.values()]
      .filter(record => record.ownerUserId === input.ownerUserId)
      .slice(0, input.limit)

  listPendingScheduledWorkOrders = async (input: Readonly<{ limit: number }>) =>
    [...this.records.values()]
      .filter(
        record =>
          record.scheduledLaunch !== null &&
          record.scheduledLaunch.dispatchedAt === null &&
          record.scheduledLaunch.expiredAt === null,
      )
      .slice(0, input.limit)

  recordScheduledLaunchTransition = async (
    input: Readonly<{
      scheduledLaunch: NonNullable<AutopilotWorkOrderRecord['scheduledLaunch']>
      state: AutopilotWorkOrderRecord['state']
      updatedAt: string
      workOrderRef: string
    }>,
  ) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.scheduledLaunch === null) {
      return undefined
    }

    const updated = {
      ...existing,
      scheduledLaunch: input.scheduledLaunch,
      state: input.state,
      updatedAt: input.updatedAt,
    }

    this.records.set(existing.workOrderRef, updated)

    return updated
  }

  recordPylonAssignmentDispatch = async () => undefined

  recordExecutionCloseout = async () => undefined

  recordBuyerPaymentProof = async () => undefined

  recordReviewDecision = async (
    input: Readonly<{
      ownerUserId: string
      reviewDecision: AutopilotWorkReviewDecisionRecord
      state: 'accepted' | 'rejected' | 'revision_required'
      updatedAt: string
      workOrderRef: string
    }>,
  ) => {
    const existing = this.records.get(input.workOrderRef)

    if (existing === undefined || existing.ownerUserId !== input.ownerUserId) {
      return undefined
    }

    if (existing.reviewDecision !== null) {
      if (
        existing.reviewDecision.idempotencyKeyHash ===
        input.reviewDecision.idempotencyKeyHash
      ) {
        return { idempotent: true, record: existing }
      }

      throw new AutopilotWorkStoreError({
        kind: 'conflict',
        reason:
          'Autopilot work already has a review decision with a different idempotency key.',
      })
    }

    if (existing.state !== 'delivered') {
      throw new AutopilotWorkStoreError({
        kind: 'conflict',
        reason: 'Autopilot work must be delivered before review.',
      })
    }

    const updated = {
      ...existing,
      reviewDecision: input.reviewDecision,
      state: input.state,
      updatedAt: input.updatedAt,
    }

    this.records.set(existing.workOrderRef, updated)

    return { idempotent: false, record: updated }
  }

  recordDecisionCloseoutReceipt = async (
    receipt: AutopilotDecisionCloseoutReceipt,
  ) => {
    this.closeoutReceipts.set(receipt.closeoutRef, receipt)
  }

  listDecisionCloseoutReceiptsForWorkOrder = async (
    input: Readonly<{ ownerUserId: string; workOrderRef: string }>,
  ) => {
    const record = this.records.get(input.workOrderRef)

    if (record?.ownerUserId !== input.ownerUserId) {
      return []
    }

    return [...this.closeoutReceipts.values()].filter(
      receipt => receipt.workOrderRef === input.workOrderRef,
    )
  }

  readDecisionCloseoutReceipt = async (
    input: Readonly<{ closeoutRef: string; ownerUserId: string }>,
  ) => {
    const receipt = this.closeoutReceipts.get(input.closeoutRef)

    if (receipt === undefined) {
      return undefined
    }

    const record = this.records.get(receipt.workOrderRef)

    return record?.ownerUserId === input.ownerUserId ? receipt : undefined
  }
}

const agentStoreForScopes = (
  scopes: ReadonlyArray<string> = [
    'customer_orders.read',
    'customer_orders.write',
  ],
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: 'agent_credential_autopilot_decision_test',
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
      tokenPrefix: `${AGENT_TOKEN_PREFIX}autopilot`,
      user: {
        avatarUrl: null,
        createdAt: fixtureNowIso,
        displayName: 'Autopilot Decision Agent',
        id: 'agent_user_autopilot_decision',
        kind: 'agent',
        primaryEmail: null,
        status: 'active',
        updatedAt: fixtureNowIso,
      },
    }),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

const fixtureRequest = decodeOpenAgentsAutopilotWorkRequest(
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
)

const deliveredCloseout = (): AutopilotWorkExecutionCloseoutRecord => ({
  assignmentRefs: ['assignment.public.autopilot_docs.worker'],
  closeoutRefs: ['closeout.public.autopilot_docs.worker_summary'],
  proofRefs: ['proof.public.autopilot_docs.worker_closeout'],
  resultRefs: ['result.public.autopilot_docs.delivered'],
  runnerKind: 'requester_pylon',
})

const workOrderRecord = (
  override: Partial<AutopilotWorkOrderRecord> = {},
): AutopilotWorkOrderRecord => ({
  accessRequestRefs: [],
  agentCredentialId: 'agent_credential_autopilot_decision_test',
  agentUserId: 'agent_user_autopilot_decision',
  archivedAt: null,
  buyerPaymentProofRef: null,
  clientRequestRef: 'client.decision_test.001',
  createdAt: '2026-06-11T16:00:00.000Z',
  eventStreamRef: 'event_stream.autopilot_work_order.decision_test',
  executionCloseout: null,
  id: 'autopilot_work_order.decision_test_1',
  idempotencyKeyHash: 'hash.decision_test_1',
  ownerUserId,
  paymentChallengeRef: null,
  request: fixtureRequest,
  reviewDecision: null,
  scheduledLaunch: null,
  state: 'queued_or_running',
  statusUrlRef: 'status.autopilot_work_order.decision_test_1',
  taskRefs: ['task.autopilot_coder.docs_contract'],
  updatedAt: '2026-06-11T16:30:00.000Z',
  workOrderRef: 'autopilot_work_order.decision_test_1',
  ...override,
})

const deliveredWorkOrderRecord = (
  override: Partial<AutopilotWorkOrderRecord> = {},
): AutopilotWorkOrderRecord =>
  workOrderRecord({
    executionCloseout: deliveredCloseout(),
    state: 'delivered',
    ...override,
  })

const route = async (
  store: MemoryAutopilotWorkStore,
  path: string,
  options: Readonly<{
    body?: unknown
    idempotencyKey?: string
    method?: string
    scopes?: ReadonlyArray<string>
    sessionUserId?: string
    token?: string
  }> = {},
) => {
  const routes = makeAutopilotDecisionRoutes<Record<string, unknown>>({
    agentStore: () => agentStoreForScopes(options.scopes),
    makeStore: () => store,
    nowIso: () => fixtureNowIso,
    ...(options.sessionUserId === undefined
      ? {}
      : {
          requireBrowserSession: () =>
            Promise.resolve({
              user: { userId: options.sessionUserId ?? ownerUserId },
            }),
        }),
  })
  const request = new Request(`https://openagents.com${path}`, {
    ...(options.body === undefined
      ? {}
      : { body: JSON.stringify(options.body) }),
    headers: {
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
  const response = routes.routeAutopilotDecisionRequest(
    request,
    {},
    {} as ExecutionContext,
  )

  if (response === undefined) {
    throw new Error(`No Autopilot decision route matched ${path}`)
  }

  return Effect.runPromise(response)
}

type DecisionListBody = Readonly<{
  decisions: ReadonlyArray<
    Readonly<{
      decision: Readonly<{
        actionKind: string
        actionLabel: string
        actionRef: string
        actionSubmissionRequired: boolean
        audience: string
        blockedReasonRefs: ReadonlyArray<string>
        customerNextActionRef: string
        directEffectPermitted: boolean
        id: string
        receiptRefs: ReadonlyArray<string>
        safeSummaryRef: string
        status: string
        statusLabel: string
      }>
      closeoutReceipts: ReadonlyArray<
        Readonly<{
          action: string
          closeoutRef: string
          outcome: string
          resolvedState: string
        }>
      >
      work: Readonly<{
        state: string
        taskRefs: ReadonlyArray<string>
        updatedAt: string
        workOrderRef: string
      }>
    }>
  >
  directEffectPermitted: boolean
  generatedAt: string
  pendingCount: number
}>

type DecisionActBody = Readonly<{
  closeout: Readonly<{
    closeoutRef: string
    outcome: string
    resolvedState: string
  }>
  decision: Readonly<{
    actionKind: string
    receiptRefs: ReadonlyArray<string>
    status: string
  }> | null
  directEffectPermitted: boolean
  error?: string
  generatedAt: string
  idempotent: boolean
  work: Readonly<{
    state: string
    workOrderRef: string
  }>
}>

describe('autopilot decision queue routes', () => {
  test('rejects unauthenticated decision list requests', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/decisions', {
      token: '',
    })

    expect(response.status).toBe(401)
  })

  test('rejects agent credentials without customer_orders.read scope', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/decisions', {
      scopes: [],
    })

    expect(response.status).toBe(401)
  })

  test('lists a pending review decision for delivered unreviewed work', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const response = await route(store, '/api/autopilot/decisions')
    const body = (await response.json()) as DecisionListBody

    expect(response.status).toBe(200)
    expect(body.generatedAt).toBe(fixtureNowIso)
    expect(body.directEffectPermitted).toBe(false)
    expect(body.pendingCount).toBe(1)
    expect(body.decisions).toHaveLength(1)

    const item = body.decisions[0]

    expect(item?.decision.actionKind).toBe('approve_pr_draft')
    expect(item?.decision.status).toBe('available')
    expect(item?.decision.directEffectPermitted).toBe(false)
    expect(item?.decision.actionSubmissionRequired).toBe(true)
    expect(item?.decision.audience).toBe('customer')
    expect(item?.decision.customerNextActionRef).toBe(
      'next_action.review_delivered_work',
    )
    expect(item?.decision.id).toBe(
      'decision_action.autopilot_work_order.decision_test_1.approve_pr_draft',
    )
    expect(item?.work.state).toBe('delivered')
    expect(item?.work.workOrderRef).toBe('autopilot_work_order.decision_test_1')
    expect(JSON.stringify(item?.decision)).not.toMatch(
      /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/,
    )
  })

  test('lists blocked customer-input decisions for access and payment gated work', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(
      workOrderRecord({
        accessRequestRefs: ['access_request.github_repo_read.docs_contract'],
        id: 'autopilot_work_order.decision_access',
        state: 'access_required',
        workOrderRef: 'autopilot_work_order.decision_access',
      }),
    )
    await store.createWorkOrder(
      workOrderRecord({
        id: 'autopilot_work_order.decision_payment',
        state: 'payment_required',
        workOrderRef: 'autopilot_work_order.decision_payment',
      }),
    )

    const response = await route(store, '/api/autopilot/decisions')
    const body = (await response.json()) as DecisionListBody

    expect(response.status).toBe(200)
    expect(body.pendingCount).toBe(2)

    const accessItem = body.decisions.find(
      item => item.work.workOrderRef === 'autopilot_work_order.decision_access',
    )
    const paymentItem = body.decisions.find(
      item =>
        item.work.workOrderRef === 'autopilot_work_order.decision_payment',
    )

    expect(accessItem?.decision.actionKind).toBe('request_customer_input')
    expect(accessItem?.decision.status).toBe('blocked')
    expect(accessItem?.decision.blockedReasonRefs).toContain(
      'blocked.access_required',
    )
    expect(accessItem?.decision.customerNextActionRef).toBe(
      'next_action.grant_required_access',
    )
    expect(paymentItem?.decision.status).toBe('blocked')
    expect(paymentItem?.decision.blockedReasonRefs).toEqual([
      'blocked.payment_required',
    ])
  })

  test('omits other owners work and supports browser-session listing', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())
    await store.createWorkOrder(
      deliveredWorkOrderRecord({
        id: 'autopilot_work_order.decision_other',
        ownerUserId: 'github:someone-else',
        workOrderRef: 'autopilot_work_order.decision_other',
      }),
    )

    const response = await route(store, '/api/autopilot/decisions', {
      sessionUserId: ownerUserId,
      token: '',
    })
    const body = (await response.json()) as DecisionListBody

    expect(response.status).toBe(200)
    expect(body.decisions).toHaveLength(1)
    expect(body.decisions[0]?.work.workOrderRef).toBe(
      'autopilot_work_order.decision_test_1',
    )
  })

  test('requires an idempotency key before acting on a decision', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        method: 'POST',
      },
    )

    expect(response.status).toBe(400)
  })

  test('acts on a pending decision through the gated review submission', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-accept-1',
        method: 'POST',
      },
    )
    const body = (await response.json()) as DecisionActBody

    expect(response.status).toBe(201)
    expect(body.idempotent).toBe(false)
    expect(body.directEffectPermitted).toBe(false)
    expect(body.work.state).toBe('accepted')
    expect(body.closeout.closeoutRef).toBe(
      'decision.closeout.accept.autopilot_work_order.decision_test_1',
    )
    expect(body.closeout.resolvedState).toBe('accepted')
    expect(body.decision?.status).toBe('completed')
    expect(body.decision?.receiptRefs).toContain(
      'decision.queue.accept.autopilot_work_order.decision_test_1',
    )

    const record = store.records.get('autopilot_work_order.decision_test_1')

    expect(record?.reviewDecision?.action).toBe('accept')
    expect(record?.reviewDecision?.decisionRefs).toContain(
      'decision.queue.accept.autopilot_work_order.decision_test_1',
    )
    expect(
      store.closeoutReceipts.get(
        'decision.closeout.accept.autopilot_work_order.decision_test_1',
      )?.receiptRefs,
    ).toContain('receipt.review.accept.autopilot_work_order.decision_test_1')
  })

  test('accepts the public accept command name for the review action', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-public-accept-1',
        method: 'POST',
      },
    )
    const body = (await response.json()) as DecisionActBody

    expect(response.status).toBe(201)
    expect(body.work.state).toBe('accepted')
    expect(body.decision?.receiptRefs).toContain(
      'decision.queue.accept.autopilot_work_order.decision_test_1',
    )
  })

  test('rejects unknown public decision commands at the schema boundary', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'delete-repo' },
        idempotencyKey: 'decision-unknown-command-1',
        method: 'POST',
      },
    )

    expect(response.status).toBe(400)
  })

  test('requires owner approval before sensitive evidence commands apply', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(workOrderRecord())

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.stop/actions',
      {
        body: { action: 'stop' },
        idempotencyKey: 'decision-stop-1',
        method: 'POST',
      },
    )
    const body = (await response.json()) as DecisionActBody & Readonly<{
      authorityBoundary: string
      reason: string
    }>

    expect(response.status).toBe(403)
    expect(body.error).toBe('autopilot_decision_owner_approval_required')
    expect(body.authorityBoundary).toBe('owner_approval_required')
    expect(body.reason).toContain('ownerApprovalRef')
  })

  test('accepts typed evidence commands with owner approval as evidence-only receipts', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(workOrderRecord())

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.stop/actions',
      {
        body: {
          action: 'stop',
          ownerApprovalRef: 'approval.public.autopilot.stop.test',
        },
        idempotencyKey: 'decision-stop-approved-1',
        method: 'POST',
      },
    )
    const body = (await response.json()) as Readonly<{
      command: Readonly<{
        authorityBoundary: string
        directEffectPermitted: false
        ownerApprovalRef: string
        resolution: string
      }>
      directEffectPermitted: false
      receipt: Readonly<{ outcome: string }>
    }>

    expect(response.status).toBe(202)
    expect(body.directEffectPermitted).toBe(false)
    expect(body.command.resolution).toBe('stop')
    expect(body.command.ownerApprovalRef).toBe(
      'approval.public.autopilot.stop.test',
    )
    expect(body.command.authorityBoundary).toBe('evidence_only')
    expect(body.receipt.outcome).toBe('accepted_for_evidence')
  })

  test('replays an identical decision action idempotently', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const first = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'request_changes' },
        idempotencyKey: 'decision-request-changes-1',
        method: 'POST',
      },
    )
    const replay = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'request_changes' },
        idempotencyKey: 'decision-request-changes-1',
        method: 'POST',
      },
    )
    const replayBody = (await replay.json()) as DecisionActBody

    expect(first.status).toBe(201)
    expect(replay.status).toBe(200)
    expect(replayBody.idempotent).toBe(true)
    expect(replayBody.work.state).toBe('revision_required')
  })

  test('rejects conflicting decision actions after a recorded decision', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())
    await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-accept-1',
        method: 'POST',
      },
    )

    const conflicting = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'reject' },
        idempotencyKey: 'decision-reject-1',
        method: 'POST',
      },
    )

    expect(conflicting.status).toBe(409)
  })

  test('rejects acting on non-actionable decision kinds', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(workOrderRecord({ state: 'access_required' }))

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.request_customer_input/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-non-actionable-1',
        method: 'POST',
      },
    )

    expect(response.status).toBe(400)
  })

  test('rejects acting on another owners decision', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(
      deliveredWorkOrderRecord({
        ownerUserId: 'github:someone-else',
      }),
    )

    const response = await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-foreign-1',
        method: 'POST',
      },
    )

    expect(response.status).toBe(404)
  })

  test('rebuilds the queue after a decision transition (rebuild-on-transition)', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())
    await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-accept-1',
        method: 'POST',
      },
    )

    const response = await route(store, '/api/autopilot/decisions')
    const body = (await response.json()) as DecisionListBody

    expect(body.pendingCount).toBe(0)
    expect(body.decisions).toHaveLength(1)
    expect(body.decisions[0]?.decision.status).toBe('completed')
    expect(body.decisions[0]?.decision.receiptRefs).toContain(
      'receipt.review.accept.autopilot_work_order.decision_test_1',
    )
    expect(body.decisions[0]?.closeoutReceipts[0]?.closeoutRef).toBe(
      'decision.closeout.accept.autopilot_work_order.decision_test_1',
    )
  })

  test('returns pending and decided decisions for one work order with closeout receipt rows', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())

    const pending = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.decision_test_1/decisions',
    )
    const pendingBody = (await pending.json()) as DecisionListBody

    expect(pending.status).toBe(200)
    expect(pendingBody.pendingCount).toBe(1)
    expect(pendingBody.decisions[0]?.decision.status).toBe('available')
    expect(pendingBody.decisions[0]?.closeoutReceipts).toEqual([])

    await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-accept-scoped-1',
        method: 'POST',
      },
    )

    const decided = await route(
      store,
      '/api/autopilot/work/autopilot_work_order.decision_test_1/decisions',
    )
    const decidedBody = (await decided.json()) as DecisionListBody

    expect(decided.status).toBe(200)
    expect(decidedBody.pendingCount).toBe(0)
    expect(decidedBody.decisions[0]?.decision.status).toBe('completed')
    expect(decidedBody.decisions[0]?.closeoutReceipts[0]?.outcome).toBe(
      'applied',
    )
  })

  test('dereferences an owner-scoped decision closeout receipt', async () => {
    const store = new MemoryAutopilotWorkStore()

    await store.createWorkOrder(deliveredWorkOrderRecord())
    await route(
      store,
      '/api/autopilot/decisions/decision_action.autopilot_work_order.decision_test_1.approve_pr_draft/actions',
      {
        body: { action: 'accept' },
        idempotencyKey: 'decision-closeout-read-1',
        method: 'POST',
      },
    )

    const response = await route(
      store,
      '/api/autopilot/decision-closeouts/decision.closeout.accept.autopilot_work_order.decision_test_1',
    )
    const body = (await response.json()) as Readonly<{
      directEffectPermitted: false
      receipt: Readonly<{
        closeoutRef: string
        line: string
        workOrderRef: string
      }>
    }>

    expect(response.status).toBe(200)
    expect(body.directEffectPermitted).toBe(false)
    expect(body.receipt.closeoutRef).toBe(
      'decision.closeout.accept.autopilot_work_order.decision_test_1',
    )
    expect(body.receipt.workOrderRef).toBe(
      'autopilot_work_order.decision_test_1',
    )
    expect(body.receipt.line).toContain('closed out as applied')
  })

  test('rejects mutation methods on the decision list route', async () => {
    const store = new MemoryAutopilotWorkStore()
    const response = await route(store, '/api/autopilot/decisions', {
      body: { action: 'accept' },
      method: 'POST',
    })

    expect(response.status).toBe(405)
  })
})
