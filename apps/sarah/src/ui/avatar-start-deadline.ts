export const SARAH_AVATAR_START_DEADLINE_MS = 15_000
export const SARAH_AVATAR_START_DEADLINE_MAX_MS = 60_000

type TimerHandle = ReturnType<typeof setTimeout>

export type AvatarStartClock = Readonly<{
  setTimeout: (callback: () => void, delayMs: number) => TimerHandle
  clearTimeout: (handle: TimerHandle) => void
}>

export type AvatarStartTerminalOutcome<A> =
  | Readonly<{ status: "started"; value: A }>
  | Readonly<{ status: "failed" }>
  | Readonly<{ status: "cleanup_unconfirmed" }>

export type AvatarStartDeadlineOutcome<A> =
  | AvatarStartTerminalOutcome<A>
  | Readonly<{ status: "timed_out" }>

export type AvatarStartAttempt<A> = Readonly<{
  /** Settles no later than the configured deadline. */
  outcome: Promise<AvatarStartDeadlineOutcome<A>>
  /** Never rejects; preserves the eventual start truth after a timeout. */
  completion: Promise<AvatarStartTerminalOutcome<A>>
}>

const browserClock: AvatarStartClock = {
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (handle) => clearTimeout(handle),
}

/**
 * Bound how long UI interaction waits for an avatar start without pretending
 * that timeout cancels the underlying mint/SDK/WebRTC acquisition. The owner
 * must fence the timed-out generation and dispose any eventually returned
 * handle before allowing a replacement session.
 */
export function beginBoundedAvatarStart<A>(
  start: () => Promise<A>,
  options: Readonly<{
    deadlineMs?: number
    clock?: AvatarStartClock
    classifyFailure?: (error: unknown) => "failed" | "cleanup_unconfirmed"
  }> = {},
): AvatarStartAttempt<A> {
  const deadlineMs = options.deadlineMs ?? SARAH_AVATAR_START_DEADLINE_MS
  const clock = options.clock ?? browserClock
  if (
    !Number.isSafeInteger(deadlineMs) ||
    deadlineMs <= 0 ||
    deadlineMs > SARAH_AVATAR_START_DEADLINE_MAX_MS
  ) {
    throw new Error("sarah_avatar_start_invalid_deadline")
  }

  const classifyFailure = (
    error: unknown,
  ): "failed" | "cleanup_unconfirmed" => {
    try {
      return options.classifyFailure?.(error) === "cleanup_unconfirmed"
        ? "cleanup_unconfirmed"
        : "failed"
    } catch {
      return "failed"
    }
  }

  const completion: Promise<AvatarStartTerminalOutcome<A>> = Promise.resolve()
    .then(start)
    .then(
      (value) => ({ status: "started", value }) as const,
      (error) => ({ status: classifyFailure(error) }),
    )

  let timer: TimerHandle | null = null
  const deadline = new Promise<AvatarStartDeadlineOutcome<A>>((resolve) => {
    try {
      timer = clock.setTimeout(() => {
        timer = null
        resolve({ status: "timed_out" })
      }, deadlineMs)
    } catch {
      resolve({ status: "timed_out" })
    }
  })
  const outcome = Promise.race<AvatarStartDeadlineOutcome<A>>([
    completion,
    deadline,
  ]).finally(() => {
    if (timer === null) return
    try { clock.clearTimeout(timer) } catch { /* already cleared */ }
    timer = null
  })

  return { outcome, completion }
}
