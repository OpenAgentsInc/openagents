import { describe, expect, test } from 'vitest'

import {
  pylonAssignmentIntentsForAutopilotWork,
} from './autopilot-work-pylon-assignment-synthesizer'

const readyAssignment = {
  accessState: 'satisfied',
  assignmentIntentRef:
    'assignment_intent.autopilot_work_order.test.task.docs',
  assignmentKind: 'repo_change',
  deployAuthority: false,
  paymentState: 'not_required',
  placementState: 'ready_for_assignment',
  plannerReasonRefs: [
    'assignment.free_slice',
    'assignment.ready_for_assignment',
  ],
  plannerState: 'free_slice',
  readyForAssignment: true,
  repository: {
    branch: 'main',
    fullName: 'OpenAgentsInc/openagents',
    provider: 'github',
    visibility: 'public',
  },
  spendAuthority: false,
  taskRef: 'task.docs',
  workerPayoutEligible: false,
  workOrderRef: 'autopilot_work_order.test',
} as const

const task = {
  acceptanceCriteriaRefs: ['acceptance.docs.updated'],
  accessRequirements: [],
  accessState: 'satisfied',
  kind: 'code_change',
  lifecycleState: 'ready_for_assignment',
  paymentState: 'not_required',
  placementState: 'ready_for_assignment',
  repository: readyAssignment.repository,
  taskRef: 'task.docs',
} as const

describe('Autopilot Pylon assignment synthesizer', () => {
  test('creates no-spend controlled Pylon assignment intents for ready tasks', () => {
    const intents = pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents: [readyAssignment],
      placementDecision: {
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [],
        reasonRefs: [
          'placement.selected.requester_pylon',
          'placement.pylon.preferred_before_fallback',
        ],
        selectedPylonRef: 'pylon.local.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      tasks: [task],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents).toEqual([
      {
        acceptanceCriteriaRefs: ['acceptance.docs.updated'],
        assignmentRef:
          'pylon_assignment.autopilot_work_order.test.task.docs',
        closeoutPathRefs: [
          'closeout.pylon_assignment.autopilot_work_order.test.task.docs.diff_or_summary_required',
          'closeout.pylon_assignment.autopilot_work_order.test.task.docs.tests_or_blocker_required',
          'closeout.pylon_assignment.autopilot_work_order.test.task.docs.accepted_work_not_implied',
        ],
        forumAutoPublishAllowed: false,
        jobKind: 'validation',
        noForumAutoPublishRefs: [
          'forum_autopublish_disabled.pylon_assignment.autopilot_work_order.test.task.docs',
        ],
        paymentMode: 'unpaid_smoke',
        pylonRef: 'pylon.local.docs_agent',
        requiredCapabilityRefs: [
          'capability.pylon.assignment_ready',
          'capability.pylon.local_codex',
          'capability.pylon.local_coding_agent',
        ],
        resultExpectationRefs: [
          'result.pylon_assignment.autopilot_work_order.test.task.docs.public_safe_closeout',
        ],
        rollbackRefs: [
          'rollback.pylon_assignment.autopilot_work_order.test.task.docs.no_deploy_without_owner_acceptance',
        ],
        selectionPolicyRefs: [
          'placement.selected.requester_pylon',
          'placement.pylon.preferred_before_fallback',
        ],
        spendCapRefs: ['spend_cap.no_spend.autopilot_pylon_assignment'],
        taskRef: 'task.docs',
      },
    ])
  })

  test('does not synthesize leases without a selected requester Pylon', () => {
    expect(
      pylonAssignmentIntentsForAutopilotWork({
        assignmentIntents: [readyAssignment],
        placementDecision: {
          fallbackRunnerKind: 'openagents_shc',
          pylonCandidates: [],
          reasonRefs: ['placement.selected.fallback'],
          selectedPylonRef: null,
          selectedRunnerKind: 'openagents_shc',
          source: 'fallback',
        },
        tasks: [task],
        workOrderRef: 'autopilot_work_order.test',
      }),
    ).toEqual([])
  })
})
