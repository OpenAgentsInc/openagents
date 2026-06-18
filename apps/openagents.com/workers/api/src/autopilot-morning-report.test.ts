import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { AutopilotContinuationEventRecord } from './autopilot-continuation-policy'
import { autopilotMorningReportForOwner } from './autopilot-morning-report'
import { makeAutopilotMorningReportRoutes } from './autopilot-morning-report-routes'
import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'
import type { AutopilotWorkOrderRecord } from './autopilot-work-routes'

const nowIso = '2026-06-11T07:00:00.000Z'
const sinceIso = '2026-06-10T19:00:00.000Z'

const workOrder = (
  override: Partial<AutopilotWorkOrderRecord>,
): AutopilotWorkOrderRecord => ({
  accessRequestRefs: [],
  agentCredentialId: 'agent_credential_morning_report',
  agentUserId: 'agent_user_morning_report',
  archivedAt: null,
  buyerPaymentProofRef: null,
  clientRequestRef: 'client.morning.report',
  createdAt: '2026-06-10T21:00:00.000Z',
  eventStreamRef: 'events.morning.report',
  executionCloseout: null,
  id: 'morning_report_order',
  idempotencyKeyHash: 'hash.morning.report',
  ownerUserId: 'github:morning-owner',
  paymentChallengeRef: null,
  request: decodeOpenAgentsAutopilotWorkRequest(
    OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
  ),
  reviewDecision: null,
  scheduledLaunch: null,
  state: 'delivered',
  statusUrlRef: 'status.morning.report',
  taskRefs: ['task.autopilot_coder.docs_contract'],
  updatedAt: '2026-06-11T02:00:00.000Z',
  workOrderRef: 'autopilot_work_order.morning_report',
  ...override,
})

const continuationEvent = (
  override: Partial<AutopilotContinuationEventRecord>,
): AutopilotContinuationEventRecord => ({
  attempt: 1,
  createdAt: '2026-06-11T03:00:00.000Z',
  decision: 'dispatched',
  goalId: 'goal_morning',
  id: 'continuation_event_morning',
  mode: 'goal_continuation',
  reasonRef: 'continuation.policy_dispatch',
  runId: 'run_morning_1',
  userId: 'github:morning-owner',
  ...override,
})

describe('Autopilot morning report projection (M6)', () => {
  test('groups overnight work into decision, blocked, running, and scheduled lanes', () => {
    const report = autopilotMorningReportForOwner({
      continuationEvents: [continuationEvent({})],
      nowIso,
      sinceIso,
      workOrders: [
        workOrder({
          state: 'delivered',
          workOrderRef: 'autopilot_work_order.delivered_overnight',
        }),
        workOrder({
          state: 'blocked',
          workOrderRef: 'autopilot_work_order.blocked_overnight',
        }),
        workOrder({
          state: 'queued_or_running',
          workOrderRef: 'autopilot_work_order.still_running',
        }),
        workOrder({
          state: 'accepted',
          workOrderRef: 'autopilot_work_order.already_reviewed',
        }),
        workOrder({
          scheduledLaunch: {
            dispatchedAt: null,
            expiredAt: null,
            launchAt: '2026-06-12T03:00:00Z',
            windowMinutes: 360,
          },
          state: 'scheduled',
          workOrderRef: 'autopilot_work_order.queued_for_tonight',
        }),
        workOrder({
          scheduledLaunch: {
            dispatchedAt: '2026-06-11T03:00:00.000Z',
            expiredAt: null,
            launchAt: '2026-06-11T03:00:00Z',
            windowMinutes: 360,
          },
          state: 'accepted_free_slice',
          updatedAt: '2026-06-11T03:00:00.000Z',
          workOrderRef: 'autopilot_work_order.launched_overnight',
        }),
        workOrder({
          state: 'delivered',
          updatedAt: '2026-06-09T01:00:00.000Z',
          workOrderRef: 'autopilot_work_order.stale_outside_window',
        }),
      ],
    })

    expect(report.generatedAt).toBe(nowIso)
    expect(report.reportRef).toBe('openagents.autopilot_morning_report.v1')
    expect(report.staleness.composition).toBe('live_at_read')
    expect(report.counts).toEqual({
      awaitingDecision: 1,
      blocked: 1,
      continuations: 1,
      launched: 1,
      reviewed: 1,
      running: 1,
      scheduled: 1,
    })
    expect(
      report.workItems.map(item => `${item.group}:${item.workOrderRef}`),
    ).toEqual([
      'awaiting_decision:autopilot_work_order.delivered_overnight',
      'blocked:autopilot_work_order.blocked_overnight',
      'running:autopilot_work_order.still_running',
      'reviewed:autopilot_work_order.already_reviewed',
      'scheduled:autopilot_work_order.queued_for_tonight',
      'launched:autopilot_work_order.launched_overnight',
    ])
    expect(report.continuations).toEqual([
      {
        attempt: 1,
        decision: 'dispatched',
        mode: 'goal_continuation',
        occurredAt: '2026-06-11T03:00:00.000Z',
        reasonRef: 'continuation.policy_dispatch',
        runId: 'run_morning_1',
      },
    ])
  })

  test('pending scheduled launches stay visible even when older than the window', () => {
    const report = autopilotMorningReportForOwner({
      continuationEvents: [],
      nowIso,
      sinceIso,
      workOrders: [
        workOrder({
          scheduledLaunch: {
            dispatchedAt: null,
            expiredAt: null,
            launchAt: '2026-06-12T03:00:00Z',
            windowMinutes: 360,
          },
          state: 'scheduled',
          updatedAt: '2026-06-08T01:00:00.000Z',
          workOrderRef: 'autopilot_work_order.old_but_pending',
        }),
      ],
    })

    expect(report.workItems).toHaveLength(1)
    expect(report.workItems[0]?.group).toBe('scheduled')
    expect(report.workItems[0]?.scheduledLaunchAt).toBe('2026-06-12T03:00:00Z')
  })
})

