/**
 * FA-05 / MOB-FA-02: apply a mobile Pause/Resume/Stop intent through the same
 * full-auto-run-actions surface the owner UI and control API use.
 *
 * Mobile never writes durable run state directly. Every intent returns a typed
 * applied/rejected outcome — never a silent drop.
 */

import {
  pauseFullAutoRunAction,
  resumeFullAutoRunAction,
  stopFullAutoRunAction,
  type FullAutoRunActionContext,
} from "./full-auto-run-actions.ts"

export type FullAutoRunControlAction = "pause" | "resume" | "stop"

export type FullAutoRunControlIntent = Readonly<{
  intentId: string
  runRef: string
  action: FullAutoRunControlAction
}>

export type FullAutoRunControlRejectionReason =
  | "run_not_found"
  | "illegal_transition"
  | "workspace_mismatch"
  | "lane_not_eligible"
  | "storage_unavailable"

export type FullAutoRunControlIntentOutcome =
  | Readonly<{
      intentId: string
      status: "applied"
      resultLifecycleState: string
    }>
  | Readonly<{
      intentId: string
      status: "rejected"
      rejectionReason: FullAutoRunControlRejectionReason
    }>

const rejectionReasonForActionError = (
  errorCode: string,
): FullAutoRunControlRejectionReason => {
  if (errorCode === "not_found") return "run_not_found"
  if (errorCode === "illegal_transition") return "illegal_transition"
  if (errorCode === "workspace_mismatch") return "workspace_mismatch"
  if (errorCode === "lane_not_eligible") return "lane_not_eligible"
  return "storage_unavailable"
}

/** Pure application of one control intent. Never throws. */
export const applyFullAutoRunControlIntent = (
  actionContext: FullAutoRunActionContext,
  intent: FullAutoRunControlIntent,
): FullAutoRunControlIntentOutcome => {
  const outcome =
    intent.action === "pause"
      ? pauseFullAutoRunAction(actionContext, intent.runRef)
      : intent.action === "resume"
        ? resumeFullAutoRunAction(actionContext, intent.runRef)
        : stopFullAutoRunAction(actionContext, intent.runRef)

  if (outcome.ok) {
    return {
      intentId: intent.intentId,
      status: "applied",
      resultLifecycleState: outcome.value.state,
    }
  }
  return {
    intentId: intent.intentId,
    status: "rejected",
    rejectionReason: rejectionReasonForActionError(outcome.error.error),
  }
}
