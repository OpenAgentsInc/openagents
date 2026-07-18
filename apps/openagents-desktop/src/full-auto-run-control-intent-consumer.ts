import type { FullAutoRunControlIntent, FullAutoRunControlIntentOutcomeReport } from "@openagentsinc/khala-sync"
import {
  fetchFullAutoRunControlIntents,
  reportFullAutoRunControlIntentOutcome,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import type { DesktopSessionCredential } from "./desktop-session-vault.ts"
import {
  pauseFullAutoRunAction,
  resumeFullAutoRunAction,
  stopFullAutoRunAction,
  type FullAutoRunActionContext,
} from "./full-auto-run-actions.ts"

/**
 * MOB-FA-02 (#8994): consumes the durable Pause/Resume/Stop control intents
 * mobile dispatches through `/api/full-auto-runs/control-intents`.
 *
 * Desktop is not always running or reachable when the phone dispatches an
 * intent -- so per the issue's explicit architectural steer, this is a
 * SERVER-MEDIATED poll, not a live push: on each tick (wired into the same
 * heartbeat cadence as `full-auto-run-projection-publisher.ts`'s existing
 * publish timer, `main.ts`), Desktop GETs every recent intent for the
 * signed-in owner, applies each still-`pending` one through the SAME
 * `full-auto-run-actions.ts` functions the loopback control API and the
 * owner UI's own IPC bridge already use (`actor: "mobile"` -- the run's
 * hardened invariants, workspace binding, and exactly-once dispatch lease
 * are untouched; this is additive, not a bypass), and POSTs the typed
 * `applied`/`rejected` outcome back. A phone-originated intent is NEVER
 * silently dropped: every intent this consumer sees gets an outcome report,
 * even when the action itself is refused.
 */

const rejectionReasonForActionError = (
  errorCode: string,
): FullAutoRunControlIntentOutcomeReport["rejectionReason"] => {
  if (errorCode === "not_found") return "run_not_found"
  if (errorCode === "illegal_transition") return "illegal_transition"
  if (errorCode === "workspace_mismatch") return "workspace_mismatch"
  if (errorCode === "lane_not_eligible") return "lane_not_eligible"
  return "storage_unavailable"
}

/**
 * Pure application of one control intent against the run registry -- the
 * exhaustive-unit-test surface. Never throws: an action refusal becomes a
 * typed `rejected` outcome report, never an unhandled exception.
 */
export const applyFullAutoRunControlIntent = (
  actionContext: FullAutoRunActionContext,
  intent: FullAutoRunControlIntent,
): FullAutoRunControlIntentOutcomeReport => {
  const outcome = intent.action === "pause"
    ? pauseFullAutoRunAction(actionContext, intent.runRef)
    : intent.action === "resume"
    ? resumeFullAutoRunAction(actionContext, intent.runRef)
    : stopFullAutoRunAction(actionContext, intent.runRef)

  if (outcome.ok) {
    return { intentId: intent.intentId, status: "applied", resultLifecycleState: outcome.value.state }
  }
  return {
    intentId: intent.intentId,
    status: "rejected",
    rejectionReason: rejectionReasonForActionError(outcome.error.error),
  }
}

export type FullAutoRunControlIntentConsumer = Readonly<{
  /** Fire-and-forget: pulls pending intents and applies them. Never throws,
   * never blocks the caller -- always resolves. */
  tick: () => Effect.Effect<void>
}>

export const makeFullAutoRunControlIntentConsumer = (input: Readonly<{
  sessionReady: () => boolean
  credential: () => DesktopSessionCredential | null
  baseUrl: string
  /** Builds the `mobile`-attributed action context fresh per tick, so it
   * always reflects the current registry/capabilities (the same discipline
   * `main.ts`'s `fullAutoRunActionContext` follows for `owner_ui`). */
  actionContext: () => FullAutoRunActionContext
  fetchImpl?: typeof fetch
}>): FullAutoRunControlIntentConsumer => {
  const tick = Effect.fn("FullAutoRunControlIntentConsumer.tick")(function* () {
    if (!input.sessionReady()) return
    const credential = input.credential()
    if (credential === null) return

    const listResult = yield* Effect.promise(() =>
      fetchFullAutoRunControlIntents({
        baseUrl: input.baseUrl,
        accessToken: credential.accessToken,
        ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
      }),
    )
    if (listResult.state !== "available") return

    const pending = listResult.intents.filter(intent => intent.status === "pending")
    if (pending.length === 0) return

    const actionContext = input.actionContext()
    for (const intent of pending) {
      const outcomeReport = applyFullAutoRunControlIntent(actionContext, intent)
      yield* Effect.promise(() =>
        reportFullAutoRunControlIntentOutcome({
          baseUrl: input.baseUrl,
          accessToken: credential.accessToken,
          outcome: outcomeReport,
          ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
        }),
      )
    }
  })

  return {
    tick: () => tick().pipe(Effect.catch(() => Effect.void)),
  }
}
