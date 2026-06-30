import { describe, expect, test } from 'vitest'

import {
  KhalaDelegationGepaFeedbackUnsafe,
  buildKhalaDelegationGepaFeedback,
} from './khala-delegation-gepa-feedback'
import {
  buildKhalaDelegationExampleDataset,
  type KhalaDelegationAssignmentRow,
  type KhalaDelegationEventRow,
  type KhalaDelegationTokenUsageRow,
  type KhalaDelegationTraceRow,
} from './khala-delegation-example-dataset'

const rowFor = (
  overrides: Partial<KhalaDelegationAssignmentRow>,
): KhalaDelegationAssignmentRow => ({
  assignment_ref: 'pylon_assignment_good',
  pylon_ref: 'pylon.openagents.khala',
  job_kind: 'codex_agent_task',
  state: 'accepted',
  payment_mode: 'no-spend',
  task_refs_json: JSON.stringify(['issue:7738', 'task:gd1_metric']),
  acceptance_criteria_refs_json: JSON.stringify([
    'acceptance.delegation.metric.feedback',
  ]),
  result_expectation_refs_json: JSON.stringify([
    'expectation.delegation.merged_clean',
  ]),
  artifact_refs_json: JSON.stringify(['artifact.delegation.metric.good']),
  proof_refs_json: JSON.stringify(['proof.delegation.metric.good']),
  accepted_work_refs_json: JSON.stringify([
    'accepted_work.github.OpenAgentsInc.openagents.pull.7738',
  ]),
  rejection_refs_json: JSON.stringify([]),
  closeout_refs_json: JSON.stringify(['khala.closeout.accepted.7738']),
  public_projection_json: JSON.stringify({
    assignmentRef: 'pylon_assignment_good',
    pullRequestRef: 'github.com/OpenAgentsInc/openagents/pull/7738',
    mergeRef: 'merge.main.commit.7738',
  }),
  created_at: '2026-06-30T12:00:00.000Z',
  updated_at: '2026-06-30T12:15:00.000Z',
  ...overrides,
})

const tokenFor = (
  assignmentRef: string,
  totalTokens: number,
): KhalaDelegationTokenUsageRow => ({
  id: `token-${assignmentRef}`,
  observed_at: '2026-06-30T12:05:00.000Z',
  run_ref: `run.${assignmentRef}`,
  session_ref: `session.${assignmentRef}`,
  task_ref: assignmentRef,
  repository_ref: 'github:OpenAgentsInc/openagents',
  provider: 'openai',
  model: 'codex',
  input_tokens: Math.floor(totalTokens * 0.7),
  output_tokens: Math.floor(totalTokens * 0.2),
  reasoning_tokens: Math.floor(totalTokens * 0.1),
  cache_read_tokens: 0,
  cache_write_tokens: 0,
  total_tokens: totalTokens,
  usage_truth: 'exact',
  demand_kind: 'own_capacity',
  demand_source: 'khala_fleet_delegate',
  demand_client: 'pylon',
})

const traceFor = (
  assignmentRef: string,
  message: string,
): KhalaDelegationTraceRow => ({
  trace_uuid: `trace-${assignmentRef}`,
  schema_version: 'atif.v1.7',
  trajectory_id: `pylon_codex:${assignmentRef}:turn:0`,
  session_id: `session.${assignmentRef}`,
  visibility: 'owner_only',
  step_count: 2,
  trajectory_json: JSON.stringify({
    trajectoryId: `pylon_codex:${assignmentRef}:turn:0`,
    steps: [{ role: 'assistant', message }],
  }),
  demand_kind: 'own_capacity',
  demand_source: 'khala_fleet_delegate',
  created_at: '2026-06-30T12:07:00.000Z',
})

const eventsFor = (
  assignmentRef: string,
  projection: Record<string, unknown>,
): ReadonlyArray<KhalaDelegationEventRow> => [
  {
    assignment_ref: assignmentRef,
    event_ref: `pylon.event.${assignmentRef}.created`,
    event_kind: 'assignment_created',
    status: 'accepted',
    public_projection_json: JSON.stringify({ assignmentRef }),
    created_at: '2026-06-30T12:00:00.000Z',
  },
  {
    assignment_ref: assignmentRef,
    event_ref: `pylon.event.${assignmentRef}.progress`,
    event_kind: 'assignment_progress',
    status: 'reported',
    public_projection_json: JSON.stringify(projection),
    created_at: '2026-06-30T12:04:00.000Z',
  },
]

const exampleFor = (
  assignment: KhalaDelegationAssignmentRow,
  events: ReadonlyArray<KhalaDelegationEventRow>,
  tokenRows: ReadonlyArray<KhalaDelegationTokenUsageRow>,
  traceRows: ReadonlyArray<KhalaDelegationTraceRow>,
) =>
  buildKhalaDelegationExampleDataset({
    generatedAt: '2026-06-30T13:00:00.000Z',
    assignments: [assignment],
    eventsByAssignmentRef: {
      [assignment.assignment_ref]: events,
    },
    tokenRowsByAssignmentRef: {
      [assignment.assignment_ref]: tokenRows,
    },
    traceRowsByAssignmentRef: {
      [assignment.assignment_ref]: traceRows,
    },
  }).examples[0]!

