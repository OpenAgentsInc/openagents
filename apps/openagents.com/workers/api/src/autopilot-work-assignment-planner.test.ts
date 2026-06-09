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
          kind: 'research_and_patch',
          lifecycleState: 'blocked',
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
})