describe('Autopilot morning report route (M6)', () => {
  const reportRoute = async (
    options: Readonly<{ sessionUserId?: string; url?: string }> = {},
  ) => {
    const routes = makeAutopilotMorningReportRoutes<Record<string, unknown>>({
      agentStore: () => ({
        createAgentRegistration: () => Promise.resolve(),
        findAgentByTokenHash: () => Promise.resolve(undefined),
        touchAgentCredential: () => Promise.resolve(),
        updateAgentDisplayName: () => Promise.resolve(0),
      }),
      makeContinuationStore: () => ({
        claimContinuationAttempt: async () => ({ claimed: false }),
        countAttemptsForRun: async () => 0,
        countAttemptsForUserSince: async () => 0,
        listEnabledPolicies: async () => [],
        listEventsForUserSince: async () => [continuationEvent({})],
        markContinuationAttemptFailed: async () => undefined,
        readPolicy: async () => undefined,
        upsertPolicy: async record => record,
      }),
      makeWorkStore: () => ({
        createWorkOrder: async record => ({ idempotent: false, record }),
        listPendingScheduledWorkOrders: async () => [],
        listWorkOrdersForOwner: async () => [
          workOrder({ ownerUserId: 'github:morning-owner' }),
        ],
        readWorkOrder: async () => undefined,
        readWorkOrderByIdempotency: async () => undefined,
        recordBuyerPaymentProof: async () => undefined,
        recordExecutionCloseout: async () => undefined,
        recordPylonAssignmentDispatch: async () => undefined,
        recordReviewDecision: async () => undefined,
        recordScheduledLaunchTransition: async () => undefined,
      }),
      nowIso: () => nowIso,
      requireBrowserSession: () =>
        Promise.resolve(
          options.sessionUserId === undefined
            ? undefined
            : { user: { userId: options.sessionUserId } },
        ),
    })
    const request = new Request(
      options.url ?? 'https://openagents.com/api/autopilot/morning-report',
    )

    return Effect.runPromise(
      routes.routeAutopilotMorningReportRequest(
        request,
        {},
        {} as ExecutionContext,
      ),
    )
  }

  test('serves the report to a signed-in owner with generatedAt and staleness', async () => {
    const response = await reportRoute({
      sessionUserId: 'github:morning-owner',
    })
    const json = (await response.json()) as Readonly<{
      report?: Readonly<{
        counts: Readonly<{ awaitingDecision: number; continuations: number }>
        generatedAt: string
        sinceIso: string
        staleness: Readonly<{ composition: string }>
      }>
    }>

    expect(response.status).toBe(200)
    expect(json.report?.generatedAt).toBe(nowIso)
    expect(json.report?.staleness.composition).toBe('live_at_read')
    expect(json.report?.counts.awaitingDecision).toBe(1)
    expect(json.report?.counts.continuations).toBe(1)
    expect(json.report?.sinceIso).toBe('2026-06-10T19:00:00.000Z')
  })

  test('bounds the sinceHours query parameter', async () => {
    const response = await reportRoute({
      sessionUserId: 'github:morning-owner',
      url: 'https://openagents.com/api/autopilot/morning-report?sinceHours=9000',
    })
    const json = (await response.json()) as Readonly<{
      report?: Readonly<{ sinceIso: string }>
    }>

    expect(json.report?.sinceIso).toBe('2026-06-10T19:00:00.000Z')
  })

  test('requires a browser session or agent token', async () => {
    const response = await reportRoute()

    expect(response.status).toBe(401)
  })
})
