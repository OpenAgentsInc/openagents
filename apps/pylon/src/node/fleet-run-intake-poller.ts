import type {
  PylonFleetRunRemoteIntakeProjection,
  PylonFleetRunRemoteIntakeService,
} from "../orchestration/fleet-run-remote-intake.js"

export const PYLON_FLEET_RUN_INTAKE_POLLER_SCHEMA =
  "openagents.pylon.fleet_run_intake_poller.v1" as const

export type PylonFleetRunIntakePollerStatus = {
  readonly schema: typeof PYLON_FLEET_RUN_INTAKE_POLLER_SCHEMA
  readonly state: "closed" | "disabled" | "idle" | "polling"
  readonly pollCount: number
  readonly lastProjection: PylonFleetRunRemoteIntakeProjection | null
  readonly blockerRefs: readonly string[]
}

export const disabledPylonFleetRunIntakePollerStatus =
  (): PylonFleetRunIntakePollerStatus => ({
    schema: PYLON_FLEET_RUN_INTAKE_POLLER_SCHEMA,
    state: "disabled",
    pollCount: 0,
    lastProjection: null,
    blockerRefs: [
      "blocker.pylon.fleet_run_intake.transport_not_configured",
    ],
  })

export type OpenPylonFleetRunIntakePollerInput = {
  readonly intake: PylonFleetRunRemoteIntakeService
  readonly intervalMs?: number | undefined
  readonly setTimer?: typeof globalThis.setTimeout | undefined
  readonly clearTimer?: typeof globalThis.clearTimeout | undefined
  readonly startImmediately?: boolean | undefined
}

export type PylonFleetRunIntakePoller = {
  readonly close: () => Promise<void>
  readonly runNow: () => Promise<PylonFleetRunRemoteIntakeProjection | null>
  readonly status: () => PylonFleetRunIntakePollerStatus
}

/**
 * Standing serialized poll loop. A new timer is scheduled only after the
 * previous intake reconciliation settles, so slow network/storage never
 * produces overlapping claims.
 */
export function openPylonFleetRunIntakePoller(
  input: OpenPylonFleetRunIntakePollerInput,
): PylonFleetRunIntakePoller {
  const intervalMs = input.intervalMs ?? 5_000
  if (!Number.isInteger(intervalMs) || intervalMs < 250 || intervalMs > 300_000) {
    throw new Error("FleetRun intake poll interval is invalid")
  }
  const setTimer = input.setTimer ?? globalThis.setTimeout
  const clearTimer = input.clearTimer ?? globalThis.clearTimeout
  let closed = false
  let timer: ReturnType<typeof globalThis.setTimeout> | undefined
  let current: Promise<PylonFleetRunRemoteIntakeProjection | null> | null = null
  let pollCount = 0
  let lastProjection: PylonFleetRunRemoteIntakeProjection | null = null
  let failed = false

  const schedule = (): void => {
    if (closed || timer !== undefined) return
    timer = setTimer(() => {
      timer = undefined
      void runNow().finally(schedule)
    }, intervalMs)
  }

  const execute = async (): Promise<PylonFleetRunRemoteIntakeProjection | null> => {
    if (closed) return null
    pollCount += 1
    try {
      const projection = await input.intake.runOnce()
      lastProjection = projection
      failed = false
      return projection
    } catch {
      failed = true
      return null
    }
  }

  const runNow = (): Promise<PylonFleetRunRemoteIntakeProjection | null> => {
    if (closed) return Promise.resolve(null)
    if (current !== null) return current
    current = execute().finally(() => {
      current = null
    })
    return current
  }

  const poller: PylonFleetRunIntakePoller = {
    runNow,
    status: () => ({
      schema: PYLON_FLEET_RUN_INTAKE_POLLER_SCHEMA,
      state: closed ? "closed" : current === null ? "idle" : "polling",
      pollCount,
      lastProjection,
      blockerRefs: failed
        ? ["blocker.pylon.fleet_run_intake.poll_failed"]
        : [],
    }),
    close: async () => {
      if (closed) return
      closed = true
      if (timer !== undefined) {
        clearTimer(timer)
        timer = undefined
      }
      await current?.catch(() => undefined)
      await input.intake.close()
    },
  }

  if (input.startImmediately !== false) {
    void runNow().finally(schedule)
  } else {
    schedule()
  }
  return poller
}
