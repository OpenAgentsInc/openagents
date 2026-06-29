import { describe, expect, test } from 'vitest'

import type {
  AutopilotWorkCompactionBoundary,
  AutopilotWorkProjection,
  AutopilotWorkState,
} from '../model'
import {
  buildForgeCompactionSummaryInput,
  projectForgeCompactionSummary,
} from './compaction-summary'

const boundary = (
  overrides: Partial<AutopilotWorkCompactionBoundary> = {},
): AutopilotWorkCompactionBoundary => ({
  automaticFailureCount: 0,
  blockerRefs: [],
  boundaryRef: 'compaction-boundary.public.work_1.1',
  failureRefs: [],
  generatedAt: '2026-06-17T18:00:00.000Z',
  hookRefs: ['hook.public.compaction_policy'],
  policyRefs: ['policy.public.compaction.v1'],
  postEstimate: {
    estimateRef: 'context-estimate.public.work_1.post',
    messageCount: 44,
    tokenCount: 24_000,
  },
  preEstimate: {
    contextWindow: 128_000,
    estimateRef: 'context-estimate.public.work_1.pre',
    messageCount: 120,
    tokenCount: 112_000,
  },
  preservedAdapterRefs: ['adapter.public.codex.ready'],
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
  restoredPlanRefs: ['plan.public.work_1.active'],
  restoredSkillRefs: ['skill.public.typescript'],
  restoredTaskRefs: ['task.public.work_1'],
  retryRefs: [],
  state: 'compacted',
  strategy: 'summary_compact',
  summarySourceRefs: ['summary-source.public.work_1.boundary_1'],
  trigger: 'automatic',
  ...overrides,
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
    executionCloseout: null,
    fallbackLeaseIntents: [],
    funding: {},
    generatedAt: '2026-06-17T18:10:00.000Z',
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
    updatedAt: '2026-06-17T18:10:00.000Z',
    workOrderRef: 'work_1',
    ...overrides,
  }) as AutopilotWorkProjection

describe('Forge compaction summary projection', () => {
  test('projects compacted boundaries with explicit non-authority flags', () => {
    const view = projectForgeCompactionSummary({
      boundaries: [boundary()],
      compactionRef: 'compaction.public.work_1',
      generatedAt: '2026-06-17T18:10:00.000Z',
      workOrderRef: 'work_1',
    })

    expect(view).toMatchObject({
      authority: {
        acceptedOutcomeAuthority: false,
        automaticCompactionAuthority: false,
        deploymentAuthority: false,
        modelSummarizationAuthority: false,
        publicClaimAuthority: false,
        runtimeRetryAuthority: false,
        settlementAuthority: false,
        transcriptMutationAuthority: false,
        workerPayoutAuthority: false,
      },
      publicSafe: true,
      status: 'compacted',
      workOrderRef: 'work_1',
    })
    expect(view.boundaries[0]).toMatchObject({
      boundaryRef: 'compaction-boundary.public.work_1.1',
      state: 'compacted',
      strategy: 'summary_compact',
      trigger: 'automatic',
    })
    expect(view.boundaries[0]?.preEstimate?.tokenCount).toBe(112_000)
    expect(view.boundaries[0]?.postEstimate?.tokenCount).toBe(24_000)
    expect(view.boundaries[0]?.restoredFileRefs).toContain(
      'file.public.work_1.app_ts',
    )
  })

  test('keeps normal Runs empty when no compaction evidence exists', () => {
    const view = projectForgeCompactionSummary(
      buildForgeCompactionSummaryInput(work('queued_or_running')),
    )

    expect(view.status).toBe('empty')
    expect(view.boundaries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks failed or cancelled boundaries that pretend to have post-state', () => {
    const failed = projectForgeCompactionSummary({
      boundaries: [
        boundary({
          boundaryRef: 'compaction-boundary.public.work_1.failed',
          state: 'failed',
          strategy: 'reactive_compact',
          trigger: 'reactive',
        }),
      ],
      generatedAt: '2026-06-17T18:10:00.000Z',
      workOrderRef: 'work_1',
    })
    const cancelled = projectForgeCompactionSummary({
      boundaries: [
        boundary({
          boundaryRef: 'compaction-boundary.public.work_1.cancelled',
          state: 'cancelled',
          strategy: 'summary_compact',
          trigger: 'manual',
        }),
      ],
      generatedAt: '2026-06-17T18:10:00.000Z',
      workOrderRef: 'work_1',
    })

    expect(failed.status).toBe('blocked')
    expect(failed.blockerRefs.some(ref =>
      ref.includes('failed-compaction-has-post-state')
    )).toBe(true)
    expect(cancelled.status).toBe('blocked')
    expect(cancelled.blockerRefs.some(ref =>
      ref.includes('cancelled-compaction-has-post-state')
    )).toBe(true)
  })

  test('blocks unmatched tool request/result pairs', () => {
    const view = projectForgeCompactionSummary({
      boundaries: [
        boundary({
          preservedToolPairs: [
            {
              requestRef: 'tool-request.public.work_1.shell_1',
            },
          ],
        }),
      ],
      generatedAt: '2026-06-17T18:10:00.000Z',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs.some(ref =>
      ref.includes('unmatched-tool-pair:tool-request.public.work_1.shell_1')
    )).toBe(true)
  })

  test('blocks repeated failed automatic compactions with circuit breaker ref', () => {
    const automaticFailedBoundary = boundary({
      automaticFailureCount: 2,
      boundaryRef: 'compaction-boundary.public.work_1.automatic_failed',
      state: 'failed',
      trigger: 'automatic',
    })
    const {
      postEstimate: _postEstimate,
      ...automaticFailedBoundaryWithoutPostState
    } = automaticFailedBoundary
    const view = projectForgeCompactionSummary({
      boundaries: [automaticFailedBoundaryWithoutPostState],
      generatedAt: '2026-06-17T18:10:00.000Z',
      workOrderRef: 'work_1',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs.some(ref =>
      ref.includes('automatic-compaction-circuit-breaker')
    )).toBe(true)
  })

  test('omits unsafe private compaction material before projection', () => {
    const view = projectForgeCompactionSummary({
      boundaries: [
        boundary({
          boundaryRef: 'compaction-boundary.public.safe',
          failureRefs: ['raw transcript /Users/christopher/private.jsonl'],
          hookRefs: ['hook.public.safe', 'raw shell log /Users/christopher/run.log'],
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
          restoredFileRefs: ['file.public.safe', '/Users/christopher/app.ts'],
          summarySourceRefs: [
            'summary-source.public.safe',
            'raw summary /Users/christopher/summary.md',
          ],
        }),
      ],
      compactionRef: 'compaction.public.work_1',
      generatedAt: '2026-06-17T18:10:00.000Z',
      workOrderRef: 'work_1',
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.boundaries[0]?.preservedRecentMessageRefs).toEqual([
      'message.public.safe',
    ])
    expect(view.boundaries[0]?.restoredFileRefs).toEqual(['file.public.safe'])
    expect(view.boundaries[0]?.summarySourceRefs).toEqual([
      'summary-source.public.safe',
    ])
    expect(view.blockerRefs).toContain(
      'forge-compaction-blocker:work_1:unsafe-compaction-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw prompt')
    expect(payload).not.toContain('raw shell')
    expect(payload).not.toContain('raw transcript')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
  })
})
