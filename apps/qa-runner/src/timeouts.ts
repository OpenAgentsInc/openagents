// Per-step timeouts + bounded, deterministic, opt-in flaky retries.
//
// The "last 10%" of a trustworthy harness is the failure plumbing: a step that
// hangs must NOT hang the whole run, and a genuinely-flaky step may be retried a
// BOUNDED number of times — but a retry must never manufacture a green. This
// module is pure (no real timers leak into tests): a deadline is raced against
// an injectable timer, and the retry loop is deterministic (no randomized
// backoff, no wall-clock sleep). The runner composes these around each step.
//
// Honesty rules encoded here:
//   - A timeout is a FAILURE, surfaced as `StepTimeoutError` — never swallowed.
//   - Retries are OPT-IN and BOUNDED. A step that only passes after a retry is
//     recorded with `attempts > 1` so a reviewer SEES the flake (no silent
//     flaky-pass). A step that never passes within `maxAttempts` fails honestly.
//   - Deterministic: the same inputs produce the same number of attempts and the
//     same outcome. No `Math.random`, no `setTimeout`-based jitter in the policy.

/** A bounded, deterministic retry policy for a single step. Opt-in. */
export interface RetryPolicy {
  /**
   * Total attempts including the first (so `1` = no retry, `3` = try once then
   * retry up to twice). Must be >= 1; values < 1 are clamped to 1.
   */
  readonly maxAttempts: number;
  /**
   * Decide whether a given error is worth retrying. Defaults to "retry any
   * error" when omitted, but a scenario can scope retries to, e.g., transient
   * navigation/timeout errors only. A non-retryable error fails immediately.
   */
  readonly retryable?: (error: unknown) => boolean;
  /**
   * Optional deterministic per-attempt delay, in ms. The DELAY ITSELF is run
   * through the injectable timer (so tests stay instant); it is NOT a wall-clock
   * sleep against real time. Defaults to 0 (retry immediately). A function form
   * lets a policy express bounded linear backoff deterministically.
   */
  readonly delayMs?: number | ((attempt: number) => number);
}

/** Per-step execution policy: a timeout and an optional retry policy. */
export interface StepPolicy {
  /** Hard per-step timeout in ms. <= 0 or undefined means "no timeout". */
  readonly timeoutMs?: number;
  /** Bounded, deterministic, opt-in retry policy. Omitted means no retry. */
  readonly retry?: RetryPolicy;
}

/**
 * An injectable timer so deadlines and retry delays never touch the real clock
 * in tests. `delay(ms)` resolves after `ms` and exposes `cancel()` so a settled
 * race can clear a pending timer (no leaked handles, no false "still running").
 */
export interface TimerLike {
  readonly delay: (ms: number) => { readonly promise: Promise<void>; readonly cancel: () => void };
}

/** The real timer (production default): `setTimeout`/`clearTimeout`. */
export const realTimer: TimerLike = {
  delay: (ms) => {
    let handle: ReturnType<typeof setTimeout> | undefined;
    const promise = new Promise<void>((resolve) => {
      handle = setTimeout(resolve, Math.max(0, ms));
    });
    return { promise, cancel: () => (handle !== undefined ? clearTimeout(handle) : undefined) };
  },
};

/** Thrown when a step does not settle within its timeout. A real failure. */
export class StepTimeoutError extends Error {
  constructor(
    readonly label: string,
    readonly timeoutMs: number,
  ) {
    super(`step "${label}" timed out after ${timeoutMs}ms`);
    this.name = "StepTimeoutError";
  }
}

/**
 * Race `run()` against a deadline. If `run()` settles first, its result/throw
 * propagates and the pending timer is cancelled. If the deadline wins, this
 * rejects with `StepTimeoutError` — the step's own promise is abandoned (we
 * cannot force-cancel an arbitrary promise), but the run does not hang and the
 * browser-surface release/flush still fires upstream.
 *
 * Deterministic: with the injected fake timer, a test fires the deadline
 * explicitly and gets a `StepTimeoutError` with zero real wall-clock wait.
 */
export async function withDeadline<A>(
  label: string,
  timeoutMs: number | undefined,
  run: () => Promise<A>,
  timer: TimerLike = realTimer,
): Promise<A> {
  if (timeoutMs === undefined || timeoutMs <= 0) return run();
  const deadline = timer.delay(timeoutMs);
  try {
    return await Promise.race([
      run(),
      deadline.promise.then((): never => {
        throw new StepTimeoutError(label, timeoutMs);
      }),
    ]);
  } finally {
    deadline.cancel();
  }
}

/** The outcome of running one step under a policy: value + attempt accounting. */
export interface PolicyOutcome<A> {
  readonly value: A;
  /** How many attempts were spent (>= 1). `> 1` means a flake was retried. */
  readonly attempts: number;
}

const resolveDelay = (delayMs: RetryPolicy["delayMs"], attempt: number): number => {
  if (delayMs === undefined) return 0;
  return typeof delayMs === "function" ? delayMs(attempt) : delayMs;
};

/**
 * Run `step()` under a `StepPolicy`: each attempt is bounded by the per-step
 * timeout, and a retryable failure is retried up to `maxAttempts` total, with a
 * deterministic delay run through the injectable timer.
 *
 * On success it returns the value AND the attempt count (so the caller records
 * `attempts > 1` as a visible flake — never a silent flaky-pass). On exhausted
 * retries it throws the LAST error (honest red). A non-retryable error throws
 * immediately without burning the remaining attempts.
 */
export async function runStepWithPolicy<A>(
  label: string,
  policy: StepPolicy | undefined,
  step: () => Promise<A>,
  timer: TimerLike = realTimer,
): Promise<PolicyOutcome<A>> {
  const maxAttempts = Math.max(1, policy?.retry?.maxAttempts ?? 1);
  const retryable = policy?.retry?.retryable ?? (() => true);
  let lastError: unknown;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    try {
      const value = await withDeadline(label, policy?.timeoutMs, step, timer);
      return { value, attempts: attempt };
    } catch (error) {
      lastError = error;
      const more = attempt < maxAttempts;
      if (!more || !retryable(error)) break;
      const delay = resolveDelay(policy?.retry?.delayMs, attempt);
      if (delay > 0) await timer.delay(delay).promise;
    }
  }
  throw lastError;
}
