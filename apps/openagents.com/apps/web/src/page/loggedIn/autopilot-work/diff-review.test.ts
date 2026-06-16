import { describe, expect, test } from 'vitest'

import type {
  AutopilotMissionBriefing,
  AutopilotWorkExecutionCloseout,
  AutopilotWorkProjection,
  AutopilotWorkReviewDecision,
  AutopilotWorkState,
} from '../model'
import {
  projectForgeDiffArtifactDrilldown,
  projectForgeDiffReview,
} from './diff-review'

const reviewDecision = (
  action: AutopilotWorkReviewDecision['action'],
): AutopilotWorkReviewDecision => ({
  acceptedWorkAuthority: action === 'accept',
  action,
  actorAgentCredentialId: 'agent_credential.browser',
  actorAgentUserId: 'agent_user.browser',
  decisionRefs: action === 'accept' ? ['review.browser.accept.work_1'] : [],
  deployAuthority: false,
  forumAutoPublishAllowed: false,
  idempotencyKeyHash: 'idem.review.work_1',
  publicSafe: true,
  recordedAt: '2026-06-16T16:00:00.000Z',
  rejectionRefs: action === 'reject' ? ['review.browser.reject.work_1'] : [],
  revisionRequestRefs:
    action === 'request_changes' ? ['review.browser.revise.work_1'] : [],
  settlementAuthority: false,
  workerPayoutAuthority: false,
})

const closeout = (
  overrides: Partial<AutopilotWorkExecutionCloseout> = {},
): AutopilotWorkExecutionCloseout => ({
  acceptedWorkAuthority: false,
  artifactRefs: ['artifact.public.work_1.diff_summary'],
  assignmentRefs: ['assignment.public.work_1'],
  authorityReceiptRefs: ['authority.public.github_writeback.work_1'],
  blockerRefs: [],
  buildRefs: ['build.public.work_1.not_required'],
  changeCaptureRefs: ['change-capture:pack-c:work_1'],
  changeCaptureStatus: 'review_ready',
  closeoutRefs: ['closeout.public.work_1.summary'],
  deliveryReadinessFreshness: 'fresh',
  deliveryReadinessRefs: ['delivery:pack-c:work_1'],
  deliveryReadinessStatus: 'ready',
  fileCount: 3,
  forumAutoPublishAllowed: false,
  addedLineCount: 42,
  patchDigestRef: 'patch-digest:sha256:work_1',
  previewRefs: ['preview.public.work_1.not_required'],
  proofRefs: ['proof.public.work_1.closeout'],
  publicSafe: true,
  removedLineCount: 7,
  resultRefs: ['result.public.work_1.delivered'],
  reviewCaveatRefs: ['review-caveat:summary-only'],
  runnerKind: 'requester_pylon',
  summaryRefs: ['summary.public.work_1.customer_safe'],
  testRefs: ['test.public.work_1.pass'],
  verificationRefs: ['verification.public.work_1.bun_test'],
  worktreeIdentityStatus: 'ready',
  writebackRequired: true,
  workerPayoutAuthority: false,
  ...overrides,
})

const work = (
  state: AutopilotWorkState,
  executionCloseout: AutopilotWorkExecutionCloseout | null,
  review: AutopilotWorkReviewDecision | null = null,
): AutopilotWorkProjection => ({
  accessRequestRefs: [],
  accessRequirements: [],
  assignmentIntents: [],
  buyerPaymentProofRef: null,
  clientRequestRef: 'client.browser.work_1',
  createdAt: '2026-06-16T15:00:00.000Z',
  eventStreamRef: 'event-stream.public.work_1',
  executionCloseout,
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
  placementDecision: {},
  placementPolicy: {},
  promiseRef: {
    blockerRefs: [],
    promiseId: 'autopilot.mission_briefing.v1',
    registryVersion: '2026-06-15.6',
  },
  pylonAssignmentIntents: [],
  quote: {},
  repositoryAuthorities: [],
  reviewDecision: review,
  state,
  statusUrlRef: 'status.public.work_1',
  taskRefs: ['task.public.work_1'],
  tasks: [],
  updatedAt: '2026-06-16T16:00:00.000Z',
  workOrderRef: 'work_1',
})

const briefing = (
  overrides: Partial<AutopilotMissionBriefing> = {},
): AutopilotMissionBriefing => ({
  briefingRef: 'briefing.public.work_1',
  costs: {},
  decisionsWaiting: {
    callerActionRefs: [],
    nextActionState: 'delivered',
    reasonRefs: [],
    reviewAction: null,
    reviewRecordedAt: null,
  },
  drilldown: [],
  generatedAt: '2026-06-16T16:00:00.000Z',
  kind: 'autopilot_mission_briefing',
  promiseRef: null,
  publicSafe: true,
  state: 'delivered',
  whatChanged: {
    artifactRefs: ['artifact.public.work_1.briefing_diff'],
    resultRefs: ['result.public.work_1.briefing_result'],
    runnerKind: 'requester_pylon',
    summaryRefs: ['summary.public.work_1.briefing'],
  },
  whatHappened: [],
  whatIsBlocked: {
    accessRequirementRefs: [],
    blockerRefs: [],
    placementRefusalReasonRefs: [],
  },
  whatIsRunning: {
    pylonAssignmentIntentRefs: [],
    running: false,
    selectedRunnerKind: 'requester_pylon',
    taskRefs: ['task.public.work_1'],
  },
  workOrderRef: 'work_1',
  ...overrides,
})

