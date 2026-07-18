import {
  FullAutoRunControlIntent as FullAutoRunControlIntentSchema,
  type FullAutoRunControlAction,
  type FullAutoRunControlIntent,
  type FullAutoRunControlIntentOutcomeReport,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"

/**
 * MOB-FA-02 (#8994): fetch/dispatch/report-outcome ergonomics for the
 * FullAutoRun control-intent sibling route, mirroring
 * `fetchFullAutoRunClientProjection`/`publishFullAutoRunClientProjection`'s
 * shape one-for-one (`full-auto-run-client-projection.ts` in this package).
 *
 * Three call sites use this module:
 *  - mobile dispatches a Pause/Resume/Stop intent with
 *    `dispatchFullAutoRunControlIntent`, then polls for its outcome with
 *    `fetchFullAutoRunControlIntents` (filtering client-side by `intentId`);
 *  - Desktop's control-intent consumer pulls ALL recent intents with
 *    `fetchFullAutoRunControlIntents` on its heartbeat tick, applies the
 *    still-`pending` ones addressed to its active run, and reports the
 *    outcome with `reportFullAutoRunControlIntentOutcome`.
 */
export const FULL_AUTO_RUN_CONTROL_INTENTS_PATH = "/api/full-auto-runs/control-intents"

const ListResponseEnvelope = S.Struct({
  ok: S.Literal(true),
  intents: S.Array(FullAutoRunControlIntentSchema),
})
const DispatchResponseEnvelope = S.Struct({
  ok: S.Literal(true),
  intent: FullAutoRunControlIntentSchema,
})
const OutcomeResponseEnvelope = S.Struct({
  ok: S.Literal(true),
  intent: FullAutoRunControlIntentSchema,
})

export type FullAutoRunControlIntentListResult =
  | Readonly<{ state: "available"; intents: ReadonlyArray<FullAutoRunControlIntent> }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

export type FullAutoRunControlIntentDispatchResult =
  | Readonly<{ state: "dispatched"; intent: FullAutoRunControlIntent }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "rejected"; code: string }>
  | Readonly<{ state: "unavailable" }>

export type FullAutoRunControlIntentOutcomeResult =
  | Readonly<{ state: "reported"; intent: FullAutoRunControlIntent }>
  | Readonly<{ state: "unauthorized" }>
  | Readonly<{ state: "unavailable" }>

export type FullAutoRunControlIntentFetch = (
  input: URL | RequestInfo,
  init?: RequestInit,
) => Promise<Response>

/** Lists recent control intents for the signed-in owner, newest last. Both
 * mobile (polling for one intent's outcome) and Desktop (pulling every
 * still-pending intent) call this same GET. */
export const fetchFullAutoRunControlIntents = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  fetchImpl?: FullAutoRunControlIntentFetch
}>): Promise<FullAutoRunControlIntentListResult> => {
  try {
    const response = await (input.fetchImpl ?? fetch)(
      new URL(FULL_AUTO_RUN_CONTROL_INTENTS_PATH, input.baseUrl),
      {
        method: "GET",
        headers: { authorization: `Bearer ${input.accessToken}` },
        cache: "no-store",
      },
    )
    if (response.status === 401 || response.status === 403) {
      return { state: "unauthorized" }
    }
    if (!response.ok) return { state: "unavailable" }
    const envelope = S.decodeUnknownSync(ListResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return { state: "available", intents: envelope.intents }
  } catch {
    return { state: "unavailable" }
  }
}

/** Mobile dispatches a Pause/Resume/Stop request. Never blocks on Desktop --
 * the server durably records the intent `pending` and returns immediately;
 * the caller polls `fetchFullAutoRunControlIntents` for the eventual
 * `applied`/`rejected` outcome. */
export const dispatchFullAutoRunControlIntent = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  intentId: string
  idempotencyKey: string
  runRef: string
  action: FullAutoRunControlAction
  fetchImpl?: FullAutoRunControlIntentFetch
}>): Promise<FullAutoRunControlIntentDispatchResult> => {
  try {
    const response = await (input.fetchImpl ?? fetch)(
      new URL(FULL_AUTO_RUN_CONTROL_INTENTS_PATH, input.baseUrl),
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${input.accessToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          intent: {
            intentId: input.intentId,
            idempotencyKey: input.idempotencyKey,
            runRef: input.runRef,
            action: input.action,
          },
        }),
      },
    )
    if (response.status === 401 || response.status === 403) {
      return { state: "unauthorized" }
    }
    if (!response.ok) {
      const body = await response.json().catch(() => null) as { error?: { code?: string } } | null
      const code = body?.error?.code
      return typeof code === "string" ? { state: "rejected", code } : { state: "unavailable" }
    }
    const envelope = S.decodeUnknownSync(DispatchResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return { state: "dispatched", intent: envelope.intent }
  } catch {
    return { state: "unavailable" }
  }
}

/** Desktop reports the applied/rejected outcome for an intent it just
 * consumed and acted on. */
export const reportFullAutoRunControlIntentOutcome = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  outcome: FullAutoRunControlIntentOutcomeReport
  fetchImpl?: FullAutoRunControlIntentFetch
}>): Promise<FullAutoRunControlIntentOutcomeResult> => {
  try {
    const response = await (input.fetchImpl ?? fetch)(
      new URL(FULL_AUTO_RUN_CONTROL_INTENTS_PATH, input.baseUrl),
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${input.accessToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({ outcome: input.outcome }),
      },
    )
    if (response.status === 401 || response.status === 403) {
      return { state: "unauthorized" }
    }
    if (!response.ok) return { state: "unavailable" }
    const envelope = S.decodeUnknownSync(OutcomeResponseEnvelope)(await response.json(), {
      onExcessProperty: "preserve",
    })
    return { state: "reported", intent: envelope.intent }
  } catch {
    return { state: "unavailable" }
  }
}
