import { describe, expect, test } from 'vitest'

import type {
  AutopilotWorkExecutionCloseout,
  AutopilotWorkPlanMutationReceipt,
  AutopilotWorkPlanMutationRequest,
  AutopilotWorkProjection,
  AutopilotWorkState,
} from '../model'
import { projectForgePlanMutationReceipts } from './plan-mutation-receipts'

const closeout = (): AutopilotWorkExecutionCloseout => ({
  acceptedWorkAuthority: false,
  artifactRefs: ['artifact.public.work_1.diff_summary'],
  assignmentRefs: ['assignment.public.work_1'],
  authorityReceiptRefs: ['authority.public.work_1.writeback'],
  blockerRefs: [],
  buildRefs: ['build.public.work_1.not_required'],
  changeCaptureRefs: ['change-capture.public.work_1.pack_c'],
  changeCaptureStatus: 'review_ready',
  closeoutRefs: ['closeout.public.work_1.summary'],
  deliveryReadinessFreshness: 'fresh',
  deliveryReadinessRefs: ['delivery.public.work_1.ready'],
  deliveryReadinessStatus: 'ready',
  fileCount: 2,
  forumAutoPublishAllowed: false,
  addedLineCount: 18,
  patchDigestRef: 'patch-digest.public.work_1.sha256_abc123',
  previewRefs: [],
  proofRefs: ['proof.public.work_1.closeout'],
  publicSafe: true,
  removedLineCount: 4,
  resultRefs: ['result.public.work_1.delivered'],
  reviewCaveatRefs: ['review-caveat.public.work_1.summary_only'],
  runnerKind: 'requester_pylon',
  summaryRefs: ['summary.public.work_1.customer_safe'],
  testRefs: ['test.public.work_1.pass'],
  verificationRefs: ['verification.public.work_1.bun_test'],
  worktreeIdentityStatus: 'ready',
  writebackRequired: true,
  workerPayoutAuthority: false,
})

