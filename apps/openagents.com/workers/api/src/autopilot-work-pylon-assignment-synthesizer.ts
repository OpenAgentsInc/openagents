import type { AutopilotWorkAssignmentIntentProjection } from './autopilot-work-assignment-planner'
import type { AutopilotPlacementDecisionProjection } from './autopilot-work-placement-selector'
import {
  DEFAULT_CODING_ADAPTER,
  adapterCapabilityRefs,
  adapterJobKinds,
  selectCodingAdapter,
  type AutopilotCodingAdapter,
} from './autopilot-work-adapter-selection'
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

// CX5 (#4792): the work class is chosen per placed Pylon by the typed
// adapter-selection policy — requester-required adapter wins, a
// single-capability Pylon gets its one adapter, dual-capability Pylons
// get the documented default. Never a silent substitution: the emitted
// jobKind, capability ref, and reason ref all name the choice.
const codingAdapterForPlacement = (
  placementDecision: AutopilotPlacementDecisionProjection,
  pylonRef: string,
  requestedAdapter: AutopilotCodingAdapter | null,
): Readonly<
  | {
      selected: true
      capabilityRef: string
      jobKind: PylonApiAssignmentJobKind
      reasonRef: string
    }
  | {
      selected: false
      blockerRefs: ReadonlyArray<string>
    }
> => {
  const candidate = placementDecision.pylonCandidates.find(
    entry => entry.selected && entry.pylonRef === pylonRef,
  )
  const selection = selectCodingAdapter({
    pylonCapabilityRefs: candidate?.capabilityRefs ?? [],
    requestedAdapter: requestedAdapter ?? undefined,
  })
  if (selection.selected) {
    return {
      capabilityRef: selection.capabilityRef,
      jobKind: selection.jobKind,
      reasonRef: selection.reasonRef,
      selected: true,
    }
  }
  if (requestedAdapter !== null) {
    return {
      blockerRefs: selection.blockerRefs,
      selected: false,
    }
  }
  return {
    capabilityRef: adapterCapabilityRefs[DEFAULT_CODING_ADAPTER],
    jobKind: adapterJobKinds[DEFAULT_CODING_ADAPTER],
    reasonRef: 'adapter_selection.default_no_candidate_capabilities',
    selected: true,
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
    .flatMap(assignment => {
      const task = tasksByRef.get(assignment.taskRef)
      const assignmentRef =
        `pylon_assignment.${input.workOrderRef}.${assignment.taskRef}`
      const adapter = codingAdapterForPlacement(
        input.placementDecision,
        pylonRef,
        task?.requestedAdapter ?? null,
      )
      if (!adapter.selected) {
        return []
      }

      return [{
        acceptanceCriteriaRefs: task?.acceptanceCriteriaRefs ?? [],
        assignmentRef,
        closeoutPathRefs: [
          `closeout.${assignmentRef}.diff_or_summary_required`,
          `closeout.${assignmentRef}.tests_or_blocker_required`,
          `closeout.${assignmentRef}.accepted_work_not_implied`,
        ],
        forumAutoPublishAllowed: false,
        jobKind: adapter.jobKind,
        noForumAutoPublishRefs: [
          `forum_autopublish_disabled.${assignmentRef}`,
        ],
        paymentMode: task?.paymentState === 'funded'
          ? 'payable_pending_settlement'
          : 'unpaid_smoke',
        pylonRef,
        requiredCapabilityRefs: [
          'capability.pylon.assignment_ready',
          adapter.capabilityRef,
        ],
        resultExpectationRefs: [
          `result.${assignmentRef}.public_safe_closeout`,
        ],
        rollbackRefs: [
          `rollback.${assignmentRef}.no_deploy_without_owner_acceptance`,
        ],
        selectionPolicyRefs: [
          ...input.placementDecision.reasonRefs,
          adapter.reasonRef,
          ...(task?.requestedAdapterProfileRef === undefined ||
          task.requestedAdapterProfileRef === null
            ? []
            : [task.requestedAdapterProfileRef]),
        ],
        spendCapRefs: task?.paymentState === 'funded'
          ? ['spend_cap.buyer_funded.autopilot_pylon_assignment']
          : ['spend_cap.no_spend.autopilot_pylon_assignment'],
        taskRef: assignment.taskRef,
      }]
    })
}
