import { describe, expect, test } from 'vitest'

import {
  assignmentIntentsForWorkOrder,
} from './autopilot-work-assignment-planner'

describe('Autopilot work assignment planner', () => {
  test('records blocked tasks without granting authority', () => {
    const work = {
      tasks: [
        {
          acceptanceCriteriaRefs: ['acceptance.blocked.manual_review'],
          accessRequirements: [],
          accessState: 'satisfied',
          checkout: null,
          kind: 'research_and_patch',
          lifecycleState: 'blocked',
          objective: 'Research a public-safe blocker and return refs.',
          paymentState: 'not_required',
          placementState: 'blocked',
          repository: null,
          taskRef: 'task.autopilot_coder.blocked_research',
        },
      ],
      workOrderRef: 'autopilot_work_order.blocked_test',
    } as const

    expect(assignmentIntentsForWorkOrder(work)).toEqual([
      {
        accessState: 'satisfied',
        assignmentIntentRef:
          'assignment_intent.autopilot_work_order.blocked_test.task.autopilot_coder.blocked_research',
        assignmentKind: 'research_and_patch',
        deployAuthority: false,
        paymentState: 'not_required',
        placementState: 'blocked',
        plannerReasonRefs: ['assignment.blocked.lifecycle_blocked'],
        plannerState: 'blocked',
        readyForAssignment: false,
        repository: null,
        spendAuthority: false,
        taskRef: 'task.autopilot_coder.blocked_research',
        workerPayoutEligible: false,
        workOrderRef: 'autopilot_work_order.blocked_test',
      },
    ])
  })

  test('records queued tasks without producing another ready assignment', () => {
    const work = {
      tasks: [
        {
          acceptanceCriteriaRefs: ['acceptance.docs.updated'],
          accessRequirements: [],
          accessState: 'satisfied',
          checkout: null,
          kind: 'code_change',
          lifecycleState: 'queued_or_running',
          objective: 'Update the public docs contract.',
          paymentState: 'not_required',
          placementState: 'queued_or_running',
          repository: {
            branch: 'main',
            fullName: 'OpenAgentsInc/openagents',
            provider: 'github',
            visibility: 'public',
          },
          taskRef: 'task.autopilot_coder.docs_contract',
        },
      ],
      workOrderRef: 'autopilot_work_order.queued_test',
    } as const

    expect(assignmentIntentsForWorkOrder(work)).toEqual([
      expect.objectContaining({
        assignmentIntentRef:
          'assignment_intent.autopilot_work_order.queued_test.task.autopilot_coder.docs_contract',
        plannerReasonRefs: ['assignment.queued_or_running'],
        plannerState: 'queued_or_running',
        readyForAssignment: false,
        taskRef: 'task.autopilot_coder.docs_contract',
      }),
    ])
  })
})
