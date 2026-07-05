/**
 * Renderer-unresponsive watchdog for Khala Code desktop (issue #8441).
 *
 * Electrobun (the Bun-native webview shell this app is built on) does not
 * currently expose an Electron-style `webContents.on("unresponsive")` hook —
 * see docs/khala-code/2026-07-05-opencode-desktop-parity-gap-audit.md and the
 * upstream `packages/bun/src/events/webviewEvents.ts` event list, which stops
 * at navigation/download events. This watchdog is the honest substitute: the
 * renderer calls a heartbeat RPC on an interval, and this pure state machine
 * flags "unresponsive" the moment a heartbeat is overdue.
 *
 * This deliberately cannot detect a renderer whose JS thread is fully frozen
 * (a frozen thread cannot send a heartbeat, so a *missing* heartbeat is
 * exactly what a freeze looks like from the main-process side — the failure
 * mode we want to catch). It also fires for a merely slow renderer (GC
 * pause, huge synchronous handler), which is an acceptable false-positive
 * shape for a recovery affordance: false positives ask the user to choose
 * "keep waiting", which is a no-op recovery action.
 *
 * Pure and clock-injectable so it can be unit tested deterministically
 * without real timers or a real webview.
 */

export type KhalaCodeDesktopUnresponsiveWatchdogState = "responsive" | "unresponsive"

export type KhalaCodeDesktopUnresponsiveWatchdog = Readonly<{
  /** Records a heartbeat observed at `atMs` (defaults to `now()`). */
  recordHeartbeat: (atMs?: number) => void
  /** Re-evaluates state at `atMs` (defaults to `now()`) and returns it. */
  checkNow: (atMs?: number) => KhalaCodeDesktopUnresponsiveWatchdogState
  /** Returns the last-evaluated state without re-checking the clock. */
  state: () => KhalaCodeDesktopUnresponsiveWatchdogState
  /** Milliseconds since the last heartbeat, as of `atMs` (defaults to `now()`). */
  msSinceLastHeartbeat: (atMs?: number) => number
}>

export type KhalaCodeDesktopUnresponsiveWatchdogOptions = Readonly<{
  now?: () => number
  onStateChange?: (state: KhalaCodeDesktopUnresponsiveWatchdogState) => void
  /** Heartbeats older than this are considered overdue. Must be > 0. */
  timeoutMs: number
}>

export const createKhalaCodeDesktopUnresponsiveWatchdog = (
  options: KhalaCodeDesktopUnresponsiveWatchdogOptions,
): KhalaCodeDesktopUnresponsiveWatchdog => {
  if (!(options.timeoutMs > 0)) {
    throw new Error("Unresponsive watchdog timeoutMs must be a positive number.")
  }
  const now = options.now ?? (() => Date.now())
  let lastHeartbeatAt = now()
  let currentState: KhalaCodeDesktopUnresponsiveWatchdogState = "responsive"

  const setState = (next: KhalaCodeDesktopUnresponsiveWatchdogState): void => {
    if (next === currentState) return
    currentState = next
    options.onStateChange?.(next)
  }

  const msSinceLastHeartbeat = (atMs?: number): number =>
    Math.max(0, (atMs ?? now()) - lastHeartbeatAt)

  const checkNow = (atMs?: number): KhalaCodeDesktopUnresponsiveWatchdogState => {
    const elapsed = msSinceLastHeartbeat(atMs)
    setState(elapsed > options.timeoutMs ? "unresponsive" : "responsive")
    return currentState
  }

  const recordHeartbeat = (atMs?: number): void => {
    lastHeartbeatAt = atMs ?? now()
    setState("responsive")
  }

  return {
    checkNow,
    msSinceLastHeartbeat,
    recordHeartbeat,
    state: () => currentState,
  }
}