describe('Forge diff review projection', () => {
  test('joins closeout, briefing, Pack C refs, and artifact review facts', () => {
    const view = projectForgeDiffReview(
      work('delivered', closeout()),
      briefing(),
    )

    expect(view).toMatchObject({
      addedLineCount: 42,
      authorityReceiptRefs: ['authority.public.github_writeback.work_1'],
      blockerRefs: [],
      changeCaptureRefs: ['change-capture:pack-c:work_1'],
      deliveryReadinessRefs: ['delivery:pack-c:work_1'],
      fileCount: 3,
      patchDigestRef: 'patch-digest:sha256:work_1',
      removedLineCount: 7,
      status: 'review_ready',
      verificationRefs: [
        'verification.public.work_1.bun_test',
        'test.public.work_1.pass',
        'proof.public.work_1.closeout',
      ],
      verificationState: 'present',
    })
    expect(view.artifactRefs).toEqual([
      'artifact.public.work_1.diff_summary',
      'artifact.public.work_1.briefing_diff',
    ])
    expect(view.artifactReview.editedFileCount).toBe(3)
    expect(view.artifactReview.devCheckState).toBe('verification_refs_present')
  })

  test.each([
    ['delivered', null],
    ['accepted', reviewDecision('accept')],
    ['revision_required', reviewDecision('request_changes')],
  ] as const)('renders review evidence for %s runs', (state, decision) => {
    const view = projectForgeDiffReview(work(state, closeout(), decision), null)

    expect(view.status).toBe('review_ready')
    expect(view.blockerRefs).toEqual([])
    expect(view.patchDigestRef).toBe('patch-digest:sha256:work_1')
  })

  test('derives explicit blockers for missing and stale Pack C evidence', () => {
    const view = projectForgeDiffReview(
      work(
        'delivered',
        closeout({
          authorityReceiptRefs: [],
          changeCaptureRefs: [],
          changeCaptureStatus: 'stale',
          deliveryReadinessFreshness: 'stale',
          deliveryReadinessRefs: [],
          patchDigestRef: null,
          proofRefs: [],
          publicSafe: false,
          testRefs: [],
          verificationRefs: [],
          worktreeIdentityStatus: 'stale',
        }),
      ),
      null,
    )

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toEqual(
      expect.arrayContaining([
        'forge-diff-review-blocker:work_1:missing-change-capture',
        'forge-diff-review-blocker:work_1:missing-delivery-readiness',
        'forge-diff-review-blocker:work_1:missing-patch-digest',
        'forge-diff-review-blocker:work_1:missing-verification',
        'forge-diff-review-blocker:work_1:missing-writeback-authority',
        'forge-diff-review-blocker:work_1:unsafe-public-visibility',
        'forge-diff-review-blocker:work_1:stale-change-capture',
        'forge-diff-review-blocker:work_1:stale-worktree-identity',
        'forge-diff-review-blocker:work_1:stale-delivery-readiness',
      ]),
    )
  })

  test('omits raw patches, local paths, and private material before rendering', () => {
    const view = projectForgeDiffReview(
      work(
        'delivered',
        closeout({
          artifactRefs: [
            'artifact.public.work_1.diff_summary',
            'diff --git a/private.ts b/private.ts',
          ],
          patchDigestRef: 'diff --git a/private.ts b/private.ts',
          resultRefs: [
            'result.public.work_1.delivered',
            '/Users/christopherdavid/work/openagents/private.ts',
          ],
        }),
      ),
      null,
    )
    const renderedPayload = JSON.stringify(view)

    expect(view.omittedUnsafeRefCount).toBe(3)
    expect(view.artifactRefs).toEqual(['artifact.public.work_1.diff_summary'])
    expect(view.resultRefs).toEqual(['result.public.work_1.delivered'])
    expect(view.patchDigestRef).toBeNull()
    expect(view.blockerRefs).toEqual(
      expect.arrayContaining([
        'forge-diff-review-blocker:work_1:missing-patch-digest',
        'forge-diff-review-blocker:work_1:unsafe-review-material-omitted',
      ]),
    )
    expect(renderedPayload).not.toContain('diff --git')
    expect(renderedPayload).not.toContain('/Users/christopherdavid')
  })

  test('projects a public-safe diff artifact drilldown from bounded evidence refs', () => {
    const view = projectForgeDiffArtifactDrilldown(
      work('delivered', closeout()),
      briefing({
        drilldown: [
          {
            kind: 'diff_file',
            refs: [
              'diff-file.public.work_1.src_app_ts.modified',
              'diff-file.public.work_1.src_test_ts.added',
            ],
          },
          {
            kind: 'hunk_summary',
            refs: [
              'diff-hunk.public.work_1.src_app_ts.summary_1',
              'diff-hunk.public.work_1.src_test_ts.summary_1',
            ],
          },
          {
            kind: 'diff_summary',
            refs: ['diff-summary.public.work_1.pack_c'],
          },
        ],
      }),
    )

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        deployAuthority: false,
        rawPatchAuthority: false,
        settlementAuthority: false,
        writebackAuthority: false,
      },
      patchDigestRef: 'patch-digest:sha256:work_1',
      publicSafe: true,
      status: 'ready',
      workOrderRef: 'work_1',
    })
    expect(view.fileGroups).toEqual([
      {
        artifactRefs: [
          'artifact.public.work_1.diff_summary',
          'artifact.public.work_1.briefing_diff',
        ],
        fileRefs: ['diff-file.public.work_1.src_app_ts.modified'],
        groupRef: 'forge-diff-artifact-drilldown:work_1:file-1',
        hunkSummaryRefs: [
          'diff-hunk.public.work_1.src_app_ts.summary_1',
          'diff-hunk.public.work_1.src_test_ts.summary_1',
        ],
        summaryRefs: [
          'summary.public.work_1.customer_safe',
          'summary.public.work_1.briefing',
          'diff-summary.public.work_1.pack_c',
        ],
      },
      {
        artifactRefs: [
          'artifact.public.work_1.diff_summary',
          'artifact.public.work_1.briefing_diff',
        ],
        fileRefs: ['diff-file.public.work_1.src_test_ts.added'],
        groupRef: 'forge-diff-artifact-drilldown:work_1:file-2',
        hunkSummaryRefs: [
          'diff-hunk.public.work_1.src_app_ts.summary_1',
          'diff-hunk.public.work_1.src_test_ts.summary_1',
        ],
        summaryRefs: [
          'summary.public.work_1.customer_safe',
          'summary.public.work_1.briefing',
          'diff-summary.public.work_1.pack_c',
        ],
      },
    ])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks the diff artifact drilldown when artifact evidence is missing', () => {
    const view = projectForgeDiffArtifactDrilldown(
      work(
        'delivered',
        closeout({
          artifactRefs: [],
        }),
      ),
      briefing({
        whatChanged: {
          artifactRefs: [],
          resultRefs: ['result.public.work_1.briefing_result'],
          runnerKind: 'requester_pylon',
          summaryRefs: ['summary.public.work_1.briefing'],
        },
      }),
    )

    expect(view.status).toBe('blocked')
    expect(view.fileGroups).toEqual([])
    expect(view.blockerRefs).toEqual(
      expect.arrayContaining([
        'forge-diff-artifact-drilldown-blocker:work_1:missing-artifact-ref',
      ]),
    )
  })

  test('keeps stale diff artifact evidence distinct from ready evidence', () => {
    const view = projectForgeDiffArtifactDrilldown(
      work(
        'delivered',
        closeout({
          changeCaptureStatus: 'stale',
          deliveryReadinessFreshness: 'stale',
          worktreeIdentityStatus: 'stale',
        }),
      ),
      null,
    )

    expect(view.status).toBe('stale')
    expect(view.blockerRefs).toEqual([
      'forge-diff-review-blocker:work_1:stale-change-capture',
      'forge-diff-review-blocker:work_1:stale-worktree-identity',
      'forge-diff-review-blocker:work_1:stale-delivery-readiness',
    ])
  })

  test('omits unsafe private drilldown refs before projection', () => {
    const view = projectForgeDiffArtifactDrilldown(
      work('delivered', closeout()),
      briefing({
        drilldown: [
          {
            kind: 'diff_file',
            refs: [
              'diff-file.public.work_1.safe',
              '/Users/christopher/private.ts',
              'diff --git a/private.ts b/private.ts',
            ],
          },
          {
            kind: 'hunk_summary',
            refs: [
              'diff-hunk.public.work_1.safe',
              'raw patch @@ -1 +1',
            ],
          },
        ],
      }),
    )
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.omittedUnsafeRefCount).toBe(3)
    expect(view.fileGroups[0]?.fileRefs).toEqual([
      'diff-file.public.work_1.safe',
    ])
    expect(view.hunkSummaryRefs).toEqual(['diff-hunk.public.work_1.safe'])
    expect(view.blockerRefs).toContain(
      'forge-diff-artifact-drilldown-blocker:work_1:unsafe-artifact-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('diff --git')
    expect(payload).not.toContain('raw patch')
  })
})
