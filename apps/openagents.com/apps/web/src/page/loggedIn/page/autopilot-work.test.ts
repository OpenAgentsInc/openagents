import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import type {
  AutopilotWorkProjection,
  AutopilotWorkReviewDecision,
  AutopilotWorkState,
  Model,
} from '../model'
import { detailView } from './autopilot-work'

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

const reviewDecision = (
  action: AutopilotWorkReviewDecision['action'],
): AutopilotWorkReviewDecision => ({
  acceptedWorkAuthority: action === 'accept',
  action,
  actorAgentCredentialId: 'agent_credential.browser',
  actorAgentUserId: 'agent_user.browser',
  decisionRefs: action === 'accept' ? ['review.public.accept.work_1'] : [],
  deployAuthority: false,
  forumAutoPublishAllowed: false,
  idempotencyKeyHash: 'idem.review.work_1',
  publicSafe: true,
  recordedAt: '2026-06-16T16:00:00.000Z',
  rejectionRefs: [],
  revisionRequestRefs:
    action === 'request_changes' ? ['review.public.revise.work_1'] : [],
  settlementAuthority: false,
  workerPayoutAuthority: false,
})

const workForState = (
  state: AutopilotWorkState,
  review: AutopilotWorkReviewDecision | null,
): AutopilotWorkProjection =>
  ({
    accessRequestRefs: [],
    accessRequirements: [],
    assignmentIntents: [],
    buyerPaymentProofRef: null,
    clientRequestRef: 'client.public.work_1',
    createdAt: '2026-06-16T15:00:00.000Z',
    eventStreamRef: 'event-stream.public.work_1',
    executionCloseout: {
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
    },
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
    placementDecision: {
      selectedRunnerKind: 'requester_pylon',
    },
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
  }) as AutopilotWorkProjection

const modelForWork = (work: AutopilotWorkProjection): Model =>
  ({
    autopilotWorkBriefing: { _tag: 'AutopilotWorkBriefingIdle' },
    autopilotWorkDetail: {
      _tag: 'AutopilotWorkDetailLoaded',
      response: { work },
    },
    autopilotWorkEvents: { _tag: 'AutopilotWorkEventsIdle' },
    autopilotWorkReview: { _tag: 'AutopilotWorkReviewIdle' },
  }) as Model

describe('autopilot work detail view', () => {
  test.each([
    ['delivered', null],
    ['accepted', reviewDecision('accept')],
    ['revision_required', reviewDecision('request_changes')],
  ] as const)('renders Review changes lane for %s Runs', (state, review) => {
    const rendered = renderHtml(detailView(modelForWork(workForState(state, review))))

    expect(rendered).toContain('Review changes')
    expect(rendered).toContain('change-capture.public.work_1.pack_c')
    expect(rendered).toContain('patch-digest.public.work_1.sha256_abc123')
    expect(rendered).toContain('verification.public.work_1.bun_test')
    expect(rendered).toContain('Accepted-outcome receipt')
  })
})
