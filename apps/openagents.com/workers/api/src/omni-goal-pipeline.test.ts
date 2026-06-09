import { describe, expect, test } from 'vitest'

import {
  goalRuntimeEventFromRunEvent,
  goalRuntimeEventFromRunStatus,
} from './omni-goal-event-mapping'
import type { OmniEventRecord } from './omni-runs'

const omniEvent = (
  input: Partial<OmniEventRecord> & Pick<OmniEventRecord, 'sequence' | 'type'>,
): OmniEventRecord => ({
  artifactRefs: input.artifactRefs ?? [],
  createdAt: input.createdAt ?? '2026-06-04T00:00:00.000Z',
  externalEventId: input.externalEventId ?? null,
  id: input.id ?? `event_${input.sequence}`,
  parentId: input.parentId ?? 'run_1',
  payloadJson: input.payloadJson ?? null,
  sequence: input.sequence,
  source: input.source ?? 'runner',
  status: input.status ?? null,
  summary: input.summary ?? 'Runner event.',
  type: input.type,
})

describe('omni goal pipeline mapping', () => {
  test('maps accepted launch event into a goal run attachment event', () => {
    const event = goalRuntimeEventFromRunEvent(
      'agent_goal_1',
      omniEvent({
        sequence: 1,
        source: 'openagents',
        status: 'accepted',
        type: 'agent_run.accepted',
      }),
    )

    expect(event).toMatchObject({
      externalEventId: 'run_1:1:agent_run.accepted',
      goalId: 'agent_goal_1',
      runId: 'run_1',
      type: 'RunAccepted',
    })
  })

  test('maps token usage event into idempotent goal usage accounting', () => {
    const event = goalRuntimeEventFromRunEvent(
      'agent_goal_1',
      omniEvent({
        payloadJson: JSON.stringify({
          dataJson: JSON.stringify({
            detail: JSON.stringify({
              type: 'turn.completed',
              usage: {
                cached_input_tokens: 5,
                input_tokens: 20,
                output_tokens: 8,
                reasoning_output_tokens: 2,
              },
            }),
          }),
        }),
        sequence: 4,
        type: 'runner.log',
      }),
    )

    expect(event).toMatchObject({
      externalEventId: 'usage:run_1:4',
      goalId: 'agent_goal_1',
      runId: 'run_1',
      tokenDelta: 30,
      type: 'UsageAccounted',
    })
  })

  test('keeps body-level completion as one synthetic status event', () => {
    const records = [
      omniEvent({ sequence: 3, type: 'runner.log' }),
      omniEvent({ sequence: 4, type: 'runner.log' }),
    ]
    const firstRecordEvent = goalRuntimeEventFromRunEvent(
      'agent_goal_1',
      records[0]!,
    )
    const statusEvent = goalRuntimeEventFromRunStatus(
      'agent_goal_1',
      'run_1',
      'completed',
      records,
      42,
    )

    expect(firstRecordEvent).toBeUndefined()
    expect(statusEvent).toMatchObject({
      externalEventId: 'run-status:run_1:completed:4',
      goalId: 'agent_goal_1',
      runId: 'run_1',
      timeDeltaSeconds: 42,
      type: 'RunCompleted',
    })
  })
})
