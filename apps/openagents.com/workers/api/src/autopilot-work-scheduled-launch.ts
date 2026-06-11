import { Schema as S } from 'effect'

import type { OpenAgentsAutopilotWorkRequest } from './autopilot-work-request'

export const AUTOPILOT_SCHEDULED_LAUNCH_DEFAULT_WINDOW_MINUTES = 360
export const AUTOPILOT_SCHEDULED_LAUNCH_MAX_HORIZON_MS = 7 * 24 * 60 * 60_000

export const AutopilotWorkScheduledLaunchRecord = S.Struct({
  dispatchedAt: S.NullOr(S.String),
  expiredAt: S.NullOr(S.String),
  launchAt: S.String,
  windowMinutes: S.Number,
})
export type AutopilotWorkScheduledLaunchRecord =
  typeof AutopilotWorkScheduledLaunchRecord.Type

export type AutopilotWorkScheduledLaunchState =
  | 'dispatched'
  | 'expired'
  | 'pending'

export type AutopilotWorkScheduledLaunchProjection = Readonly<{
  dispatchedAt: string | null
  expiredAt: string | null
  launchAt: string
  launchState: AutopilotWorkScheduledLaunchState
  reasonRefs: ReadonlyArray<string>
  windowMinutes: number
}>

const epochMillis = (iso: string): number => Date.parse(iso)

export const scheduledLaunchRecordForRequest = (
  request: OpenAgentsAutopilotWorkRequest,
  nowIso: string,
): AutopilotWorkScheduledLaunchRecord | null => {
  const launchPolicy = request.launchPolicy

  if (launchPolicy === undefined) {
    return null
  }

  const windowMinutes =
    launchPolicy.launchWindowMinutes ??
    AUTOPILOT_SCHEDULED_LAUNCH_DEFAULT_WINDOW_MINUTES
  const launchDue = epochMillis(launchPolicy.launchAt) <= epochMillis(nowIso)

  return {
    dispatchedAt: launchDue ? nowIso : null,
    expiredAt: null,
    launchAt: launchPolicy.launchAt,
    windowMinutes,
  }
}

export const scheduledLaunchHorizonReason = (
  record: AutopilotWorkScheduledLaunchRecord | null,
  nowIso: string,
): string | undefined =>
  record !== null &&
  epochMillis(record.launchAt) >
    epochMillis(nowIso) + AUTOPILOT_SCHEDULED_LAUNCH_MAX_HORIZON_MS
    ? 'launchPolicy.launchAt must be within 7 days of submission.'
    : undefined

export const scheduledLaunchHoldsDispatch = (
  record: AutopilotWorkScheduledLaunchRecord | null,
): boolean =>
  record !== null && record.dispatchedAt === null && record.expiredAt === null

export const scheduledLaunchDue = (
  record: AutopilotWorkScheduledLaunchRecord,
  nowIso: string,
): boolean =>
  scheduledLaunchHoldsDispatch(record) &&
  epochMillis(record.launchAt) <= epochMillis(nowIso)

export const scheduledLaunchWindowExpired = (
  record: AutopilotWorkScheduledLaunchRecord,
  nowIso: string,
): boolean =>
  scheduledLaunchHoldsDispatch(record) &&
  epochMillis(record.launchAt) + record.windowMinutes * 60_000 <
    epochMillis(nowIso)

export const dispatchedScheduledLaunch = (
  record: AutopilotWorkScheduledLaunchRecord,
  nowIso: string,
): AutopilotWorkScheduledLaunchRecord => ({
  ...record,
  dispatchedAt: nowIso,
})

export const expiredScheduledLaunch = (
  record: AutopilotWorkScheduledLaunchRecord,
  nowIso: string,
): AutopilotWorkScheduledLaunchRecord => ({
  ...record,
  expiredAt: nowIso,
})

export const scheduledLaunchRetryAfterSeconds = (
  record: AutopilotWorkScheduledLaunchRecord,
  nowIso: string,
): number =>
  Math.max(
    0,
    Math.ceil((epochMillis(record.launchAt) - epochMillis(nowIso)) / 1000),
  )

const scheduledLaunchState = (
  record: AutopilotWorkScheduledLaunchRecord,
): AutopilotWorkScheduledLaunchState =>
  record.expiredAt !== null
    ? 'expired'
    : record.dispatchedAt !== null
      ? 'dispatched'
      : 'pending'

export const scheduledLaunchProjection = (
  record: AutopilotWorkScheduledLaunchRecord | null,
): AutopilotWorkScheduledLaunchProjection | null => {
  if (record === null) {
    return null
  }

  const launchState = scheduledLaunchState(record)

  return {
    dispatchedAt: record.dispatchedAt,
    expiredAt: record.expiredAt,
    launchAt: record.launchAt,
    launchState,
    reasonRefs: [
      `scheduled_launch.${launchState}`,
      'scheduled_launch.placement_at_launch_time',
    ],
    windowMinutes: record.windowMinutes,
  }
}
