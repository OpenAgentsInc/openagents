// HANDS-6 (#9177): the automatic self-update scheduler.
//
// The signed update state machine already lives in `update-staging-host.ts`.
// It verifies the feed signature, pins the release key, checks the digest and
// byte length of the downloaded artifact, and swaps in place with a first-launch
// health watchdog and rollback. That host never runs on its own — until this
// module, a check only happened when the user pressed a button in Settings, and
// the durable `updates.autoCheck` / `updates.autoDownload` preferences were
// dormant (no code read them).
//
// This scheduler is the thin, preference-gated WHEN around that proven host. It
// decides when to call `check` and, when allowed, `download`. It NEVER bypasses
// a signature or digest gate: it only calls the exact serialized host methods.
// Every pass is fail-soft — an automatic check can never crash or block the app,
// and only bounded, public-safe reason codes are logged.

import type { DesktopUpdateProjection } from "./update-staging-host.ts"

/**
 * The subset of the serialized staging host the automatic scheduler drives.
 * These are the exact host methods, so every trust gate inside the host still
 * runs. The scheduler only decides the timing of the calls.
 */
export type DesktopUpdateSchedulerHost = Readonly<{
  snapshot: () => DesktopUpdateProjection
  check: () => Promise<DesktopUpdateProjection>
  download: () => Promise<DesktopUpdateProjection>
}>

/** The two dormant preference fields that gate the automatic path. */
export type DesktopUpdateSchedulerPreferences = Readonly<{
  autoCheck: boolean
  autoDownload: boolean
}>

export type DesktopUpdateScheduler = Readonly<{
  /**
   * Run one preference-gated pass now and resolve with the resulting host
   * projection, or `null` when the pass did nothing (auto-check disabled, a
   * pass already in flight, or a fail-soft error). Never rejects.
   */
  runOnce: () => Promise<DesktopUpdateProjection | null>
  /** Arm the periodic re-check and run the launch pass. Idempotent. */
  start: () => void
  /** Cancel the periodic re-check. Safe to call more than once. */
  stop: () => void
}>

/** Default periodic re-check cadence: every 6 hours while the app runs. */
export const DEFAULT_UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1_000

/** Never re-check faster than once a minute, whatever the caller requests. */
const MINIMUM_UPDATE_CHECK_INTERVAL_MS = 60_000

/**
 * Only log bounded, lower/underscore reason codes. The host already returns
 * public-safe `reason` codes in its projection; this guards the rare case where
 * an unexpected throw carries a transport URL or a local path in its message.
 */
const publicSafeReason = (error: unknown): string =>
  error instanceof Error && /^[a-z0-9_]+$/.test(error.message) && error.message.length <= 80
    ? error.message
    : "update_scheduler_error"

export const openDesktopUpdateScheduler = (
  input: Readonly<{
    host: DesktopUpdateSchedulerHost
    /** Read the current durable update preferences on every pass. */
    readPreferences: () => DesktopUpdateSchedulerPreferences
    intervalMs?: number
    /** Injectable timer seam so tests never wait on real time. */
    setTimer?: (handler: () => void, ms: number) => unknown
    clearTimer?: (handle: unknown) => void
    /** Public-safe diagnostic sink. Defaults to `console.error`. */
    log?: (message: string) => void
  }>,
): DesktopUpdateScheduler => {
  const intervalMs = Math.max(MINIMUM_UPDATE_CHECK_INTERVAL_MS, input.intervalMs ?? DEFAULT_UPDATE_CHECK_INTERVAL_MS)
  const setTimer =
    input.setTimer ??
    ((handler: () => void, ms: number): unknown => {
      const handle = setInterval(handler, ms)
      // Do not let the re-check timer keep the process alive on its own.
      handle.unref?.()
      return handle
    })
  const clearTimer = input.clearTimer ?? ((handle: unknown): void => clearInterval(handle as ReturnType<typeof setInterval>))
  const log = input.log ?? ((message: string): void => console.error(`[desktop-update] ${message}`))

  let timer: unknown = null
  let inFlight = false

  const runOnce = async (): Promise<DesktopUpdateProjection | null> => {
    // One automatic pass never overlaps another. The host serializes its own
    // mutations too, but this keeps redundant passes from stacking behind a
    // slow network and racing a user-initiated Settings action.
    if (inFlight) return null
    inFlight = true
    try {
      const preferences = input.readPreferences()
      // Gate 1: auto-check must be enabled for any automatic feed contact.
      if (!preferences.autoCheck) return null

      let projection: DesktopUpdateProjection
      try {
        projection = await input.host.check()
      } catch (error) {
        log(`automatic update check failed: ${publicSafeReason(error)}`)
        return null
      }

      // Only a verified, monotonic, newer signed release reaches "available".
      if (projection.phase !== "available") return projection
      // Gate 2: without auto-download, leave the available signal for the UI
      // to surface and let the owner apply it from Settings.
      if (!preferences.autoDownload) return projection

      try {
        // Download re-verifies the artifact sha256 + byte length inside the
        // host before it stages; a bad artifact returns a rejected projection.
        return await input.host.download()
      } catch (error) {
        log(`automatic update download failed: ${publicSafeReason(error)}`)
        return projection
      }
    } finally {
      inFlight = false
    }
  }

  const start = (): void => {
    if (timer === null) {
      timer = setTimer(() => {
        void runOnce()
      }, intervalMs)
    }
    void runOnce()
  }

  const stop = (): void => {
    if (timer !== null) {
      clearTimer(timer)
      timer = null
    }
  }

  return { runOnce, start, stop }
}
