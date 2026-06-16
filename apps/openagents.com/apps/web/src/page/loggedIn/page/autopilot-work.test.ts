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
                changedProfileKinds: ['command', 'test'],
                commandProfileRefs: [
                  'profile.command.public.package_scripts.sha256_abcd',
                ],
                generatedAt: '2026-06-16T18:00:00.000Z',
                instructionRefs: ['profile.instruction.public.AGENTS.sha256_abcd'],
                profileRef: 'repository-profile.public.openagents.main',
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
                testProfileRefs: ['profile.test.public.vitest.sha256_abcd'],
              },
            },
          }),
        ),
      ),
    )

    expect(rendered).toContain('Repository memory profile')
    expect(rendered).toContain('repository-profile.public.openagents.main')
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
                commandProfileRefs: [
                  'profile.command.public.safe',
                  'raw command /Users/christopher/private.sh',
                ],
                generatedAt: '2026-06-16T18:00:00.000Z',
                instructionRefs: [
                  'profile.instruction.public.safe',
                  'raw prompt /Users/christopher/private.md',
                ],
                profileRef: 'repository-profile.public.openagents.main',
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
    expect(rendered).toContain('unsafe-profile-material-omitted')
    expect(rendered).toContain('unsafe repository-memory ref(s) were omitted')
    expect(rendered).not.toContain('/Users/christopher')
    expect(rendered).not.toContain('raw command')
    expect(rendered).not.toContain('raw prompt')
    expect(rendered).not.toContain('raw test')
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
