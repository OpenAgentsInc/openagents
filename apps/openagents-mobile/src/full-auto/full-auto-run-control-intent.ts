import {
  dispatchFullAutoRunControlIntent,
  fetchFullAutoRunControlIntents,
  type FullAutoRunControlIntentDispatchResult,
  type FullAutoRunControlIntentFetch,
  type FullAutoRunControlIntentListResult,
} from "@openagentsinc/khala-sync-client"
import type { FullAutoRunControlAction } from "@openagentsinc/khala-sync"

import type { FullAutoRunLifecycleState } from "./full-auto-run-projection"

/**
 * MOB-FA-02 (#8994): mobile's Pause/Resume/Stop dispatch + outcome-poll
 * idiom, mirroring `mobile-conversation.ts`'s `controlTurn()` dispatch/poll
 * shape for the SEPARATE Full Auto run-control substrate (durable REST
 * intents through `/api/full-auto-runs/control-intents`, not Khala Sync
 * runtime-control entities -- different domain, same "dispatch then poll for
 * a durable outcome, never complete from optimistic state" discipline).
 *
 * Desktop is not always reachable (asleep, offline, owner away from the
 * Mac): a dispatch always returns fast (the server durably records
 * `pending` and returns immediately); this module then polls for the
 * eventual `applied`/`rejected` outcome up to a bounded deadline. If that
 * deadline elapses with no outcome, the honest result is `"pending"` --
 * NEVER treated as success, and the caller should keep the run's UI in a
 * "pending" state rather than assuming the action took effect.
 */

export type FullAutoRunControlDispatchOutcome =
  | Readonly<{ state: "applied"; resultLifecycleState: FullAutoRunLifecycleState | null }>
  | Readonly<{ state: "rejected"; reason: string }>
  /** The poll deadline elapsed with no outcome yet -- Desktop has not (yet)
   * picked up the intent. Not a failure: the intent is still durably
   * pending server-side and may still apply later. The caller must render
   * this honestly as pending, never as success or failure. */
  | Readonly<{ state: "pending" }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>
  | Readonly<{ state: "rejected_at_dispatch"; code: string }>

export type FullAutoRunControlDispatcher = (input: Readonly<{
  runRef: string
  action: FullAutoRunControlAction
}>) => Promise<FullAutoRunControlDispatchOutcome>

const DEFAULT_POLL_ATTEMPTS = 10
const DEFAULT_POLL_INTERVAL_MS = 1500

export const makeFullAutoRunControlDispatcher = (input: Readonly<{
  baseUrl: string
  accessToken: () => string | null
  fetchImpl?: FullAutoRunControlIntentFetch
  pollAttempts?: number
  pollIntervalMs?: number
  sleep?: (ms: number) => Promise<void>
  randomId?: () => string
}>): FullAutoRunControlDispatcher => {
  const sleep = input.sleep ?? (ms => new Promise<void>(resolve => setTimeout(resolve, ms)))
  const pollAttempts = input.pollAttempts ?? DEFAULT_POLL_ATTEMPTS
  const pollIntervalMs = input.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS
  const randomId = input.randomId ?? (() => Math.random().toString(36).slice(2, 12))

  return async ({ runRef, action }) => {
    const accessToken = input.accessToken()
    if (accessToken === null) return { state: "unauthorized" }

    const intentId = `intent.mobile.${randomId()}`
    const idempotencyKey = `idem.mobile.${randomId()}`

    const dispatchResult: FullAutoRunControlIntentDispatchResult = await dispatchFullAutoRunControlIntent({
      baseUrl: input.baseUrl,
      accessToken,
      intentId,
      idempotencyKey,
      runRef,
      action,
      ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
    })
    if (dispatchResult.state === "unauthorized") return { state: "unauthorized" }
    if (dispatchResult.state === "rejected") return { state: "rejected_at_dispatch", code: dispatchResult.code }
    if (dispatchResult.state === "unavailable") return { state: "unavailable" }

    const dispatchedIntentId = dispatchResult.intent.intentId

    for (let attempt = 0; attempt < pollAttempts; attempt += 1) {
      await sleep(pollIntervalMs)
      const listResult: FullAutoRunControlIntentListResult = await fetchFullAutoRunControlIntents({
        baseUrl: input.baseUrl,
        accessToken,
        ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
      })
      if (listResult.state === "unauthorized") return { state: "unauthorized" }
      if (listResult.state !== "available") continue
      const match = listResult.intents.find(intent => intent.intentId === dispatchedIntentId)
      if (match === undefined) continue
      if (match.status === "applied") {
        return { state: "applied", resultLifecycleState: match.resultLifecycleState as FullAutoRunLifecycleState | null }
      }
      if (match.status === "rejected") {
        return { state: "rejected", reason: match.rejectionReason ?? "storage_unavailable" }
      }
      // status === "pending": keep polling until the deadline.
    }
    return { state: "pending" }
  }
}
