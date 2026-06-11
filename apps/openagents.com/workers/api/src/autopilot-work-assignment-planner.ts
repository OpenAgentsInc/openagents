import type {
  AutopilotWorkTaskRecordProjection,
} from './autopilot-work-routes'

export type AutopilotWorkAssignmentKind =
  | 'repo_change'
  | 'research_and_patch'
  | 'site_adjustment'
  | 'site_generation'
  | 'test_repair'

export type AutopilotWorkAssignmentPlannerState =
  | 'access_required'
  | 'blocked'
  | 'delivered'
  | 'free_slice'
  | 'paid_ready'
  | 'payment_required'
  | 'queued_or_running'
  | 'ready_for_assignment'
  | 'scheduled'

export type AutopilotWorkAssignmentIntentProjection = Readonly<{
  accessState: AutopilotWorkTaskRecordProjection['accessState']
  assignmentIntentRef: string
  assignmentKind: AutopilotWorkAssignmentKind
  deployAuthority: false
  paymentState: AutopilotWorkTaskRecordProjection['paymentState']
  placementState: AutopilotWorkTaskRecordProjection['placementState']
  plannerReasonRefs: ReadonlyArray<string>
  plannerState: AutopilotWorkAssignmentPlannerState
  readyForAssignment: boolean
  repository: AutopilotWorkTaskRecordProjection['repository']
  spendAuthority: false
  taskRef: string
  workerPayoutEligible: false
  workOrderRef: string
}>

const assignmentKindForTask = (
  task: AutopilotWorkTaskRecordProjection,
): AutopilotWorkAssignmentKind => {
  switch (task.kind) {
    case 'benchmark_or_gepa':
      return 'research_and_patch'
    case 'code_change':
    case 'repo_change':
      return 'repo_change'
    case 'research_and_patch':
      return 'research_and_patch'
    case 'site_adjustment':
      return 'site_adjustment'
    case 'site_generation':
      return 'site_generation'
    case 'test_repair':
      return 'test_repair'
  }
}

const plannerStateForTask = (
  task: AutopilotWorkTaskRecordProjection,
): AutopilotWorkAssignmentPlannerState => {
  if (task.lifecycleState === 'access_required') {
    return 'access_required'
  }

  if (task.lifecycleState === 'payment_required') {
    return 'payment_required'
  }

  if (task.lifecycleState === 'blocked') {
    return 'blocked'
  }

  if (task.lifecycleState === 'delivered') {
    return 'delivered'
  }

  if (task.lifecycleState === 'queued_or_running') {
    return 'queued_or_running'
  }

  if (task.lifecycleState === 'scheduled') {
    return 'scheduled'
  }

  if (task.lifecycleState === 'ready_for_assignment') {
    return task.paymentState === 'funded' ? 'paid_ready' : 'free_slice'
  }

  return 'ready_for_assignment'
}

const plannerReasonRefsForTask = (
  task: AutopilotWorkTaskRecordProjection,
  plannerState: AutopilotWorkAssignmentPlannerState,
): ReadonlyArray<string> => {
  switch (plannerState) {
    case 'access_required':
      return [
        'assignment.blocked.access_required',
        ...task.accessRequirements.map(
          requirement => requirement.accessRequestRef,
        ),
      ]
    case 'blocked':
      return ['assignment.blocked.lifecycle_blocked']
    case 'delivered':
      return ['assignment.delivered']
    case 'free_slice':
      return [
        'assignment.free_slice',
        'assignment.ready_for_assignment',
      ]
    case 'paid_ready':
      return [
        'assignment.paid_ready',
        'assignment.ready_for_assignment',
      ]
    case 'payment_required':
      return ['assignment.blocked.payment_required']
    case 'queued_or_running':
      return ['assignment.queued_or_running']
    case 'ready_for_assignment':
      return ['assignment.ready_for_assignment']
    case 'scheduled':
      return [
        'assignment.blocked.scheduled_launch_pending',
        'scheduled_launch.placement_at_launch_time',
      ]
  }
}

const readyForAssignment = (
  plannerState: AutopilotWorkAssignmentPlannerState,
): boolean =>
  plannerState === 'free_slice' ||
  plannerState === 'paid_ready' ||
  plannerState === 'ready_for_assignment'

export const assignmentIntentsForWorkOrder = (
  work: Readonly<{
    tasks: ReadonlyArray<AutopilotWorkTaskRecordProjection>
    workOrderRef: string
  }>,
): ReadonlyArray<AutopilotWorkAssignmentIntentProjection> =>
  work.tasks.map(task => {
    const plannerState = plannerStateForTask(task)

    return {
      accessState: task.accessState,
      assignmentIntentRef:
        `assignment_intent.${work.workOrderRef}.${task.taskRef}`,
      assignmentKind: assignmentKindForTask(task),
      deployAuthority: false,
      paymentState: task.paymentState,
      placementState: task.placementState,
      plannerReasonRefs: plannerReasonRefsForTask(task, plannerState),
      plannerState,
      readyForAssignment: readyForAssignment(plannerState),
      repository: task.repository,
      spendAuthority: false,
      taskRef: task.taskRef,
      workerPayoutEligible: false,
      workOrderRef: work.workOrderRef,
    }
  })
