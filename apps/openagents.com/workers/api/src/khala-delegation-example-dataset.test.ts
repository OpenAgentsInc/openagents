import { describe, expect, test } from 'vitest'

import {
  KHALA_DELEGATION_EXAMPLE_DATASET_SCHEMA,
  KhalaDelegationExampleDatasetUnsafe,
  assertKhalaDelegationExampleDatasetPublicSafe,
  buildKhalaDelegationExampleDataset,
  readKhalaDelegationExampleDataset,
  type KhalaDelegationAssignmentRow,
  type KhalaDelegationEventRow,
  type KhalaDelegationTokenUsageRow,
  type KhalaDelegationTraceRow,
} from './khala-delegation-example-dataset'

const assignment: KhalaDelegationAssignmentRow = {
  assignment_ref: 'pylon_assignment_7734',
  pylon_ref: 'pylon.openagents.khala',
  job_kind: 'codex_agent_task',
  state: 'accepted',
  payment_mode: 'no-spend',
  task_refs_json: JSON.stringify(['issue:7734', 'task:delegation_example']),
  acceptance_criteria_refs_json: JSON.stringify([
    'acceptance.dataset.public_safe',
  ]),
  result_expectation_refs_json: JSON.stringify([
    'expectation.mutalisk_readable_delegation_example',
  ]),
  artifact_refs_json: JSON.stringify(['artifact.dataset.sample']),
  proof_refs_json: JSON.stringify([
    'proof.github.OpenAgentsInc.openagents.pull.7734',
  ]),
  accepted_work_refs_json: JSON.stringify([
    'accepted_work.github.OpenAgentsInc.openagents.pull.7734',
  ]),
  rejection_refs_json: JSON.stringify([]),
  closeout_refs_json: JSON.stringify(['khala.closeout.accepted.7734']),
  public_projection_json: JSON.stringify({
    assignmentRef: 'pylon_assignment_7734',
    pullRequestRef: 'github.com/OpenAgentsInc/openagents/pull/7734',
    mergeRef: 'merge.main.commit.7734',
  }),
  created_at: '2026-06-30T12:00:00.000Z',
  updated_at: '2026-06-30T12:15:00.000Z',
}

const eventRows: ReadonlyArray<KhalaDelegationEventRow> = [
  {
    assignment_ref: 'pylon_assignment_7734',
    event_ref: 'pylon.event.assignment_created.7734',
    event_kind: 'assignment_created',
    status: 'accepted',
    public_projection_json: JSON.stringify({
      assignmentRef: 'pylon_assignment_7734',
      state: 'accepted',
    }),
    created_at: '2026-06-30T12:00:00.000Z',
  },
  {
    assignment_ref: 'pylon_assignment_7734',
    event_ref: 'pylon.event.assignment_closeout.7734',
    event_kind: 'assignment_closeout',
    status: 'closed',
    public_projection_json: JSON.stringify({
      closeoutRef: 'khala.closeout.accepted.7734',
      mergeRef: 'merge.main.commit.7734',
    }),
    created_at: '2026-06-30T12:15:00.000Z',
  },
]

const tokenRows: ReadonlyArray<KhalaDelegationTokenUsageRow> = [
  {
    id: 'token-row-7734',
    observed_at: '2026-06-30T12:05:00.000Z',
    run_ref: 'run.pylon_assignment_7734',
    session_ref: 'session.pylon_assignment_7734',
    task_ref: 'pylon_assignment_7734',
    repository_ref: 'github:OpenAgentsInc/openagents',
    provider: 'openai',
    model: 'codex',
    input_tokens: 1234,
    output_tokens: 456,
    reasoning_tokens: 78,
    cache_read_tokens: 9,
    cache_write_tokens: 10,
    total_tokens: 1787,
    usage_truth: 'exact',
    demand_kind: 'own_capacity',
    demand_source: 'khala_fleet_delegate',
    demand_client: 'pylon',
  },
]

const traceRows: ReadonlyArray<KhalaDelegationTraceRow> = [
  {
    trace_uuid: 'trace-uuid-7734',
    schema_version: 'atif.v1.7',
    trajectory_id: 'pylon_codex:pylon_assignment_7734:turn:0',
    session_id: 'session.pylon_assignment_7734',
    visibility: 'owner_only',
    step_count: 2,
    trajectory_json: JSON.stringify({
      trajectoryId: 'pylon_codex:pylon_assignment_7734:turn:0',
      steps: [
        {
          role: 'assistant',
          message:
            'Checked a fake local path /Users/alice/work/openagents and fake key sk-abcdefghijklmnop0123456789ABCD.',
        },
      ],
    }),
    demand_kind: 'own_capacity',
    demand_source: 'khala_fleet_delegate',
    created_at: '2026-06-30T12:07:00.000Z',
  },
]

