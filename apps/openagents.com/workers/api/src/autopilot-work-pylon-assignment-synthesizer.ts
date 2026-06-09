import type { AutopilotWorkAssignmentIntentProjection } from './autopilot-work-assignment-planner'
import type { AutopilotPlacementDecisionProjection } from './autopilot-work-placement-selector'
import { localCodingAgentCapabilityRefs } from './autopilot-work-placement-selector'
import type { AutopilotWorkTaskRecordProjection } from './autopilot-work-routes'
import type { PylonApiAssignmentJobKind } from './pylon-api'

export type AutopilotPylonAssignmentIntentProjection = Readonly<{
  acceptanceCriteriaRefs: ReadonlyArray<string>
  assignmentRef: string
  closeoutPathRefs: ReadonlyArray<string>
  forumAutoPublishAllowed: false
  jobKind: PylonApiAssignmentJobKind
  noForumAutoPublishRefs: ReadonlyArray<string>
  paymentMode: 'payable_pending_settlement' | 'unpaid_smoke'
  pylonRef: string
  requiredCapabilityRefs: ReadonlyArray<string>
  resultExpectationRefs: ReadonlyArray<string>
  rollbackRefs: ReadonlyArray<string>
  selectionPolicyRefs: ReadonlyArray<string>
  spendCapRefs: ReadonlyArray<string>
  taskRef: string
}>

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

export const pylonAssignmentIntentsForAutopilotWork = (
  input: Readonly<{
    assignmentIntents: ReadonlyArray<AutopilotWorkAssignmentIntentProjection>
    placementDecision: AutopilotPlacementDecisionProjection
    tasks: ReadonlyArray<AutopilotWorkTaskRecordProjection>
    workOrderRef: string
  }>,
): ReadonlyArray<AutopilotPylonAssignmentIntentProjection> => {
  if (
    input.placementDecision.source !== 'requester_pylon' ||
    input.placementDecision.selectedPylonRef === null
  ) {
    return []
  }

  const pylonRef = input.placementDecision.selectedPylonRef
  const tasksByRef = new Map(input.tasks.map(task => [task.taskRef, task]))

  return input.assignmentIntents
    .filter(assignment => assignment.readyForAssignment)
    .map(assignment => {
      const task = tasksByRef.get(assignment.taskRef)
      const assignmentRef =
        `pylon_assignment.${input.workOrderRef}.${assignment.taskRef}`

      return {
        acceptanceCriteriaRefs: task?.acceptanceCriteriaRefs ?? [],
        assignmentRef,
        closeoutPathRefs: [
          `closeout.${assignmentRef}.diff_or_summary_required`,
          `closeout.${assignmentRef}.tests_or_blocker_required`,
          `closeout.${assignmentRef}.accepted_work_not_implied`,
        ],
        forumAutoPublishAllowed: false,
        jobKind: jobKindForAssignment(assignment),
        noForumAutoPublishRefs: [
          `forum_autopublish_disabled.${assignmentRef}`,
        ],
        paymentMode: task?.paymentState === 'funded'
          ? 'payable_pending_settlement'
          : 'unpaid_smoke',
        pylonRef,
        requiredCapabilityRefs: [
          'capability.pylon.assignment_ready',
          ...localCodingAgentCapabilityRefs,
        ],
        resultExpectationRefs: [
          `result.${assignmentRef}.public_safe_closeout`,
        ],
        rollbackRefs: [
          `rollback.${assignmentRef}.no_deploy_without_owner_acceptance`,
        ],
        selectionPolicyRefs: input.placementDecision.reasonRefs,
        spendCapRefs: task?.paymentState === 'funded'
          ? ['spend_cap.buyer_funded.autopilot_pylon_assignment']
          : ['spend_cap.no_spend.autopilot_pylon_assignment'],
        taskRef: assignment.taskRef,
      }
    })
}