describe('Khala delegation GEPA feedback', () => {
  test('scores a known-good merged-clean delegation highly', () => {
    const good = exampleFor(
      rowFor({}),
      eventsFor('pylon_assignment_good', {
        state: 'advertise_capacity_recovered',
        availableCodexAssignments: 0,
        maxCodexAssignments: 1,
      }),
      [tokenFor('pylon_assignment_good', 1787)],
      [traceFor('pylon_assignment_good', 'Produced a clean PR and merge refs.')],
    )

    const feedback = buildKhalaDelegationGepaFeedback({
      candidateRef: 'candidate.khala.fleet.delegation.seed',
      example: good,
    })

    expect(feedback.dimensions).toMatchObject({
      admitted_first_try: 1,
      conflict_churn: 0,
      idle_gap_seconds: 0,
      merged_clean: 1,
      single_prompt_success: 1,
      token_cost_tokens: 1787,
      wall_clock_seconds: 900,
    })
    expect(feedback.scoreBps).toBeGreaterThanOrEqual(8900)
    expect(feedback.failureRefs).toEqual([])
    expect(feedback.preconditionRefs).toEqual([
      'blocker.public.pylon_dispatch.no_available_codex_capacity',
    ])
    expect(feedback.runtimePromotionAllowed).toBe(false)
    expect(feedback.rawPromptIncluded).toBe(false)
  })

  test('scores a known-bad delegation lower and names the real blocker refs, including 0/1 capacity', () => {
    const badAssignment = rowFor({
      assignment_ref: 'pylon_assignment_bad',
      accepted_work_refs_json: JSON.stringify([]),
      artifact_refs_json: JSON.stringify([]),
      closeout_refs_json: JSON.stringify(['khala.closeout.rejected.7738']),
      proof_refs_json: JSON.stringify([]),
      public_projection_json: JSON.stringify({
        assignmentRef: 'pylon_assignment_bad',
        blockerRefs: [
          'blocker.public.pylon_dispatch.no_available_codex_capacity',
          'blocker.public.pylon_dispatch.duplicate_active_assignment',
          'evidence.khala_coding.target_pylon_ref.unavailable.stale_or_missing_heartbeat',
          'blocker.public.khala_delegation.pr_conflicted',
          'blocker.public.khala_delegation.objective_too_vague',
        ],
        capacity: {
          availableCodexAssignments: 0,
          maxCodexAssignments: 1,
        },
      }),
      rejection_refs_json: JSON.stringify([
        'rejection.public.pylon_assignment.verify_failed',
      ]),
      state: 'accepted',
      updated_at: '2026-06-30T12:45:00.000Z',
    })
    const bad = exampleFor(
      badAssignment,
      eventsFor('pylon_assignment_bad', {
        blockerRefs: [
          'blocker.public.pylon_dispatch.no_available_codex_capacity',
          'blocker.public.pylon_dispatch.duplicate_active_assignment',
        ],
        idleGapSeconds: 120,
        maxCodexAssignments: 1,
        availableCodexAssignments: 0,
      }),
      [tokenFor('pylon_assignment_bad', 22_000)],
      [
        traceFor(
          'pylon_assignment_bad',
          'verify_failed after stale_or_missing_heartbeat; pr_conflicted and objective_too_vague.',
        ),
      ],
    )

    const feedback = buildKhalaDelegationGepaFeedback({
      candidateRef: 'candidate.khala.fleet.delegation.bad',
      example: bad,
    })

    expect(feedback.scoreBps).toBeLessThan(1000)
    expect(feedback.dimensions).toMatchObject({
      admitted_first_try: 0,
      conflict_churn: 1,
      idle_gap_seconds: 120,
      merged_clean: 0,
      single_prompt_success: 0,
      token_cost_tokens: 22000,
      wall_clock_seconds: 2700,
    })
    expect(feedback.failureRefs).toEqual([
      'blocker.public.khala_delegation.objective_too_vague',
      'blocker.public.khala_delegation.pr_conflicted',
      'blocker.public.khala_delegation.vacuous_pr',
      'blocker.public.pylon_assignment.verify_failed',
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
      'blocker.public.pylon_dispatch.no_available_codex_capacity',
      'blocker.public.pylon_dispatch.pylon_stale',
    ])
    expect(feedback.preconditionRefs).toEqual([
      'blocker.public.pylon_dispatch.no_available_codex_capacity',
    ])
    expect(JSON.stringify(feedback)).not.toContain('verify_failed after')
  })

  test('rejects unsafe feedback refs', () => {
    const good = exampleFor(
      rowFor({}),
      eventsFor('pylon_assignment_good', {
        state: 'running',
        availableCodexAssignments: 0,
        maxCodexAssignments: 1,
      }),
      [tokenFor('pylon_assignment_good', 1787)],
      [traceFor('pylon_assignment_good', 'Produced a clean PR and merge refs.')],
    )

    expect(() =>
      buildKhalaDelegationGepaFeedback({
        candidateRef: '/Users/alice/private/candidate',
        example: good,
      }),
    ).toThrow(KhalaDelegationGepaFeedbackUnsafe)
  })
})
