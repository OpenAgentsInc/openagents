import { describe, expect, test } from 'vitest'

import {
  autopilotCodingAssignmentsForWork,
  decodeOpenAgentsAutopilotCodingAssignmentPayload,
  OpenAgentsAutopilotCodingAssignmentUnsafe,
} from './autopilot-coding-assignment'

const baseTask = {
  acceptanceCriteriaRefs: [
    'acceptance.docs.updated',
    'acceptance.tests.contract',
  ],
  accessRequirements: [],
  accessState: 'satisfied',
  checkout: {
    commitSha: '1745cd4b54b8a12a50922f80b5d345314c91d70d',
    kind: 'git_checkout',
    verificationCommand: {
      args: ['bun', 'test'],
      commandRef: 'command.public.autopilot_coder.bun_test',
    },
  },
  kind: 'code_change',
  lifecycleState: 'ready_for_assignment',
  objective: 'Add public-safe Autopilot coder contract docs.',
  paymentState: 'not_required',
  placementState: 'ready_for_assignment',
  repository: {
    branch: 'main',
    fullName: 'OpenAgentsInc/openagents',
    provider: 'github',
    visibility: 'public',
  },
  taskRef: 'task.autopilot_coder.docs_contract',
} as const

const pylonIntent = {
  acceptanceCriteriaRefs: baseTask.acceptanceCriteriaRefs,
  assignmentRef:
    'pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract',
  closeoutPathRefs: [
    'closeout.pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract.diff_or_summary_required',
    'closeout.pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract.tests_or_blocker_required',
    'closeout.pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract.accepted_work_not_implied',
  ],
  forumAutoPublishAllowed: false,
  jobKind: 'claude_agent_task',
  noForumAutoPublishRefs: [
    'forum_autopublish_disabled.pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract',
  ],
  paymentMode: 'unpaid_smoke',
  pylonRef: 'pylon.production.docs_agent',
  requiredCapabilityRefs: [
    'capability.pylon.assignment_ready',
    'capability.pylon.local_claude_agent',
  ],
  resultExpectationRefs: [
    'result.pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract.public_safe_closeout',
  ],
  rollbackRefs: [
    'rollback.pylon_assignment.autopilot_work_order.test_1.task.autopilot_coder.docs_contract.no_deploy_without_owner_acceptance',
  ],
  selectionPolicyRefs: [
    'placement.selected.requester_pylon',
    'placement.pylon.preferred_before_fallback',
  ],
  spendCapRefs: ['spend_cap.no_spend.autopilot_pylon_assignment'],
  taskRef: baseTask.taskRef,
} as const

const fallbackIntent = {
  acceptanceCriteriaRefs: ['acceptance.tests.pass'],
  assignmentRef:
    'fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair',
  closeoutPathRefs: [
    'closeout.fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair.diff_or_summary_required',
    'closeout.fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair.tests_or_blocker_required',
    'closeout.fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair.accepted_work_not_implied',
  ],
  fallbackLaneRef: 'fallback_lane.openagents.shc',
  forumAutoPublishAllowed: false,
  jobKind: 'validation',
  noForumAutoPublishRefs: [
    'forum_autopublish_disabled.fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair',
  ],
  paymentMode: 'buyer_funded',
  requiredCapabilityRefs: [
    'capability.fallback.assignment_ready',
    'capability.openagents.shc',
  ],
  resultExpectationRefs: [
    'result.fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair.public_safe_closeout',
  ],
  rollbackRefs: [
    'rollback.fallback_assignment.autopilot_work_order.test_2.task.autopilot_coder.paid_test_repair.no_deploy_without_owner_acceptance',
  ],
  runnerKind: 'openagents_shc',
  selectionPolicyRefs: [
    'placement.selected.fallback',
    'placement.fallback.openagents_shc',
  ],
  spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
  taskRef: 'task.autopilot_coder.paid_test_repair',
  workerPayoutAuthority: false,
} as const