const work = (
  state: AutopilotWorkState,
  overrides: Partial<AutopilotWorkProjection> = {},
): AutopilotWorkProjection =>
  ({
    accessRequestRefs: [],
    accessRequirements: [],
    assignmentIntents: [],
    buyerPaymentProofRef: null,
    clientRequestRef: 'client.public.work_1',
    createdAt: '2026-06-16T15:00:00.000Z',
    eventStreamRef: 'event-stream.public.work_1',
    executionCloseout:
      state === 'delivered' ||
      state === 'accepted' ||
      state === 'revision_required' ||
      state === 'rejected'
        ? closeout()
        : null,
    fallbackLeaseIntents: [],
    funding: {},
    generatedAt: '2026-06-16T16:00:00.000Z',
    idempotent: false,
    nextAction: {
      callerActionRefs: [],
      reasonRefs: [],
      retryAfterSeconds: null,
      state,
    },
    paymentChallenge: null,
    paymentChallengeRef: null,
    placementDecision: { selectedRunnerKind: 'requester_pylon' },
    placementPolicy: {},
    promiseRef: {
      blockerRefs: [],
      promiseId: 'autopilot.mission_briefing.v1',
      registryVersion: '2026-06-15.6',
    },
    pylonAssignmentIntents: [],
    quote: {},
    repositoryAuthorities: [],
    reviewDecision: null,
    state,
    statusUrlRef: 'status.public.work_1',
    taskRefs: ['task.public.work_1'],
    tasks: [],
    updatedAt: '2026-06-16T16:00:00.000Z',
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkProjection

const request = (
  overrides: Partial<AutopilotWorkPlanMutationRequest> = {},
): AutopilotWorkPlanMutationRequest => ({
  action: 'add',
  actorRef: 'actor.public.operator',
  generatedAt: '2026-06-16T15:30:00.000Z',
  itemRef: 'plan-item.public.work_1.write_tests',
  provenanceRefs: ['event.public.work_1.plan_request'],
  publicSafe: true,
  requestRef: 'plan-request.public.work_1.write_tests',
  ...overrides,
})

const receipt = (
  overrides: Partial<AutopilotWorkPlanMutationReceipt> = {},
): AutopilotWorkPlanMutationReceipt => ({
  action: 'update',
  actorRef: 'actor.public.runtime',
  blockerRefs: [],
  generatedAt: '2026-06-16T15:35:00.000Z',
  itemRef: 'plan-item.public.work_1.write_tests',
  provenanceRefs: ['event.public.work_1.plan_applied'],
  publicSafe: true,
  receiptRef: 'plan-receipt.public.work_1.write_tests',
  requestRef: 'plan-request.public.work_1.write_tests',
  state: 'applied',
  ...overrides,
})

describe('Forge plan mutation receipts projection', () => {
  test('projects applied mutation receipts with explicit non-authority flags', () => {
    const view = projectForgePlanMutationReceipts(
      work('queued_or_running', {
        planMutationReceipts: [receipt()],
      }),
    )

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        deploymentAuthority: false,
        runCompletionAuthority: false,
        settlementAuthority: false,
        workerPayoutAuthority: false,
      },
      omittedUnsafeRefCount: 0,
      publicSafe: true,
      status: 'applied',
      workOrderRef: 'work_1',
    })
    expect(view.items).toEqual([
      {
        action: 'update',
        actorRef: 'actor.public.runtime',
        authority: view.authority,
        blockerRefs: [],
        generatedAt: '2026-06-16T15:35:00.000Z',
        itemRef: 'plan-item.public.work_1.write_tests',
        provenanceRefs: ['event.public.work_1.plan_applied'],
        receiptRef: 'plan-receipt.public.work_1.write_tests',
        requestRef: 'plan-request.public.work_1.write_tests',
        state: 'applied',
      },
    ])
  })

  test('projects requested mutations before runtime receipts arrive', () => {
    const view = projectForgePlanMutationReceipts(
      work('queued_or_running', {
        planMutationRequests: [request()],
      }),
    )

    expect(view.status).toBe('requested')
    expect(view.items[0]).toMatchObject({
      action: 'add',
      actorRef: 'actor.public.operator',
      receiptRef: null,
      requestRef: 'plan-request.public.work_1.write_tests',
      state: 'requested',
    })
  })

  test('projects blocked and stale mutation receipts distinctly', () => {
    const blocked = projectForgePlanMutationReceipts(
      work('queued_or_running', {
        planMutationReceipts: [
          receipt({
            blockerRefs: ['plan-blocker.public.needs_runtime_authority'],
            state: 'blocked',
          }),
        ],
      }),
    )
    const stale = projectForgePlanMutationReceipts(
      work('queued_or_running', {
        planMutationReceipts: [
          receipt({
            receiptRef: 'plan-receipt.public.work_1.stale',
            state: 'stale',
          }),
        ],
      }),
    )

    expect(blocked.status).toBe('blocked')
    expect(blocked.blockerRefs).toContain(
      'plan-blocker.public.needs_runtime_authority',
    )
    expect(stale.status).toBe('stale')
  })

  test('blocks completed-plan receipts from implying Run completion without closeout evidence', () => {
    const view = projectForgePlanMutationReceipts(
      work('delivered', {
        executionCloseout: null,
        planMutationReceipts: [
          receipt({
            action: 'complete',
            itemRef: 'plan-item.public.work_1.finish',
            receiptRef: 'plan-receipt.public.work_1.finish',
            state: 'applied',
          }),
        ],
      }),
    )

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-plan-mutation-blocker:work_1:plan-complete-without-closeout-evidence',
    )
    expect(view.items[0]?.authority.runCompletionAuthority).toBe(false)
  })

  test('omits unsafe private request and receipt refs before projection', () => {
    const view = projectForgePlanMutationReceipts(
      work('queued_or_running', {
        planMutationReceipts: [
          receipt({
            actorRef: 'provider payload sk-private',
            blockerRefs: [
              'plan-blocker.public.safe',
              '/Users/christopher/private-plan.md',
            ],
            itemRef: 'plan-item.public.safe',
            provenanceRefs: [
              'event.public.safe',
              'raw prompt /Users/christopher/private.md',
            ],
          }),
          receipt({
            itemRef: 'plan-item.public.safe',
            receiptRef: 'plan-receipt.public.safe',
          }),
        ],
        planMutationRequests: [
          request({
            itemRef: 'raw todo /Users/christopher/private.md',
          }),
        ],
      }),
    )
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.omittedUnsafeRefCount).toBe(4)
    expect(view.items).toHaveLength(1)
    expect(view.items[0]?.receiptRef).toBe('plan-receipt.public.safe')
    expect(view.blockerRefs).toContain(
      'forge-plan-mutation-blocker:work_1:unsafe-plan-mutation-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw todo')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
  })
})
