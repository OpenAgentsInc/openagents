import { describe, expect, test } from 'vitest'

import {
  classifyCodingWorkflow,
  fleetWorkerKindForCodingWorkflowClass,
} from './coding-workflow-classifier'

describe('coding workflow classifier', () => {
  test('classifies explicit structured workflow markers', () => {
    expect(
      classifyCodingWorkflow({
        messages: [{ content: 'hello', role: 'user' }],
        rawBody: {
          openagents: { workflow_class: 'codex_agent_task' },
        },
      }),
    ).toMatchObject({
      confidence: 1,
      workflowClass: 'codex_agent_task',
    })
  })

  test('classifies explicit claude_agent_task workflow markers', () => {
    expect(
      classifyCodingWorkflow({
        messages: [{ content: 'hello', role: 'user' }],
        rawBody: {
          openagents: { workflowClass: 'claude_agent_task' },
        },
      }),
    ).toMatchObject({
      confidence: 1,
      workflowClass: 'claude_agent_task',
    })
  })

  test('does not route prose by keyword', () => {
    expect(
      classifyCodingWorkflow({
        messages: [
          {
            content:
              'Please code a fix, edit the repo, run tests, and commit it.',
            role: 'user',
          },
        ],
        rawBody: {},
      }),
    ).toMatchObject({
      confidence: 0,
      workflowClass: 'none',
    })
  })

  test('projects workflow classes into fleet worker-kind hints without prose inference', () => {
    expect(fleetWorkerKindForCodingWorkflowClass('claude_agent_task')).toBe(
      'claude',
    )
    expect(fleetWorkerKindForCodingWorkflowClass('codex_agent_task')).toBe(
      'codex',
    )
    expect(fleetWorkerKindForCodingWorkflowClass('cloud_coding_session')).toBe(
      'codex',
    )
    expect(fleetWorkerKindForCodingWorkflowClass('none')).toBe('none')
  })
})