const baseFunding = {
  buyerFundingState: 'not_required',
  buyerPaymentProofRef: null,
  fundedAmountCents: 0,
  quoteRef: 'quote.autopilot_work.test_1.0.openagents.autopilot_work_quote.v1',
  settlementBlockedReasonRef: 'settlement.no_worker_payout_mode',
  settlementEligible: false,
  workerPayoutEligible: false,
} as const

const expectUnsafeReason = (
  action: () => unknown,
  reasonPattern: RegExp,
): void => {
  try {
    action()
    throw new Error('Expected unsafe coding assignment error')
  } catch (error) {
    expect(error).toBeInstanceOf(OpenAgentsAutopilotCodingAssignmentUnsafe)
    expect((error as OpenAgentsAutopilotCodingAssignmentUnsafe).reason)
      .toMatch(reasonPattern)
  }
}

describe('Autopilot coding assignment contract', () => {
  test('maps requester Pylon work into a public-safe normalized coding assignment', () => {
    const assignments = autopilotCodingAssignmentsForWork({
      fallbackLeaseIntents: [],
      funding: baseFunding,
      paymentChallengeRef: null,
      pylonAssignmentIntents: [pylonIntent],
      quote: {
        maxSpendCents: 0,
        settlementMode: 'no_worker_payout',
      },
      tasks: [baseTask],
      workOrderRef: 'autopilot_work_order.test_1',
    })
    const payload = assignments[0]!

    expect(assignments).toHaveLength(1)
    expect(payload).toMatchObject({
      acceptanceCriteriaRefs: [
        'acceptance.docs.updated',
        'acceptance.tests.contract',
      ],
      allowedToolKinds: ['edit', 'file', 'git', 'shell', 'test_runner'],
      assignmentRef: pylonIntent.assignmentRef,
      budget: {
        maxSpendCents: 0,
        paymentMode: 'unpaid_smoke',
        timeoutSeconds: 900,
        workerPayoutAuthority: false,
      },
      closeoutSchema: {
        acceptedWorkAuthority: false,
        diffOrSummaryRequired: true,
        testsOrBlockerRequired: true,
      },
      claudeAgent: {
        agentKind: 'claude_agent_sdk',
        allowedToolKinds: ['edit', 'file', 'git', 'shell', 'test_runner'],
        schema: 'openagents.pylon.claude_agent_task.v0.3',
      },
      objective: {
        mode: 'ref_only',
        objectiveRef:
          'objective.autopilot_work_order.test_1.task.autopilot_coder.docs_contract',
        publicSummary: 'Add public-safe Autopilot coder contract docs.',
      },
      publicSafe: true,
      repository: {
        branch: 'main',
        commitSha: '1745cd4b54b8a12a50922f80b5d345314c91d70d',
        fullName: 'OpenAgentsInc/openagents',
        visibility: 'public',
      },
      runnerKind: 'requester_pylon',
      runnerRef: 'pylon.production.docs_agent',
      schema: 'openagents.autopilot_coding_assignment.v1',
      tracePolicy: {
        rawPromptAllowed: false,
        rawProviderPayloadAllowed: false,
        rawRunnerLogAllowed: false,
        rawSourceArchiveAllowed: false,
      },
      workOrderRef: 'autopilot_work_order.test_1',
      workspace: {
        kind: 'git_checkout',
        verificationCommand: {
          args: ['bun', 'test'],
          commandRef: 'command.public.autopilot_coder.bun_test',
        },
      },
    })
    expect(JSON.stringify(payload)).not.toContain('raw prompt')
    expect(decodeOpenAgentsAutopilotCodingAssignmentPayload(payload)).toEqual(
      payload,
    )
  })

  test('maps fallback work into the same normalized coding assignment shape', () => {
    const paidTask = {
      ...baseTask,
      acceptanceCriteriaRefs: ['acceptance.tests.pass'],
      kind: 'test_repair',
      paymentState: 'funded',
      taskRef: fallbackIntent.taskRef,
    } as const
    const assignments = autopilotCodingAssignmentsForWork({
      fallbackLeaseIntents: [fallbackIntent],
      funding: {
        ...baseFunding,
        buyerFundingState: 'funded',
        fundedAmountCents: 6400,
        quoteRef:
          'quote.autopilot_work.test_2.6400.openagents.autopilot_work_quote.v1',
        settlementBlockedReasonRef: 'settlement.accepted_work_required',
      },
      paymentChallengeRef:
        'challenge.quote.autopilot_work.test_2.6400.openagents.autopilot_work_quote.v1',
      pylonAssignmentIntents: [],
      quote: {
        maxSpendCents: 6500,
        settlementMode: 'no_worker_payout_until_accepted_work',
      },
      tasks: [paidTask],
      workOrderRef: 'autopilot_work_order.test_2',
    })
    const payload = assignments[0]!

    expect(payload).toMatchObject({
      assignmentRef: fallbackIntent.assignmentRef,
      budget: {
        buyerFundingState: 'funded',
        maxSpendCents: 6500,
        paymentChallengeRef:
          'challenge.quote.autopilot_work.test_2.6400.openagents.autopilot_work_quote.v1',
        paymentMode: 'buyer_funded',
        settlementMode: 'no_worker_payout_until_accepted_work',
        spendCapRefs: ['spend_cap.buyer_funded.fallback_assignment'],
      },
      laneRef: 'fallback_lane.openagents.shc',
      runnerKind: 'openagents_shc',
      runnerRef: 'fallback_lane.openagents.shc',
      taskKind: 'test_repair',
    })
  })

  test('rejects unsafe assignment payload fixtures before persistence', () => {
    const [payload] = autopilotCodingAssignmentsForWork({
      fallbackLeaseIntents: [],
      funding: baseFunding,
      paymentChallengeRef: null,
      pylonAssignmentIntents: [pylonIntent],
      quote: {
        maxSpendCents: 0,
        settlementMode: 'no_worker_payout',
      },
      tasks: [baseTask],
      workOrderRef: 'autopilot_work_order.test_1',
    })

    expectUnsafeReason(() =>
      decodeOpenAgentsAutopilotCodingAssignmentPayload({
        ...payload,
        authRefs: ['secret.private_key'],
      }),
      /private repo data/,
    )
  })

  test('rejects private repository refs until private lanes are modeled', () => {
    expectUnsafeReason(() =>
      autopilotCodingAssignmentsForWork({
        fallbackLeaseIntents: [],
        funding: baseFunding,
        paymentChallengeRef: null,
        pylonAssignmentIntents: [pylonIntent],
        quote: {
          maxSpendCents: 0,
          settlementMode: 'no_worker_payout',
        },
        tasks: [
          {
            ...baseTask,
            repository: {
              ...baseTask.repository,
              fullName: 'OpenAgentsInc/private-control',
              visibility: 'private',
            },
          },
        ],
        workOrderRef: 'autopilot_work_order.test_1',
      }),
      /public repo refs/,
    )
  })

  test('rejects placeholder repository commit pins', () => {
    expectUnsafeReason(() =>
      autopilotCodingAssignmentsForWork({
        fallbackLeaseIntents: [],
        funding: baseFunding,
        paymentChallengeRef: null,
        pylonAssignmentIntents: [pylonIntent],
        quote: {
          maxSpendCents: 0,
          settlementMode: 'no_worker_payout',
        },
        tasks: [
          {
            ...baseTask,
            checkout: {
              ...baseTask.checkout,
              commitSha: '1111111111111111111111111111111111111111',
            },
          },
        ],
        workOrderRef: 'autopilot_work_order.test_1',
      }),
      /not a placeholder/,
    )
  })
})