describe('Khala delegation_example dataset', () => {
  test('builds a Mutalisk-readable public-safe delegation example', () => {
    const dataset = buildKhalaDelegationExampleDataset({
      generatedAt: '2026-06-30T13:00:00.000Z',
      assignments: [assignment],
      eventsByAssignmentRef: {
        pylon_assignment_7734: eventRows,
      },
      tokenRowsByAssignmentRef: {
        pylon_assignment_7734: tokenRows,
      },
      traceRowsByAssignmentRef: {
        pylon_assignment_7734: traceRows,
      },
    })

    expect(dataset.schemaVersion).toBe(KHALA_DELEGATION_EXAMPLE_DATASET_SCHEMA)
    expect(dataset.sourceTables).toEqual([
      'pylon_api_assignments',
      'pylon_api_events',
      'token_usage_events',
      'agent_traces',
    ])
    expect(dataset.examples).toHaveLength(1)

    const example = dataset.examples[0]!
    expect(example.assignmentRef).toBe('pylon_assignment_7734')
    expect(example.input.taskRefs).toEqual([
      'issue:7734',
      'task:delegation_example',
    ])
    expect(
      example.rolloutTrace.lifecycleEvents.map(event => event.eventRef),
    ).toEqual([
      'pylon.event.assignment_created.7734',
      'pylon.event.assignment_closeout.7734',
    ])
    expect(example.rolloutTrace.exactTokenUsage[0]?.totalTokens).toBe(1787)
    expect(example.outcome.pullRequestRefs).toContain(
      'github.com/OpenAgentsInc/openagents/pull/7734',
    )
    expect(example.outcome.mergeRefs).toContain('merge.main.commit.7734')

    const serialized = JSON.stringify(dataset)
    expect(serialized).not.toContain('/Users/alice')
    expect(serialized).not.toContain('sk-abcdefghijklmnop')
    expect(serialized).toContain('[REDACTED:home_path]')
    expect(serialized).toContain('[REDACTED:provider_key]')
  })

  test('rejects manually corrupted datasets with raw local paths', () => {
    const dataset = buildKhalaDelegationExampleDataset({
      generatedAt: '2026-06-30T13:00:00.000Z',
      assignments: [assignment],
      eventsByAssignmentRef: {
        pylon_assignment_7734: eventRows,
      },
      tokenRowsByAssignmentRef: {
        pylon_assignment_7734: tokenRows,
      },
      traceRowsByAssignmentRef: {
        pylon_assignment_7734: [],
      },
    })

    const example = dataset.examples[0]!

    expect(() =>
      assertKhalaDelegationExampleDatasetPublicSafe({
        ...dataset,
        examples: [
          {
            ...example,
            input: {
              ...example.input,
              publicProjection: {
                leakedPath: '/Users/alice/work/private-prompt.txt',
              },
            },
          },
        ],
      }),
    ).toThrow(KhalaDelegationExampleDatasetUnsafe)
  })

  test('reads the current D1 tables by assignment lifecycle, token usage, and ATIF prefix', async () => {
    const queries: Array<{ sql: string; bindings: ReadonlyArray<unknown> }> = []
    const db = {
      prepare(sql: string) {
        return {
          bind(...bindings: ReadonlyArray<unknown>) {
            return {
              async all<T>() {
                queries.push({ sql, bindings })
                if (sql.includes('FROM pylon_api_assignments')) {
                  return { results: [assignment as T] }
                }
                if (sql.includes('FROM pylon_api_events')) {
                  return { results: eventRows as ReadonlyArray<T> }
                }
                if (sql.includes('FROM token_usage_events')) {
                  return { results: tokenRows as ReadonlyArray<T> }
                }
                if (sql.includes('FROM agent_traces')) {
                  return { results: traceRows as ReadonlyArray<T> }
                }
                return { results: [] as ReadonlyArray<T> }
              },
            }
          },
        }
      },
    } as unknown as D1Database

    const dataset = await readKhalaDelegationExampleDataset(db, {
      generatedAt: '2026-06-30T13:00:00.000Z',
      limit: 1,
    })

    expect(dataset.examples[0]?.assignmentRef).toBe('pylon_assignment_7734')
    expect(
      queries.some(
        query =>
          query.sql.includes('FROM token_usage_events') &&
          query.sql.includes('task_ref = ?') &&
          query.bindings.includes('pylon_assignment_7734'),
      ),
    ).toBe(true)
    expect(
      queries.some(
        query =>
          query.sql.includes('FROM agent_traces') &&
          query.bindings.includes('pylon_codex:pylon_assignment_7734:') &&
          query.bindings.includes('pylon_claude:pylon_assignment_7734:'),
      ),
    ).toBe(true)
  })
})
