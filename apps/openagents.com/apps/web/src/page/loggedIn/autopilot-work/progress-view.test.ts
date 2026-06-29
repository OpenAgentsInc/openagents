import { describe, expect, test } from 'vitest'

import type {
  AutopilotWorkEvent,
  AutopilotWorkProjection,
  AutopilotWorkState,
} from '../model'
import { projectForgeRunProgress } from './progress-view'

const closeout = () => ({
  acceptedWorkAuthority: false,
  artifactRefs: ['artifact.public.work_1.diff_summary'],
  assignmentRefs: ['assignment.public.work_1'],
  authorityReceiptRefs: ['authority.public.work_1.writeback'],
  blockerRefs: [],
  buildRefs: ['build.public.work_1.not_required'],
  changeCaptureRefs: ['change-capture.public.work_1.pack_c'],
  changeCaptureStatus: 'review_ready' as const,
  closeoutRefs: ['closeout.public.work_1.summary'],
  deliveryReadinessFreshness: 'fresh' as const,
  deliveryReadinessRefs: ['delivery.public.work_1.ready'],
  deliveryReadinessStatus: 'ready' as const,
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
  worktreeIdentityStatus: 'ready' as const,
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
      callerActionRefs:
        state === 'delivered' ? ['next-action.public.review.work_1'] : [],
      reasonRefs: state === 'blocked' ? ['reason.public.blocked.work_1'] : [],
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

const event = (
  sequence: number,
  eventKind: AutopilotWorkEvent['eventKind'],
  state: AutopilotWorkState,
  overrides: Partial<AutopilotWorkEvent> = {},
): AutopilotWorkEvent =>
  ({
    eventKind,
    eventRef: `event.public.work_1.${sequence}`,
    occurredAt: `2026-06-16T15:0${sequence}:00.000Z`,
    publicSafe: true,
    sequence,
    state,
    taskRefs: ['task.public.work_1'],
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkEvent

describe('Forge Run progress projection', () => {
  test('projects delivered progress from lifecycle, next action, and closeout refs', () => {
    const view = projectForgeRunProgress(work('delivered'), [
      event(1, 'queued', 'queued_or_running'),
      event(2, 'delivered', 'delivered'),
    ])

    expect(view.status).toBe('delivered')
    expect(view.blockerRefs).toEqual([])
    expect(view.items.map(item => item.kind)).toEqual([
      'requested',
      'queued',
      'delivered',
      'closeout',
      'next_action',
    ])
    expect(view.items.find(item => item.kind === 'closeout')?.refs).toEqual([
      'closeout.public.work_1.summary',
      'proof.public.work_1.closeout',
      'result.public.work_1.delivered',
    ])
  })

  test.each([
    ['accepted', 'accepted', 'reviewed'],
    ['revision_required', 'revision_required', 'blocked'],
    ['rejected', 'rejected', 'failed'],
  ] as const)('distinguishes %s terminal progress', (state, eventKind, status) => {
    const view = projectForgeRunProgress(work(state), [
      event(1, 'queued', 'queued_or_running'),
      event(2, eventKind, state),
    ])

    expect(view.status).toBe(status)
    expect(view.items.map(item => item.kind)).toContain(eventKind)
  })

  test('projects queued and running work as active when lifecycle evidence exists', () => {
    const view = projectForgeRunProgress(work('queued_or_running'), [
      event(1, 'queued', 'queued_or_running'),
      event(2, 'running', 'queued_or_running'),
    ])

    expect(view.status).toBe('running')
    expect(view.blockerRefs).toEqual([])
    expect(view.items.find(item => item.kind === 'running')?.status).toBe(
      'active',
    )
  })

  test('projects blocked work with reason refs', () => {
    const view = projectForgeRunProgress(work('blocked'), [
      event(1, 'queued', 'queued_or_running'),
      event(2, 'blocked', 'blocked'),
    ])

    expect(view.status).toBe('blocked')
    expect(view.items.find(item => item.kind === 'blocked')?.status).toBe(
      'blocked',
    )
    expect(view.items.find(item => item.kind === 'next_action')?.refs).toEqual([
      'reason.public.blocked.work_1',
    ])
  })

  test('projects invalid state as failed when evidence exists', () => {
    const view = projectForgeRunProgress(work('invalid'), [
      event(1, 'queued', 'queued_or_running'),
      event(2, 'blocked', 'invalid'),
    ])

    expect(view.status).toBe('failed')
    expect(view.items.find(item => item.kind === 'failed')?.status).toBe(
      'failed',
    )
  })

  test('adds blockers for missing lifecycle and closeout evidence', () => {
    const view = projectForgeRunProgress(
      work('delivered', { executionCloseout: null }),
      null,
    )

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toEqual(
      expect.arrayContaining([
        'forge-run-progress-blocker:work_1:missing-lifecycle-events',
        'forge-run-progress-blocker:work_1:missing-closeout-evidence',
      ]),
    )
    expect(view.items.map(item => item.kind)).toContain('delivered')
  })

  test.each([
    ['scheduled', 'pending', 'pending'],
    ['queued_or_running', 'running', 'active'],
    ['blocked', 'blocked', 'blocked'],
    ['invalid', 'failed', 'failed'],
  ] as const)(
    'does not treat %s Runs as complete when stale closeout refs are present',
    (state, expectedStatus, expectedItemStatus) => {
      const view = projectForgeRunProgress(
        work(state, { executionCloseout: closeout() }),
        [
          event(
            1,
            state === 'scheduled'
              ? 'scheduled'
              : state === 'blocked' || state === 'invalid'
                ? 'blocked'
                : 'running',
            state,
          ),
        ],
      )

      expect(view.status).toBe(expectedStatus)
      expect(view.items.map(item => item.kind)).not.toContain('closeout')
      expect(
        view.items
          .filter(item => item.kind !== 'requested')
          .some(item => item.status === 'completed'),
      ).toBe(false)
      expect(view.items.at(-1)?.status).toBe(expectedItemStatus)
    },
  )

  test.each([
    ['revision_required', 'blocked', 'blocked'],
    ['rejected', 'failed', 'failed'],
  ] as const)(
    'keeps %s Runs non-completed even when closeout evidence exists',
    (state, expectedStatus, expectedTerminalStatus) => {
      const eventKind = state === 'revision_required' ? 'revision_required' : 'rejected'
      const view = projectForgeRunProgress(work(state), [
        event(1, 'queued', 'queued_or_running'),
        event(2, eventKind, state),
      ])

      expect(view.status).toBe(expectedStatus)
      expect(view.items.map(item => item.kind)).toContain('closeout')
      expect(view.items.find(item => item.kind === eventKind)?.status).toBe(
        expectedTerminalStatus,
      )
      expect(view.items.find(item => item.kind === 'next_action')?.status).toBe(
        expectedTerminalStatus,
      )
    },
  )

  test('omits unsafe progress refs before projection', () => {
    const view = projectForgeRunProgress(
      work('delivered', {
        executionCloseout: {
          ...closeout(),
          closeoutRefs: ['diff --git a/private.ts b/private.ts'],
        },
        taskRefs: [
          'task.public.work_1',
          '/Users/christopherdavid/work/openagents/private.ts',
        ],
      }),
      [
        event(1, 'queued', 'queued_or_running', {
          eventRef: 'raw shell log /Users/christopher/private.log',
        }),
        event(2, 'delivered', 'delivered'),
      ],
    )
    const renderedPayload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.omittedUnsafeRefCount).toBeGreaterThanOrEqual(3)
    expect(view.blockerRefs).toContain(
      'forge-run-progress-blocker:work_1:unsafe-progress-material-omitted',
    )
    expect(renderedPayload).not.toContain('diff --git')
    expect(renderedPayload).not.toContain('/Users/christopherdavid')
    expect(renderedPayload).not.toContain('raw shell log')
  })
})
