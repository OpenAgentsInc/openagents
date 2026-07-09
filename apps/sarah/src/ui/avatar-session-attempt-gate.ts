/**
 * Small synchronous fence around imperative avatar starts/restarts.
 *
 * A transition owns at most one stop→start sequence. Generations keep late
 * callbacks or promise completions from an older session from replacing the
 * current handle, and disposal permanently rejects every late completion.
 */
export type AvatarSessionAttemptGate = Readonly<{
  /** Cleanup transitions remain available while replacement is fail-closed. */
  tryBeginTransition: () => boolean
  /** Start/reconnect transitions refuse after an unconfirmed prior stop. */
  tryBeginReplacementTransition: () => boolean
  finishTransition: () => void
  nextAttempt: () => number
  supersedeAttempt: () => void
  accepts: (generation: number) => boolean
  blockReplacement: () => void
  unblockReplacement: () => void
  dispose: () => void
}>

export type AvatarCleanupObservation =
  | "pending"
  | "confirmed"
  | "unconfirmed"

/** The one gate transition law shared by the surface and lifecycle oracles. */
export function applyAvatarCleanupObservation(
  gate: AvatarSessionAttemptGate,
  observation: AvatarCleanupObservation,
): void {
  if (observation === "pending") {
    gate.blockReplacement()
    return
  }
  if (observation === "confirmed") {
    gate.unblockReplacement()
    return
  }
  gate.supersedeAttempt()
  gate.blockReplacement()
}

export function makeAvatarSessionAttemptGate(): AvatarSessionAttemptGate {
  let generation = 0
  let transitionInFlight = false
  let replacementBlocked = false
  let disposed = false

  return {
    tryBeginTransition: () => {
      if (disposed || transitionInFlight) return false
      transitionInFlight = true
      return true
    },
    tryBeginReplacementTransition: () => {
      if (disposed || transitionInFlight || replacementBlocked) return false
      transitionInFlight = true
      return true
    },
    finishTransition: () => {
      transitionInFlight = false
    },
    nextAttempt: () => {
      generation += 1
      return generation
    },
    supersedeAttempt: () => {
      generation += 1
    },
    accepts: (candidate) => !disposed && candidate === generation,
    blockReplacement: () => {
      replacementBlocked = true
    },
    unblockReplacement: () => {
      replacementBlocked = false
    },
    dispose: () => {
      if (disposed) return
      disposed = true
      generation += 1
      transitionInFlight = true
    },
  }
}
