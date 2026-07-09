/**
 * OpenAgents mobile (#8597) — OTA update polling against the owned OpenAgents
 * Updates server (`apps/oa-updates`, updates.openagents.com, channel
 * `openagents-production`). Never EAS.
 *
 * TEMPORARY CADENCE: the 3-second foreground poll below is a deliberately
 * aggressive owner-testing cadence so a freshly published OTA appears on
 * device within ~3s. It is NOT the long-term battery/network-sane cadence —
 * dial `TEMPORARY_OTA_POLL_INTERVAL_MS` up (minutes) or move to
 * check-on-foreground once the OTA loop is owner-proven. The constant name,
 * this comment, and the cadence test exist so that later change is a
 * one-liner, not an archaeology dig.
 */
export const TEMPORARY_OTA_POLL_INTERVAL_MS = 3000

/** The slice of the `expo-updates` module the poller uses — injected so the
 * loop is unit-testable in bun with no native host. */
export interface OtaUpdatesClient {
  /** `Updates.isEnabled` (false in Expo Go / dev client without updates). */
  readonly isEnabled: boolean
  checkForUpdateAsync(): Promise<{ readonly isAvailable: boolean }>
  fetchUpdateAsync(): Promise<unknown>
  reloadAsync(): Promise<void>
}

export interface OtaPollingOptions {
  readonly intervalMs?: number
  /** Called after an update is fetched, just before the reload — a visible
   * "update ready — restarting" beat if the shell wants one. */
  readonly onUpdateReady?: () => void
}

export interface OtaPollingHandle {
  stop(): void
}

const noopHandle: OtaPollingHandle = { stop: () => undefined }

/**
 * Starts the foreground poll loop: every `intervalMs`, `checkForUpdateAsync`;
 * when an update is available, `fetchUpdateAsync` then `reloadAsync` (which
 * restarts the JS runtime, ending the loop). ALL errors are soft — offline,
 * server hiccups, or a mid-fetch failure log nothing fatal and never crash or
 * stop the loop; the next tick simply tries again.
 */
export const startOtaPolling = (
  client: OtaUpdatesClient,
  options: OtaPollingOptions = {},
): OtaPollingHandle => {
  if (!client.isEnabled) {
    return noopHandle
  }

  const intervalMs = options.intervalMs ?? TEMPORARY_OTA_POLL_INTERVAL_MS
  let stopped = false
  let timer: ReturnType<typeof setTimeout> | undefined

  const schedule = (): void => {
    if (stopped) return
    timer = setTimeout(() => {
      void tick()
    }, intervalMs)
  }

  const tick = async (): Promise<void> => {
    if (stopped) return
    try {
      const check = await client.checkForUpdateAsync()
      if (!stopped && check.isAvailable) {
        await client.fetchUpdateAsync()
        if (stopped) return
        options.onUpdateReady?.()
        await client.reloadAsync()
        // reloadAsync restarts the app; nothing more to schedule.
        return
      }
    } catch {
      // Soft-fail by design: an offline device or a flaky response must never
      // crash the app or kill the polling loop.
    }
    schedule()
  }

  schedule()

  return {
    stop: () => {
      stopped = true
      if (timer !== undefined) {
        clearTimeout(timer)
      }
    },
  }
}
