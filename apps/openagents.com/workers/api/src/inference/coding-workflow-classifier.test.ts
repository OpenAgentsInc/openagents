import { describe, expect, test } from 'vitest'

import { classifyCodingWorkflow } from './coding-workflow-classifier'

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
})
