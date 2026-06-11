import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'
import {
  AUTOPILOT_SCHEDULED_LAUNCH_DEFAULT_WINDOW_MINUTES,
  dispatchedScheduledLaunch,
  expiredScheduledLaunch,
  scheduledLaunchDue,
  scheduledLaunchHoldsDispatch,
  scheduledLaunchHorizonReason,
  scheduledLaunchProjection,
  scheduledLaunchRecordForRequest,
  scheduledLaunchRetryAfterSeconds,
  scheduledLaunchWindowExpired,
} from './autopilot-work-scheduled-launch'

const nowIso = '2026-06-11T22:00:00.000Z'

const scheduledRequest = (launchAt: string, launchWindowMinutes?: number) =>
  decodeOpenAgentsAutopilotWorkRequest({
    ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    launchPolicy: {
      kind: 'scheduled',
      launchAt,
      ...(launchWindowMinutes === undefined ? {} : { launchWindowMinutes }),
    },
  })

describe('autopilot scheduled launch policy', () => {
  test('requests without a launch policy never schedule', () => {
    const request = decodeOpenAgentsAutopilotWorkRequest(
      OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    )

    expect(scheduledLaunchRecordForRequest(request, nowIso)).toBeNull()
    expect(scheduledLaunchHoldsDispatch(null)).toBe(false)
    expect(scheduledLaunchProjection(null)).toBeNull()
  })

  test('future launchAt holds dispatch until launch time', () => {
    const record = scheduledLaunchRecordForRequest(
      scheduledRequest('2026-06-12T03:00:00Z'),
      nowIso,
    )

    expect(record).not.toBeNull()
    expect(record?.dispatchedAt).toBeNull()
    expect(record?.windowMinutes).toBe(
      AUTOPILOT_SCHEDULED_LAUNCH_DEFAULT_WINDOW_MINUTES,
    )
    expect(scheduledLaunchHoldsDispatch(record)).toBe(true)
    expect(scheduledLaunchDue(record!, nowIso)).toBe(false)
    expect(scheduledLaunchDue(record!, '2026-06-12T03:00:00.000Z')).toBe(true)
    expect(scheduledLaunchRetryAfterSeconds(record!, nowIso)).toBe(5 * 3600)
  })

  test('past launchAt is released immediately at creation', () => {
    const record = scheduledLaunchRecordForRequest(
      scheduledRequest('2026-06-11T21:00:00Z'),
      nowIso,
    )

    expect(record?.dispatchedAt).toBe(nowIso)
    expect(scheduledLaunchHoldsDispatch(record!)).toBe(false)
  })

  test('window expiry is bounded by launchWindowMinutes', () => {
    const record = scheduledLaunchRecordForRequest(
      scheduledRequest('2026-06-12T03:00:00Z', 30),
      nowIso,
    )

    expect(scheduledLaunchWindowExpired(record!, '2026-06-12T03:29:00Z')).toBe(
      false,
    )
    expect(scheduledLaunchWindowExpired(record!, '2026-06-12T03:31:00Z')).toBe(
      true,
    )
  })

  test('transitions stamp dispatchedAt or expiredAt and stop holding', () => {
    const record = scheduledLaunchRecordForRequest(
      scheduledRequest('2026-06-12T03:00:00Z'),
      nowIso,
    )!
    const dispatched = dispatchedScheduledLaunch(
      record,
      '2026-06-12T03:01:00.000Z',
    )
    const expired = expiredScheduledLaunch(record, '2026-06-12T10:00:00.000Z')

    expect(dispatched.dispatchedAt).toBe('2026-06-12T03:01:00.000Z')
    expect(scheduledLaunchHoldsDispatch(dispatched)).toBe(false)
    expect(expired.expiredAt).toBe('2026-06-12T10:00:00.000Z')
    expect(scheduledLaunchHoldsDispatch(expired)).toBe(false)
    expect(scheduledLaunchProjection(dispatched)?.launchState).toBe(
      'dispatched',
    )
    expect(scheduledLaunchProjection(expired)?.launchState).toBe('expired')
    expect(scheduledLaunchProjection(record)?.launchState).toBe('pending')
    expect(scheduledLaunchProjection(record)?.reasonRefs).toContain(
      'scheduled_launch.placement_at_launch_time',
    )
  })

  test('launchAt beyond the seven-day horizon is rejected', () => {
    const record = scheduledLaunchRecordForRequest(
      scheduledRequest('2026-06-20T03:00:00Z'),
      nowIso,
    )

    expect(scheduledLaunchHorizonReason(record, nowIso)).toContain('7 days')
    expect(
      scheduledLaunchHorizonReason(
        scheduledLaunchRecordForRequest(
          scheduledRequest('2026-06-12T03:00:00Z'),
          nowIso,
        ),
        nowIso,
      ),
    ).toBeUndefined()
  })

  test('request validation rejects malformed launch policies', () => {
    const reasonFor = (run: () => unknown): string => {
      try {
        run()

        return 'did_not_throw'
      } catch (error) {
        return typeof error === 'object' &&
          error !== null &&
          'reason' in error &&
          typeof error.reason === 'string'
          ? error.reason
          : 'unexpected_error'
      }
    }

    expect(reasonFor(() => scheduledRequest('tomorrow at 3'))).toMatch(
      /launchAt must be a UTC ISO timestamp/,
    )
    expect(
      reasonFor(() => scheduledRequest('2026-06-12T03:00:00Z', 2)),
    ).toMatch(/launchWindowMinutes must be an integer between 5 and 1440/)
    expect(
      reasonFor(() => scheduledRequest('2026-06-12T03:00:00Z', 100_000)),
    ).toMatch(/launchWindowMinutes must be an integer between 5 and 1440/)
  })
})
