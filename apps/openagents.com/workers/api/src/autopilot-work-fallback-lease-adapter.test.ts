import { describe, expect, test } from 'vitest'

import {
  fallbackLeaseIntentsForAutopilotWork,
} from './autopilot-work-fallback-lease-adapter'

const paidReadyAssignment = {
  accessState: 'satisfied',
  assignmentIntentRef:
    'assignment_intent.autopilot_work_order.test.task.repair',
  assignmentKind: 'test_repair',
  deployAuthority: false,
  paymentState: 'funded',
  placementState: 'ready_for_assignment',
  plannerReasonRefs: [
    'assignment.paid_ready',
    'assignment.ready_for_assignment',
  ],
  plannerState: 'paid_ready',
  readyForAssignment: true,
  repository: {
    branch: 'main',
    fullName: 'OpenAgentsInc/openagents',
    provider: 'github',
    visibility: 'public',
  },
  spendAuthority: false,
  taskRef: 'task.repair',
  workerPayoutEligible: false,
  workOrderRef: 'autopilot_work_order.test',
} as const

const task = {
  acceptanceCriteriaRefs: ['acceptance.tests.pass'],
  accessRequirements: [],
  accessState: 'satisfied',
  checkout: null,
  kind: 'test_repair',
  lifecycleState: 'ready_for_assignment',
  objective: 'Repair the public test fixture.',
  paymentState: 'funded',
  placementState: 'ready_for_assignment',
  repository: paidReadyAssignment.repository,
  taskRef: 'task.repair',
} as const

describe('Autopilot fallback lease adapter', () => {
  test('creates buyer-funded fallback lease intents for SHC lanes', () => {
    const intents = fallbackLeaseIntentsForAutopilotWork({
      assignmentIntents: [paidReadyAssignment],
      placementDecision: {
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [],
        reasonRefs: [
          'placement.selected.fallback',
          'placement.fallback.openagents_shc',
        ],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
        selectedPylonRef: null,
        selectedRunnerKind: 'openagents_shc',
        source: 'fallback',
      },
      tasks: [task],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents).toEqual([
      {
        acceptanceCriteriaRefs: ['acceptance.tests.pass'],
        assignmentRef:
          'fallback_assignment.autopilot_work_order.test.task.repair',
        closeoutPathRefs: [
          'closeout.fallback_assignment.autopilot_work_order.test.task.repair.diff_or_summary_required',
          'closeout.fallback_assignment.autopilot_work_order.test.task.repair.tests_or_blocker_required',
          'closeout.fallback_assignment.autopilot_work_order.test.task.repair.accepted_work_not_implied',
        ],
        fallbackLaneRef: 'fallback_lane.openagents.shc',
        forumAutoPublishAllowed: false,
        jobKind: 'validation',
        noForumAutoPublishRefs: [
          'forum_autopublish_disabled.fallback_assignment.autopilot_work_order.test.task.repair',
        ],
        paymentMode: 'buyer_funded',
        requiredCapabilityRefs: [
          'capability.fallback.assignment_ready',
          'capability.openagents.shc',
        ],
        resultExpectationRefs: [
          'result.fallback_assignment.autopilot_work_order.test.task.repair.public_safe_closeout',
        ],
        rollbackRefs: [
          'rollback.fallback_assignment.autopilot_work_order.test.task.repair.no_deploy_without_owner_acceptance',
        ],
        runnerKind: 'openagents_shc',
        selectionPolicyRefs: [
          'placement.selected.fallback',
          'placement.fallback.openagents_shc',
        ],
        spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
        taskRef: 'task.repair',
        workerPayoutAuthority: false,
      },
    ])
  })

  test('does not create fallback leases for requester Pylon placement', () => {
    expect(
      fallbackLeaseIntentsForAutopilotWork({
        assignmentIntents: [paidReadyAssignment],
        placementDecision: {
          availabilityState: 'selected',
          callerActionRefs: [],
          fallbackRunnerKind: 'openagents_shc',
          pylonCandidates: [],
          reasonRefs: ['placement.selected.requester_pylon'],
          refusalReasonRefs: [],
          retryAfterSeconds: null,
          selectedPylonRef: 'pylon.local.docs_agent',
          selectedRunnerKind: 'requester_pylon',
          source: 'requester_pylon',
        },
        tasks: [task],
        workOrderRef: 'autopilot_work_order.test',
      }),
    ).toEqual([])
  })
})
