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
  checkout: null,
  kind: 'code_change',
  lifecycleState: 'ready_for_assignment',
  objective: 'Update public docs.',
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
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [],
        reasonRefs: [
          'placement.selected.requester_pylon',
          'placement.pylon.preferred_before_fallback',
        ],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
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
        jobKind: 'claude_agent_task',
        noForumAutoPublishRefs: [
          'forum_autopublish_disabled.pylon_assignment.autopilot_work_order.test.task.docs',
        ],
        paymentMode: 'unpaid_smoke',
        pylonRef: 'pylon.local.docs_agent',
        requiredCapabilityRefs: [
          'capability.pylon.assignment_ready',
          'capability.pylon.local_claude_agent',
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
          'adapter_selection.default_no_candidate_capabilities',
        ],
        spendCapRefs: ['spend_cap.no_spend.autopilot_pylon_assignment'],
        taskRef: 'task.docs',
      },
    ])
  })

  test('a codex-only placed Pylon gets codex_agent_task and only its capability ref (CX5)', () => {
    const intents = pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents: [readyAssignment],
      placementDecision: {
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [
          {
            assignmentReady: true,
            capabilityRefs: [
              'capability.pylon.assignment_ready',
              'capability.pylon.local_codex',
            ],
            clientVersion: 'openagents.pylon@0.3.0-rc2',
            heartbeatFresh: true,
            latestHeartbeatAt: '2026-06-11T00:00:00.000Z',
            latestHeartbeatStatus: 'online',
            latestResourceMode: null,
            localExecutionReady: true,
            ownerLinked: true,
            pylonRef: 'pylon.local.docs_agent',
            reasonRefs: [],
            selected: true,
            status: 'active',
            versionCompatible: true,
            walletReady: true,
          },
        ],
        reasonRefs: ['placement.selected.requester_pylon'],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
        selectedPylonRef: 'pylon.local.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      tasks: [task],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents).toHaveLength(1)
    expect(intents[0]).toMatchObject({
      jobKind: 'codex_agent_task',
      requiredCapabilityRefs: [
        'capability.pylon.assignment_ready',
        'capability.pylon.local_codex',
      ],
    })
    expect(intents[0]?.selectionPolicyRefs).toContain(
      'adapter_selection.single_capability',
    )
  })

  test('a dual-capability placed Pylon gets the documented claude default (CX5)', () => {
    const intents = pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents: [readyAssignment],
      placementDecision: {
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [
          {
            assignmentReady: true,
            capabilityRefs: [
              'capability.pylon.local_claude_agent',
              'capability.pylon.local_codex',
            ],
            clientVersion: 'openagents.pylon@0.3.0-rc2',
            heartbeatFresh: true,
            latestHeartbeatAt: '2026-06-11T00:00:00.000Z',
            latestHeartbeatStatus: 'online',
            latestResourceMode: null,
            localExecutionReady: true,
            ownerLinked: true,
            pylonRef: 'pylon.local.docs_agent',
            reasonRefs: [],
            selected: true,
            status: 'active',
            versionCompatible: true,
            walletReady: true,
          },
        ],
        reasonRefs: ['placement.selected.requester_pylon'],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
        selectedPylonRef: 'pylon.local.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      tasks: [task],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents[0]).toMatchObject({
      jobKind: 'claude_agent_task',
      requiredCapabilityRefs: [
        'capability.pylon.assignment_ready',
        'capability.pylon.local_claude_agent',
      ],
    })
    expect(intents[0]?.selectionPolicyRefs).toContain(
      'adapter_selection.dual_capability_default',
    )
  })

  test('a codex requested adapter selects codex_agent_task on a dual-capability Pylon', () => {
    const intents = pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents: [readyAssignment],
      placementDecision: {
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [
          {
            assignmentReady: true,
            capabilityRefs: [
              'capability.pylon.local_claude_agent',
              'capability.pylon.local_codex',
            ],
            clientVersion: 'openagents.pylon@0.3.0-rc2',
            heartbeatFresh: true,
            latestHeartbeatAt: '2026-06-11T00:00:00.000Z',
            latestHeartbeatStatus: 'online',
            latestResourceMode: null,
            localExecutionReady: true,
            ownerLinked: true,
            pylonRef: 'pylon.local.docs_agent',
            reasonRefs: [],
            selected: true,
            status: 'active',
            versionCompatible: true,
            walletReady: true,
          },
        ],
        reasonRefs: ['placement.selected.requester_pylon'],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
        selectedPylonRef: 'pylon.local.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      tasks: [{ ...task, requestedAdapter: 'codex' }],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents[0]).toMatchObject({
      jobKind: 'codex_agent_task',
      requiredCapabilityRefs: [
        'capability.pylon.assignment_ready',
        'capability.pylon.local_codex',
      ],
    })
    expect(intents[0]?.selectionPolicyRefs).toContain(
      'adapter_selection.requester_required',
    )
  })

  test('a fable profile request selects the Claude Agent lane and carries the profile ref', () => {
    const intents = pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents: [readyAssignment],
      placementDecision: {
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [
          {
            assignmentReady: true,
            capabilityRefs: [
              'capability.pylon.local_claude_agent',
              'capability.pylon.local_codex',
            ],
            clientVersion: 'openagents.pylon@0.3.0-rc2',
            heartbeatFresh: true,
            latestHeartbeatAt: '2026-06-11T00:00:00.000Z',
            latestHeartbeatStatus: 'online',
            latestResourceMode: null,
            localExecutionReady: true,
            ownerLinked: true,
            pylonRef: 'pylon.local.docs_agent',
            reasonRefs: [],
            selected: true,
            status: 'active',
            versionCompatible: true,
            walletReady: true,
          },
        ],
        reasonRefs: ['placement.selected.requester_pylon'],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
        selectedPylonRef: 'pylon.local.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      tasks: [{
        ...task,
        requestedAdapter: 'claude_agent',
        requestedAdapterProfileRef: 'profile.claude_agent.fable',
      }],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents[0]).toMatchObject({
      jobKind: 'claude_agent_task',
      requiredCapabilityRefs: [
        'capability.pylon.assignment_ready',
        'capability.pylon.local_claude_agent',
      ],
    })
    expect(intents[0]?.selectionPolicyRefs).toContain(
      'profile.claude_agent.fable',
    )
  })

  test('a requested Claude/Fable lane is not silently substituted on a Codex-only Pylon', () => {
    const intents = pylonAssignmentIntentsForAutopilotWork({
      assignmentIntents: [readyAssignment],
      placementDecision: {
        availabilityState: 'selected',
        callerActionRefs: [],
        fallbackRunnerKind: 'openagents_shc',
        pylonCandidates: [
          {
            assignmentReady: true,
            capabilityRefs: ['capability.pylon.local_codex'],
            clientVersion: 'openagents.pylon@0.3.0-rc2',
            heartbeatFresh: true,
            latestHeartbeatAt: '2026-06-11T00:00:00.000Z',
            latestHeartbeatStatus: 'online',
            latestResourceMode: null,
            localExecutionReady: true,
            ownerLinked: true,
            pylonRef: 'pylon.local.docs_agent',
            reasonRefs: [],
            selected: true,
            status: 'active',
            versionCompatible: true,
            walletReady: true,
          },
        ],
        reasonRefs: ['placement.selected.requester_pylon'],
        refusalReasonRefs: [],
        retryAfterSeconds: null,
        selectedPylonRef: 'pylon.local.docs_agent',
        selectedRunnerKind: 'requester_pylon',
        source: 'requester_pylon',
      },
      tasks: [{
        ...task,
        requestedAdapter: 'claude_agent',
        requestedAdapterProfileRef: 'profile.claude_agent.fable',
      }],
      workOrderRef: 'autopilot_work_order.test',
    })

    expect(intents).toEqual([])
  })

  test('does not synthesize leases without a selected requester Pylon', () => {
    expect(
      pylonAssignmentIntentsForAutopilotWork({
        assignmentIntents: [readyAssignment],
        placementDecision: {
          availabilityState: 'selected',
          callerActionRefs: [],
          fallbackRunnerKind: 'openagents_shc',
          pylonCandidates: [],
          reasonRefs: ['placement.selected.fallback'],
          refusalReasonRefs: [],
          retryAfterSeconds: null,
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
