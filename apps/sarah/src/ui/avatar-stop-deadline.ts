export const SARAH_AVATAR_STOP_DEADLINE_MS = 5_000
export const SARAH_AVATAR_STOP_DEADLINE_MAX_MS = 15_000

type TimerHandle = ReturnType<typeof setTimeout>

export type AvatarStopClock = Readonly<{
  setTimeout: (callback: () => void, delayMs: number) => TimerHandle
  clearTimeout: (handle: TimerHandle) => void
}>

export type AvatarStopTerminalOutcome = "stopped" | "failed"
export type AvatarStopDeadlineOutcome = AvatarStopTerminalOutcome | "timed_out"

export type AvatarStopAttempt = Readonly<{
  /** Settles no later than the configured deadline. */
  outcome: Promise<AvatarStopDeadlineOutcome>
  /** Never rejects; may settle later than the deadline. */
  completion: Promise<AvatarStopTerminalOutcome>
}>

const browserClock: AvatarStopClock = {
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (handle) => clearTimeout(handle),
}

/**
 * Start one idempotent session stop and bound only how long the caller waits.
 * A timeout is an unknown stop outcome, never permission to overlap a new
 * session. `completion` lets the owner clear that fail-closed state if the
 * original stop eventually succeeds.
 */
export function beginBoundedAvatarStop(
  stop: () => Promise<void>,
  options: Readonly<{
    deadlineMs?: number
    clock?: AvatarStopClock
  }> = {},
): AvatarStopAttempt {
  const deadlineMs = options.deadlineMs ?? SARAH_AVATAR_STOP_DEADLINE_MS
  const clock = options.clock ?? browserClock
  if (
    !Number.isSafeInteger(deadlineMs) ||
    deadlineMs <= 0 ||
    deadlineMs > SARAH_AVATAR_STOP_DEADLINE_MAX_MS
  ) {
    throw new Error("sarah_avatar_stop_invalid_deadline")
  }

  const completion: Promise<AvatarStopTerminalOutcome> = Promise.resolve()
    .then(stop)
    .then(
      () => "stopped" as const,
      () => "failed" as const,
    )

  let timer: TimerHandle | null = null
  const deadline = new Promise<AvatarStopDeadlineOutcome>((resolve) => {
    try {
      timer = clock.setTimeout(() => {
        timer = null
        resolve("timed_out")
      }, deadlineMs)
    } catch {
      resolve("timed_out")
    }
  })
  const outcome = Promise.race<AvatarStopDeadlineOutcome>([
    completion,
    deadline,
  ]).finally(() => {
    if (timer === null) return
    try { clock.clearTimeout(timer) } catch { /* already cleared */ }
    timer = null
  })

  return { outcome, completion }
}
