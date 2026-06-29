import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import type {
  AutopilotMissionBriefing,
  AutopilotWorkEvent,
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
    ...overrides,
  }) as AutopilotWorkProjection

const workEvent = (
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

const briefingForWork = (
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

const modelForWork = (
  work: AutopilotWorkProjection,
  events?: ReadonlyArray<AutopilotWorkEvent>,
  briefing?: AutopilotMissionBriefing,
): Model =>
  ({
    autopilotWorkBriefing:
      briefing === undefined
        ? { _tag: 'AutopilotWorkBriefingIdle' }
        : {
            _tag: 'AutopilotWorkBriefingLoaded',
            response: { briefing },
          },
    autopilotWorkDetail: {
      _tag: 'AutopilotWorkDetailLoaded',
      response: { work },
    },
    autopilotWorkEvents:
      events === undefined
        ? { _tag: 'AutopilotWorkEventsIdle' }
        : {
            _tag: 'AutopilotWorkEventsLoaded',
            response: {
              events,
              generatedAt: '2026-06-16T16:00:00.000Z',
              nextAfter: events.at(-1)?.sequence ?? 0,
              workOrderRef: work.workOrderRef,
            },
          },
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
    expect(rendered).toContain('Diff artifact drilldown')
    expect(rendered).toContain('Open diff artifact drilldown')
    expect(rendered).toContain('change-capture.public.work_1.pack_c')
    expect(rendered).toContain('patch-digest.public.work_1.sha256_abc123')
    expect(rendered).toContain('verification.public.work_1.bun_test')
    expect(rendered).toContain('Accepted-outcome receipt')
  })

  test('renders public-safe diff artifact drilldown refs from mission briefing', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('delivered', null),
          undefined,
          briefingForWork({
            drilldown: [
              {
                kind: 'diff_file',
                refs: ['diff-file.public.work_1.src_app_ts.modified'],
              },
              {
                kind: 'hunk_summary',
                refs: ['diff-hunk.public.work_1.src_app_ts.summary_1'],
              },
              {
                kind: 'diff_summary',
                refs: ['diff-summary.public.work_1.pack_c'],
              },
            ],
          }),
        ),
      ),
    )

    expect(rendered).toContain('Diff artifact drilldown')
    expect(rendered).toContain('#diff-artifact-drilldown-work_1')
    expect(rendered).toContain('diff-file.public.work_1.src_app_ts.modified')
    expect(rendered).toContain('diff-hunk.public.work_1.src_app_ts.summary_1')
    expect(rendered).toContain('diff-summary.public.work_1.pack_c')
    expect(rendered).toContain(
      'forge-diff-artifact-drilldown:work_1:file-1',
    )
    expect(rendered).toContain('Bounded artifact evidence only')
    expect(rendered).not.toContain('diff --git')
  })

  test('omits unsafe diff artifact drilldown refs before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('delivered', null),
          undefined,
          briefingForWork({
            drilldown: [
              {
                kind: 'diff_file',
                refs: [
                  'diff-file.public.work_1.safe',
                  '/Users/christopher/private.ts',
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
        ),
      ),
    )

    expect(rendered).toContain('Diff artifact drilldown')
    expect(rendered).toContain('diff-file.public.work_1.safe')
    expect(rendered).toContain('diff-hunk.public.work_1.safe')
    expect(rendered).toContain('unsafe-artifact-material-omitted')
    expect(rendered).toContain('unsafe diff artifact ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('@@ -1 +1')
  })

  test('renders Run progress lane for delivered Runs', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(workForState('delivered', null), [
          workEvent(1, 'queued', 'queued_or_running'),
          workEvent(2, 'delivered', 'delivered'),
        ]),
      ),
    )

    expect(rendered).toContain('Run progress')
    expect(rendered).toContain('Progress for work_1')
    expect(rendered).toContain('Delivered')
    expect(rendered).toContain('Closeout evidence')
    expect(rendered).toContain('closeout.public.work_1.summary')
    expect(rendered).toContain('Plan mutation receipts')
  })

  test('renders Error recovery lane for explicit typed recovery evidence', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            errorRecovery: {
              errors: [
                {
                  category: 'ProviderRateLimited',
                  diagnosticRef: 'diagnostic.public.work_1.rate_limit',
                  errorRef: 'error.public.work_1.provider_rate_limit',
                  occurredAt: '2026-06-16T15:30:00.000Z',
                  originServiceRef: 'adapter.openai.public',
                  publicMessage: 'Provider retry is scheduled.',
                  recoveryStrategy: 'backoff_retry',
                  redactionClass: 'public',
                  relatedRefs: [
                    'task.public.work_1',
                    'idempotency.public.work_1.provider_retry',
                  ],
                  retryability: 'retryable',
                  severity: 'warning',
                },
              ],
              events: [
                {
                  errorRef: 'error.public.work_1.provider_rate_limit',
                  eventRef: 'recovery-event.public.work_1.retry_scheduled',
                  kind: 'recovery.retry_scheduled',
                  occurredAt: '2026-06-16T15:31:00.000Z',
                  publicSafe: true,
                  receiptRefs: ['receipt.public.retry.work_1'],
                  recoveryStrategy: 'backoff_retry',
                },
              ],
              recoveryRef: 'error-recovery.public.work_1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Error recovery')
    expect(rendered).toContain('recovering')
    expect(rendered).toContain('ProviderRateLimited')
    expect(rendered).toContain('retryable')
    expect(rendered).toContain('backoff retry')
    expect(rendered).toContain('error.public.work_1.provider_rate_limit')
    expect(rendered).toContain('recovery.retry_scheduled')
    expect(rendered).toContain('receipt.public.retry.work_1')
  })

  test('omits unsafe Error recovery diagnostics before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            errorRecovery: {
              errors: [
                {
                  category: 'ToolExecutionFailed',
                  causeRef: 'raw stack trace at /Users/christopher/app.ts:1:1',
                  diagnosticRef: '/Users/christopher/.codex/private.jsonl',
                  errorRef: 'error.public.work_1.tool',
                  originServiceRef: 'raw shell command $(cat ~/.ssh/id_rsa)',
                  publicMessage: 'provider payload sk-private',
                  relatedRefs: [
                    'task.public.work_1',
                    'raw prompt /Users/christopher/private.md',
                  ],
                  recoveryStrategy: 'structured_tool_error',
                  retryability: 'conditional',
                },
              ],
              events: [
                {
                  blockerRefs: [
                    'recovery-blocker.public.safe',
                    'shell log /Users/christopher/run.log',
                  ],
                  errorRef: 'error.public.work_1.tool',
                  eventRef: 'recovery-event.public.work_1.structured_error',
                  kind: 'error.recorded',
                  occurredAt: '2026-06-16T15:31:00.000Z',
                  publicSafe: true,
                  receiptRefs: [
                    'receipt.public.work_1.structured_error',
                    'provider payload sk-private',
                  ],
                },
              ],
              recoveryRef: 'error-recovery.public.work_1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Error recovery')
    expect(rendered).toContain('error.public.work_1.tool')
    expect(rendered).toContain('receipt.public.work_1.structured_error')
    expect(rendered).toContain('unsafe-error-recovery-material-omitted')
    expect(rendered).toContain('unsafe error recovery ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw shell')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('sk-private')
    expect(rendered).not.toContain('stack trace')
  })

  test('renders derived fail-closed recovery state for invalid Runs', () => {
    const rendered = renderHtml(detailView(modelForWork(workForState('invalid', null))))

    expect(rendered).toContain('Error recovery')
    expect(rendered).toContain('failed closed')
    expect(rendered).toContain('InternalBug')
    expect(rendered).toContain('run.failed_closed')
    expect(rendered).toContain('missing-error-recovery-evidence')
    expect(rendered).toContain('terminal-fail-closed')
  })

  test('renders Compaction lane for explicit compaction boundaries', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            compaction: {
              boundaries: [
                {
                  automaticFailureCount: 0,
                  boundaryRef: 'compaction-boundary.public.work_1.1',
                  generatedAt: '2026-06-17T18:00:00.000Z',
                  policyRefs: ['policy.public.compaction.v1'],
                  postEstimate: {
                    estimateRef: 'context-estimate.public.work_1.post',
                    messageCount: 44,
                    tokenCount: 24000,
                  },
                  preEstimate: {
                    contextWindow: 128000,
                    estimateRef: 'context-estimate.public.work_1.pre',
                    messageCount: 120,
                    tokenCount: 112000,
                  },
                  preservedPlanRefs: ['plan.public.work_1.active'],
                  preservedRecentMessageRefs: ['message.public.work_1.recent_1'],
                  preservedTaskRefs: ['task.public.work_1'],
                  preservedToolPairs: [
                    {
                      requestRef: 'tool-request.public.work_1.read_file',
                      resultRef: 'tool-result.public.work_1.read_file',
                    },
                  ],
                  publicMessage: 'Older context was summarized.',
                  publicSafe: true,
                  restoredAdapterRefs: ['adapter.public.codex.ready'],
                  restoredFileRefs: ['file.public.work_1.app_ts'],
                  state: 'compacted',
                  strategy: 'summary_compact',
                  summarySourceRefs: ['summary-source.public.work_1.boundary_1'],
                  trigger: 'automatic',
                },
              ],
              compactionRef: 'compaction.public.work_1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Compaction')
    expect(rendered).toContain('compacted')
    expect(rendered).toContain('automatic / summary compact')
    expect(rendered).toContain('compaction-boundary.public.work_1.1')
    expect(rendered).toContain('summary-source.public.work_1.boundary_1')
    expect(rendered).toContain('tool-request.public.work_1.read_file')
    expect(rendered).toContain('tool-result.public.work_1.read_file')
    expect(rendered).toContain('file.public.work_1.app_ts')
  })

  test('omits unsafe Compaction boundary material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            compaction: {
              boundaries: [
                {
                  boundaryRef: 'compaction-boundary.public.safe',
                  failureRefs: ['raw transcript /Users/christopher/private.jsonl'],
                  generatedAt: '2026-06-17T18:00:00.000Z',
                  hookRefs: [
                    'hook.public.safe',
                    'raw shell log /Users/christopher/run.log',
                  ],
                  policyRefs: ['provider payload sk-private'],
                  preservedRecentMessageRefs: [
                    'message.public.safe',
                    'raw prompt /Users/christopher/private.md',
                  ],
                  preservedToolPairs: [
                    {
                      requestRef: 'tool-request.public.safe',
                      resultRef: 'tool-result.public.safe',
                      summaryRef: 'raw dropped content /Users/christopher/tool.txt',
                    },
                  ],
                  publicMessage: 'provider payload sk-private',
                  publicSafe: true,
                  restoredFileRefs: ['file.public.safe', '/Users/christopher/app.ts'],
                  state: 'compacted',
                  strategy: 'summary_compact',
                  summarySourceRefs: [
                    'summary-source.public.safe',
                    'raw summary /Users/christopher/summary.md',
                  ],
                  trigger: 'automatic',
                },
              ],
              compactionRef: 'compaction.public.work_1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Compaction')
    expect(rendered).toContain('compaction-boundary.public.safe')
    expect(rendered).toContain('message.public.safe')
    expect(rendered).toContain('file.public.safe')
    expect(rendered).toContain('summary-source.public.safe')
    expect(rendered).toContain('unsafe-compaction-material-omitted')
    expect(rendered).toContain('unsafe compaction ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw shell')
    expect(rendered).not.toContain('raw transcript')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Usage and budget lane for exact usage evidence', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            usageBudget: {
              budgetThresholds: [
                {
                  action: 'warn',
                  budgetRef: 'budget.public.work_1.run_tokens',
                  limitTokens: 100000,
                  policyRefs: ['policy.public.usage.warn_80'],
                  state: 'within',
                },
              ],
              contextEstimateRef: 'context-estimate.public.work_1.latest',
              costEstimate: {
                costRef: 'cost.public.work_1.estimate',
                currency: 'USD',
                estimatedCostCents: 42,
                pricingRef: 'pricing.public.provider.model',
                pricingState: 'known',
              },
              modelRef: 'model.public.gpt_5',
              providerRef: 'provider.public.openai',
              tokenCounts: {
                cacheReadTokens: 10000,
                cacheWriteTokens: 2000,
                contextWindowTokens: 48000,
                inputTokens: 30000,
                outputTokens: 4000,
                totalTokens: 46000,
              },
              usageRef: 'usage.public.work_1.latest',
              usageTruth: 'exact',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Usage and budget')
    expect(rendered).toContain('within')
    expect(rendered).toContain('usage.public.work_1.latest')
    expect(rendered).toContain('context-estimate.public.work_1.latest')
    expect(rendered).toContain('provider.public.openai')
    expect(rendered).toContain('model.public.gpt_5')
    expect(rendered).toContain('cost.public.work_1.estimate')
    expect(rendered).toContain('pricing.public.provider.model')
    expect(rendered).toContain('budget.public.work_1.run_tokens')
    expect(rendered).toContain('policy.public.usage.warn_80')
  })

  test('omits unsafe Usage and budget material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            usageBudget: {
              blockerRefs: [
                'usage-blocker.public.safe',
                'raw prompt /Users/christopher/a.md',
              ],
              budgetThresholds: [
                {
                  action: 'warn',
                  budgetRef: 'budget.public.safe',
                  policyRefs: ['policy.public.safe', 'provider payload sk-private'],
                  state: 'within',
                },
              ],
              contextEstimateRef: '/Users/christopher/context.json',
              costEstimate: {
                costRef: 'cost.public.safe',
                currency: 'USD',
                estimatedCostCents: 12,
                pricingRef: 'raw provider payload sk-private',
                pricingState: 'known',
              },
              modelRef: 'model.public.safe',
              providerRef: 'provider payload sk-private',
              quotaBlockerRefs: [
                'quota.public.safe',
                'shell log /Users/christopher/run.log',
              ],
              rateLimitRefs: [
                'rate-limit.public.safe',
                'raw usage /Users/christopher/u.json',
              ],
              tokenCounts: {
                inputTokens: 1000,
                outputTokens: 500,
                totalTokens: 1500,
              },
              usageRef: 'usage.public.safe',
              usageTruth: 'estimated',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Usage and budget')
    expect(rendered).toContain('usage.public.safe')
    expect(rendered).toContain('cost.public.safe')
    expect(rendered).toContain('budget.public.safe')
    expect(rendered).toContain('rate-limit.public.safe')
    expect(rendered).toContain('quota.public.safe')
    expect(rendered).toContain('unsafe-usage-budget-material-omitted')
    expect(rendered).toContain('unsafe usage/budget ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw usage')
    expect(rendered).not.toContain('shell log')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Model provider lane for selected provider evidence', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            modelProvider: {
              capabilities: {
                contextWindowTokens: 128000,
                maxOutputTokens: 16000,
                structuredOutputSupport: true,
                toolCallSupport: true,
              },
              capabilityRefs: ['capability.public.model.gpt_5'],
              entitlementRefs: ['entitlement.public.team.provider_openai'],
              modelRef: 'model.public.gpt_5',
              policyRefs: ['policy.public.model.default'],
              pricingRefs: ['pricing.public.gpt_5'],
              privacyRefs: ['privacy.public.provider.openai'],
              providerFacingModelRef: 'provider-model.public.gpt_5',
              providerRef: 'provider.public.openai',
              requestedAliasRef: 'model-alias.public.best',
              resolutionRef: 'model-resolution.public.work_1',
              resolutionSource: 'settings',
              state: 'selected',
              telemetryRefs: ['telemetry-policy.public.aggregate'],
              validationRefs: ['validation.public.model.gpt_5'],
              validationState: 'passed',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Model provider')
    expect(rendered).toContain('selected')
    expect(rendered).toContain('model-resolution.public.work_1')
    expect(rendered).toContain('model-alias.public.best')
    expect(rendered).toContain('provider.public.openai')
    expect(rendered).toContain('model.public.gpt_5')
    expect(rendered).toContain('provider-model.public.gpt_5')
    expect(rendered).toContain('capability.public.model.gpt_5')
    expect(rendered).toContain('entitlement.public.team.provider_openai')
    expect(rendered).toContain('validation.public.model.gpt_5')
  })

  test('omits unsafe Model provider material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            modelProvider: {
              blockerRefs: [
                'provider-blocker.public.safe',
                'raw request /Users/christopher/request.json',
              ],
              capabilityRefs: [
                'capability.public.safe',
                'provider payload sk-private',
              ],
              entitlementRefs: ['entitlement.public.safe'],
              modelRef: 'model.public.safe',
              policyRefs: ['policy.public.safe', 'internal codename private-model'],
              providerFacingModelRef: 'private deployment /Users/christopher/model',
              providerRef: 'provider.public.safe',
              requestedAliasRef: 'model-alias.public.safe',
              resolutionRef: 'model-resolution.public.safe',
              state: 'selected',
              validationRefs: ['validation.public.safe', 'sdk payload bearer token'],
              validationState: 'passed',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Model provider')
    expect(rendered).toContain('model-resolution.public.safe')
    expect(rendered).toContain('provider.public.safe')
    expect(rendered).toContain('model.public.safe')
    expect(rendered).toContain('capability.public.safe')
    expect(rendered).toContain('unsafe-model-provider-material-omitted')
    expect(rendered).toContain('unsafe model provider ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw request')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('private deployment')
    expect(rendered).not.toContain('internal codename')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Instruction layering lane for public instruction snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            instructionLayering: {
              layers: [
                {
                  freshness: 'fresh',
                  kind: 'runtime_policy',
                  layerRef: 'instruction-layer.public.runtime_policy',
                  policyRefs: ['policy.public.runtime_safety'],
                  precedence: 1,
                  redactionClass: 'public',
                  sourceRefs: ['source.public.runtime_policy'],
                  state: 'applied',
                  tokenEstimate: 1200,
                },
                {
                  freshness: 'fresh',
                  kind: 'workspace_instruction',
                  layerRef: 'instruction-layer.public.workspace',
                  metadataRefs: ['metadata.public.workspace_frontmatter'],
                  policyRefs: ['policy.public.workspace'],
                  precedence: 8,
                  redactionClass: 'public',
                  sourceRefs: ['source.public.AGENTS.md'],
                  state: 'applied',
                  tokenEstimate: 900,
                },
              ],
              projectionRef: 'instruction-projection.public.work_1.provider',
              snapshotRef: 'instruction-snapshot.public.work_1',
              versionRef: 'instruction-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Instruction layering')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('instruction-snapshot.public.work_1')
    expect(rendered).toContain('instruction-projection.public.work_1.provider')
    expect(rendered).toContain('instruction-layer.public.runtime_policy')
    expect(rendered).toContain('instruction-layer.public.workspace')
    expect(rendered).toContain('source.public.AGENTS.md')
    expect(rendered).toContain('metadata.public.workspace_frontmatter')
    expect(rendered).toContain('policy.public.runtime_safety')
  })

  test('omits unsafe Instruction layering material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            instructionLayering: {
              blockerRefs: [
                'instruction-blocker.public.safe',
                'prompt body /Users/christopher/private.md',
              ],
              layers: [
                {
                  allowedToolRefs: [
                    'tool.public.safe',
                    'raw instruction /Users/christopher/tool.md',
                  ],
                  capabilityDeltaRefs: ['capability.public.safe'],
                  kind: 'local_private_instruction',
                  layerRef: 'instruction-layer.public.local_private',
                  metadataRefs: [
                    'metadata.public.safe',
                    'raw memory /Users/christopher/memory.md',
                  ],
                  policyRefs: ['policy.public.safe', 'provider prompt sk-private'],
                  precedence: 9,
                  redactionClass: 'local_only',
                  sourceRefs: [
                    'source.public.safe',
                    'private instruction /Users/christopher/AGENTS.md',
                  ],
                  state: 'applied',
                },
              ],
              projectionRef: 'instruction-projection.public.safe',
              snapshotRef: 'instruction-snapshot.public.safe',
              versionRef: 'instruction-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Instruction layering')
    expect(rendered).toContain('instruction-snapshot.public.safe')
    expect(rendered).toContain('instruction-layer.public.local_private')
    expect(rendered).toContain('metadata.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('tool.public.safe')
    expect(rendered).toContain('unsafe-instruction-material-omitted')
    expect(rendered).toContain('unsafe instruction ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('prompt body')
    expect(rendered).not.toContain('raw instruction')
    expect(rendered).not.toContain('raw memory')
    expect(rendered).not.toContain('provider prompt')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Session memory lane for public memory snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            sessionMemory: {
              entries: [
                {
                  entryRef: 'memory-entry.public.repository.note',
                  freshness: 'fresh',
                  kind: 'repository_note',
                  lifecycleState: 'active',
                  policyRefs: ['policy.public.memory.project_retention'],
                  redactionClass: 'public',
                  retentionClass: 'project',
                  retrievalRefs: ['retrieval.public.repository.note'],
                  scope: 'repository',
                  sourceRefs: ['event.public.memory.repository_note'],
                  summaryRefs: ['summary.public.memory.repository_note'],
                },
                {
                  entryRef: 'memory-entry.public.run.progress',
                  freshness: 'fresh',
                  kind: 'progress_note',
                  lifecycleState: 'active',
                  redactionClass: 'public',
                  retentionClass: 'ephemeral',
                  scope: 'run',
                  sourceRefs: ['event.public.memory.progress'],
                },
              ],
              projectionRef: 'session-memory-projection.public.work_1',
              snapshotRef: 'session-memory-snapshot.public.work_1',
              versionRef: 'session-memory-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Session memory')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('session-memory-snapshot.public.work_1')
    expect(rendered).toContain('session-memory-projection.public.work_1')
    expect(rendered).toContain('memory-entry.public.repository.note')
    expect(rendered).toContain('memory-entry.public.run.progress')
    expect(rendered).toContain('policy.public.memory.project_retention')
    expect(rendered).toContain('retrieval.public.repository.note')
    expect(rendered).toContain('summary.public.memory.repository_note')
  })

  test('omits unsafe Session memory material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            sessionMemory: {
              blockerRefs: [
                'session-memory-blocker.public.safe',
                'raw memory /Users/christopher/memory.md',
              ],
              entries: [
                {
                  blockerRefs: [
                    'entry-memory-blocker.public.safe',
                    'provider payload sk-private',
                  ],
                  compactionRefs: ['compaction.public.safe', 'raw transcript private'],
                  conflictRefs: ['conflict.public.safe'],
                  entryRef: 'memory-entry.public.safe',
                  freshness: 'fresh',
                  kind: 'task_context',
                  lifecycleState: 'active',
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  redactionClass: 'local_only',
                  retentionClass: 'session',
                  retrievalRefs: ['retrieval.public.safe'],
                  scope: 'session',
                  sourceRefs: [
                    'event.public.safe',
                    'memory body /Users/christopher/private.md',
                  ],
                  summaryRefs: ['summary.public.safe', 'prompt text sk-private'],
                },
              ],
              projectionRef: 'session-memory-projection.public.safe',
              snapshotRef: 'session-memory-snapshot.public.safe',
              versionRef: 'session-memory-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Session memory')
    expect(rendered).toContain('session-memory-snapshot.public.safe')
    expect(rendered).toContain('memory-entry.public.safe')
    expect(rendered).toContain('event.public.safe')
    expect(rendered).toContain('summary.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('compaction.public.safe')
    expect(rendered).toContain('unsafe-session-memory-material-omitted')
    expect(rendered).toContain('unsafe session-memory ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw memory')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('raw transcript')
    expect(rendered).not.toContain('memory body')
    expect(rendered).not.toContain('prompt text')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Diagnostics lane for public diagnostic snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            diagnostics: {
              entries: [
                {
                  diagnosticRef: 'diagnostic.public.tsc.no_emit',
                  freshness: 'fresh',
                  languageServerRef: 'language-server.public.typescript',
                  policyRefs: ['policy.public.diagnostics.read_only'],
                  remediationRefs: ['remediation.public.fix_types'],
                  severity: 'error',
                  sourceRefs: ['source.public.diagnostic.typecheck'],
                },
              ],
              freshness: 'fresh',
              indexedAt: '2026-06-17T20:29:00.000Z',
              indexedAtRef: 'diagnostics-index.public.work_1',
              languageServerRefs: ['language-server.public.typescript'],
              policyRefs: ['policy.public.diagnostics.read_only'],
              remediationRefs: ['remediation.public.fix_types'],
              snapshotRef: 'diagnostics-snapshot.public.work_1',
              sourceRefs: ['source.public.diagnostic.typecheck'],
              versionRef: 'diagnostics-version.public.v1',
              workspaceBoundaryRefs: ['workspace-boundary.public.openagents'],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Diagnostics')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('diagnostics-snapshot.public.work_1')
    expect(rendered).toContain('diagnostics-index.public.work_1')
    expect(rendered).toContain('diagnostic.public.tsc.no_emit')
    expect(rendered).toContain('language-server.public.typescript')
    expect(rendered).toContain('workspace-boundary.public.openagents')
    expect(rendered).toContain('policy.public.diagnostics.read_only')
    expect(rendered).toContain('remediation.public.fix_types')
  })

  test('omits unsafe Diagnostics material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            diagnostics: {
              blockerRefs: [
                'diagnostics-blocker.public.safe',
                'raw diagnostic /Users/christopher/error.log',
              ],
              entries: [
                {
                  blockerRefs: [
                    'entry-diagnostics-blocker.public.safe',
                    'compiler stderr /Users/christopher',
                  ],
                  diagnosticRef: 'diagnostic.public.safe',
                  freshness: 'fresh',
                  languageServerRef: 'language-server.public.safe',
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  remediationRefs: ['remediation.public.safe', 'raw source ./private.ts'],
                  severity: 'warning',
                  sourceRefs: ['source.public.safe', 'diagnostic message sk-private'],
                },
              ],
              freshness: 'fresh',
              languageServerRefs: ['language-server.public.safe'],
              snapshotRef: 'diagnostics-snapshot.public.safe',
              sourceRefs: ['source.public.safe', 'private repo /Users/christopher/project'],
              versionRef: 'diagnostics-version.public.safe',
              workspaceBoundaryRefs: ['workspace-boundary.public.safe'],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Diagnostics')
    expect(rendered).toContain('diagnostics-snapshot.public.safe')
    expect(rendered).toContain('diagnostic.public.safe')
    expect(rendered).toContain('language-server.public.safe')
    expect(rendered).toContain('source.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('remediation.public.safe')
    expect(rendered).toContain('unsafe-diagnostics-material-omitted')
    expect(rendered).toContain('unsafe diagnostics ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw diagnostic')
    expect(rendered).not.toContain('compiler stderr')
    expect(rendered).not.toContain('raw source')
    expect(rendered).not.toContain('diagnostic message')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Help doctor and debug lane for public evidence snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            helpDoctorDebug: {
              entries: [
                {
                  debugBundleRefs: ['debug-bundle.public.safe'],
                  diagnosticRefs: ['diagnostic.public.doctor.ok'],
                  doctorCheckRefs: ['doctor-check.public.context'],
                  freshness: 'fresh',
                  helpTopicRefs: ['help-topic.public.context'],
                  policyRefs: ['policy.public.debug.safe'],
                  remediationRefs: ['remediation.public.none'],
                  severity: 'info',
                  sourceRefs: ['source.public.pylon_doctor'],
                  state: 'passed',
                  surfaceRef: 'help-doctor-debug.public.context',
                },
              ],
              snapshotRef: 'help-doctor-debug-snapshot.public.work_1',
              versionRef: 'help-doctor-debug-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Help, doctor, and debug')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('help-doctor-debug-snapshot.public.work_1')
    expect(rendered).toContain('help-doctor-debug.public.context')
    expect(rendered).toContain('help-topic.public.context')
    expect(rendered).toContain('doctor-check.public.context')
    expect(rendered).toContain('diagnostic.public.doctor.ok')
    expect(rendered).toContain('debug-bundle.public.safe')
    expect(rendered).toContain('remediation.public.none')
    expect(rendered).toContain('policy.public.debug.safe')
  })

  test('omits unsafe Help doctor and debug material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            helpDoctorDebug: {
              blockerRefs: [
                'help-doctor-blocker.public.safe',
                'raw debug /Users/christopher/debug.log',
              ],
              entries: [
                {
                  debugBundleRefs: ['debug-bundle.public.safe', 'raw log sk-private'],
                  diagnosticRefs: [
                    'diagnostic.public.safe',
                    'raw diagnostic /Users/christopher/diag',
                  ],
                  doctorCheckRefs: ['doctor-check.public.safe'],
                  freshness: 'fresh',
                  helpTopicRefs: ['help-topic.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  remediationRefs: ['remediation.public.safe'],
                  severity: 'warning',
                  sourceRefs: ['source.public.safe', 'private debug token'],
                  state: 'warning',
                  surfaceRef: 'help-doctor-debug.public.safe',
                },
              ],
              snapshotRef: 'help-doctor-debug-snapshot.public.safe',
              versionRef: 'help-doctor-debug-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Help, doctor, and debug')
    expect(rendered).toContain('help-doctor-debug-snapshot.public.safe')
    expect(rendered).toContain('help-doctor-debug.public.safe')
    expect(rendered).toContain('help-topic.public.safe')
    expect(rendered).toContain('doctor-check.public.safe')
    expect(rendered).toContain('diagnostic.public.safe')
    expect(rendered).toContain('debug-bundle.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-help-doctor-debug-material-omitted')
    expect(rendered).toContain('unsafe help/doctor/debug ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw debug')
    expect(rendered).not.toContain('raw log')
    expect(rendered).not.toContain('raw diagnostic')
    expect(rendered).not.toContain('private debug')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders MCP server export lane for public export snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            mcpServerExport: {
              entries: [
                {
                  audienceRefs: ['audience.public.operator_agents'],
                  authPolicyRefs: ['auth-policy.public.mcp_server.operator_only'],
                  capabilityRefs: ['capability.public.mcp_server.read_status'],
                  exportedPromptRefs: ['mcp-prompt.public.status_summary'],
                  exportedResourceRefs: ['mcp-resource.public.run_status'],
                  exportedToolRefs: ['mcp-tool.public.get_run_status'],
                  freshness: 'fresh',
                  invocationReceiptRefs: ['invocation-receipt.public.status_probe'],
                  policyRefs: ['policy.public.mcp_server.read_only'],
                  schemaRefs: ['schema.public.mcp_server.status.v1'],
                  serverRef: 'mcp-server.public.operator',
                  sourceRefs: ['source.public.extensibility_config'],
                  state: 'exposed',
                  transportRefs: ['transport.public.stdio_descriptor'],
                  trustTierRefs: ['trust-tier.public.operator'],
                },
              ],
              snapshotRef: 'mcp-server-export-snapshot.public.work_1',
              versionRef: 'mcp-server-export-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('MCP server export')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('mcp-server-export-snapshot.public.work_1')
    expect(rendered).toContain('mcp-server.public.operator')
    expect(rendered).toContain('capability.public.mcp_server.read_status')
    expect(rendered).toContain('mcp-tool.public.get_run_status')
    expect(rendered).toContain('mcp-resource.public.run_status')
    expect(rendered).toContain('mcp-prompt.public.status_summary')
    expect(rendered).toContain('schema.public.mcp_server.status.v1')
    expect(rendered).toContain('auth-policy.public.mcp_server.operator_only')
    expect(rendered).toContain('audience.public.operator_agents')
    expect(rendered).toContain('trust-tier.public.operator')
    expect(rendered).toContain('invocation-receipt.public.status_probe')
  })

  test('omits unsafe MCP server export material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            mcpServerExport: {
              blockerRefs: [
                'mcp-server-blocker.public.safe',
                'raw mcp /Users/christopher/mcp.log',
              ],
              entries: [
                {
                  audienceRefs: ['audience.public.safe'],
                  authPolicyRefs: ['auth-policy.public.safe', 'bearer token private'],
                  capabilityRefs: ['capability.public.safe'],
                  exportedPromptRefs: ['mcp-prompt.public.safe', 'raw prompt sk-private'],
                  exportedResourceRefs: ['mcp-resource.public.safe'],
                  exportedToolRefs: [
                    'mcp-tool.public.safe',
                    'raw tool /Users/christopher/tool.ts',
                  ],
                  freshness: 'fresh',
                  invocationReceiptRefs: ['invocation-receipt.public.safe'],
                  policyRefs: ['policy.public.safe'],
                  schemaRefs: [
                    'schema.public.safe',
                    'raw schema /Users/christopher/schema.json',
                  ],
                  serverRef: 'mcp-server.public.safe',
                  sourceRefs: ['source.public.safe', 'private server content'],
                  state: 'exposed',
                  transportRefs: ['transport.public.safe', 'raw socket /tmp/mcp.sock'],
                  trustTierRefs: ['trust-tier.public.safe'],
                },
              ],
              snapshotRef: 'mcp-server-export-snapshot.public.safe',
              versionRef: 'mcp-server-export-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('MCP server export')
    expect(rendered).toContain('mcp-server-export-snapshot.public.safe')
    expect(rendered).toContain('mcp-server.public.safe')
    expect(rendered).toContain('capability.public.safe')
    expect(rendered).toContain('mcp-tool.public.safe')
    expect(rendered).toContain('schema.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-mcp-server-export-material-omitted')
    expect(rendered).toContain('unsafe MCP server ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw mcp')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw tool')
    expect(rendered).not.toContain('raw schema')
    expect(rendered).not.toContain('raw socket')
    expect(rendered).not.toContain('private server')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Settings and configuration lane for public config snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            settingsConfiguration: {
              entries: [
                {
                  defaultRefs: ['setting-default.public.model_alias'],
                  effectiveValueRefs: ['setting-effective.public.model_alias'],
                  freshness: 'fresh',
                  policyRefs: ['policy.public.settings.model_alias'],
                  redactionClass: 'public',
                  scopeRefs: ['setting-scope.public.workspace'],
                  settingRef: 'setting.public.model_alias',
                  sourceRefs: ['source.public.settings.workspace'],
                  state: 'enabled',
                  validationRefs: ['validation.public.settings.model_alias'],
                },
              ],
              snapshotRef: 'settings-configuration-snapshot.public.work_1',
              versionRef: 'settings-configuration-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Settings and configuration')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('settings-configuration-snapshot.public.work_1')
    expect(rendered).toContain('setting.public.model_alias')
    expect(rendered).toContain('setting-scope.public.workspace')
    expect(rendered).toContain('source.public.settings.workspace')
    expect(rendered).toContain('setting-default.public.model_alias')
    expect(rendered).toContain('setting-effective.public.model_alias')
    expect(rendered).toContain('validation.public.settings.model_alias')
    expect(rendered).toContain('policy.public.settings.model_alias')
  })

  test('omits unsafe Settings and configuration material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            settingsConfiguration: {
              blockerRefs: [
                'settings-blocker.public.safe',
                'raw settings /Users/christopher/settings.json',
              ],
              entries: [
                {
                  defaultRefs: ['setting-default.public.safe'],
                  effectiveValueRefs: [
                    'setting-effective.public.safe',
                    'raw value /Users/christopher/value.json',
                  ],
                  freshness: 'fresh',
                  overrideRefs: [
                    'setting-override.public.safe',
                    'private setting token',
                  ],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  redactionClass: 'private_ref',
                  redactionRefs: ['redaction.public.safe'],
                  scopeRefs: ['setting-scope.public.safe'],
                  settingRef: 'setting.public.safe',
                  sourceRefs: [
                    'source.public.safe',
                    'private config /Users/christopher/config',
                  ],
                  state: 'overridden',
                  validationRefs: ['validation.public.safe'],
                },
              ],
              snapshotRef: 'settings-configuration-snapshot.public.safe',
              versionRef: 'settings-configuration-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Settings and configuration')
    expect(rendered).toContain('settings-configuration-snapshot.public.safe')
    expect(rendered).toContain('setting.public.safe')
    expect(rendered).toContain('setting-effective.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('validation.public.safe')
    expect(rendered).toContain('unsafe-settings-configuration-material-omitted')
    expect(rendered).toContain('unsafe settings/configuration ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw settings')
    expect(rendered).not.toContain('raw value')
    expect(rendered).not.toContain('private setting')
    expect(rendered).not.toContain('private config')
    expect(rendered).not.toContain('bearer token')
  })

  test('renders Authentication and credential storage lane for public credential snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            credentialStorage: {
              entries: [
                {
                  accountRefs: ['account.public.provider_pool.openai'],
                  credentialRef: 'credential.public.provider_pool.openai.default',
                  entitlementRefs: ['entitlement.public.provider_pool.openai'],
                  freshness: 'fresh',
                  kind: 'api_key',
                  leaseRefs: ['lease.public.provider_pool.openai.available'],
                  policyRefs: ['policy.public.credentials.provider_pool'],
                  redactionClass: 'public',
                  scopeRefs: ['credential-scope.public.provider_pool.openai'],
                  sessionRefs: ['session.public.provider_pool.openai'],
                  state: 'usable',
                  storageBackendRefs: ['storage-backend.public.secret_store'],
                  validationRefs: ['validation.public.credential.openai.ready'],
                },
              ],
              snapshotRef: 'credential-storage-snapshot.public.work_1',
              versionRef: 'credential-storage-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Authentication and credential storage')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('credential-storage-snapshot.public.work_1')
    expect(rendered).toContain('credential.public.provider_pool.openai.default')
    expect(rendered).toContain('account.public.provider_pool.openai')
    expect(rendered).toContain('storage-backend.public.secret_store')
    expect(rendered).toContain('credential-scope.public.provider_pool.openai')
    expect(rendered).toContain('entitlement.public.provider_pool.openai')
    expect(rendered).toContain('lease.public.provider_pool.openai.available')
    expect(rendered).toContain('validation.public.credential.openai.ready')
    expect(rendered).toContain('policy.public.credentials.provider_pool')
  })

  test('omits unsafe Authentication and credential storage material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            credentialStorage: {
              blockerRefs: [
                'credential-blocker.public.safe',
                'raw credential /Users/christopher/.env',
              ],
              entries: [
                {
                  accountRefs: ['account.public.safe', 'raw token sk-private'],
                  credentialRef: 'credential.public.safe',
                  entitlementRefs: ['entitlement.public.safe'],
                  freshness: 'fresh',
                  kind: 'api_key',
                  leaseRefs: ['lease.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  redactionClass: 'private_ref',
                  redactionRefs: ['redaction.public.safe'],
                  scopeRefs: ['credential-scope.public.safe'],
                  sessionRefs: [
                    'session.public.safe',
                    'raw session /Users/christopher/session',
                  ],
                  state: 'usable',
                  storageBackendRefs: ['storage-backend.public.safe'],
                  validationRefs: [
                    'validation.public.safe',
                    'private credential material',
                  ],
                },
              ],
              snapshotRef: 'credential-storage-snapshot.public.safe',
              versionRef: 'credential-storage-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Authentication and credential storage')
    expect(rendered).toContain('credential-storage-snapshot.public.safe')
    expect(rendered).toContain('credential.public.safe')
    expect(rendered).toContain('account.public.safe')
    expect(rendered).toContain('storage-backend.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('validation.public.safe')
    expect(rendered).toContain('unsafe-credential-storage-material-omitted')
    expect(rendered).toContain('unsafe credential ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw credential')
    expect(rendered).not.toContain('raw token')
    expect(rendered).not.toContain('raw session')
    expect(rendered).not.toContain('private credential')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Git and GitHub workflow lane for public workflow snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            gitWorkflow: {
              entries: [
                {
                  branchRefs: ['branch.public.work_1.feature'],
                  checkRefs: ['check.public.work_1.bun_test.pass'],
                  commitRefs: ['commit.public.work_1.head'],
                  diffRefs: ['diff.public.work_1.summary'],
                  freshness: 'fresh',
                  issueRefs: ['issue.public.5107'],
                  policyRefs: ['policy.public.github.writeback'],
                  prRefs: ['pr.public.work_1.draft'],
                  repositoryRefs: ['repo.public.OpenAgentsInc.openagents'],
                  reviewRefs: ['review.public.work_1.ready'],
                  state: 'pr_ready',
                  statusRefs: ['status.public.work_1.checks'],
                  workflowRef: 'git-workflow.public.work_1.pr',
                  worktreeRefs: ['worktree.public.work_1'],
                  writebackRefs: ['writeback.public.work_1.draft_pr'],
                },
              ],
              snapshotRef: 'git-workflow-snapshot.public.work_1',
              versionRef: 'git-workflow-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Git and GitHub workflow')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('git-workflow-snapshot.public.work_1')
    expect(rendered).toContain('git-workflow.public.work_1.pr')
    expect(rendered).toContain('repo.public.OpenAgentsInc.openagents')
    expect(rendered).toContain('branch.public.work_1.feature')
    expect(rendered).toContain('commit.public.work_1.head')
    expect(rendered).toContain('diff.public.work_1.summary')
    expect(rendered).toContain('pr.public.work_1.draft')
    expect(rendered).toContain('issue.public.5107')
    expect(rendered).toContain('review.public.work_1.ready')
    expect(rendered).toContain('check.public.work_1.bun_test.pass')
    expect(rendered).toContain('writeback.public.work_1.draft_pr')
  })

  test('omits unsafe Git and GitHub workflow material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            gitWorkflow: {
              blockerRefs: [
                'git-workflow-blocker.public.safe',
                'git status /Users/christopher/openagents',
              ],
              entries: [
                {
                  branchRefs: [
                    'branch.public.safe',
                    'raw branch /Users/christopher/repo',
                  ],
                  checkRefs: ['check.public.safe'],
                  commitRefs: ['commit.public.safe', 'raw commit sk-private'],
                  diffRefs: ['diff.public.safe', 'diff --git a/private.ts b/private.ts'],
                  freshness: 'fresh',
                  issueRefs: ['issue.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  prRefs: ['pr.public.safe', 'github comment private body'],
                  repositoryRefs: ['repo.public.safe', 'https://github.com/private/repo'],
                  reviewRefs: ['review.public.safe'],
                  state: 'pr_ready',
                  statusRefs: ['status.public.safe', 'raw status ./private'],
                  workflowRef: 'git-workflow.public.safe',
                  worktreeRefs: ['worktree.public.safe', '/Users/christopher/work/openagents'],
                  writebackRefs: ['writeback.public.safe'],
                },
              ],
              snapshotRef: 'git-workflow-snapshot.public.safe',
              versionRef: 'git-workflow-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Git and GitHub workflow')
    expect(rendered).toContain('git-workflow-snapshot.public.safe')
    expect(rendered).toContain('git-workflow.public.safe')
    expect(rendered).toContain('branch.public.safe')
    expect(rendered).toContain('diff.public.safe')
    expect(rendered).toContain('pr.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-git-workflow-material-omitted')
    expect(rendered).toContain('unsafe Git/GitHub ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('git status')
    expect(rendered).not.toContain('raw branch')
    expect(rendered).not.toContain('raw commit')
    expect(rendered).not.toContain('diff --git')
    expect(rendered).not.toContain('github comment')
    expect(rendered).not.toContain('https://github.com/private')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders IDE and editor integration lane for public editor snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            editorIntegration: {
              entries: [
                {
                  commandRefs: ['editor-command.public.open_diff'],
                  deepLinkRefs: ['deep-link.public.editor.diff'],
                  diagnosticHandoffRefs: ['diagnostic-handoff.public.editor'],
                  diagnosticRefs: ['diagnostic.public.editor.safe'],
                  editorRefs: ['editor.public.vscode'],
                  extensionRefs: ['extension.public.openagents'],
                  fileOpenRefs: ['file-open.public.diff_summary'],
                  freshness: 'fresh',
                  integrationRef: 'editor-integration.public.vscode',
                  policyRefs: ['policy.public.editor.deep_link'],
                  selectionRefs: ['selection.public.none'],
                  state: 'ready',
                  statusRefs: ['status.public.editor.connected'],
                  workspaceRefs: ['workspace.public.openagents'],
                },
              ],
              snapshotRef: 'editor-integration-snapshot.public.work_1',
              versionRef: 'editor-integration-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('IDE and editor integration')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('editor-integration-snapshot.public.work_1')
    expect(rendered).toContain('editor-integration.public.vscode')
    expect(rendered).toContain('editor.public.vscode')
    expect(rendered).toContain('workspace.public.openagents')
    expect(rendered).toContain('extension.public.openagents')
    expect(rendered).toContain('editor-command.public.open_diff')
    expect(rendered).toContain('diagnostic.public.editor.safe')
    expect(rendered).toContain('diagnostic-handoff.public.editor')
    expect(rendered).toContain('file-open.public.diff_summary')
    expect(rendered).toContain('deep-link.public.editor.diff')
    expect(rendered).toContain('policy.public.editor.deep_link')
  })

  test('omits unsafe IDE and editor integration material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            editorIntegration: {
              blockerRefs: [
                'editor-blocker.public.safe',
                'raw editor /Users/christopher/editor.log',
              ],
              entries: [
                {
                  commandRefs: [
                    'editor-command.public.safe',
                    'editor command private text',
                  ],
                  deepLinkRefs: [
                    'deep-link.public.safe',
                    'vscode://file/Users/christopher/app.ts',
                  ],
                  diagnosticHandoffRefs: ['diagnostic-handoff.public.safe'],
                  diagnosticRefs: ['diagnostic.public.safe'],
                  editorRefs: ['editor.public.safe'],
                  extensionRefs: ['extension.public.safe', 'raw extension sk-private'],
                  fileOpenRefs: [
                    'file-open.public.safe',
                    'raw file /Users/christopher/app.ts',
                  ],
                  freshness: 'fresh',
                  integrationRef: 'editor-integration.public.safe',
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  selectionRefs: [
                    'selection.public.safe',
                    'raw selection private buffer',
                  ],
                  state: 'ready',
                  statusRefs: ['status.public.safe'],
                  workspaceRefs: [
                    'workspace.public.safe',
                    '/Users/christopher/work/openagents',
                  ],
                },
              ],
              snapshotRef: 'editor-integration-snapshot.public.safe',
              versionRef: 'editor-integration-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('IDE and editor integration')
    expect(rendered).toContain('editor-integration-snapshot.public.safe')
    expect(rendered).toContain('editor-integration.public.safe')
    expect(rendered).toContain('editor-command.public.safe')
    expect(rendered).toContain('file-open.public.safe')
    expect(rendered).toContain('deep-link.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-editor-integration-material-omitted')
    expect(rendered).toContain('unsafe editor ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw editor')
    expect(rendered).not.toContain('editor command')
    expect(rendered).not.toContain('vscode://')
    expect(rendered).not.toContain('raw extension')
    expect(rendered).not.toContain('raw file')
    expect(rendered).not.toContain('raw selection')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Browser and desktop integration lane for public surface snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            browserDesktopIntegration: {
              entries: [
                {
                  browserRefs: ['browser.public.chrome'],
                  companionRefs: ['companion.public.web'],
                  deepLinkRefs: ['deep-link.public.desktop.open_run'],
                  desktopAppRefs: ['desktop-app.public.autopilot'],
                  extensionRefs: ['extension.public.openagents'],
                  freshness: 'fresh',
                  installRefs: ['install.public.desktop.autopilot'],
                  integrationRef: 'browser-desktop.public.operator',
                  notificationRefs: ['notification.public.desktop.review_ready'],
                  permissionRefs: ['permission.public.notifications.granted'],
                  policyRefs: ['policy.public.browser_desktop.operator_only'],
                  state: 'ready',
                  statusRefs: ['status.public.browser_desktop.connected'],
                  surfaceRefs: ['surface.public.autopilot.desktop'],
                  updateRefs: ['update.public.desktop.current'],
                },
              ],
              snapshotRef: 'browser-desktop-integration-snapshot.public.work_1',
              versionRef: 'browser-desktop-integration-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Browser and desktop integration')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('browser-desktop-integration-snapshot.public.work_1')
    expect(rendered).toContain('browser-desktop.public.operator')
    expect(rendered).toContain('surface.public.autopilot.desktop')
    expect(rendered).toContain('browser.public.chrome')
    expect(rendered).toContain('desktop-app.public.autopilot')
    expect(rendered).toContain('extension.public.openagents')
    expect(rendered).toContain('deep-link.public.desktop.open_run')
    expect(rendered).toContain('notification.public.desktop.review_ready')
    expect(rendered).toContain('permission.public.notifications.granted')
    expect(rendered).toContain('install.public.desktop.autopilot')
    expect(rendered).toContain('policy.public.browser_desktop.operator_only')
  })

  test('omits unsafe Browser and desktop integration material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            browserDesktopIntegration: {
              blockerRefs: [
                'browser-desktop-blocker.public.safe',
                'raw browser /Users/christopher/profile',
              ],
              entries: [
                {
                  browserRefs: ['browser.public.safe', 'raw browser cookie sk-private'],
                  companionRefs: ['companion.public.safe'],
                  deepLinkRefs: [
                    'deep-link.public.safe',
                    'openagents://run/Users/christopher/private',
                  ],
                  desktopAppRefs: [
                    'desktop-app.public.safe',
                    'desktop app path /Users/christopher/app',
                  ],
                  extensionRefs: ['extension.public.safe', 'raw extension private'],
                  freshness: 'fresh',
                  installRefs: ['install.public.safe'],
                  integrationRef: 'browser-desktop.public.safe',
                  notificationRefs: ['notification.public.safe'],
                  permissionRefs: ['permission.public.safe', 'raw permission ~/Library'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  state: 'ready',
                  statusRefs: ['status.public.safe'],
                  surfaceRefs: ['surface.public.safe', 'private session token'],
                  updateRefs: ['update.public.safe'],
                },
              ],
              snapshotRef: 'browser-desktop-integration-snapshot.public.safe',
              versionRef: 'browser-desktop-integration-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Browser and desktop integration')
    expect(rendered).toContain('browser-desktop-integration-snapshot.public.safe')
    expect(rendered).toContain('browser-desktop.public.safe')
    expect(rendered).toContain('browser.public.safe')
    expect(rendered).toContain('deep-link.public.safe')
    expect(rendered).toContain('desktop-app.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-browser-desktop-integration-material-omitted')
    expect(rendered).toContain('unsafe browser/desktop ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw browser')
    expect(rendered).not.toContain('openagents://')
    expect(rendered).not.toContain('desktop app path')
    expect(rendered).not.toContain('raw extension')
    expect(rendered).not.toContain('raw permission')
    expect(rendered).not.toContain('private session')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Voice and multimodal input lane for public input snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            multimodalInput: {
              entries: [
                {
                  attachmentRefs: ['attachment.public.voice_note'],
                  captureSurfaceRefs: ['capture-surface.public.browser_audio'],
                  consentRefs: ['consent.public.voice_note'],
                  contextIngestionRefs: ['context-ingestion.public.voice_note'],
                  endpointRefs: ['endpoint.public.vad_summary'],
                  freshness: 'fresh',
                  inputRef: 'multimodal-input.public.voice_note',
                  modality: 'audio',
                  policyRefs: ['policy.public.multimodal.consent_required'],
                  redactionRefs: ['redaction.public.transcript.safe'],
                  state: 'ingested',
                  transcriptRefs: ['transcript.public.voice_note.summary'],
                  vadRefs: ['vad.public.voice_note.boundary'],
                },
              ],
              snapshotRef: 'multimodal-input-snapshot.public.work_1',
              versionRef: 'multimodal-input-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Voice and multimodal input')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('multimodal-input-snapshot.public.work_1')
    expect(rendered).toContain('multimodal-input.public.voice_note')
    expect(rendered).toContain('capture-surface.public.browser_audio')
    expect(rendered).toContain('attachment.public.voice_note')
    expect(rendered).toContain('transcript.public.voice_note.summary')
    expect(rendered).toContain('vad.public.voice_note.boundary')
    expect(rendered).toContain('consent.public.voice_note')
    expect(rendered).toContain('redaction.public.transcript.safe')
    expect(rendered).toContain('context-ingestion.public.voice_note')
    expect(rendered).toContain('policy.public.multimodal.consent_required')
  })

  test('omits unsafe Voice and multimodal input material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            multimodalInput: {
              blockerRefs: [
                'multimodal-blocker.public.safe',
                'raw audio /Users/christopher/audio.wav',
              ],
              entries: [
                {
                  attachmentRefs: [
                    'attachment.public.safe',
                    'raw file /Users/christopher/image.png',
                  ],
                  captureSurfaceRefs: ['capture-surface.public.safe'],
                  consentRefs: ['consent.public.safe'],
                  contextIngestionRefs: ['context-ingestion.public.safe'],
                  endpointRefs: ['endpoint.public.safe'],
                  freshness: 'fresh',
                  inputRef: 'multimodal-input.public.safe',
                  modality: 'audio',
                  policyRefs: ['policy.public.safe', 'provider prompt sk-private'],
                  redactionRefs: ['redaction.public.safe'],
                  state: 'ingested',
                  transcriptRefs: [
                    'transcript.public.safe',
                    'transcript body private prompt',
                  ],
                  vadRefs: ['vad.public.safe', 'raw vad /Users/christopher/vad.json'],
                },
              ],
              snapshotRef: 'multimodal-input-snapshot.public.safe',
              versionRef: 'multimodal-input-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Voice and multimodal input')
    expect(rendered).toContain('multimodal-input-snapshot.public.safe')
    expect(rendered).toContain('multimodal-input.public.safe')
    expect(rendered).toContain('attachment.public.safe')
    expect(rendered).toContain('transcript.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-multimodal-input-material-omitted')
    expect(rendered).toContain('unsafe multimodal ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw audio')
    expect(rendered).not.toContain('raw file')
    expect(rendered).not.toContain('provider prompt')
    expect(rendered).not.toContain('transcript body')
    expect(rendered).not.toContain('raw vad')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Remote Session Bridge lane for public bridge snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            remoteSessionBridge: {
              entries: [
                {
                  bridgeRef: 'remote-bridge.public.autopilot_control',
                  controllerRefs: ['controller.public.browser'],
                  freshness: 'fresh',
                  heartbeatRefs: ['heartbeat.public.remote_session.ok'],
                  permissionRefs: ['permission.public.remote_control.read_only'],
                  policyRefs: ['policy.public.remote_bridge.read_only'],
                  protocolRefs: ['protocol.public.autopilot_control.v1'],
                  sessionRefs: ['remote-session.public.work_1'],
                  state: 'ready',
                  transportRefs: ['transport.public.websocket.bridge'],
                },
              ],
              snapshotRef: 'remote-session-bridge-snapshot.public.work_1',
              versionRef: 'remote-session-bridge-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Remote Session Bridge')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('remote-session-bridge-snapshot.public.work_1')
    expect(rendered).toContain('remote-bridge.public.autopilot_control')
    expect(rendered).toContain('remote-session.public.work_1')
    expect(rendered).toContain('transport.public.websocket.bridge')
    expect(rendered).toContain('protocol.public.autopilot_control.v1')
    expect(rendered).toContain('controller.public.browser')
    expect(rendered).toContain('heartbeat.public.remote_session.ok')
    expect(rendered).toContain('permission.public.remote_control.read_only')
    expect(rendered).toContain('policy.public.remote_bridge.read_only')
  })

  test('omits unsafe Remote Session Bridge material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            remoteSessionBridge: {
              blockerRefs: [
                'remote-bridge-blocker.public.safe',
                'raw remote /Users/christopher/remote.log',
              ],
              entries: [
                {
                  bridgeRef: 'remote-bridge.public.safe',
                  controllerRefs: ['controller.public.safe', 'raw command sk-private'],
                  freshness: 'fresh',
                  heartbeatRefs: [
                    'heartbeat.public.safe',
                    'remote log /Users/christopher/log',
                  ],
                  permissionRefs: ['permission.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  protocolRefs: ['protocol.public.safe'],
                  reconnectRefs: ['reconnect.public.safe'],
                  sessionRefs: ['remote-session.public.safe', 'ssh://private-host/session'],
                  state: 'ready',
                  transportRefs: [
                    'transport.public.safe',
                    'raw transport /Users/christopher/socket',
                  ],
                },
              ],
              snapshotRef: 'remote-session-bridge-snapshot.public.safe',
              versionRef: 'remote-session-bridge-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Remote Session Bridge')
    expect(rendered).toContain('remote-session-bridge-snapshot.public.safe')
    expect(rendered).toContain('remote-bridge.public.safe')
    expect(rendered).toContain('remote-session.public.safe')
    expect(rendered).toContain('transport.public.safe')
    expect(rendered).toContain('protocol.public.safe')
    expect(rendered).toContain('controller.public.safe')
    expect(rendered).toContain('permission.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-remote-session-bridge-material-omitted')
    expect(rendered).toContain('unsafe remote bridge ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw remote')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('remote log')
    expect(rendered).not.toContain('ssh://')
    expect(rendered).not.toContain('raw transport')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Mobile and web companion lane for public companion snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            companionSurface: {
              entries: [
                {
                  artifactRefs: ['artifact.public.work_1.summary'],
                  budgetRefs: ['budget.public.work_1.status'],
                  closeoutRefs: ['closeout.public.work_1.summary'],
                  companionRef: 'companion.public.mobile.work_1',
                  cursorRefs: ['cursor.public.event_stream.42'],
                  decisionRefs: ['decision.public.review_required'],
                  deliveryTierRefs: ['delivery-tier.public.lossless'],
                  freshness: 'fresh',
                  notificationRefs: ['notification.public.review_required'],
                  pairingRefs: ['pairing.public.mobile.browser'],
                  policyRefs: ['policy.public.companion.read_only'],
                  progressRefs: ['progress.public.work_1.latest'],
                  runRefs: ['run.public.work_1'],
                  sessionRefs: ['session.public.work_1'],
                  state: 'ready',
                  streamRefs: ['event-stream.public.work_1'],
                  surfaceRefs: ['surface.public.mobile.status'],
                },
              ],
              snapshotRef: 'companion-surface-snapshot.public.work_1',
              versionRef: 'companion-surface-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Mobile and web companion')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('companion-surface-snapshot.public.work_1')
    expect(rendered).toContain('companion.public.mobile.work_1')
    expect(rendered).toContain('surface.public.mobile.status')
    expect(rendered).toContain('pairing.public.mobile.browser')
    expect(rendered).toContain('event-stream.public.work_1')
    expect(rendered).toContain('cursor.public.event_stream.42')
    expect(rendered).toContain('decision.public.review_required')
    expect(rendered).toContain('notification.public.review_required')
    expect(rendered).toContain('artifact.public.work_1.summary')
    expect(rendered).toContain('budget.public.work_1.status')
    expect(rendered).toContain('policy.public.companion.read_only')
  })

  test('omits unsafe Mobile and web companion material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            companionSurface: {
              blockerRefs: [
                'companion-blocker.public.safe',
                'raw terminal /Users/christopher/terminal.log',
              ],
              entries: [
                {
                  actionRefs: ['action.public.safe', 'raw action sk-private'],
                  artifactRefs: [
                    'artifact.public.safe',
                    'private artifact /Users/christopher/a.md',
                  ],
                  blockerRefs: ['entry-companion-blocker.public.safe'],
                  budgetRefs: ['budget.public.safe'],
                  capabilityRefs: ['capability.public.safe'],
                  closeoutRefs: ['closeout.public.safe'],
                  companionRef: 'companion.public.safe',
                  cursorRefs: ['cursor.public.safe'],
                  decisionRefs: [
                    'decision.public.safe',
                    'raw decision /Users/christopher/d.json',
                  ],
                  deliveryTierRefs: ['delivery-tier.public.lossless'],
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.safe'],
                  lagRefs: ['lag.public.safe'],
                  notificationRefs: [
                    'notification.public.safe',
                    'mobile payload private token',
                  ],
                  pairingRefs: ['pairing.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  progressRefs: [
                    'progress.public.safe',
                    'raw progress /Users/christopher/p.log',
                  ],
                  receiptRefs: ['receipt.public.safe'],
                  runRefs: ['run.public.safe'],
                  sessionRefs: [
                    'session.public.safe',
                    'terminal session /Users/christopher/s',
                  ],
                  state: 'ready',
                  streamRefs: ['event-stream.public.safe'],
                  surfaceRefs: ['surface.public.safe', 'https://private.example/session'],
                },
              ],
              snapshotRef: 'companion-surface-snapshot.public.safe',
              versionRef: 'companion-surface-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Mobile and web companion')
    expect(rendered).toContain('companion-surface-snapshot.public.safe')
    expect(rendered).toContain('companion.public.safe')
    expect(rendered).toContain('surface.public.safe')
    expect(rendered).toContain('pairing.public.safe')
    expect(rendered).toContain('event-stream.public.safe')
    expect(rendered).toContain('cursor.public.safe')
    expect(rendered).toContain('action.public.safe')
    expect(rendered).toContain('capability.public.safe')
    expect(rendered).toContain('receipt.public.safe')
    expect(rendered).toContain('unsafe-companion-surface-material-omitted')
    expect(rendered).toContain('unsafe companion ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw terminal')
    expect(rendered).not.toContain('raw action')
    expect(rendered).not.toContain('private artifact')
    expect(rendered).not.toContain('raw decision')
    expect(rendered).not.toContain('mobile payload')
    expect(rendered).not.toContain('raw progress')
    expect(rendered).not.toContain('terminal session')
    expect(rendered).not.toContain('https://')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Team and shared memory lane for public memory snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            teamSharedMemory: {
              entries: [
                {
                  applicationReceiptRefs: ['memory-application.public.work_1.repo_style'],
                  evidenceRefs: ['evidence.public.review.accepted'],
                  freshness: 'fresh',
                  kind: 'repo_style',
                  memoryRef: 'shared-memory.public.repo_style',
                  ownerRefs: ['owner.public.user_1'],
                  policyRefs: ['policy.public.team_memory.visible'],
                  redactionClass: 'team_ref',
                  retrievalPolicyRefs: ['retrieval-policy.public.semantic_typed'],
                  reviewState: 'accepted',
                  scope: 'team',
                  semanticQueryRefs: ['semantic-query.public.repo_style'],
                  teamRefs: ['team.public.engineering'],
                  typedQueryRefs: ['typed-query.public.repo_style'],
                  visibility: 'team',
                },
              ],
              projectionRef: 'team-shared-memory-projection.public.work_1',
              snapshotRef: 'team-shared-memory-snapshot.public.work_1',
              versionRef: 'team-shared-memory-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Team and shared memory')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('team-shared-memory-snapshot.public.work_1')
    expect(rendered).toContain('team-shared-memory-projection.public.work_1')
    expect(rendered).toContain('shared-memory.public.repo_style')
    expect(rendered).toContain('owner.public.user_1')
    expect(rendered).toContain('team.public.engineering')
    expect(rendered).toContain('evidence.public.review.accepted')
    expect(rendered).toContain('retrieval-policy.public.semantic_typed')
    expect(rendered).toContain('typed-query.public.repo_style')
    expect(rendered).toContain('semantic-query.public.repo_style')
    expect(rendered).toContain('memory-application.public.work_1.repo_style')
    expect(rendered).toContain('policy.public.team_memory.visible')
  })

  test('omits unsafe Team and shared memory material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            teamSharedMemory: {
              blockerRefs: [
                'shared-memory-blocker.public.safe',
                'raw memory /Users/christopher/memory.md',
              ],
              entries: [
                {
                  applicationReceiptRefs: ['memory-application.public.safe'],
                  blockerRefs: ['entry-shared-memory-blocker.public.safe'],
                  consentRefs: ['consent.public.safe'],
                  deletionReceiptRefs: ['memory-deletion.public.safe'],
                  evidenceRefs: [
                    'evidence.public.safe',
                    'memory body /Users/christopher/private.md',
                  ],
                  expiryRefs: ['expiry.public.safe'],
                  freshness: 'fresh',
                  kind: 'repo_style',
                  memoryRef: 'shared-memory.public.safe',
                  ownerRefs: ['owner.public.safe', 'raw prompt sk-private'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  promotionRefs: ['promotion.public.safe'],
                  redactionClass: 'team_ref',
                  retrievalPolicyRefs: ['retrieval-policy.public.safe'],
                  reviewRefs: ['review.public.safe', 'provider payload sk-private'],
                  reviewState: 'accepted',
                  scope: 'team',
                  semanticQueryRefs: ['semantic-query.public.safe'],
                  teamRefs: ['team.public.safe'],
                  tombstoneRefs: ['tombstone.public.safe'],
                  typedQueryRefs: ['typed-query.public.safe'],
                  visibility: 'team',
                },
              ],
              projectionRef: 'team-shared-memory-projection.public.safe',
              snapshotRef: 'team-shared-memory-snapshot.public.safe',
              versionRef: 'team-shared-memory-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Team and shared memory')
    expect(rendered).toContain('team-shared-memory-snapshot.public.safe')
    expect(rendered).toContain('shared-memory.public.safe')
    expect(rendered).toContain('owner.public.safe')
    expect(rendered).toContain('team.public.safe')
    expect(rendered).toContain('evidence.public.safe')
    expect(rendered).toContain('retrieval-policy.public.safe')
    expect(rendered).toContain('typed-query.public.safe')
    expect(rendered).toContain('semantic-query.public.safe')
    expect(rendered).toContain('memory-application.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-team-shared-memory-material-omitted')
    expect(rendered).toContain('unsafe shared-memory ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw memory')
    expect(rendered).not.toContain('memory body')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Multi-agent coordination lane for public lane snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            multiAgentCoordination: {
              entries: [
                {
                  acceptancePolicyRefs: ['acceptance-policy.public.all_mandatory'],
                  adapterRefs: ['adapter.public.pylon.local'],
                  artifactRefs: ['artifact.public.lane_1.summary'],
                  assignmentRefs: ['assignment.public.lane_1'],
                  budgetCapRefs: ['budget-cap.public.lane_1'],
                  capabilityRefs: ['capability.public.repo_read'],
                  closeoutRefs: ['closeout.public.lane_1'],
                  criticality: 'mandatory',
                  dependencyRefs: ['dependency.public.none'],
                  freshness: 'fresh',
                  kind: 'local',
                  laneRef: 'coordination-lane.public.lane_1',
                  policyRefs: ['policy.public.lane.local'],
                  receiptRefs: ['receipt.public.lane_1.assigned'],
                  state: 'completed',
                },
              ],
              parentRunRef: 'run.public.parent',
              planRef: 'coordination-plan.public.work_1',
              snapshotRef: 'multi-agent-coordination-snapshot.public.work_1',
              versionRef: 'multi-agent-coordination-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Multi-agent coordination')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('multi-agent-coordination-snapshot.public.work_1')
    expect(rendered).toContain('coordination-plan.public.work_1')
    expect(rendered).toContain('run.public.parent')
    expect(rendered).toContain('coordination-lane.public.lane_1')
    expect(rendered).toContain('assignment.public.lane_1')
    expect(rendered).toContain('dependency.public.none')
    expect(rendered).toContain('budget-cap.public.lane_1')
    expect(rendered).toContain('adapter.public.pylon.local')
    expect(rendered).toContain('capability.public.repo_read')
    expect(rendered).toContain('artifact.public.lane_1.summary')
    expect(rendered).toContain('receipt.public.lane_1.assigned')
    expect(rendered).toContain('closeout.public.lane_1')
    expect(rendered).toContain('acceptance-policy.public.all_mandatory')
    expect(rendered).toContain('policy.public.lane.local')
  })

  test('omits unsafe Multi-agent coordination material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            multiAgentCoordination: {
              blockerRefs: [
                'coordination-blocker.public.safe',
                'raw lane /Users/christopher/lane.log',
              ],
              entries: [
                {
                  acceptancePolicyRefs: ['acceptance-policy.public.safe'],
                  adapterRefs: ['adapter.public.safe'],
                  artifactRefs: [
                    'artifact.public.safe',
                    'raw artifact /Users/christopher/a.diff',
                  ],
                  assignmentRefs: ['assignment.public.safe'],
                  blockerRefs: ['lane-blocker.public.safe'],
                  budgetCapRefs: ['budget-cap.public.safe'],
                  capabilityRefs: ['capability.public.safe'],
                  closeoutRefs: ['closeout.public.safe'],
                  conflictRefs: ['conflict.public.safe'],
                  criticality: 'mandatory',
                  dependencyRefs: ['dependency.public.safe'],
                  freshness: 'fresh',
                  inboxRefs: ['lane-inbox.public.safe', 'raw message sk-private'],
                  kind: 'local',
                  laneRef: 'coordination-lane.public.safe',
                  mergeStrategyRefs: ['merge-strategy.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  providerRefs: ['provider.public.safe', 'provider payload sk-private'],
                  receiptRefs: ['receipt.public.safe'],
                  state: 'completed',
                  steeringReceiptRefs: ['steering-receipt.public.safe'],
                },
              ],
              parentRunRef: 'run.public.safe',
              planRef: 'coordination-plan.public.safe',
              snapshotRef: 'multi-agent-coordination-snapshot.public.safe',
              versionRef: 'multi-agent-coordination-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Multi-agent coordination')
    expect(rendered).toContain('multi-agent-coordination-snapshot.public.safe')
    expect(rendered).toContain('coordination-plan.public.safe')
    expect(rendered).toContain('coordination-lane.public.safe')
    expect(rendered).toContain('assignment.public.safe')
    expect(rendered).toContain('adapter.public.safe')
    expect(rendered).toContain('artifact.public.safe')
    expect(rendered).toContain('lane-inbox.public.safe')
    expect(rendered).toContain('steering-receipt.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-multi-agent-coordination-material-omitted')
    expect(rendered).toContain('unsafe coordination ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw lane')
    expect(rendered).not.toContain('raw artifact')
    expect(rendered).not.toContain('raw message')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders External work intake lane for public intake snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            externalWorkIntake: {
              entries: [
                {
                  acceptancePolicyRefs: ['acceptance-policy.public.review_required'],
                  accountRefs: ['account.public.requester'],
                  adapterPreferenceRefs: ['adapter-preference.public.pylon'],
                  admissionReceiptRefs: ['admission-receipt.public.work_1'],
                  apiParityRefs: ['api-parity.public.work_intake'],
                  budgetRefs: ['budget.public.work_1'],
                  budgetRequired: true,
                  capabilityRefs: ['capability.public.repo_write'],
                  channel: 'ui',
                  dataClassificationRefs: ['data-classification.public_safe'],
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.work_1'],
                  intakeRef: 'intake.public.work_1',
                  policyRefs: ['policy.public.intake.ui'],
                  requesterRefs: ['requester.public.user_1'],
                  reviewPolicyRefs: ['review-policy.public.requester_required'],
                  routingReceiptRefs: ['routing-receipt.public.work_1'],
                  scopeRefs: ['scope.public.repository'],
                  status: 'admitted',
                  statusReceiptRefs: ['status-receipt.public.admitted'],
                  verificationRefs: ['verification.public.bun_test'],
                  workKind: 'coding_task',
                  workOrderRefs: ['work-order.public.work_1'],
                },
              ],
              snapshotRef: 'external-work-intake-snapshot.public.work_1',
              versionRef: 'external-work-intake-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('External work intake')
    expect(rendered).toContain('admitted')
    expect(rendered).toContain('external-work-intake-snapshot.public.work_1')
    expect(rendered).toContain('intake.public.work_1')
    expect(rendered).toContain('requester.public.user_1')
    expect(rendered).toContain('account.public.requester')
    expect(rendered).toContain('scope.public.repository')
    expect(rendered).toContain('data-classification.public_safe')
    expect(rendered).toContain('capability.public.repo_write')
    expect(rendered).toContain('adapter-preference.public.pylon')
    expect(rendered).toContain('budget.public.work_1')
    expect(rendered).toContain('idempotency.public.work_1')
    expect(rendered).toContain('admission-receipt.public.work_1')
    expect(rendered).toContain('routing-receipt.public.work_1')
    expect(rendered).toContain('work-order.public.work_1')
    expect(rendered).toContain('api-parity.public.work_intake')
  })

  test('omits unsafe External work intake material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            externalWorkIntake: {
              blockerRefs: [
                'intake-blocker.public.safe',
                'raw intake /Users/christopher/intake.json',
              ],
              entries: [
                {
                  acceptancePolicyRefs: ['acceptance-policy.public.safe'],
                  accountRefs: ['account.public.safe'],
                  adapterPreferenceRefs: ['adapter-preference.public.safe'],
                  admissionReceiptRefs: ['admission-receipt.public.safe'],
                  apiParityRefs: ['api-parity.public.safe'],
                  blockerRefs: ['entry-intake-blocker.public.safe'],
                  budgetRefs: ['budget.public.safe'],
                  capabilityRefs: ['capability.public.safe'],
                  channel: 'ui',
                  dataClassificationRefs: [
                    'data-classification.public_safe',
                    'raw request /Users/christopher/private.md',
                  ],
                  deliveryReceiptRefs: ['delivery-receipt.public.safe'],
                  expirationRefs: ['expiration.public.safe'],
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.safe'],
                  intakeRef: 'intake.public.safe',
                  paymentRefs: ['payment.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  requestRefs: ['request.public.safe', 'provider payload sk-private'],
                  requesterRefs: ['requester.public.safe'],
                  reviewPolicyRefs: ['review-policy.public.safe'],
                  routingReceiptRefs: ['routing-receipt.public.safe'],
                  scopeRefs: ['scope.public.safe'],
                  status: 'admitted',
                  statusReceiptRefs: ['status-receipt.public.safe'],
                  verificationRefs: ['verification.public.safe'],
                  workKind: 'coding_task',
                  workOrderRefs: ['work-order.public.safe'],
                },
              ],
              snapshotRef: 'external-work-intake-snapshot.public.safe',
              versionRef: 'external-work-intake-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('External work intake')
    expect(rendered).toContain('external-work-intake-snapshot.public.safe')
    expect(rendered).toContain('intake.public.safe')
    expect(rendered).toContain('request.public.safe')
    expect(rendered).toContain('requester.public.safe')
    expect(rendered).toContain('account.public.safe')
    expect(rendered).toContain('capability.public.safe')
    expect(rendered).toContain('payment.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('routing-receipt.public.safe')
    expect(rendered).toContain('api-parity.public.safe')
    expect(rendered).toContain('unsafe-external-work-intake-material-omitted')
    expect(rendered).toContain('unsafe intake ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw intake')
    expect(rendered).not.toContain('raw request')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Artifact and receipt index lane for public artifact snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            artifactReceiptIndex: {
              artifacts: [
                {
                  artifactRef: 'artifact.public.work_1.diff_summary',
                  digestRefs: ['digest.public.artifact.sha256'],
                  freshness: 'fresh',
                  kind: 'diff',
                  mediaTypeRefs: ['media-type.public.text_markdown'],
                  policyRefs: ['policy.public.artifact.redacted'],
                  producerRefs: ['producer.public.pylon.local'],
                  redactionClass: 'public_safe',
                  relatedReceiptRefs: ['receipt.public.delivery.work_1'],
                  retentionRefs: ['retention.public.receipt_index'],
                  runRefs: ['run.public.work_1'],
                  sizeRefs: ['size.public.artifact.2048'],
                  summaryRefs: ['summary.public.diff'],
                  visibility: 'public',
                  workOrderRefs: ['work-order.public.work_1'],
                },
              ],
              receipts: [
                {
                  actorRefs: ['actor.public.agent'],
                  caveatRefs: ['caveat.public.summary_only'],
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.delivery.work_1'],
                  inputRefs: ['artifact.public.work_1.diff_summary'],
                  outputRefs: ['receipt-output.public.delivery'],
                  policyRefs: ['policy.public.delivery'],
                  receiptRef: 'receipt.public.delivery.work_1',
                  serviceRefs: ['service.public.autopilot'],
                  subjectRefs: ['artifact.public.work_1.diff_summary'],
                  transitionKind: 'delivery',
                  verificationRefs: ['verification.public.bun_test'],
                },
              ],
              snapshotRef: 'artifact-receipt-index-snapshot.public.work_1',
              versionRef: 'artifact-receipt-index-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Artifact and receipt index')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('artifact-receipt-index-snapshot.public.work_1')
    expect(rendered).toContain('artifact.public.work_1.diff_summary')
    expect(rendered).toContain('digest.public.artifact.sha256')
    expect(rendered).toContain('media-type.public.text_markdown')
    expect(rendered).toContain('producer.public.pylon.local')
    expect(rendered).toContain('policy.public.artifact.redacted')
    expect(rendered).toContain('receipt.public.delivery.work_1')
    expect(rendered).toContain('idempotency.public.delivery.work_1')
    expect(rendered).toContain('service.public.autopilot')
    expect(rendered).toContain('verification.public.bun_test')
  })

  test('omits unsafe Artifact and receipt index material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            artifactReceiptIndex: {
              artifacts: [
                {
                  artifactRef: 'artifact.public.safe',
                  digestRefs: ['digest.public.safe'],
                  freshness: 'fresh',
                  kind: 'diff',
                  mediaTypeRefs: [
                    'media-type.public.text_markdown',
                    'raw artifact /Users/christopher/a.bin',
                  ],
                  policyRefs: ['policy.public.safe'],
                  producerRefs: ['producer.public.safe', 'provider payload sk-private'],
                  redactionClass: 'public_safe',
                  relatedReceiptRefs: ['receipt.public.safe'],
                  summaryRefs: [
                    'summary.public.safe',
                    'raw patch /Users/christopher/a.diff',
                  ],
                  visibility: 'public',
                },
              ],
              blockerRefs: [
                'artifact-index-blocker.public.safe',
                'raw artifact /Users/christopher/artifact.log',
              ],
              receipts: [
                {
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.safe'],
                  inputRefs: [
                    'artifact.public.safe',
                    'raw receipt /Users/christopher/receipt.json',
                  ],
                  outputRefs: ['receipt-output.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  receiptRef: 'receipt.public.safe',
                  serviceRefs: ['service.public.safe'],
                  subjectRefs: ['artifact.public.safe'],
                  transitionKind: 'delivery',
                  verificationRefs: ['verification.public.safe'],
                },
              ],
              snapshotRef: 'artifact-receipt-index-snapshot.public.safe',
              versionRef: 'artifact-receipt-index-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Artifact and receipt index')
    expect(rendered).toContain('artifact-receipt-index-snapshot.public.safe')
    expect(rendered).toContain('artifact.public.safe')
    expect(rendered).toContain('digest.public.safe')
    expect(rendered).toContain('media-type.public.text_markdown')
    expect(rendered).toContain('producer.public.safe')
    expect(rendered).toContain('receipt.public.safe')
    expect(rendered).toContain('idempotency.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-artifact-receipt-material-omitted')
    expect(rendered).toContain('unsafe artifact/receipt ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw artifact')
    expect(rendered).not.toContain('raw patch /Users/christopher/a.diff')
    expect(rendered).not.toContain('raw receipt')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Scheduling and cron lane for public schedule snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('scheduled', null, {
            schedulingCron: {
              schedules: [
                {
                  adapterPreferenceRefs: ['adapter-preference.public.pylon.local'],
                  budgetPolicyRefs: ['budget-policy.public.overnight.cap'],
                  freshness: 'fresh',
                  nextRunRefs: ['next-run.public.2026-06-18T09:00:00Z'],
                  noDoubleFireReceiptRefs: [
                    'no-double-fire-receipt.public.schedule_1',
                  ],
                  notificationPolicyRefs: ['notification-policy.public.completion'],
                  ownerRefs: ['owner.public.user_1'],
                  permissionPolicyRefs: ['permission-policy.public.recurring.safe'],
                  providerPreferenceRefs: ['provider-preference.public.own_pylon'],
                  repoRefs: ['repo.public.openagents'],
                  retentionPolicyRefs: ['retention-policy.public.schedule_receipts'],
                  scheduleRef: 'schedule.public.work_1.overnight',
                  status: 'active',
                  teamRefs: ['team.public.autopilot'],
                  timezoneRefs: ['timezone.public.America/Chicago'],
                  triggerKind: 'recurring',
                  workOrderTemplateRefs: ['work-template.public.overnight'],
                  workspaceRefs: ['workspace.public.repo'],
                },
              ],
              snapshotRef: 'scheduling-cron-snapshot.public.work_1',
              versionRef: 'scheduling-cron-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Scheduling and cron')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('scheduling-cron-snapshot.public.work_1')
    expect(rendered).toContain('schedule.public.work_1.overnight')
    expect(rendered).toContain('next-run.public.2026-06-18T09:00:00Z')
    expect(rendered).toContain('budget-policy.public.overnight.cap')
    expect(rendered).toContain('permission-policy.public.recurring.safe')
    expect(rendered).toContain('repo.public.openagents')
    expect(rendered).toContain('notification-policy.public.completion')
    expect(rendered).toContain('no-double-fire-receipt.public.schedule_1')
  })

  test('omits unsafe Scheduling and cron material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('scheduled', null, {
            schedulingCron: {
              blockerRefs: [
                'schedule-blocker.public.safe',
                'raw schedule /Users/christopher/schedule.json',
              ],
              schedules: [
                {
                  adapterPreferenceRefs: ['adapter-preference.public.safe'],
                  budgetPolicyRefs: ['budget-policy.public.safe'],
                  freshness: 'fresh',
                  nextRunRefs: ['next-run.public.safe'],
                  notificationPolicyRefs: ['notification-policy.public.safe'],
                  ownerRefs: ['owner.public.safe', 'customer data private'],
                  permissionPolicyRefs: ['permission-policy.public.safe'],
                  providerPreferenceRefs: ['provider-preference.public.safe'],
                  repoRefs: ['repo.public.safe'],
                  retentionPolicyRefs: ['retention-policy.public.safe'],
                  scheduleRef: 'schedule.public.safe',
                  status: 'active',
                  teamRefs: ['team.public.safe'],
                  timezoneRefs: ['timezone.public.America/Chicago'],
                  triggerKind: 'one_shot',
                  workOrderTemplateRefs: [
                    'work-template.public.safe',
                    'cron body /Users/christopher/private.txt',
                  ],
                  workspaceRefs: [
                    'workspace.public.safe',
                    'provider payload sk-private',
                  ],
                },
              ],
              snapshotRef: 'scheduling-cron-snapshot.public.safe',
              versionRef: 'scheduling-cron-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Scheduling and cron')
    expect(rendered).toContain('scheduling-cron-snapshot.public.safe')
    expect(rendered).toContain('schedule.public.safe')
    expect(rendered).toContain('next-run.public.safe')
    expect(rendered).toContain('budget-policy.public.safe')
    expect(rendered).toContain('permission-policy.public.safe')
    expect(rendered).toContain('workspace.public.safe')
    expect(rendered).toContain('unsafe-scheduling-cron-material-omitted')
    expect(rendered).toContain('unsafe scheduling/cron ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw schedule')
    expect(rendered).not.toContain('cron body /Users/christopher/private.txt')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('customer data')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Structured event log lane for public event snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            structuredEventLog: {
              eventStreamRefs: ['event-stream.public.work_1.structured'],
              events: [
                {
                  actorRefs: ['actor.public.agent'],
                  correlationRefs: ['correlation.public.work_1'],
                  eventKind: 'status_transition',
                  eventRef: 'event.public.work_1.1',
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.event_1'],
                  occurredAt: '2026-06-18T02:39:00.000Z',
                  parentRefs: ['event-parent.public.root'],
                  payloadSchemaVersionRefs: [
                    'schema.public.event.status_transition.v1',
                  ],
                  policyRefs: ['policy.public.event_log.redacted'],
                  projectionRefs: ['event-projection.public.run_detail'],
                  redactionClass: 'public_safe',
                  replayRefs: ['event-replay.public.event_1'],
                  retentionRefs: ['retention.public.event_log'],
                  runRefs: ['run.public.work_1'],
                  sequence: 1,
                  sequenceRef: 'event-sequence.public.work_1.1',
                  serviceRefs: ['service.public.autopilot'],
                  status: 'appended',
                  subjectRefs: ['work-order.public.work_1'],
                  timestampRefs: ['timestamp.public.event_1'],
                  visibility: 'public',
                },
              ],
              exportRefs: ['event-export.public.support_bundle'],
              policyRefs: ['policy.public.event_log.redacted'],
              projectionRefs: ['event-projection.public.run_detail'],
              replayRefs: ['event-replay.public.deterministic'],
              retentionRefs: ['retention.public.event_log'],
              snapshotRef: 'structured-event-log-snapshot.public.work_1',
              versionRef: 'structured-event-log-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Structured event log')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('structured-event-log-snapshot.public.work_1')
    expect(rendered).toContain('event-stream.public.work_1.structured')
    expect(rendered).toContain('event.public.work_1.1')
    expect(rendered).toContain('schema.public.event.status_transition.v1')
    expect(rendered).toContain('idempotency.public.event_1')
    expect(rendered).toContain('event-replay.public.event_1')
    expect(rendered).toContain('event-export.public.support_bundle')
    expect(rendered).toContain('retention.public.event_log')
  })

  test('omits unsafe Structured event log material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            structuredEventLog: {
              blockerRefs: [
                'event-log-blocker.public.safe',
                'raw event /Users/christopher/event.json',
              ],
              eventStreamRefs: ['event-stream.public.safe'],
              events: [
                {
                  actorRefs: ['actor.public.safe', 'customer data private'],
                  eventKind: 'status_transition',
                  eventRef: 'event.public.safe',
                  freshness: 'fresh',
                  idempotencyRefs: ['idempotency.public.safe'],
                  payloadSchemaVersionRefs: [
                    'schema.public.status_transition.v1',
                    'raw event payload /Users/christopher/payload.json',
                  ],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  redactionClass: 'public_safe',
                  runRefs: ['run.public.safe'],
                  sequence: 1,
                  serviceRefs: ['service.public.safe', 'provider payload sk-private'],
                  status: 'appended',
                  subjectRefs: [
                    'subject.public.safe',
                    'raw prompt /Users/christopher/prompt.md',
                  ],
                  visibility: 'public',
                },
              ],
              policyRefs: ['policy.public.safe'],
              snapshotRef: 'structured-event-log-snapshot.public.safe',
              versionRef: 'structured-event-log-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Structured event log')
    expect(rendered).toContain('structured-event-log-snapshot.public.safe')
    expect(rendered).toContain('event.public.safe')
    expect(rendered).toContain('schema.public.status_transition.v1')
    expect(rendered).toContain('idempotency.public.safe')
    expect(rendered).toContain('actor.public.safe')
    expect(rendered).toContain('subject.public.safe')
    expect(rendered).toContain('unsafe-structured-event-log-material-omitted')
    expect(rendered).toContain('unsafe structured event ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw event')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('customer data')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Telemetry and privacy lane for public telemetry snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            telemetryPrivacy: {
              classes: [
                {
                  aggregateRefs: ['telemetry-aggregate.public.health'],
                  classKind: 'health_events',
                  deliveryRefs: ['telemetry-delivery.public.local'],
                  exportabilityRefs: ['telemetry-exportability.public.local_bundle'],
                  freshness: 'fresh',
                  mode: 'local_only',
                  policyRefs: ['policy.public.telemetry.redacted'],
                  privacyFilterRefs: ['privacy-filter.public.public_safe_payloads'],
                  redactionScanRefs: ['redaction-scan.public.telemetry.pass'],
                  retentionRefs: ['retention.public.telemetry.30d'],
                  sinkRefs: ['telemetry-sink.public.local'],
                  status: 'enabled',
                  telemetryRef: 'telemetry.public.health',
                  visibilityRefs: ['visibility.public.local_only'],
                },
              ],
              modeRefs: ['telemetry-mode.public.local_only'],
              policyRefs: ['policy.public.telemetry.redacted'],
              privacyFilterRefs: ['privacy-filter.public.public_safe_payloads'],
              redactionScanRefs: ['redaction-scan.public.telemetry.pass'],
              retentionRefs: ['retention.public.telemetry.30d'],
              sinkRefs: ['telemetry-sink.public.local'],
              snapshotRef: 'telemetry-privacy-snapshot.public.work_1',
              versionRef: 'telemetry-privacy-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Telemetry and privacy')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('telemetry-privacy-snapshot.public.work_1')
    expect(rendered).toContain('telemetry.public.health')
    expect(rendered).toContain('telemetry-mode.public.local_only')
    expect(rendered).toContain('telemetry-sink.public.local')
    expect(rendered).toContain('policy.public.telemetry.redacted')
    expect(rendered).toContain('privacy-filter.public.public_safe_payloads')
    expect(rendered).toContain('redaction-scan.public.telemetry.pass')
    expect(rendered).toContain('retention.public.telemetry.30d')
    expect(rendered).toContain('telemetry-aggregate.public.health')
  })

  test('omits unsafe Telemetry and privacy material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            telemetryPrivacy: {
              blockerRefs: [
                'telemetry-blocker.public.safe',
                'raw telemetry /Users/christopher/telemetry.json',
              ],
              classes: [
                {
                  aggregateRefs: [
                    'telemetry-aggregate.public.safe',
                    'raw prompt /Users/christopher/prompt.md',
                  ],
                  classKind: 'health_events',
                  diagnosticBundleRefs: ['diagnostic-bundle.public.safe'],
                  deliveryRefs: ['telemetry-delivery.public.safe'],
                  exportabilityRefs: ['telemetry-exportability.public.safe'],
                  freshness: 'fresh',
                  mode: 'local_only',
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  privacyFilterRefs: ['privacy-filter.public.safe'],
                  redactionScanRefs: ['redaction-scan.public.safe'],
                  retentionRefs: ['retention.public.safe'],
                  sinkRefs: ['telemetry-sink.public.safe', 'provider payload sk-private'],
                  status: 'enabled',
                  telemetryRef: 'telemetry.public.safe',
                  visibilityRefs: ['visibility.public.safe', 'customer data private'],
                },
              ],
              modeRefs: ['telemetry-mode.public.safe'],
              policyRefs: ['policy.public.safe'],
              snapshotRef: 'telemetry-privacy-snapshot.public.safe',
              versionRef: 'telemetry-privacy-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Telemetry and privacy')
    expect(rendered).toContain('telemetry-privacy-snapshot.public.safe')
    expect(rendered).toContain('telemetry.public.safe')
    expect(rendered).toContain('telemetry-aggregate.public.safe')
    expect(rendered).toContain('telemetry-sink.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-telemetry-privacy-material-omitted')
    expect(rendered).toContain('unsafe telemetry/privacy ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw telemetry')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('customer data')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Performance diagnostics lane for public performance snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            performanceDiagnostics: {
              entries: [
                {
                  backpressureRefs: ['backpressure.public.queue.normal'],
                  counterRefs: ['counter.public.model.first_token_ms'],
                  freshness: 'fresh',
                  latencyClass: 'normal',
                  outputVolumeRefs: ['output-volume.public.bounded'],
                  policyRefs: ['policy.public.performance.redacted'],
                  redactionRefs: ['redaction.public.performance_profile'],
                  resourceClass: 'model',
                  runRefs: ['run.public.work_1'],
                  spanRef: 'performance-span.public.model.first_token',
                  status: 'ok',
                },
              ],
              profileRefs: ['performance-profile.public.redacted'],
              snapshotRef: 'performance-diagnostics-snapshot.public.work_1',
              versionRef: 'performance-diagnostics-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Performance diagnostics')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('performance-diagnostics-snapshot.public.work_1')
    expect(rendered).toContain('performance-profile.public.redacted')
    expect(rendered).toContain('performance-span.public.model.first_token')
    expect(rendered).toContain('counter.public.model.first_token_ms')
    expect(rendered).toContain('backpressure.public.queue.normal')
    expect(rendered).toContain('output-volume.public.bounded')
    expect(rendered).toContain('policy.public.performance.redacted')
  })

  test('omits unsafe Performance diagnostics material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            performanceDiagnostics: {
              blockerRefs: [
                'performance-blocker.public.safe',
                'raw output /Users/christopher/output.log',
              ],
              entries: [
                {
                  counterRefs: [
                    'counter.public.safe',
                    'raw prompt /Users/christopher/prompt.md',
                  ],
                  freshness: 'fresh',
                  latencyClass: 'normal',
                  outputVolumeRefs: ['output-volume.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  profileRefs: [
                    'performance-profile.public.safe',
                    'profile detail /Users/christopher/profile.json',
                  ],
                  redactionRefs: ['redaction.public.safe'],
                  resourceClass: 'model',
                  spanRef: 'performance-span.public.safe',
                  status: 'ok',
                  timeoutRefs: ['timeout.public.safe', 'provider payload sk-private'],
                },
              ],
              profileRefs: ['performance-profile.public.safe'],
              snapshotRef: 'performance-diagnostics-snapshot.public.safe',
              versionRef: 'performance-diagnostics-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Performance diagnostics')
    expect(rendered).toContain('performance-diagnostics-snapshot.public.safe')
    expect(rendered).toContain('performance-span.public.safe')
    expect(rendered).toContain('counter.public.safe')
    expect(rendered).toContain('timeout.public.safe')
    expect(rendered).toContain('performance-profile.public.safe')
    expect(rendered).toContain('unsafe-performance-diagnostics-material-omitted')
    expect(rendered).toContain('unsafe performance ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw output')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('profile detail')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Update and release lane for public release snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            updateRelease: {
              entries: [
                {
                  artifactRefs: ['release-artifact.public.darwin_arm64'],
                  channel: 'stable',
                  channelRefs: ['release-channel.public.stable'],
                  checksumRefs: ['release-checksum.public.v1_3_0'],
                  compatibilityRefs: ['release-compat.public.runtime_v1'],
                  freshness: 'fresh',
                  manifestRefs: ['release-manifest.public.v1_3_0'],
                  platformRefs: ['release-platform.public.darwin_arm64'],
                  policyRefs: ['release-policy.public.managed'],
                  releaseNoteRefs: ['release-notes.public.v1_3_0'],
                  releaseRef: 'release.public.v1_3_0',
                  rollbackRefs: ['release-rollback.public.v1_2_9'],
                  rolloutRefs: ['release-rollout.public.stable_10pct'],
                  runtimeRequirementRefs: ['release-runtime.public.node_bun'],
                  signatureRefs: ['release-signature.public.v1_3_0'],
                  smokeReceiptRefs: ['release-smoke.public.v1_3_0'],
                  status: 'recommended',
                  supportRefs: ['release-support.public.v1_3_0'],
                  versionRef: 'release-version.public.v1_3_0',
                },
              ],
              manifestRefs: ['release-manifest.public.v1_3_0'],
              policyRefs: ['release-policy.public.managed'],
              snapshotRef: 'update-release-snapshot.public.work_1',
              versionRef: 'update-release-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Update and release')
    expect(rendered).toContain('update available')
    expect(rendered).toContain('update-release-snapshot.public.work_1')
    expect(rendered).toContain('release.public.v1_3_0')
    expect(rendered).toContain('release-version.public.v1_3_0')
    expect(rendered).toContain('release-manifest.public.v1_3_0')
    expect(rendered).toContain('release-checksum.public.v1_3_0')
    expect(rendered).toContain('release-signature.public.v1_3_0')
    expect(rendered).toContain('release-smoke.public.v1_3_0')
    expect(rendered).toContain('release-policy.public.managed')
  })

  test('omits unsafe Update and release material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            updateRelease: {
              blockerRefs: [
                'release-blocker.public.safe',
                'raw manifest /Users/christopher/release.json',
              ],
              entries: [
                {
                  artifactRefs: [
                    'release-artifact.public.safe',
                    'artifact payload /Users/christopher/pkg.tgz',
                  ],
                  channel: 'stable',
                  checksumRefs: ['release-checksum.public.safe'],
                  compatibilityRefs: ['release-compat.public.safe'],
                  freshness: 'fresh',
                  manifestRefs: [
                    'release-manifest.public.safe',
                    'manifest body bearer token private',
                  ],
                  platformRefs: ['release-platform.public.safe'],
                  policyRefs: ['release-policy.public.safe', 'credential private'],
                  releaseNoteRefs: [
                    'release-note.public.safe',
                    'release note body /Users/christopher/notes.md',
                  ],
                  releaseRef: 'release.public.safe',
                  signatureRefs: [
                    'release-signature.public.safe',
                    'provider payload sk-private',
                  ],
                  smokeReceiptRefs: ['release-smoke.public.safe'],
                  status: 'recommended',
                  versionRef: 'release-version.public.safe',
                },
              ],
              manifestRefs: ['release-manifest.public.safe'],
              policyRefs: ['release-policy.public.safe'],
              snapshotRef: 'update-release-snapshot.public.safe',
              versionRef: 'update-release-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Update and release')
    expect(rendered).toContain('update-release-snapshot.public.safe')
    expect(rendered).toContain('release.public.safe')
    expect(rendered).toContain('release-manifest.public.safe')
    expect(rendered).toContain('release-artifact.public.safe')
    expect(rendered).toContain('release-policy.public.safe')
    expect(rendered).toContain('unsafe-update-release-material-omitted')
    expect(rendered).toContain('unsafe update/release ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw manifest')
    expect(rendered).not.toContain('artifact payload')
    expect(rendered).not.toContain('manifest body')
    expect(rendered).not.toContain('release note body')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('credential private')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Migration evidence lane for public migration snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            migrationEvidence: {
              entries: [
                {
                  domain: 'settings',
                  domainRef: 'migration-domain.public.settings',
                  freshness: 'fresh',
                  idempotencyRefs: ['migration-idempotency.public.settings'],
                  migrationRefs: ['migration-step.public.settings.v1_to_v2'],
                  policyRefs: ['migration-policy.public.redacted'],
                  receiptRefs: ['migration-receipt.public.settings.v2'],
                  redactionRefs: ['migration-redaction.public.settings'],
                  registryRefs: ['migration-registry.public.v1'],
                  required: true,
                  restorePointRefs: ['migration-restore.public.settings.v1'],
                  rollbackBoundaryRefs: [
                    'migration-rollback-boundary.public.settings.v1',
                  ],
                  schemaFromRef: 'schema.public.settings.v1',
                  schemaToRef: 'schema.public.settings.v2',
                  status: 'completed',
                  validationRefs: ['migration-validation.public.settings.v2'],
                },
              ],
              registryRefs: ['migration-registry.public.v1'],
              snapshotRef: 'migration-evidence-snapshot.public.work_1',
              versionRef: 'migration-evidence-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Migration evidence')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('migration-evidence-snapshot.public.work_1')
    expect(rendered).toContain('migration-domain.public.settings')
    expect(rendered).toContain('schema.public.settings.v1')
    expect(rendered).toContain('schema.public.settings.v2')
    expect(rendered).toContain('migration-step.public.settings.v1_to_v2')
    expect(rendered).toContain('migration-restore.public.settings.v1')
    expect(rendered).toContain('migration-validation.public.settings.v2')
    expect(rendered).toContain('migration-receipt.public.settings.v2')
  })

  test('omits unsafe Migration evidence material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            migrationEvidence: {
              blockerRefs: [
                'migration-blocker.public.safe',
                'raw fixture /Users/christopher/state.json',
              ],
              entries: [
                {
                  domain: 'settings',
                  domainRef: 'migration-domain.public.safe',
                  freshness: 'fresh',
                  idempotencyRefs: [
                    'migration-idempotency.public.safe',
                    'credential value password private',
                  ],
                  migrationRefs: [
                    'migration-step.public.safe',
                    'state payload /Users/christopher/state.json',
                  ],
                  policyRefs: [
                    'migration-policy.public.safe',
                    'bearer token private',
                  ],
                  receiptRefs: ['migration-receipt.public.safe'],
                  redactionRefs: ['migration-redaction.public.safe'],
                  registryRefs: [
                    'migration-registry.public.safe',
                    'provider payload sk-private',
                  ],
                  required: true,
                  restorePointRefs: ['migration-restore.public.safe'],
                  rollbackBoundaryRefs: ['migration-rollback-boundary.public.safe'],
                  schemaFromRef: 'schema.public.safe.v1',
                  schemaToRef: 'schema.public.safe.v2',
                  status: 'completed',
                  validationRefs: ['migration-validation.public.safe'],
                },
              ],
              registryRefs: ['migration-registry.public.safe'],
              snapshotRef: 'migration-evidence-snapshot.public.safe',
              versionRef: 'migration-evidence-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Migration evidence')
    expect(rendered).toContain('migration-evidence-snapshot.public.safe')
    expect(rendered).toContain('migration-domain.public.safe')
    expect(rendered).toContain('migration-step.public.safe')
    expect(rendered).toContain('migration-policy.public.safe')
    expect(rendered).toContain('migration-receipt.public.safe')
    expect(rendered).toContain('unsafe-migration-evidence-material-omitted')
    expect(rendered).toContain('unsafe migration ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw fixture')
    expect(rendered).not.toContain('credential value')
    expect(rendered).not.toContain('state payload')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Testing and smoke lane for public smoke snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            testingSmokeEvidence: {
              entries: [
                {
                  classifications: ['ci_safe', 'no_spend'],
                  commandRefs: ['test-command.public.bun_unit'],
                  environmentRefs: ['test-env.public.ci'],
                  fixtureRefs: ['fixture.public.redacted.pack_a'],
                  freshness: 'fresh',
                  layer: 'ci_smoke',
                  policyRefs: ['test-policy.public.no_live_write'],
                  proofBoundaryRefs: ['proof-boundary.public.schema_reducers_only'],
                  redactionScanRefs: ['redaction-scan.public.test_output'],
                  smokeReceiptRefs: ['smoke-receipt.public.bun_unit'],
                  status: 'passed',
                  testRef: 'test.public.bun_unit',
                  versionRefs: ['test-version.public.v1'],
                },
              ],
              snapshotRef: 'testing-smoke-snapshot.public.work_1',
              versionRef: 'testing-smoke-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Testing and smoke')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('testing-smoke-snapshot.public.work_1')
    expect(rendered).toContain('test.public.bun_unit')
    expect(rendered).toContain('test-command.public.bun_unit')
    expect(rendered).toContain('fixture.public.redacted.pack_a')
    expect(rendered).toContain('proof-boundary.public.schema_reducers_only')
    expect(rendered).toContain('smoke-receipt.public.bun_unit')
    expect(rendered).toContain('redaction-scan.public.test_output')
  })

  test('omits unsafe Testing and smoke material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            testingSmokeEvidence: {
              blockerRefs: [
                'test-blocker.public.safe',
                'raw test log /Users/christopher/test.log',
              ],
              entries: [
                {
                  classifications: ['ci_safe', 'no_spend'],
                  commandRefs: [
                    'test-command.public.safe',
                    'raw command rm -rf /Users/christopher',
                  ],
                  environmentRefs: [
                    'test-env.public.safe',
                    'workspace path /Users/christopher/work',
                  ],
                  fixtureRefs: [
                    'fixture.public.safe',
                    'fixture body bearer token private',
                  ],
                  freshness: 'fresh',
                  layer: 'ci_smoke',
                  policyRefs: ['test-policy.public.safe'],
                  proofBoundaryRefs: ['proof-boundary.public.safe'],
                  redactionScanRefs: ['redaction-scan.public.safe'],
                  smokeReceiptRefs: [
                    'smoke-receipt.public.safe',
                    'smoke output provider payload sk-private',
                  ],
                  status: 'passed',
                  testRef: 'test.public.safe',
                },
              ],
              snapshotRef: 'testing-smoke-snapshot.public.safe',
              versionRef: 'testing-smoke-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Testing and smoke')
    expect(rendered).toContain('testing-smoke-snapshot.public.safe')
    expect(rendered).toContain('test.public.safe')
    expect(rendered).toContain('test-command.public.safe')
    expect(rendered).toContain('fixture.public.safe')
    expect(rendered).toContain('smoke-receipt.public.safe')
    expect(rendered).toContain('unsafe-testing-smoke-material-omitted')
    expect(rendered).toContain('unsafe testing/smoke ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw test log')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('workspace path')
    expect(rendered).not.toContain('fixture body')
    expect(rendered).not.toContain('smoke output')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Evaluation and regression lane for public eval snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            evaluationRegressionEvidence: {
              entries: [
                {
                  adapterRefs: ['adapter.public.pylon'],
                  artifactRefs: ['eval-artifact.public.summary'],
                  budgetPolicyRefs: ['budget-policy.public.equivalent'],
                  costSummaryRefs: ['eval-cost.public.summary'],
                  evaluationRef: 'evaluation.public.suite_small',
                  firstDivergenceRefs: ['first-divergence.public.none'],
                  fixtureProvenanceRefs: [
                    'fixture-provenance.public.redacted_failure',
                  ],
                  fixtureRedactionRefs: ['fixture-redaction.public.reviewed'],
                  fixtureRefs: ['fixture.public.redacted_task'],
                  freshness: 'fresh',
                  latencySummaryRefs: ['eval-latency.public.summary'],
                  modelRefs: ['model.public.gpt'],
                  productClaimRefs: ['product-claim.public.bounded_eval'],
                  providerRefs: ['provider.public.openai'],
                  publicReportRefs: ['eval-report.public.summary'],
                  resultVerdictRefs: ['eval-result.public.solved'],
                  safetyVerdictRefs: ['eval-safety.public.public_safe'],
                  status: 'passed',
                  suiteRefs: ['eval-suite.public.small'],
                  toolPolicyRefs: ['tool-policy.public.equivalent'],
                  versionRefs: ['runtime-version.public.v1'],
                },
              ],
              snapshotRef: 'evaluation-regression-snapshot.public.work_1',
              versionRef: 'evaluation-regression-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Evaluation and regression')
    expect(rendered).toContain('passed')
    expect(rendered).toContain('evaluation-regression-snapshot.public.work_1')
    expect(rendered).toContain('evaluation.public.suite_small')
    expect(rendered).toContain('eval-suite.public.small')
    expect(rendered).toContain('fixture.public.redacted_task')
    expect(rendered).toContain('first-divergence.public.none')
    expect(rendered).toContain('eval-report.public.summary')
    expect(rendered).toContain('eval-safety.public.public_safe')
  })

  test('omits unsafe Evaluation and regression material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            evaluationRegressionEvidence: {
              blockerRefs: [
                'eval-blocker.public.safe',
                'raw transcript /Users/christopher/eval.log',
              ],
              entries: [
                {
                  adapterRefs: ['adapter.public.safe'],
                  artifactRefs: [
                    'eval-artifact.public.safe',
                    'artifact content /Users/christopher/artifact.json',
                  ],
                  budgetPolicyRefs: ['budget-policy.public.safe'],
                  evaluationRef: 'evaluation.public.safe',
                  firstDivergenceRefs: ['first-divergence.public.safe'],
                  fixtureProvenanceRefs: ['fixture-provenance.public.safe'],
                  fixtureRedactionRefs: ['fixture-redaction.public.safe'],
                  fixtureRefs: [
                    'fixture.public.safe',
                    'fixture body customer data private',
                  ],
                  freshness: 'fresh',
                  modelRefs: ['model.public.safe'],
                  privateReportRefs: [
                    'eval-report.private.safe',
                    'provider payload sk-private',
                  ],
                  providerRefs: ['provider.public.safe'],
                  publicReportRefs: ['eval-report.public.safe'],
                  resultVerdictRefs: ['eval-result.public.safe'],
                  safetyVerdictRefs: ['eval-safety.public.safe'],
                  status: 'passed',
                  suiteRefs: [
                    'eval-suite.public.safe',
                    'task body bearer token private',
                  ],
                  toolPolicyRefs: ['tool-policy.public.safe'],
                },
              ],
              snapshotRef: 'evaluation-regression-snapshot.public.safe',
              versionRef: 'evaluation-regression-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Evaluation and regression')
    expect(rendered).toContain('evaluation-regression-snapshot.public.safe')
    expect(rendered).toContain('evaluation.public.safe')
    expect(rendered).toContain('eval-suite.public.safe')
    expect(rendered).toContain('fixture.public.safe')
    expect(rendered).toContain('eval-report.public.safe')
    expect(rendered).toContain('unsafe-evaluation-regression-material-omitted')
    expect(rendered).toContain('unsafe evaluation/regression ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw transcript')
    expect(rendered).not.toContain('artifact content')
    expect(rendered).not.toContain('fixture body')
    expect(rendered).not.toContain('customer data')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('task body')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Security review lane for public security snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            securityReviewEvidence: {
              entries: [
                {
                  approvalGateRefs: ['security-gate.public.shell.write'],
                  denialReceiptRefs: ['security-receipt.public.no_denial'],
                  domain: 'shell_execution',
                  domainRef: 'security-domain.public.shell',
                  freshness: 'fresh',
                  ownerPolicyRefs: ['security-policy.public.shell'],
                  redactionScanRefs: ['security-redaction.public.shell'],
                  regressionFixtureRefs: ['security-fixture.public.shell'],
                  risk: 'high',
                  status: 'approved',
                  threatModelRefs: ['threat-model.public.shell'],
                },
              ],
              snapshotRef: 'security-review-snapshot.public.work_1',
              versionRef: 'security-review-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Security review')
    expect(rendered).toContain('approved')
    expect(rendered).toContain('security-review-snapshot.public.work_1')
    expect(rendered).toContain('security-domain.public.shell')
    expect(rendered).toContain('threat-model.public.shell')
    expect(rendered).toContain('security-policy.public.shell')
    expect(rendered).toContain('security-gate.public.shell.write')
    expect(rendered).toContain('security-redaction.public.shell')
    expect(rendered).toContain('security-fixture.public.shell')
  })

  test('omits unsafe Security review material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            securityReviewEvidence: {
              blockerRefs: [
                'security-blocker.public.safe',
                'raw secret /Users/christopher/secret.txt',
              ],
              entries: [
                {
                  approvalGateRefs: [
                    'security-gate.public.safe',
                    'credential value password private',
                  ],
                  denialReceiptRefs: ['security-receipt.public.safe'],
                  diagnosticBundleRefs: [
                    'security-diagnostic.public.safe',
                    'diagnostic content /Users/christopher/diag.json',
                  ],
                  domain: 'shell_execution',
                  domainRef: 'security-domain.public.safe',
                  freshness: 'fresh',
                  ownerPolicyRefs: ['security-policy.public.safe'],
                  redactionScanRefs: [
                    'security-redaction.public.safe',
                    'provider payload sk-private',
                  ],
                  regressionFixtureRefs: ['security-fixture.public.safe'],
                  risk: 'high',
                  status: 'approved',
                  threatModelRefs: [
                    'threat-model.public.safe',
                    'shell log bearer token private',
                  ],
                },
              ],
              snapshotRef: 'security-review-snapshot.public.safe',
              versionRef: 'security-review-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Security review')
    expect(rendered).toContain('security-review-snapshot.public.safe')
    expect(rendered).toContain('security-domain.public.safe')
    expect(rendered).toContain('security-gate.public.safe')
    expect(rendered).toContain('security-diagnostic.public.safe')
    expect(rendered).toContain('security-redaction.public.safe')
    expect(rendered).toContain('unsafe-security-review-material-omitted')
    expect(rendered).toContain('unsafe security review ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw secret')
    expect(rendered).not.toContain('credential value')
    expect(rendered).not.toContain('diagnostic content')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('shell log')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Data retention and deletion lane for public lifecycle snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            dataRetentionDeletionEvidence: {
              entries: [
                {
                  dataClass: 'memory_records',
                  dataClassRef: 'data-class.public.memory_records',
                  exportManifestRefs: ['export-manifest.public.memory_records'],
                  freshness: 'fresh',
                  retentionPolicyRefs: ['retention-policy.public.memory_records'],
                  retentionSweepRefs: ['retention-sweep.public.memory_records'],
                  status: 'retained',
                },
              ],
              snapshotRef: 'retention-deletion-snapshot.public.work_1',
              versionRef: 'retention-deletion-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Data retention and deletion')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('retention-deletion-snapshot.public.work_1')
    expect(rendered).toContain('data-class.public.memory_records')
    expect(rendered).toContain('retention-policy.public.memory_records')
    expect(rendered).toContain('export-manifest.public.memory_records')
    expect(rendered).toContain('retention-sweep.public.memory_records')
  })

  test('omits unsafe Data retention and deletion material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            dataRetentionDeletionEvidence: {
              blockerRefs: [
                'retention-blocker.public.safe',
                'raw deleted payload /Users/christopher/deleted.json',
              ],
              entries: [
                {
                  blockerRefs: ['retention-entry-blocker.public.safe'],
                  dataClass: 'memory_records',
                  dataClassRef: 'data-class.public.safe_memory',
                  deletionReceiptRefs: [
                    'deletion-receipt.public.safe',
                    'event payload private',
                  ],
                  deletionRequestRefs: ['deletion-request.public.safe'],
                  exportManifestRefs: [
                    'export-manifest.public.safe',
                    'export content secret',
                  ],
                  freshness: 'fresh',
                  projectionFreshnessRefs: ['projection-freshness.public.safe'],
                  projectionInvalidationRefs: ['projection-invalidation.public.safe'],
                  retentionPolicyRefs: [
                    'retention-policy.public.safe',
                    'credential value password private',
                  ],
                  status: 'retained',
                  tombstoneRefs: [
                    'tombstone.public.safe',
                    'cache content /Users/christopher/cache',
                  ],
                },
              ],
              snapshotRef: 'retention-deletion-snapshot.public.safe',
              versionRef: 'retention-deletion-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Data retention and deletion')
    expect(rendered).toContain('retention-deletion-snapshot.public.safe')
    expect(rendered).toContain('data-class.public.safe_memory')
    expect(rendered).toContain('retention-policy.public.safe')
    expect(rendered).toContain('export-manifest.public.safe')
    expect(rendered).toContain('deletion-receipt.public.safe')
    expect(rendered).toContain('tombstone.public.safe')
    expect(rendered).toContain('unsafe-data-retention-deletion-material-omitted')
    expect(rendered).toContain('unsafe retention/deletion ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw deleted payload')
    expect(rendered).not.toContain('event payload')
    expect(rendered).not.toContain('export content')
    expect(rendered).not.toContain('credential value')
    expect(rendered).not.toContain('cache content')
    expect(rendered).not.toContain('password')
    expect(rendered).not.toContain('secret')
  })

  test('renders Onboarding capability lane for public setup snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            onboardingCapabilityEvidence: {
              entries: [
                {
                  capabilityProbeRefs: ['capability-probe.public.repo_profile'],
                  completionReceiptRefs: ['onboarding-completion.public.repo_profile'],
                  dataScopeRefs: ['data-scope.public.repo_refs_only'],
                  freshness: 'fresh',
                  instructionRefs: ['instructions.public.project_refs'],
                  invariantRefs: ['invariants.public.workspace_contract'],
                  mode: 'local_only',
                  permissionDecisionRefs: ['permission.public.repo_read_refs'],
                  repositoryProfileRefs: ['repo-profile.public.openagents'],
                  status: 'ready',
                  stepKind: 'repository_profile',
                  stepRef: 'onboarding-step.public.repo_profile',
                  workspaceRefs: ['workspace.public.openagents'],
                },
              ],
              snapshotRef: 'onboarding-capability-snapshot.public.work_1',
              versionRef: 'onboarding-capability-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Onboarding capability')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('onboarding-capability-snapshot.public.work_1')
    expect(rendered).toContain('onboarding-step.public.repo_profile')
    expect(rendered).toContain('capability-probe.public.repo_profile')
    expect(rendered).toContain('repo-profile.public.openagents')
    expect(rendered).toContain('instructions.public.project_refs')
    expect(rendered).toContain('invariants.public.workspace_contract')
    expect(rendered).toContain('permission.public.repo_read_refs')
    expect(rendered).toContain('data-scope.public.repo_refs_only')
  })

  test('omits unsafe Onboarding capability material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            onboardingCapabilityEvidence: {
              blockerRefs: [
                'onboarding-blocker.public.safe',
                'raw secret /Users/christopher/secret.txt',
              ],
              entries: [
                {
                  capabilityProbeRefs: [
                    'capability-probe.public.safe',
                    'raw device id private',
                  ],
                  completionReceiptRefs: ['onboarding-completion.public.safe'],
                  dataScopeRefs: [
                    'data-scope.public.safe',
                    'workspace path /Users/christopher/work',
                  ],
                  firstRunSmokeRefs: [
                    'first-smoke.public.safe',
                    'smoke log bearer token private',
                  ],
                  freshness: 'fresh',
                  instructionRefs: [
                    'instructions.public.safe',
                    'instruction body secret',
                  ],
                  integrationRefs: [
                    'integration.public.safe',
                    'integration payload sk-private',
                  ],
                  invariantRefs: ['invariants.public.safe'],
                  mode: 'local_only',
                  permissionDecisionRefs: ['permission.public.safe'],
                  repositoryProfileRefs: [
                    'repo-profile.public.safe',
                    'repository private data',
                  ],
                  status: 'ready',
                  stepKind: 'repository_profile',
                  stepRef: 'onboarding-step.public.safe',
                  userDeviceRefs: ['device.public.safe', 'user email private'],
                  workspaceRefs: ['workspace.public.safe'],
                },
              ],
              snapshotRef: 'onboarding-capability-snapshot.public.safe',
              versionRef: 'onboarding-capability-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Onboarding capability')
    expect(rendered).toContain('onboarding-capability-snapshot.public.safe')
    expect(rendered).toContain('onboarding-step.public.safe')
    expect(rendered).toContain('capability-probe.public.safe')
    expect(rendered).toContain('first-smoke.public.safe')
    expect(rendered).toContain('instructions.public.safe')
    expect(rendered).toContain('integration.public.safe')
    expect(rendered).toContain('repo-profile.public.safe')
    expect(rendered).toContain('device.public.safe')
    expect(rendered).toContain('unsafe-onboarding-capability-material-omitted')
    expect(rendered).toContain('unsafe onboarding ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw secret')
    expect(rendered).not.toContain('raw device id')
    expect(rendered).not.toContain('workspace path')
    expect(rendered).not.toContain('smoke log')
    expect(rendered).not.toContain('instruction body')
    expect(rendered).not.toContain('integration payload')
    expect(rendered).not.toContain('repository private data')
    expect(rendered).not.toContain('user email')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Output style and persona lane for public style snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            outputStylePersonaEvidence: {
              entries: [
                {
                  citationRequirementRefs: ['citation-policy.public.receipts_required'],
                  domainMode: 'review',
                  evidenceRequirementRefs: ['evidence-policy.public.review_claims'],
                  finalAnswerExpectationRefs: [
                    'final-answer.public.review_findings_first',
                  ],
                  formattingRefs: ['formatting.public.markdown_bullets'],
                  freshness: 'fresh',
                  productDefaultRefs: ['product-style.public.engineering_plain'],
                  projectConstraintRefs: ['project-style.public.openagents'],
                  safetyPolicyRefs: ['safety-policy.public.style_cannot_override'],
                  status: 'ready',
                  stylePolicyRef: 'style-policy.public.review',
                  toolAuthorityBoundaryRefs: ['tool-authority.public.style_no_change'],
                  userPreferenceRefs: ['style-preference.public.concise'],
                  verbosity: 'concise',
                },
              ],
              snapshotRef: 'output-style-snapshot.public.work_1',
              versionRef: 'output-style-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Output style and persona')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('output-style-snapshot.public.work_1')
    expect(rendered).toContain('style-policy.public.review')
    expect(rendered).toContain('style-preference.public.concise')
    expect(rendered).toContain('product-style.public.engineering_plain')
    expect(rendered).toContain('project-style.public.openagents')
    expect(rendered).toContain('formatting.public.markdown_bullets')
    expect(rendered).toContain('final-answer.public.review_findings_first')
    expect(rendered).toContain('evidence-policy.public.review_claims')
    expect(rendered).toContain('tool-authority.public.style_no_change')
  })

  test('omits unsafe Output style and persona material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            outputStylePersonaEvidence: {
              blockerRefs: [
                'style-blocker.public.safe',
                'raw prompt /Users/christopher/prompt.md',
              ],
              entries: [
                {
                  audienceRefs: [
                    'audience.public.engineering',
                    'private user preference',
                  ],
                  claimReceiptRefs: [
                    'claim-receipt.public.safe',
                    'capability claim private',
                  ],
                  conflictResolutionRefs: ['style-conflict.public.safe'],
                  disallowedClaimRefs: ['claim.public.safe'],
                  domainMode: 'review',
                  evidenceRequirementRefs: ['evidence-policy.public.safe'],
                  finalAnswerExpectationRefs: ['final-answer.public.safe'],
                  formattingRefs: [
                    'formatting.public.safe',
                    'raw output body secret',
                  ],
                  freshness: 'fresh',
                  overrideRefs: [
                    'style-override.public.safe',
                    'secret-bearing-override password',
                  ],
                  personaConstraintRefs: [
                    'persona.public.safe',
                    'persona text hidden chain',
                  ],
                  productDefaultRefs: ['product-style.public.safe'],
                  projectConstraintRefs: [
                    'project-style.public.safe',
                    'project instruction private',
                  ],
                  safetyPolicyRefs: ['safety-policy.public.safe'],
                  status: 'ready',
                  styleAuditRefs: [
                    'style-audit.public.safe',
                    'hidden chain state private',
                  ],
                  stylePolicyRef: 'style-policy.public.safe',
                  toolAuthorityBoundaryRefs: ['tool-authority.public.safe'],
                  userPreferenceRefs: ['style-preference.public.safe'],
                  verbosity: 'concise',
                },
              ],
              snapshotRef: 'output-style-snapshot.public.safe',
              versionRef: 'output-style-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Output style and persona')
    expect(rendered).toContain('output-style-snapshot.public.safe')
    expect(rendered).toContain('style-policy.public.safe')
    expect(rendered).toContain('audience.public.engineering')
    expect(rendered).toContain('claim-receipt.public.safe')
    expect(rendered).toContain('formatting.public.safe')
    expect(rendered).toContain('style-override.public.safe')
    expect(rendered).toContain('persona.public.safe')
    expect(rendered).toContain('project-style.public.safe')
    expect(rendered).toContain('style-audit.public.safe')
    expect(rendered).toContain('unsafe-output-style-persona-material-omitted')
    expect(rendered).toContain('unsafe output style ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('private user preference')
    expect(rendered).not.toContain('capability claim')
    expect(rendered).not.toContain('raw output body')
    expect(rendered).not.toContain('secret-bearing-override')
    expect(rendered).not.toContain('persona text')
    expect(rendered).not.toContain('project instruction')
    expect(rendered).not.toContain('hidden chain')
    expect(rendered).not.toContain('password')
    expect(rendered).not.toContain('secret')
  })

  test('renders Prompt suggestions lane for public suggestion snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            promptSuggestionsEvidence: {
              entries: [
                {
                  confidenceRefs: ['suggestion-confidence.public.high'],
                  displayRefs: ['suggestion-display.public.file_name'],
                  expirationRefs: ['suggestion-expiration.public.fresh'],
                  freshness: 'fresh',
                  insertTextRefs: ['suggestion-insert.public.file_ref'],
                  kind: 'file',
                  privacy: 'scoped_private',
                  privacyRefs: ['suggestion-privacy.public.current_workspace'],
                  provenanceRefs: ['suggestion-provenance.public.workspace_index'],
                  rankingRefs: ['suggestion-ranking.public.path_and_recency'],
                  scopeRefs: ['suggestion-scope.public.current_workspace'],
                  status: 'ready',
                  suggestionRef: 'suggestion.public.file_readme',
                  validationRefs: ['suggestion-validation.public.file_exists'],
                },
              ],
              snapshotRef: 'prompt-suggestions-snapshot.public.work_1',
              versionRef: 'prompt-suggestions-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Prompt suggestions')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('prompt-suggestions-snapshot.public.work_1')
    expect(rendered).toContain('suggestion.public.file_readme')
    expect(rendered).toContain('suggestion-display.public.file_name')
    expect(rendered).toContain('suggestion-insert.public.file_ref')
    expect(rendered).toContain('suggestion-scope.public.current_workspace')
    expect(rendered).toContain('suggestion-privacy.public.current_workspace')
    expect(rendered).toContain('suggestion-ranking.public.path_and_recency')
  })

  test('omits unsafe Prompt suggestions material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            promptSuggestionsEvidence: {
              blockerRefs: [
                'suggestion-blocker.public.safe',
                'raw prompt text /Users/christopher/prompt.md',
              ],
              entries: [
                {
                  actionRef: 'suggestion-action.public.safe',
                  actionSeparationRefs: ['action-separation.public.safe'],
                  auditRefs: [
                    'suggestion-audit.public.safe',
                    'unvalidated model output private',
                  ],
                  confidenceRefs: ['suggestion-confidence.public.safe'],
                  displayRefs: [
                    'suggestion-display.public.safe',
                    'repository private data',
                  ],
                  expirationRefs: ['suggestion-expiration.public.safe'],
                  freshness: 'fresh',
                  insertTextRefs: [
                    'suggestion-insert.public.safe',
                    'inserted text body secret',
                  ],
                  kind: 'file',
                  permissionRefs: ['permission.public.safe'],
                  privacy: 'scoped_private',
                  privacyRefs: ['suggestion-privacy.public.safe'],
                  provenanceRefs: ['suggestion-provenance.public.safe'],
                  rankingRefs: ['suggestion-ranking.public.safe'],
                  scopeRefs: [
                    'suggestion-scope.public.safe',
                    'raw file /Users/christopher/file.ts',
                  ],
                  status: 'ready',
                  suggestionRef: 'suggestion.public.safe',
                  validationRefs: ['suggestion-validation.public.safe'],
                },
              ],
              snapshotRef: 'prompt-suggestions-snapshot.public.safe',
              versionRef: 'prompt-suggestions-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Prompt suggestions')
    expect(rendered).toContain('prompt-suggestions-snapshot.public.safe')
    expect(rendered).toContain('suggestion.public.safe')
    expect(rendered).toContain('suggestion-action.public.safe')
    expect(rendered).toContain('suggestion-audit.public.safe')
    expect(rendered).toContain('suggestion-display.public.safe')
    expect(rendered).toContain('suggestion-insert.public.safe')
    expect(rendered).toContain('suggestion-scope.public.safe')
    expect(rendered).toContain('unsafe-prompt-suggestions-material-omitted')
    expect(rendered).toContain('unsafe prompt suggestion ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw prompt text')
    expect(rendered).not.toContain('unvalidated model output')
    expect(rendered).not.toContain('repository private data')
    expect(rendered).not.toContain('inserted text body')
    expect(rendered).not.toContain('raw file')
    expect(rendered).not.toContain('secret')
  })

  test('renders Tips and education lane for public education snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            tipsEducationEvidence: {
              entries: [
                {
                  audienceRefs: ['education-audience.public.first_run'],
                  capabilityRefs: ['capability.public.review_mode'],
                  docsRefs: ['docs.public.review_mode'],
                  freshness: 'fresh',
                  liveStateRefs: ['live-state.public.review_mode_ready'],
                  scopeRefs: ['education-scope.public.current_run'],
                  status: 'ready',
                  tipRef: 'tip.public.review_mode',
                  topic: 'capability',
                  triggerRefs: ['tip-trigger.public.first_run'],
                  versionRefs: ['tip-version.public.v1'],
                },
              ],
              snapshotRef: 'tips-education-snapshot.public.work_1',
              versionRef: 'tips-education-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Tips and education')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('tips-education-snapshot.public.work_1')
    expect(rendered).toContain('tip.public.review_mode')
    expect(rendered).toContain('capability.public.review_mode')
    expect(rendered).toContain('live-state.public.review_mode_ready')
    expect(rendered).toContain('docs.public.review_mode')
    expect(rendered).toContain('tip-trigger.public.first_run')
  })

  test('omits unsafe Tips and education material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            tipsEducationEvidence: {
              blockerRefs: [
                'tips-blocker.public.safe',
                'raw tip copy /Users/christopher/tip.md',
              ],
              entries: [
                {
                  audienceRefs: ['education-audience.public.safe'],
                  capabilityRefs: ['capability.public.safe'],
                  caveatRefs: ['caveat.public.safe', 'payment payload private'],
                  docsRefs: ['docs.public.safe', 'docs content secret'],
                  freshness: 'fresh',
                  helpTopicRefs: [
                    'help-topic.public.safe',
                    'secret-bearing-help payload',
                  ],
                  liveStateRefs: ['live-state.public.safe'],
                  requiredWarningRefs: ['warning.public.safe'],
                  scopeRefs: ['education-scope.public.safe'],
                  status: 'ready',
                  tipRef: 'tip.public.safe',
                  topic: 'capability',
                  triggerRefs: ['tip-trigger.public.safe', 'raw run data private'],
                  versionRefs: ['tip-version.public.safe'],
                },
              ],
              snapshotRef: 'tips-education-snapshot.public.safe',
              versionRef: 'tips-education-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Tips and education')
    expect(rendered).toContain('tips-education-snapshot.public.safe')
    expect(rendered).toContain('tip.public.safe')
    expect(rendered).toContain('caveat.public.safe')
    expect(rendered).toContain('docs.public.safe')
    expect(rendered).toContain('help-topic.public.safe')
    expect(rendered).toContain('tip-trigger.public.safe')
    expect(rendered).toContain('unsafe-tips-education-material-omitted')
    expect(rendered).toContain('unsafe tips/education ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw tip copy')
    expect(rendered).not.toContain('payment payload')
    expect(rendered).not.toContain('docs content')
    expect(rendered).not.toContain('secret-bearing-help')
    expect(rendered).not.toContain('raw run data')
    expect(rendered).not.toContain('secret')
  })

  test('renders Theme and visual design lane for public visual snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            themeVisualEvidence: {
              entries: [
                {
                  contrastCheckRefs: ['contrast.public.status_pass'],
                  crossSurfaceRefs: [
                    'surface.public.terminal',
                    'surface.public.web',
                    'surface.public.mobile',
                    'surface.public.operator',
                  ],
                  freshness: 'fresh',
                  highContrastRefs: ['high-contrast.public.available'],
                  monochromeRefs: ['monochrome.public.labels_icons'],
                  reducedMotionRefs: ['motion.public.reduced'],
                  runtimeReceiptRefs: ['runtime-receipt.public.success_state'],
                  status: 'ready',
                  statusIconRefs: ['status-icon.public.success'],
                  statusLabelRefs: ['status-label.public.success'],
                  statusVisualRefs: ['status-visual.public.success_green'],
                  surface: 'web',
                  themeRef: 'theme.public.default',
                  tokenRefs: ['theme-token.public.roles'],
                  warningPreservationRefs: ['warning-preservation.public.narrow_width'],
                },
              ],
              snapshotRef: 'theme-visual-snapshot.public.work_1',
              versionRef: 'theme-visual-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Theme and visual design')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('theme-visual-snapshot.public.work_1')
    expect(rendered).toContain('theme.public.default')
    expect(rendered).toContain('theme-token.public.roles')
    expect(rendered).toContain('status-visual.public.success_green')
    expect(rendered).toContain('runtime-receipt.public.success_state')
    expect(rendered).toContain('contrast.public.status_pass')
    expect(rendered).toContain('surface.public.operator')
  })

  test('omits unsafe Theme and visual design material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            themeVisualEvidence: {
              blockerRefs: [
                'theme-blocker.public.safe',
                'executable-theme /Users/christopher/theme.js',
              ],
              entries: [
                {
                  contrastCheckRefs: [
                    'contrast.public.safe',
                    'raw css /Users/christopher/style.css',
                  ],
                  crossSurfaceRefs: [
                    'surface.public.terminal',
                    'surface.public.web',
                    'surface.public.mobile',
                    'surface.public.operator',
                  ],
                  freshness: 'fresh',
                  highContrastRefs: ['high-contrast.public.safe'],
                  monochromeRefs: ['monochrome.public.safe'],
                  runtimeReceiptRefs: ['runtime-receipt.public.safe'],
                  snapshotRefs: [
                    'visual-snapshot.public.safe',
                    'visual snapshot content private',
                  ],
                  status: 'ready',
                  statusIconRefs: ['status-icon.public.safe'],
                  statusLabelRefs: ['status-label.public.safe'],
                  statusVisualRefs: [
                    'status-visual.public.safe',
                    'unsupported green claim private',
                  ],
                  surface: 'web',
                  themeRef: 'theme.public.safe',
                  tokenRefs: ['theme-token.public.safe', 'plugin theme code private'],
                  warningPreservationRefs: ['warning-preservation.public.safe'],
                },
              ],
              snapshotRef: 'theme-visual-snapshot.public.safe',
              versionRef: 'theme-visual-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Theme and visual design')
    expect(rendered).toContain('theme-visual-snapshot.public.safe')
    expect(rendered).toContain('theme.public.safe')
    expect(rendered).toContain('contrast.public.safe')
    expect(rendered).toContain('theme-token.public.safe')
    expect(rendered).toContain('visual-snapshot.public.safe')
    expect(rendered).toContain('unsafe-theme-visual-material-omitted')
    expect(rendered).toContain('unsafe theme/visual ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('executable-theme')
    expect(rendered).not.toContain('raw css')
    expect(rendered).not.toContain('visual snapshot content')
    expect(rendered).not.toContain('unsupported green claim')
    expect(rendered).not.toContain('plugin theme code')
  })

  test('renders Accessibility and non-interactive mode lane for public mode evidence', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            accessibilityNonInteractiveEvidence: {
              entries: [
                {
                  ciPolicyRefs: ['ci-policy.public.no_mutation_default'],
                  deployCaveatRefs: ['deploy-caveat.public.disabled'],
                  exitCodeRefs: ['exit-code.public.stable'],
                  freshness: 'fresh',
                  highContrastRefs: ['high-contrast.public.available'],
                  keyboardNavigationRefs: ['keyboard-navigation.public.available'],
                  mode: 'ci',
                  modeRef: 'interaction-mode.public.ci',
                  noColorRefs: ['no-color.public.status_labels'],
                  notificationAvailabilityRefs: ['notification.public.structured'],
                  promptAvailabilityRefs: ['prompt-availability.public.none_required'],
                  providerMutationCaveatRefs: [
                    'provider-mutation.public.disabled',
                  ],
                  pushCaveatRefs: ['push-caveat.public.disabled'],
                  reducedMotionRefs: ['reduced-motion.public.no_spinners'],
                  remoteBridgeAvailabilityRefs: ['remote-bridge.public.unavailable'],
                  schemaRefs: ['schema.public.structured_output_v1'],
                  spendCaveatRefs: ['spend-caveat.public.disabled'],
                  status: 'ready',
                  statusLabelRefs: ['status-label.public.ready'],
                  structuredOutputRefs: ['structured-output.public.json'],
                  terminalCapabilityRefs: ['terminal-capability.public.ci'],
                },
              ],
              snapshotRef: 'accessibility-non-interactive-snapshot.public.work_1',
              versionRef: 'accessibility-non-interactive-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Accessibility and non-interactive mode')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('accessibility-non-interactive-snapshot.public.work_1')
    expect(rendered).toContain('interaction-mode.public.ci')
    expect(rendered).toContain('structured-output.public.json')
    expect(rendered).toContain('schema.public.structured_output_v1')
    expect(rendered).toContain('exit-code.public.stable')
    expect(rendered).toContain('status-label.public.ready')
    expect(rendered).toContain('no-color.public.status_labels')
    expect(rendered).toContain('ci-policy.public.no_mutation_default')
    expect(rendered).toContain('spend-caveat.public.disabled')
  })

  test('omits unsafe Accessibility and non-interactive material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            accessibilityNonInteractiveEvidence: {
              blockerRefs: [
                'accessibility-blocker.public.safe',
                'raw terminal /Users/christopher/output.log',
              ],
              entries: [
                {
                  ciPolicyRefs: ['ci-policy.public.safe'],
                  deployCaveatRefs: ['deploy-caveat.public.safe'],
                  exitCodeRefs: ['exit-code.public.safe'],
                  freshness: 'fresh',
                  highContrastRefs: ['high-contrast.public.safe'],
                  keyboardNavigationRefs: ['keyboard-navigation.public.safe'],
                  mode: 'ci',
                  modeRef: 'interaction-mode.public.safe',
                  noColorRefs: ['no-color.public.safe'],
                  notificationAvailabilityRefs: ['notification.public.safe'],
                  promptAvailabilityRefs: [
                    'prompt-availability.public.safe',
                    'prompt text private',
                  ],
                  providerMutationCaveatRefs: ['provider-mutation.public.safe'],
                  pushCaveatRefs: ['push-caveat.public.safe'],
                  reducedMotionRefs: ['reduced-motion.public.safe'],
                  remoteBridgeAvailabilityRefs: ['remote-bridge.public.safe'],
                  schemaRefs: [
                    'schema.public.safe',
                    'structured output payload private',
                  ],
                  spendCaveatRefs: ['spend-caveat.public.safe'],
                  status: 'ready',
                  statusLabelRefs: ['status-label.public.safe'],
                  structuredOutputRefs: [
                    'structured-output.public.safe',
                    'private output /Users/christopher/output.json',
                  ],
                  terminalCapabilityRefs: [
                    'terminal-capability.public.safe',
                    'terminal capture content private',
                  ],
                },
              ],
              snapshotRef: 'accessibility-non-interactive-snapshot.public.safe',
              versionRef: 'accessibility-non-interactive-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Accessibility and non-interactive mode')
    expect(rendered).toContain('accessibility-non-interactive-snapshot.public.safe')
    expect(rendered).toContain('interaction-mode.public.safe')
    expect(rendered).toContain('structured-output.public.safe')
    expect(rendered).toContain('schema.public.safe')
    expect(rendered).toContain('terminal-capability.public.safe')
    expect(rendered).toContain('unsafe-accessibility-non-interactive-material-omitted')
    expect(rendered).toContain(
      'unsafe accessibility/non-interactive ref(s) were omitted',
    )
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw terminal')
    expect(rendered).not.toContain('prompt text')
    expect(rendered).not.toContain('structured output payload')
    expect(rendered).not.toContain('private output')
    expect(rendered).not.toContain('terminal capture content')
  })

  test('renders Localization boundary lane for public localization snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            localizationBoundaryEvidence: {
              entries: [
                {
                  catalogRefs: ['message-catalog.public.en_US'],
                  catalogValidationRefs: ['catalog-validation.public.valid'],
                  fallbackRefs: ['fallback.public.visible'],
                  formatterRefs: ['formatter.public.date_number_currency'],
                  freshness: 'fresh',
                  localePreferenceRefs: [
                    'locale-preference.public.user_team_system',
                  ],
                  localeRefs: ['locale.public.en-US'],
                  localizationRef: 'localization.public.ui',
                  scope: 'ui',
                  stableIdBoundaryRefs: [
                    'stable-id-boundary.public.canonical_ids',
                  ],
                  status: 'ready',
                },
              ],
              snapshotRef: 'localization-boundary-snapshot.public.work_1',
              versionRef: 'localization-boundary-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Localization boundary')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('localization-boundary-snapshot.public.work_1')
    expect(rendered).toContain('localization.public.ui')
    expect(rendered).toContain('locale.public.en-US')
    expect(rendered).toContain('locale-preference.public.user_team_system')
    expect(rendered).toContain('message-catalog.public.en_US')
    expect(rendered).toContain('catalog-validation.public.valid')
    expect(rendered).toContain('fallback.public.visible')
    expect(rendered).toContain('stable-id-boundary.public.canonical_ids')
  })

  test('omits unsafe Localization boundary material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            localizationBoundaryEvidence: {
              blockerRefs: [
                'localization-blocker.public.safe',
                'raw catalog /Users/christopher/catalog.json',
              ],
              entries: [
                {
                  catalogRefs: [
                    'message-catalog.public.safe',
                    'private catalog /Users/christopher/catalog.json',
                  ],
                  catalogValidationRefs: ['catalog-validation.public.safe'],
                  fallbackRefs: ['fallback.public.safe'],
                  formatterRefs: ['formatter.public.safe'],
                  freshness: 'fresh',
                  localePreferenceRefs: ['locale-preference.public.safe'],
                  localeRefs: ['locale.public.safe'],
                  localizationRef: 'localization.public.safe',
                  paymentLanguageReviewRefs: [
                    'payment-language-review.public.safe',
                    'payment payload private',
                  ],
                  scope: 'ui',
                  stableIdBoundaryRefs: [
                    'stable-id-boundary.public.safe',
                    'translated identifier private',
                  ],
                  status: 'ready',
                  toolIdStabilityRefs: [
                    'tool-id-stability.public.safe',
                    'translation content private',
                  ],
                },
              ],
              snapshotRef: 'localization-boundary-snapshot.public.safe',
              versionRef: 'localization-boundary-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Localization boundary')
    expect(rendered).toContain('localization-boundary-snapshot.public.safe')
    expect(rendered).toContain('localization.public.safe')
    expect(rendered).toContain('message-catalog.public.safe')
    expect(rendered).toContain('stable-id-boundary.public.safe')
    expect(rendered).toContain('tool-id-stability.public.safe')
    expect(rendered).toContain('unsafe-localization-material-omitted')
    expect(rendered).toContain('unsafe localization ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw catalog')
    expect(rendered).not.toContain('private catalog')
    expect(rendered).not.toContain('payment payload')
    expect(rendered).not.toContain('translated identifier')
    expect(rendered).not.toContain('translation content')
  })

  test('renders Enterprise managed policy lane for public policy snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            enterpriseManagedPolicyEvidence: {
              entries: [
                {
                  allowRefs: ['allow.public.provider_openai'],
                  auditRefs: ['policy-audit.public.created'],
                  budgetPolicyRefs: ['budget-policy.public.team_cap'],
                  changeRefs: ['policy-change.public.v1'],
                  decision: 'allow',
                  effectiveAtRefs: ['effective-at.public.2026-06-18'],
                  effectivePolicyRefs: ['effective-policy.public.team_provider'],
                  enforcementModeRefs: ['enforcement-mode.public.enforce'],
                  freshness: 'fresh',
                  organizationPolicyRefs: [
                    'organization-policy.public.openagents',
                  ],
                  ownerAdminRefs: ['admin.public.policy_owner'],
                  policyRef: 'managed-policy.public.provider',
                  providerPolicyRefs: ['provider-policy.public.allowlist'],
                  publicSummaryRefs: ['policy-summary.public.safe'],
                  ruleKindRefs: ['rule-kind.public.provider_allowlist'],
                  runtimeCapabilityBoundaryRefs: [
                    'runtime-capability-boundary.public.no_grant',
                  ],
                  scopeRefs: ['scope.public.team'],
                  status: 'ready',
                  teamPolicyRefs: ['team-policy.public.engineering'],
                  versionRefs: ['policy-version.public.v1'],
                },
              ],
              snapshotRef: 'enterprise-managed-policy-snapshot.public.work_1',
              versionRef: 'enterprise-managed-policy-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Enterprise managed policy')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('enterprise-managed-policy-snapshot.public.work_1')
    expect(rendered).toContain('managed-policy.public.provider')
    expect(rendered).toContain('effective-policy.public.team_provider')
    expect(rendered).toContain('scope.public.team')
    expect(rendered).toContain('policy-audit.public.created')
    expect(rendered).toContain('policy-change.public.v1')
    expect(rendered).toContain('runtime-capability-boundary.public.no_grant')
    expect(rendered).toContain('provider-policy.public.allowlist')
  })

  test('omits unsafe Enterprise managed policy material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            enterpriseManagedPolicyEvidence: {
              blockerRefs: [
                'managed-policy-blocker.public.safe',
                'raw policy /Users/christopher/policy.json',
              ],
              entries: [
                {
                  allowRefs: ['allow.public.safe'],
                  auditRefs: ['policy-audit.public.safe'],
                  budgetPolicyRefs: ['budget-policy.public.safe'],
                  caveatRefs: ['policy-caveat.public.safe', 'silent broadening private'],
                  changeRefs: ['policy-change.public.safe'],
                  decision: 'allow',
                  effectivePolicyRefs: ['effective-policy.public.safe'],
                  enforcementModeRefs: ['enforcement-mode.public.safe'],
                  freshness: 'fresh',
                  organizationPolicyRefs: [
                    'organization-policy.public.safe',
                    'private org /Users/christopher/org.json',
                  ],
                  ownerAdminRefs: ['admin.public.safe'],
                  policyRef: 'managed-policy.public.safe',
                  providerPolicyRefs: [
                    'provider-policy.public.safe',
                    'provider payload private',
                  ],
                  publicSummaryRefs: [
                    'policy-summary.public.safe',
                    'policy internals private',
                  ],
                  ruleKindRefs: ['rule-kind.public.safe'],
                  runtimeCapabilityBoundaryRefs: [
                    'runtime-capability-boundary.public.safe',
                  ],
                  scopeRefs: ['scope.public.safe'],
                  status: 'ready',
                  versionRefs: ['policy-version.public.safe'],
                },
              ],
              snapshotRef: 'enterprise-managed-policy-snapshot.public.safe',
              versionRef: 'enterprise-managed-policy-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Enterprise managed policy')
    expect(rendered).toContain('enterprise-managed-policy-snapshot.public.safe')
    expect(rendered).toContain('managed-policy.public.safe')
    expect(rendered).toContain('organization-policy.public.safe')
    expect(rendered).toContain('provider-policy.public.safe')
    expect(rendered).toContain('policy-summary.public.safe')
    expect(rendered).toContain('unsafe-managed-policy-material-omitted')
    expect(rendered).toContain('unsafe managed policy ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw policy')
    expect(rendered).not.toContain('private org')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('policy internals')
    expect(rendered).not.toContain('silent broadening')
  })

  test('renders Terminal UI shell lane for public terminal surfaces', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            terminalUiShell: {
              snapshotRef: 'terminal-snapshot.public.work_1',
              surfaces: [
                {
                  accessibilityRefs: ['accessibility.public.non_interactive'],
                  commandDescriptorRefs: ['command-descriptor.public.review'],
                  freshness: 'fresh',
                  inputDescriptorRefs: ['input-descriptor.public.keys'],
                  mode: 'interactive',
                  nonInteractiveRefs: ['non-interactive.public.json'],
                  paneRefs: ['terminal-pane.public.main'],
                  parityRefs: ['parity.public.web_tui'],
                  policyRefs: ['policy.public.terminal.interactive'],
                  shellRefs: ['terminal-shell.public.pylon'],
                  state: 'available',
                  streamRefs: ['terminal-stream.public.work_1'],
                  surfaceRef: 'terminal-surface.public.tui',
                  transcriptSummaryRefs: ['terminal-transcript-summary.public.work_1'],
                },
              ],
              versionRef: 'terminal-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Terminal UI shell')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('terminal-snapshot.public.work_1')
    expect(rendered).toContain('terminal-surface.public.tui')
    expect(rendered).toContain('terminal-shell.public.pylon')
    expect(rendered).toContain('terminal-pane.public.main')
    expect(rendered).toContain('terminal-stream.public.work_1')
    expect(rendered).toContain('terminal-transcript-summary.public.work_1')
    expect(rendered).toContain('command-descriptor.public.review')
    expect(rendered).toContain('input-descriptor.public.keys')
    expect(rendered).toContain('policy.public.terminal.interactive')
  })

  test('omits unsafe Terminal UI shell material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            terminalUiShell: {
              blockerRefs: [
                'terminal-blocker.public.safe',
                'raw terminal /Users/christopher/term.log',
              ],
              snapshotRef: 'terminal-snapshot.public.safe',
              surfaces: [
                {
                  accessibilityRefs: ['accessibility.public.safe'],
                  blockerRefs: ['surface-blocker.public.safe', 'raw output sk-private'],
                  commandDescriptorRefs: [
                    'command-descriptor.public.safe',
                    'raw command rm -rf',
                  ],
                  freshness: 'fresh',
                  inputDescriptorRefs: [
                    'input-descriptor.public.safe',
                    'private input token',
                  ],
                  mode: 'interactive',
                  nonInteractiveRefs: ['non-interactive.public.safe'],
                  paneRefs: ['terminal-pane.public.safe'],
                  parityRefs: ['parity.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  shellRefs: ['terminal-shell.public.safe'],
                  state: 'available',
                  streamRefs: [
                    'terminal-stream.public.safe',
                    'terminal output /Users/christopher',
                  ],
                  surfaceRef: 'terminal-surface.public.safe',
                  transcriptSummaryRefs: [
                    'terminal-transcript-summary.public.safe',
                    'raw transcript ./private.log',
                  ],
                },
              ],
              versionRef: 'terminal-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Terminal UI shell')
    expect(rendered).toContain('terminal-snapshot.public.safe')
    expect(rendered).toContain('terminal-surface.public.safe')
    expect(rendered).toContain('command-descriptor.public.safe')
    expect(rendered).toContain('terminal-stream.public.safe')
    expect(rendered).toContain('terminal-transcript-summary.public.safe')
    expect(rendered).toContain('unsafe-terminal-material-omitted')
    expect(rendered).toContain('unsafe terminal ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw terminal')
    expect(rendered).not.toContain('raw output')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('private input')
    expect(rendered).not.toContain('terminal output')
    expect(rendered).not.toContain('raw transcript')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Input and keybinding lane for public input snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            inputKeybinding: {
              entries: [
                {
                  accessibilityRefs: ['accessibility.public.keyboard'],
                  bindingMapRefs: ['binding-map.public.default'],
                  commandDescriptorRefs: ['command-descriptor.public.review'],
                  conflictRefs: [],
                  freshness: 'fresh',
                  inputModeRef: 'input-mode.public.keyboard',
                  keymapRefs: ['keymap.public.default'],
                  mode: 'keyboard',
                  nonInteractiveFallbackRefs: ['non-interactive.public.json'],
                  platformRefs: ['platform.public.mac'],
                  policyRefs: ['policy.public.input.keyboard'],
                  state: 'available',
                },
              ],
              snapshotRef: 'input-keybinding-snapshot.public.work_1',
              versionRef: 'input-keybinding-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Input and keybinding')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('input-keybinding-snapshot.public.work_1')
    expect(rendered).toContain('input-mode.public.keyboard')
    expect(rendered).toContain('binding-map.public.default')
    expect(rendered).toContain('keymap.public.default')
    expect(rendered).toContain('command-descriptor.public.review')
    expect(rendered).toContain('non-interactive.public.json')
    expect(rendered).toContain('policy.public.input.keyboard')
  })

  test('omits unsafe Input and keybinding material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            inputKeybinding: {
              blockerRefs: [
                'input-blocker.public.safe',
                'raw input /Users/christopher/input.log',
              ],
              entries: [
                {
                  accessibilityRefs: ['accessibility.public.safe'],
                  bindingMapRefs: ['binding-map.public.safe', 'key log sk-private'],
                  blockerRefs: [
                    'entry-input-blocker.public.safe',
                    'raw key /Users/christopher/key.log',
                  ],
                  commandDescriptorRefs: [
                    'command-descriptor.public.safe',
                    'raw command rm -rf',
                  ],
                  conflictRefs: ['conflict.public.safe'],
                  freshness: 'fresh',
                  inputModeRef: 'input-mode.public.safe',
                  keymapRefs: ['keymap.public.safe', 'private key token'],
                  mode: 'keyboard',
                  nonInteractiveFallbackRefs: ['non-interactive.public.safe'],
                  platformRefs: ['platform.public.safe', 'private input token'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  state: 'available',
                },
              ],
              snapshotRef: 'input-keybinding-snapshot.public.safe',
              versionRef: 'input-keybinding-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Input and keybinding')
    expect(rendered).toContain('input-keybinding-snapshot.public.safe')
    expect(rendered).toContain('input-mode.public.safe')
    expect(rendered).toContain('binding-map.public.safe')
    expect(rendered).toContain('command-descriptor.public.safe')
    expect(rendered).toContain('keymap.public.safe')
    expect(rendered).toContain('unsafe-input-keybinding-material-omitted')
    expect(rendered).toContain('unsafe input/keybinding ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw input')
    expect(rendered).not.toContain('key log')
    expect(rendered).not.toContain('raw key')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('private key')
    expect(rendered).not.toContain('private input')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Command system lane for public command catalogs', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            commandSystem: {
              commands: [
                {
                  capabilityRefs: ['capability.public.command.review'],
                  commandDescriptorRefs: ['command-descriptor.public.review'],
                  commandRef: 'command.public.review_changes',
                  freshness: 'fresh',
                  inputModeRefs: ['input-mode.public.keyboard'],
                  kind: 'built_in',
                  parserRefs: ['parser.public.typed_command'],
                  plannerRefs: ['planner.public.command_route'],
                  policyRefs: ['policy.public.command.review'],
                  selectorRefs: ['semantic-selector.public.command.review'],
                  state: 'available',
                },
              ],
              snapshotRef: 'command-snapshot.public.work_1',
              versionRef: 'command-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Command system')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('command-snapshot.public.work_1')
    expect(rendered).toContain('command.public.review_changes')
    expect(rendered).toContain('command-descriptor.public.review')
    expect(rendered).toContain('parser.public.typed_command')
    expect(rendered).toContain('planner.public.command_route')
    expect(rendered).toContain('semantic-selector.public.command.review')
    expect(rendered).toContain('policy.public.command.review')
  })

  test('omits unsafe Command system material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            commandSystem: {
              blockerRefs: [
                'command-blocker.public.safe',
                'raw command /Users/christopher/command.log',
              ],
              commands: [
                {
                  blockerRefs: ['entry-command-blocker.public.safe', 'raw shell sk-private'],
                  capabilityRefs: ['capability.public.safe'],
                  commandDescriptorRefs: [
                    'command-descriptor.public.safe',
                    'command text rm -rf',
                  ],
                  commandRef: 'command.public.safe',
                  conflictRefs: ['conflict.public.safe'],
                  fallbackRefs: ['fallback-command.public.safe'],
                  freshness: 'fresh',
                  inputModeRefs: ['input-mode.public.safe', 'private input token'],
                  kind: 'built_in',
                  parserRefs: [
                    'parser.public.safe',
                    'raw prompt /Users/christopher/prompt.md',
                  ],
                  plannerRefs: ['planner.public.safe'],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  selectorRefs: ['semantic-selector.public.safe'],
                  state: 'available',
                },
              ],
              snapshotRef: 'command-snapshot.public.safe',
              versionRef: 'command-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Command system')
    expect(rendered).toContain('command-snapshot.public.safe')
    expect(rendered).toContain('command.public.safe')
    expect(rendered).toContain('command-descriptor.public.safe')
    expect(rendered).toContain('parser.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-command-material-omitted')
    expect(rendered).toContain('unsafe command ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('raw shell')
    expect(rendered).not.toContain('command text')
    expect(rendered).not.toContain('private input')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Notifications and attention lane for public attention snapshots', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            notificationAttention: {
              attention: [
                {
                  actionRefs: ['action.public.review_required'],
                  attentionRef: 'attention.public.review_required',
                  channelRefs: ['channel.public.web'],
                  decisionRefs: ['decision.public.review_required'],
                  deliveryRefs: ['delivery.public.web.review_required'],
                  dedupeRefs: ['dedupe.public.review_required'],
                  freshness: 'fresh',
                  notificationRefs: ['notification.public.review_required'],
                  policyRefs: ['policy.public.attention.review'],
                  severity: 'warning',
                  state: 'waiting',
                },
              ],
              snapshotRef: 'attention-snapshot.public.work_1',
              versionRef: 'attention-version.public.v1',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Notifications and attention')
    expect(rendered).toContain('attention')
    expect(rendered).toContain('attention-snapshot.public.work_1')
    expect(rendered).toContain('attention.public.review_required')
    expect(rendered).toContain('notification.public.review_required')
    expect(rendered).toContain('delivery.public.web.review_required')
    expect(rendered).toContain('decision.public.review_required')
    expect(rendered).toContain('action.public.review_required')
    expect(rendered).toContain('policy.public.attention.review')
  })

  test('omits unsafe Notifications and attention material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            notificationAttention: {
              attention: [
                {
                  actionRefs: ['action.public.safe'],
                  attentionRef: 'attention.public.safe',
                  blockerRefs: [
                    'entry-attention-blocker.public.safe',
                    'raw notification sk-private',
                  ],
                  channelRefs: ['channel.public.safe'],
                  decisionRefs: ['decision.public.safe', 'private notification token'],
                  deliveryRefs: ['delivery.public.safe'],
                  dedupeRefs: ['dedupe.public.safe'],
                  freshness: 'fresh',
                  notificationRefs: [
                    'notification.public.safe',
                    'notification body /Users/christopher/message.md',
                  ],
                  policyRefs: ['policy.public.safe', 'bearer token private'],
                  severity: 'warning',
                  state: 'waiting',
                },
              ],
              blockerRefs: [
                'attention-blocker.public.safe',
                'attention message /Users/christopher/attention.md',
              ],
              snapshotRef: 'attention-snapshot.public.safe',
              versionRef: 'attention-version.public.safe',
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Notifications and attention')
    expect(rendered).toContain('attention-snapshot.public.safe')
    expect(rendered).toContain('attention.public.safe')
    expect(rendered).toContain('notification.public.safe')
    expect(rendered).toContain('decision.public.safe')
    expect(rendered).toContain('policy.public.safe')
    expect(rendered).toContain('unsafe-notification-attention-material-omitted')
    expect(rendered).toContain('unsafe notification/attention ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw notification')
    expect(rendered).not.toContain('private notification')
    expect(rendered).not.toContain('notification body')
    expect(rendered).not.toContain('attention message')
    expect(rendered).not.toContain('bearer token')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Plan mutation receipts beside Run progress', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            planMutationReceipts: [
              {
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
              },
            ],
            planMutationRequests: [
              {
                action: 'add',
                actorRef: 'actor.public.operator',
                generatedAt: '2026-06-16T15:30:00.000Z',
                itemRef: 'plan-item.public.work_1.review_docs',
                provenanceRefs: ['event.public.work_1.plan_requested'],
                publicSafe: true,
                requestRef: 'plan-request.public.work_1.review_docs',
              },
            ],
          }),
          [
            workEvent(1, 'queued', 'queued_or_running'),
            workEvent(2, 'running', 'queued_or_running'),
          ],
        ),
      ),
    )

    expect(rendered).toContain('Plan mutation receipts')
    expect(rendered).toContain('plan-item.public.work_1.review_docs')
    expect(rendered).toContain('plan-request.public.work_1.review_docs')
    expect(rendered).toContain('actor.public.operator')
    expect(rendered).toContain('plan-item.public.work_1.write_tests')
    expect(rendered).toContain('plan-receipt.public.work_1.write_tests')
    expect(rendered).toContain('event.public.work_1.plan_applied')
  })

  test('renders plan completion blockers when closeout evidence is missing', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('delivered', null, {
            executionCloseout: null,
            planMutationReceipts: [
              {
                action: 'complete',
                actorRef: 'actor.public.runtime',
                blockerRefs: [],
                generatedAt: '2026-06-16T15:35:00.000Z',
                itemRef: 'plan-item.public.work_1.finish',
                provenanceRefs: ['event.public.work_1.plan_complete'],
                publicSafe: true,
                receiptRef: 'plan-receipt.public.work_1.finish',
                requestRef: 'plan-request.public.work_1.finish',
                state: 'applied',
              },
            ],
          }),
          [workEvent(1, 'delivered', 'delivered')],
        ),
      ),
    )

    expect(rendered).toContain('Plan mutation receipts')
    expect(rendered).toContain('plan-complete-without-closeout-evidence')
    expect(rendered).toContain('missing-closeout-evidence')
  })

  test('omits unsafe plan mutation receipt refs before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            planMutationReceipts: [
              {
                action: 'update',
                actorRef: 'provider payload sk-private',
                blockerRefs: ['/Users/christopher/private-plan.md'],
                generatedAt: '2026-06-16T15:35:00.000Z',
                itemRef: 'plan-item.public.safe',
                provenanceRefs: ['raw prompt /Users/christopher/private.md'],
                publicSafe: true,
                receiptRef: 'plan-receipt.public.safe',
                requestRef: 'plan-request.public.safe',
                state: 'applied',
              },
              {
                action: 'update',
                actorRef: 'actor.public.runtime',
                blockerRefs: [],
                generatedAt: '2026-06-16T15:36:00.000Z',
                itemRef: 'plan-item.public.safe',
                provenanceRefs: ['event.public.safe'],
                publicSafe: true,
                receiptRef: 'plan-receipt.public.safe',
                requestRef: 'plan-request.public.safe',
                state: 'applied',
              },
            ],
          }),
          [workEvent(1, 'running', 'queued_or_running')],
        ),
      ),
    )

    expect(rendered).toContain('Plan mutation receipts')
    expect(rendered).toContain('plan-receipt.public.safe')
    expect(rendered).toContain('unsafe-plan-mutation-material-omitted')
    expect(rendered).toContain('unsafe plan mutation ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders Context snapshot lane with repo, instruction, adapter, and job refs', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            contextSnapshot: {
              adapters: {
                capabilityRefs: ['capability.codex_agent_task.v1'],
                refs: ['adapter.codex.ready'],
              },
              currentJob: {
                capabilityRefs: ['capability.codex_agent_task.v1'],
                jobRefs: ['assignment.public.work_1', 'workspace.public.work_1'],
                verificationRefs: ['verification-command.public.bun_test'],
              },
              devDoctor: {
                refs: ['doctor.public.pylon.context.v0_3'],
              },
              instructions: {
                configRefs: ['config.pylon.default_adapter.codex'],
                refs: ['instructions.public.AGENTS.md.sha256_abcd'],
              },
              observedAt: '2026-06-16T15:55:00.000Z',
              repo: {
                changedCount: 0,
                dirtyState: 'clean',
                identityRefs: [
                  'repo.github.OpenAgentsInc.openagents',
                  'branch.main',
                  'commit.f9793718e000',
                ],
              },
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Context snapshot')
    expect(rendered).toContain('repo.github.OpenAgentsInc.openagents')
    expect(rendered).toContain('instructions.public.AGENTS.md.sha256_abcd')
    expect(rendered).toContain('adapter.codex.ready')
    expect(rendered).toContain('doctor.public.pylon.context.v0_3')
    expect(rendered).toContain('assignment.public.work_1')
    expect(rendered).toContain('verification-command.public.bun_test')
    expect(rendered).toContain('ready')
    expect(rendered).toContain('fresh')
  })

  test('renders Context snapshot missing-evidence blockers when context is absent', () => {
    const rendered = renderHtml(detailView(modelForWork(workForState('delivered', null))))

    expect(rendered).toContain('Context snapshot')
    expect(rendered).toContain('No context evidence available yet.')
    expect(rendered).toContain('missing-context-evidence')
    expect(rendered).toContain('unknown-context-freshness')
  })

  test('omits unsafe context refs before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            contextSnapshot: {
              adapters: {
                refs: ['adapter.codex.ready', 'provider payload sk-private'],
              },
              blockerRefs: ['private repo content /Users/christopher/src/openagents'],
              currentJob: {
                jobRefs: [
                  'assignment.public.work_1',
                  'raw shell command $(cat ~/.ssh/id_rsa)',
                ],
              },
              devDoctor: {
                refs: ['doctor.public.safe', 'diff --git a/private.ts b/private.ts'],
              },
              instructions: {
                refs: [
                  'instructions.public.AGENTS.md.sha256_abcd',
                  'raw prompt /Users/christopher/private.md',
                ],
              },
              observedAt: '2026-06-16T15:55:00.000Z',
              repo: {
                dirtyState: 'clean',
                identityRefs: [
                  'repo.github.OpenAgentsInc.openagents',
                  '/Users/christopher/work/openagents',
                ],
              },
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Context snapshot')
    expect(rendered).toContain('repo.github.OpenAgentsInc.openagents')
    expect(rendered).toContain('adapter.codex.ready')
    expect(rendered).toContain('assignment.public.work_1')
    expect(rendered).toContain('unsafe-context-material-omitted')
    expect(rendered).toContain('unsafe context ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('diff --git')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw shell')
    expect(rendered).not.toContain('sk-private')
  })

  test('renders latest repository memory profile in Context snapshot', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            contextSnapshot: {
              devDoctor: {
                refs: ['doctor.public.pylon.context.v0_3'],
              },
              instructions: {
                refs: ['profile.instruction.public.AGENTS.sha256_abcd'],
              },
              observedAt: '2026-06-16T17:55:00.000Z',
              repo: {
                dirtyState: 'clean',
                identityRefs: ['repo.github.OpenAgentsInc.openagents'],
              },
              repositoryMemoryProfile: {
                blockedClaimRefs: [
                  'claim.openagents_studybench.blueprint_authority.c7',
                ],
                changedProfileKinds: ['command', 'test'],
                commandProfileRefs: [
                  'profile.command.public.package_scripts.sha256_abcd',
                ],
                corpusManifestRef: 'corpus_manifest.openagents.repo.sha256_abcd',
                datasetRefs: [
                  'dataset.openagents.studybench.public_retained.v0',
                  'hf://jacobli/studybench/dspy',
                ],
                generatedAt: '2026-06-16T18:00:00.000Z',
                holdoutEvaluationRef:
                  'evaluation.openagents_studybench.holdout.sha256_abcd',
                instructionRefs: ['profile.instruction.public.AGENTS.sha256_abcd'],
                privateValidationTrendRef:
                  'trend.openagents_studybench.private_validation.sha256_abcd',
                profileRef: 'repository-profile.public.openagents.main',
                publicRetainedScoreRef:
                  'score.openagents_studybench.public_retained.sha256_abcd',
                refreshedAt: '2026-06-16T17:59:00.000Z',
                refreshEvents: [
                  {
                    commandProfileRefs: [
                      'profile.command.public.package_scripts.sha256_abcd',
                    ],
                    generatedAt: '2026-06-16T18:00:00.000Z',
                    refreshedAt: '2026-06-16T17:59:00.000Z',
                    repoIdentityRefs: ['repo.github.OpenAgentsInc.openagents'],
                    testProfileRefs: ['profile.test.public.vitest.sha256_abcd'],
                    workOrderRef: 'work_1',
                  },
                ],
                studyPacketFreshness: 'fresh',
                studyPacketRef: 'study_packet.openagents.launch.v0',
                testProfileRefs: ['profile.test.public.vitest.sha256_abcd'],
              },
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Repository memory profile')
    expect(rendered).toContain('repository-profile.public.openagents.main')
    expect(rendered).toContain('Study packet memory')
    expect(rendered).toContain('Internal dogfood')
    expect(rendered).toContain('evidence_only')
    expect(rendered).toContain('no mutation authority')
    expect(rendered).toContain('study_packet.openagents.launch.v0')
    expect(rendered).toContain('corpus_manifest.openagents.repo.sha256_abcd')
    expect(rendered).toContain('dataset.openagents.studybench.public_retained.v0')
    expect(rendered).toContain('hf://jacobli/studybench/dspy')
    expect(rendered).toContain(
      'score.openagents_studybench.public_retained.sha256_abcd',
    )
    expect(rendered).toContain(
      'trend.openagents_studybench.private_validation.sha256_abcd',
    )
    expect(rendered).toContain(
      'evaluation.openagents_studybench.holdout.sha256_abcd',
    )
    expect(rendered).toContain('claim.openagents_studybench.blueprint_authority.c7')
    expect(rendered).toContain('Changed kinds')
    expect(rendered).toContain('command, test')
    expect(rendered).toContain('profile.command.public.package_scripts.sha256_abcd')
    expect(rendered).toContain('profile.test.public.vitest.sha256_abcd')
    expect(rendered).toContain(
      'forge.repository_profile_refresh.work_1.2026-06-16T18_00_00.000Z',
    )
  })

  test('omits unsafe repository memory profile material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            contextSnapshot: {
              devDoctor: {
                refs: ['doctor.public.pylon.context.v0_3'],
              },
              instructions: {
                refs: ['profile.instruction.public.safe'],
              },
              repo: {
                dirtyState: 'clean',
                identityRefs: ['repo.github.OpenAgentsInc.openagents'],
              },
              repositoryMemoryProfile: {
                blockedClaimRefs: [
                  'claim.openagents_studybench.safe',
                  'hidden_rubric.openagents_studybench.private_holdout.c1',
                ],
                commandProfileRefs: [
                  'profile.command.public.safe',
                  'raw command /Users/christopher/private.sh',
                ],
                corpusManifestRef: 'raw_repo_archive.openagents.tar',
                datasetRefs: [
                  'dataset.openagents.studybench.public_retained.v0',
                  'gold_answer.private_holdout.row_1',
                ],
                generatedAt: '2026-06-16T18:00:00.000Z',
                holdoutEvaluationRef:
                  'hidden_gold_answer.private_holdout.row_1',
                instructionRefs: [
                  'profile.instruction.public.safe',
                  'raw prompt /Users/christopher/private.md',
                ],
                privateValidationTrendRef:
                  'private_customer_source.repo_a.file_1',
                profileRef: 'repository-profile.public.openagents.main',
                publicRetainedScoreRef:
                  'score.openagents_studybench.public_retained.safe',
                refreshedAt: '2026-06-16T17:59:00.000Z',
                repoIdentityRefs: [
                  'repo.github.OpenAgentsInc.openagents',
                  '/Users/christopher/work/openagents',
                ],
                testProfileRefs: ['raw test /Users/christopher/private.test.ts'],
              },
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Repository memory profile')
    expect(rendered).toContain('profile.command.public.safe')
    expect(rendered).toContain('Study packet memory')
    expect(rendered).toContain('dataset.openagents.studybench.public_retained.v0')
    expect(rendered).toContain('score.openagents_studybench.public_retained.safe')
    expect(rendered).toContain('unsafe-profile-material-omitted')
    expect(rendered).toContain('unsafe repository-memory ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw test')
    expect(rendered).not.toContain('hidden_rubric')
    expect(rendered).not.toContain('gold_answer')
    expect(rendered).not.toContain('raw_repo_archive')
    expect(rendered).not.toContain('hidden_gold_answer')
    expect(rendered).not.toContain('private_customer_source')
  })

  test('renders Session navigation lane with unavailable controls', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            executionCloseout: null,
            sessionNavigation: {
              localPylonSessions: [
                {
                  artifactRefs: ['artifact.public.pylon.session_summary'],
                  bridgeRefs: ['bridge.public.pylon.loopback'],
                  checkpointRefs: ['checkpoint.public.pylon.before_edit'],
                  eventRefs: ['event.public.pylon.running'],
                  observedAt: '2026-06-16T15:45:00.000Z',
                  sessionRef: 'pylon.session.work_1',
                  state: 'running',
                  title: 'Pylon local session',
                },
              ],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Session navigation')
    expect(rendered).toContain('Pylon local session')
    expect(rendered).toContain('pylon.session.work_1')
    expect(rendered).toContain('artifact.public.pylon.session_summary')
    expect(rendered).toContain('event.public.pylon.running')
    expect(rendered).toContain('checkpoint.public.pylon.before_edit')
    expect(rendered).toContain('bridge.public.pylon.loopback')
    expect(rendered).toContain('Resume')
    expect(rendered).toContain('Fork')
    expect(rendered).toContain('Rewind')
    expect(rendered).toContain('Cancel')
    expect(rendered).toContain('resume-control-verb-unavailable')
  })

  test('renders authority-gated session controls and public-safe receipts', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            executionCloseout: null,
            sessionNavigation: {
              controlReceipts: [
                {
                  action: 'resume',
                  actorRef: 'actor.public.raynor',
                  generatedAt: '2026-06-16T17:06:00.000Z',
                  outcome: 'queued',
                  provenanceRefs: ['bridge.public.session-control'],
                  publicSafe: true,
                  receiptRef: 'receipt.public.resume.1',
                  requestRef:
                    'forge-session-control-request:pylon.session.work_1:resume',
                  sessionRef: 'pylon.session.work_1',
                },
              ],
              localPylonSessions: [
                {
                  controlAuthorityRefs: ['authority.public.session-control.bridge'],
                  controlPolicyRefs: ['policy.public.resume.allowed'],
                  eventRefs: ['event.public.pylon.running'],
                  observedAt: '2026-06-16T15:45:00.000Z',
                  sessionRef: 'pylon.session.work_1',
                  state: 'running',
                  supportedControlActions: ['resume'],
                  title: 'Pylon local session',
                },
              ],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('/api/autopilot/session-control')
    expect(rendered).toContain(
      'forge-session-control-request:pylon.session.work_1:resume',
    )
    expect(rendered).toContain('authority.public.session-control.bridge')
    expect(rendered).toContain('policy.public.resume.allowed')
    expect(rendered).toContain('Session control receipts')
    expect(rendered).toContain('receipt.public.resume.1')
    expect(rendered).toContain('actor.public.raynor')
    expect(rendered).toContain('bridge.public.session-control')
    expect(rendered).toContain('fork-control-verb-unavailable')
  })

  test('renders Session navigation empty state when summaries are absent', () => {
    const rendered = renderHtml(detailView(modelForWork(workForState('delivered', null))))

    expect(rendered).toContain('Session navigation')
    expect(rendered).toContain('No session summaries available yet.')
    expect(rendered).toContain('no-session-summaries')
  })

  test('omits unsafe session navigation refs before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            executionCloseout: null,
            sessionNavigation: {
              codexSessions: [
                {
                  artifactRefs: [
                    'artifact.public.codex.safe',
                    'raw transcript /Users/christopher/private.jsonl',
                  ],
                  eventRefs: ['diff --git a/private.ts b/private.ts'],
                  sessionRef: '/Users/christopher/.codex/session.jsonl',
                  state: 'running',
                },
                {
                  artifactRefs: ['artifact.public.codex.safe'],
                  sessionRef: 'codex.session.safe',
                  state: 'running',
                },
              ],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Session navigation')
    expect(rendered).toContain('codex.session.safe')
    expect(rendered).toContain('unsafe-session-material-omitted')
    expect(rendered).toContain('unsafe session ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('diff --git')
    expect(rendered).not.toContain('raw transcript')
  })

  test('renders Retrieval search lane with selected and skipped candidates', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            retrievalPlan: {
              candidates: [
                {
                  candidateRef: 'candidate.public.file_progress',
                  freshness: 'fresh',
                  mode: 'exact',
                  provenanceRefs: [
                    'retrieval-source-kind.file',
                    'provenance.public.file_index',
                  ],
                  rank: 1,
                  score: 0.95,
                  sourceRef: 'source.public.file_progress',
                },
              ],
              freshness: 'fresh',
              mode: 'exact',
              planRef: 'retrieval-plan.public.work_1',
              queryRefs: ['query.public.progress'],
              requestRef: 'retrieval-request.public.work_1',
              skippedCandidates: [
                {
                  blockerRefs: ['retrieval-blocker.public.low_score'],
                  candidateRef: 'candidate.public.low_score',
                  reason: 'low_score',
                  sourceRef: 'source.public.low_score',
                },
              ],
              sourceRefs: ['source.public.seed'],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Retrieval search')
    expect(rendered).toContain('candidate.public.file_progress')
    expect(rendered).toContain('source.public.file_progress')
    expect(rendered).toContain('retrieval-source-kind.file')
    expect(rendered).toContain('provenance.public.file_index')
    expect(rendered).toContain('Skipped candidates')
    expect(rendered).toContain('candidate.public.low_score')
    expect(rendered).toContain('retrieval-blocker.public.low_score')
    expect(rendered).toContain('retrieval-plan.public.work_1')
    expect(rendered).toContain('query.public.progress')
  })

  test('renders Retrieval search lane from bounded live adapters', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            retrievalPlan: {
              liveAdapter: {
                queryRefs: ['query.public.progress'],
                sources: [
                  {
                    candidateRef: 'candidate.public.file_progress',
                    exactRefs: ['query.public.progress'],
                    provenanceRefs: ['file-index.public.autopilot_work'],
                    sourceKind: 'file',
                    sourceRef: 'source.public.file_progress',
                  },
                  {
                    candidateRef: 'candidate.public.low_score',
                    exactRefs: ['query.public.other'],
                    sourceKind: 'diagnostic',
                    sourceRef: 'source.public.low_score',
                  },
                ],
                workspaceBoundaryRefs: ['workspace-boundary.public.openagents.work_1'],
              },
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Retrieval search')
    expect(rendered).toContain('candidate.public.file_progress')
    expect(rendered).toContain('source.public.file_progress')
    expect(rendered).toContain('retrieval-source-kind.file')
    expect(rendered).toContain('file-index.public.autopilot_work')
    expect(rendered).toContain('workspace-boundary.public.openagents.work_1')
    expect(rendered).toContain('candidate.public.low_score')
    expect(rendered).toContain('low score')
    expect(rendered).toContain('forge-live-retrieval-plan:work_1')
  })

  test('renders Retrieval search missing-evidence blockers when plan is absent', () => {
    const rendered = renderHtml(detailView(modelForWork(workForState('delivered', null))))

    expect(rendered).toContain('Retrieval search')
    expect(rendered).toContain('No retrieval candidates selected yet.')
    expect(rendered).toContain('missing-query-ref')
  })

  test('omits unsafe retrieval refs before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            retrievalPlan: {
              candidates: [
                {
                  candidateRef: '/Users/christopher/private/file.ts',
                  provenanceRefs: ['diff --git a/private.ts b/private.ts'],
                  sourceRef: 'source.public.safe',
                },
                {
                  candidateRef: 'candidate.public.safe',
                  provenanceRefs: [
                    'provenance.public.safe',
                    'provider payload sk-private',
                  ],
                  sourceRef: 'raw file /Users/christopher/private.md',
                },
              ],
              mode: 'exact',
              planRef: 'diff --git a/plan b/plan',
              queryRefs: [
                'query.public.safe',
                'raw prompt /Users/christopher/private.md',
              ],
              requestRef: 'retrieval-request.public.work_1',
              skippedCandidates: [
                {
                  candidateRef: 'candidate.public.skipped',
                  reason: 'filtered_private',
                  sourceRef: 'https://private.example.test/repo',
                },
              ],
              sourceRefs: [
                'source.public.seed',
                '/Users/christopher/private/source',
              ],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Retrieval search')
    expect(rendered).toContain('candidate.public.safe')
    expect(rendered).toContain('provenance.public.safe')
    expect(rendered).toContain('unsafe-retrieval-material-omitted')
    expect(rendered).toContain('unsafe retrieval ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('diff --git')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw file')
    expect(rendered).not.toContain('provider payload')
    expect(rendered).not.toContain('sk-private')
    expect(rendered).not.toContain('private.example')
  })

  test('renders Extensibility request receipts with guard refs', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            extensibility: {
              effectiveConfig: {
                configRef: 'extensibility-config.public.work_1',
                entries: [
                  {
                    catalogRefs: ['mcp-catalog.public.work_1'],
                    configRefs: ['mcp-config.public.filesystem'],
                    domain: 'mcp',
                    effectiveState: 'enabled',
                    policyRefs: ['mcp-policy.public.workspace_read'],
                    sourceRefs: ['mcp-source.public.filesystem'],
                  },
                  {
                    catalogRefs: ['skill-catalog.public.work_1'],
                    configRefs: ['skill-config.public.context_summary'],
                    domain: 'skills',
                    effectiveState: 'enabled',
                    policyRefs: ['skill-policy.public.disclosure_required'],
                    sourceRefs: ['skill-source.public.context_summary'],
                  },
                ],
                freshness: 'fresh',
              },
              executionRequests: [
                {
                  configRefs: ['mcp-config.public.filesystem'],
                  domain: 'mcp',
                  policyRefs: ['mcp-policy.public.workspace_read'],
                  providerAccountRefs: ['provider-account.public.local_mcp'],
                  requestKind: 'mcp_tool_call',
                  requestRef: 'extensibility-request.public.mcp_files',
                  targetRef: 'mcp-source.public.filesystem',
                },
                {
                  configRefs: ['skill-config.public.context_summary'],
                  domain: 'skills',
                  requestKind: 'skill_body_disclosure',
                  requestRef: 'extensibility-request.public.skill_context',
                  targetRef: 'skill-source.public.context_summary',
                },
              ],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Extensibility requests')
    expect(rendered).toContain('mcp tool call')
    expect(rendered).toContain('callable')
    expect(rendered).toContain('provider-account.public.local_mcp')
    expect(rendered).toContain('skill body disclosure')
    expect(rendered).toContain('skill-body-disclosure-not-explicit')
    expect(rendered).toContain('Skill bodies loaded')
    expect(rendered).toContain('false')
  })

  test('omits unsafe extensibility execution request material before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            extensibility: {
              effectiveConfig: {
                configRef: 'extensibility-config.public.work_1',
                entries: [
                  {
                    configRefs: ['skill-config.public.context_summary'],
                    domain: 'skills',
                    effectiveState: 'enabled',
                    policyRefs: ['skill-policy.public.disclosure_required'],
                    sourceRefs: ['skill-source.public.context_summary'],
                  },
                ],
              },
              executionRequests: [
                {
                  blockerRefs: ['raw shell command $(cat ~/.ssh/id_rsa)'],
                  configRefs: [
                    'skill-config.public.context_summary',
                    'raw config /Users/christopher/private.json',
                  ],
                  domain: 'skills',
                  explicitDisclosure: true,
                  requestKind: 'skill_body_disclosure',
                  requestRef: 'extensibility-request.public.skill_context',
                  sourceRefs: ['skill-source.public.safe', 'skill body raw text'],
                  targetRef: '/Users/christopher/private/skill.md',
                },
              ],
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Extensibility requests')
    expect(rendered).toContain('unsafe-target-ref-omitted')
    expect(rendered).toContain('unsafe-extensibility-request-material-omitted')
    expect(rendered).toContain('unsafe extensibility ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw shell')
    expect(rendered).not.toContain('raw config')
    expect(rendered).not.toContain('skill body raw text')
  })

  test('renders active progress for running Runs', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('queued_or_running', null, {
            executionCloseout: null,
            nextAction: {
              callerActionRefs: ['next-action.public.poll.work_1'],
              reasonRefs: [],
              retryAfterSeconds: 15,
              state: 'queued_or_running',
            },
          }),
          [
            workEvent(1, 'queued', 'queued_or_running'),
            workEvent(2, 'running', 'queued_or_running'),
          ],
        ),
      ),
    )

    expect(rendered).toContain('Run progress')
    expect(rendered).toContain('Running')
    expect(rendered).toContain('next-action.public.poll.work_1')
  })

  test('renders blocked progress and blocker refs', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(
          workForState('blocked', null, {
            executionCloseout: null,
            nextAction: {
              callerActionRefs: [],
              reasonRefs: ['reason.public.blocked.work_1'],
              retryAfterSeconds: null,
              state: 'blocked',
            },
          }),
          [
            workEvent(1, 'queued', 'queued_or_running'),
            workEvent(2, 'blocked', 'blocked'),
          ],
        ),
      ),
    )

    expect(rendered).toContain('Run progress')
    expect(rendered).toContain('Blocked')
    expect(rendered).toContain('reason.public.blocked.work_1')
  })

  test('omits unsafe progress refs before rendering', () => {
    const rendered = renderHtml(
      detailView(
        modelForWork(workForState('delivered', null), [
          workEvent(1, 'queued', 'queued_or_running', {
            eventRef: 'raw shell log /Users/christopher/private.log',
          }),
          workEvent(2, 'delivered', 'delivered'),
        ]),
      ),
    )

    expect(rendered).toContain('Run progress')
    expect(rendered).toContain('unsafe-progress-material-omitted')
    expect(rendered).toContain('unsafe progress ref(s) were omitted')
    expect(rendered).not.toContain('raw shell log')
    expect(rendered).not.toContain('/Users/christopher')
  })
})
