import type { AutopilotWorkAssignmentIntentProjection } from './autopilot-work-assignment-planner'
import type { AutopilotPlacementDecisionProjection } from './autopilot-work-placement-selector'
import type { OpenAgentsAutopilotRunnerKind } from './autopilot-work-request'
import type { AutopilotWorkTaskRecordProjection } from './autopilot-work-routes'
import type { PylonApiAssignmentJobKind } from './pylon-api'

type FallbackLeasePaymentMode = 'buyer_funded' | 'unpaid_smoke'

export type AutopilotFallbackLeaseIntentProjection = Readonly<{
  acceptanceCriteriaRefs: ReadonlyArray<string>
  assignmentRef: string
  closeoutPathRefs: ReadonlyArray<string>
  fallbackLaneRef: string
  forumAutoPublishAllowed: false
  jobKind: PylonApiAssignmentJobKind
  noForumAutoPublishRefs: ReadonlyArray<string>
  paymentMode: FallbackLeasePaymentMode
  requiredCapabilityRefs: ReadonlyArray<string>
  resultExpectationRefs: ReadonlyArray<string>
  rollbackRefs: ReadonlyArray<string>
  runnerKind: OpenAgentsAutopilotRunnerKind
  selectionPolicyRefs: ReadonlyArray<string>
  spendCapRefs: ReadonlyArray<string>
  taskRef: string
  workerPayoutAuthority: false
}>

const fallbackRunnerKinds = new Set<OpenAgentsAutopilotRunnerKind>([
  'cloud_sandbox',
  'hosted_gemini',
  'openagents_shc',
  'shc',
])

const jobKindForAssignment = (
  assignment: AutopilotWorkAssignmentIntentProjection,
): PylonApiAssignmentJobKind => {
  switch (assignment.assignmentKind) {
    case 'repo_change':
    case 'research_and_patch':
    case 'site_adjustment':
    case 'site_generation':
    case 'test_repair':
      return 'validation'
  }
}

const fallbackLaneRefForRunner = (
  runnerKind: OpenAgentsAutopilotRunnerKind,
): string => {
  switch (runnerKind) {
    case 'cloud_sandbox':
      return 'fallback_lane.openagents.cloud_sandbox'
    case 'hosted_gemini':
      return 'fallback_lane.openagents.hosted_gemini'
    case 'openagents_shc':
      return 'fallback_lane.openagents.shc'
    case 'shc':
      return 'fallback_lane.shc'
    case 'maple_ai':
    case 'pylon_network':
    case 'requester_pylon':
    case 'tee':
      return `fallback_lane.unsupported.${runnerKind}`
  }
}

const requiredCapabilityRefsForRunner = (
  runnerKind: OpenAgentsAutopilotRunnerKind,
): ReadonlyArray<string> => {
  switch (runnerKind) {
    case 'cloud_sandbox':
      return [
        'capability.fallback.assignment_ready',
        'capability.openagents.cloud_sandbox',
      ]
    case 'hosted_gemini':
      return [
        'capability.fallback.assignment_ready',
        'capability.openagents.hosted_gemini',
      ]
    case 'openagents_shc':
      return [
        'capability.fallback.assignment_ready',
        'capability.openagents.shc',
      ]
    case 'shc':
      return [
        'capability.fallback.assignment_ready',
        'capability.shc',
      ]
    case 'maple_ai':
    case 'pylon_network':
    case 'requester_pylon':
    case 'tee':
      return ['capability.fallback.unsupported_runner']
  }
}

export const fallbackLeaseIntentsForAutopilotWork = (
  input: Readonly<{
    assignmentIntents: ReadonlyArray<AutopilotWorkAssignmentIntentProjection>
    placementDecision: AutopilotPlacementDecisionProjection
    tasks: ReadonlyArray<AutopilotWorkTaskRecordProjection>
    workOrderRef: string
  }>,
): ReadonlyArray<AutopilotFallbackLeaseIntentProjection> => {
  if (
    input.placementDecision.source !== 'fallback' ||
    input.placementDecision.selectedRunnerKind === null ||
    !fallbackRunnerKinds.has(input.placementDecision.selectedRunnerKind)
  ) {
    return []
  }

  const runnerKind = input.placementDecision.selectedRunnerKind
  const fallbackLaneRef = fallbackLaneRefForRunner(runnerKind)
  const requiredCapabilityRefs = requiredCapabilityRefsForRunner(runnerKind)
  const tasksByRef = new Map(input.tasks.map(task => [task.taskRef, task]))

  return input.assignmentIntents
    .filter(assignment => assignment.readyForAssignment)
    .map(assignment => {
      const task = tasksByRef.get(assignment.taskRef)
      const assignmentRef =
        `fallback_assignment.${input.workOrderRef}.${assignment.taskRef}`
      const paymentMode: FallbackLeasePaymentMode =
        assignment.paymentState === 'funded'
          ? 'buyer_funded'
          : 'unpaid_smoke'

      return {
        acceptanceCriteriaRefs: task?.acceptanceCriteriaRefs ?? [],
        assignmentRef,
        closeoutPathRefs: [
          `closeout.${assignmentRef}.diff_or_summary_required`,
          `closeout.${assignmentRef}.tests_or_blocker_required`,
          `closeout.${assignmentRef}.accepted_work_not_implied`,
        ],
        fallbackLaneRef,
        forumAutoPublishAllowed: false,
        jobKind: jobKindForAssignment(assignment),
        noForumAutoPublishRefs: [
          `forum_autopublish_disabled.${assignmentRef}`,
        ],
        paymentMode,
        requiredCapabilityRefs,
        resultExpectationRefs: [
          `result.${assignmentRef}.public_safe_closeout`,
        ],
        rollbackRefs: [
          `rollback.${assignmentRef}.no_deploy_without_owner_acceptance`,
        ],
        runnerKind,
        selectionPolicyRefs: input.placementDecision.reasonRefs,
        spendCapRefs: [
          paymentMode === 'buyer_funded'
            ? 'spend_cap.buyer_funded.fallback_assignment'
            : 'spend_cap.no_spend.fallback_assignment',
        ],
        taskRef: assignment.taskRef,
        workerPayoutAuthority: false,
      }
    })
}
