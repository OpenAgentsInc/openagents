import { Match as M } from 'effect'

import {
  type AutopilotContinuationEventRecord,
} from './autopilot-continuation-policy'
import type {
  AutopilotWorkOrderRecord,
} from './autopilot-work-routes'
import { scheduledLaunchHoldsDispatch } from './autopilot-work-scheduled-launch'
import {
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'

export const AUTOPILOT_MORNING_REPORT_DEFAULT_SINCE_HOURS = 12
export const AUTOPILOT_MORNING_REPORT_MAX_SINCE_HOURS = 48

export type AutopilotMorningReportGroup =
  | 'awaiting_decision'
  | 'blocked'
  | 'launched'
  | 'reviewed'
  | 'running'
  | 'scheduled'

export type AutopilotMorningReportWorkItem = Readonly<{
  group: AutopilotMorningReportGroup
  scheduledLaunchAt: string | null
  state: AutopilotWorkOrderRecord['state']
  taskRefs: ReadonlyArray<string>
  updatedAt: string
  workOrderRef: string
}>

export type AutopilotMorningReportContinuationItem = Readonly<{
  attempt: number
  decision: AutopilotContinuationEventRecord['decision']
  mode: AutopilotContinuationEventRecord['mode']
  occurredAt: string
  reasonRef: string
  runId: string
}>

export type AutopilotMorningReport = Readonly<{
  continuations: ReadonlyArray<AutopilotMorningReportContinuationItem>
  counts: Readonly<{
    awaitingDecision: number
    blocked: number
    continuations: number
    launched: number
    reviewed: number
    running: number
    scheduled: number
  }>
  generatedAt: string
  reportRef: 'openagents.autopilot_morning_report.v1'
  sinceIso: string
  staleness: PublicProjectionStalenessContract
  workItems: ReadonlyArray<AutopilotMorningReportWorkItem>
}>

const morningReportGroupForRecord = (
  record: AutopilotWorkOrderRecord,
  sinceIso: string,
): AutopilotMorningReportGroup | undefined => {
  if (scheduledLaunchHoldsDispatch(record.scheduledLaunch)) {
    return 'scheduled'
  }

  if (record.updatedAt < sinceIso) {
    return undefined
  }

  return M.value(record.state).pipe(
    M.withReturnType<AutopilotMorningReportGroup | undefined>(),
    M.when('delivered', () => 'awaiting_decision' as const),
    M.when('blocked', () => 'blocked' as const),
    M.when('invalid', () => 'blocked' as const),
    M.when('accepted', () => 'reviewed' as const),
    M.when('rejected', () => 'reviewed' as const),
    M.when('revision_required', () => 'reviewed' as const),
    M.when('queued_or_running', () => 'running' as const),
    M.when('accepted_free_slice', () =>
      record.scheduledLaunch?.dispatchedAt !== null &&
      record.scheduledLaunch !== null
        ? ('launched' as const)
        : undefined
    ),
    M.when('paid_ready', () =>
      record.scheduledLaunch?.dispatchedAt !== null &&
      record.scheduledLaunch !== null
        ? ('launched' as const)
        : undefined
    ),
    M.orElse(() => undefined),
  )
}
export const autopilotMorningReportForOwner = (
  input: Readonly<{
    continuationEvents: ReadonlyArray<AutopilotContinuationEventRecord>
    nowIso: string
    sinceIso: string
    workOrders: ReadonlyArray<AutopilotWorkOrderRecord>
  }>,
): AutopilotMorningReport => {
  const workItems = input.workOrders.flatMap(record => {
    const group = morningReportGroupForRecord(record, input.sinceIso)

    return group === undefined
      ? []
      : [
          {
            group,
            scheduledLaunchAt: record.scheduledLaunch?.launchAt ?? null,
            state: record.state,
            taskRefs: record.taskRefs,
            updatedAt: record.updatedAt,
            workOrderRef: record.workOrderRef,
          },
        ]
  })
  const continuations = input.continuationEvents.map(event => ({
    attempt: event.attempt,
    decision: event.decision,
    mode: event.mode,
    occurredAt: event.createdAt,
    reasonRef: event.reasonRef,
    runId: event.runId,
  }))
  const countForGroup = (group: AutopilotMorningReportGroup): number =>
    workItems.filter(item => item.group === group).length

  return {
    continuations,
    counts: {
      awaitingDecision: countForGroup('awaiting_decision'),
      blocked: countForGroup('blocked'),
      continuations: continuations.length,
      launched: countForGroup('launched'),
      reviewed: countForGroup('reviewed'),
      running: countForGroup('running'),
      scheduled: countForGroup('scheduled'),
    },
    generatedAt: input.nowIso,
    reportRef: 'openagents.autopilot_morning_report.v1',
    sinceIso: input.sinceIso,
    staleness: liveAtReadStaleness([
      'autopilot_work_order_state_transition',
      'autopilot_scheduled_launch_transition',
      'autopilot_continuation_attempt_recorded',
    ]),
    workItems,
  }
}
