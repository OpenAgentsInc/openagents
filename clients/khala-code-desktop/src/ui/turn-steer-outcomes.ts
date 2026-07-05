import { settleFanout } from "./isolated-fanout"

/**
 * Shared per-turn-isolated steering helper for the desktop composer's
 * "steer this text into the active Codex turn(s)" call sites
 * (follow-up drafts, diff review comments, source-control action prompts).
 *
 * All three call sites map the same shape over `activeTurnIds`: steer the
 * same text into every concurrently active turn. Previously they used a bare
 * `Promise.all(targets.map(turnId => rpc.request.codexTurnSteer(...)))`,
 * so one turn's rejection threw before the per-turn `results.find(...)`
 * reporting logic ever ran, collapsing every sibling turn's outcome
 * (success or failure) into a single opaque generic error message.
 *
 * `steerEachTurn` isolates each turn's RPC call so every targeted turn
 * always produces its own outcome, even when siblings reject.
 */

export type TurnSteerTarget = string | undefined

export type TurnSteerOutcome = {
  readonly turnId: TurnSteerTarget
  readonly ok: boolean
  readonly error?: string
}

export const steerEachTurn = async (
  targets: ReadonlyArray<TurnSteerTarget>,
  steer: (turnId: TurnSteerTarget) => Promise<{ readonly ok: boolean; readonly error?: string | undefined }>,
): Promise<ReadonlyArray<TurnSteerOutcome>> => {
  const settled = await settleFanout(targets, steer)
  return settled.map((outcome): TurnSteerOutcome => {
    if (!outcome.ok) return { turnId: outcome.item, ok: false, error: outcome.error }
    if (!outcome.value.ok) {
      return { turnId: outcome.item, ok: false, error: outcome.value.error ?? "unknown error" }
    }
    return { turnId: outcome.item, ok: true }
  })
}

export const allTurnSteerOutcomesOk = (outcomes: ReadonlyArray<TurnSteerOutcome>): boolean =>
  outcomes.every(outcome => outcome.ok)

/**
 * Builds a user-facing message that reflects EVERY targeted turn's outcome,
 * never collapsing a partial failure into a single sibling's error. When
 * only some turns fail, the message names how many of how many failed so a
 * successful sibling's outcome is never silently lost.
 */
export const summarizeTurnSteerOutcomes = (
  outcomes: ReadonlyArray<TurnSteerOutcome>,
  labels: { readonly success: string; readonly failurePrefix: string },
): string => {
  const failures = outcomes.filter((outcome): outcome is TurnSteerOutcome & { ok: false } => !outcome.ok)
  if (failures.length === 0) return labels.success
  const firstError = failures[0]?.error ?? "unknown error"
  if (outcomes.length === 1) return `${labels.failurePrefix}: ${firstError}`
  if (failures.length === outcomes.length) {
    return `${labels.failurePrefix} for all ${outcomes.length} turns: ${firstError}`
  }
  return `${labels.failurePrefix} for ${failures.length} of ${outcomes.length} turns: ${firstError}`
}
