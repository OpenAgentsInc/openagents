import { Effect, Schedule } from "effect"

/**
 * Shared Pylon runtime patterns for Effect-backed resource lifecycles.
 *
 * Use `Effect.acquireRelease` for resources that must be cleaned up when the
 * owning fiber completes, fails, or is interrupted: subprocess handles,
 * WebSocket clients, workspace/file locks, temporary worktrees, and assignment
 * runner timers. Keep the acquire step small and make release idempotent.
 */
export function scopedTimeout(input: {
  delayMs: number
  onTimeout: () => void
  setTimeout?: typeof setTimeout
  clearTimeout?: typeof clearTimeout
}) {
  const scheduleTimeout = input.setTimeout ?? setTimeout
  const clearScheduledTimeout = input.clearTimeout ?? clearTimeout
  return Effect.acquireRelease(
    Effect.sync(() => scheduleTimeout(input.onTimeout, Math.max(0, input.delayMs))),
    (timer) => Effect.sync(() => clearScheduledTimeout(timer)),
  )
}

export const PylonRuntimeRetrySchedules = {
  externalHttpProviderCall: Schedule.recurs(3),
  durableObjectCall: Schedule.recurs(2),
  walletAdjacentCall: Schedule.recurs(2),
  gitGithubOperation: Schedule.recurs(3),
  d1TransientFailure: Schedule.recurs(3),
  publicProjectionSync: Schedule.recurs(2),
} as const
