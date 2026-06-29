// Verdict-callback ingest route for the Khala acceptance-dispatch loop (EPIC #6017).
//
// A node-side runner (a Pylon / `oa-workroomd` sandbox / Cloud Run service) runs the
// headless acceptance suite OUT of the Worker and POSTs its `AcceptanceVerdict` back
// here. This authenticated route BACKFILLS the khala-code verification verdict for the
// request: `unverified` -> `test_passed`/`failed`, with `verified`, `scalarReward`,
// and per-test results. So a real `verified` (and the M6 training reward) finally
// means "we ran it and it did what the user asked," not a regex over source.
//
// FAIL-CLOSED + RECEIPT-FIRST + IDEMPOTENT:
//   - rejects an unauthenticated / forged verdict (constant-time bearer check against
//     ACCEPTANCE_VERDICT_CALLBACK_TOKEN; absent token => every verdict rejected);
//   - rejects a malformed body or a verdict that did not actually execute;
//   - backfills through the SAME `verifyKhalaCodeCompletion` the hot path uses, so the
//     receipt ref + verification states are identical (no parallel mapping);
//   - a redelivered callback for an already-executed verdict is a no-op (200, no
//     regression, no double-write).
//
// INERT BY DEFAULT: gated by INFERENCE_GATEWAY_ENABLED like the rest of the gateway,
// AND closed unless the callback token is configured. With either off the route is a
// 404 / rejects everything, so prod behaviour is unchanged until a runner host is
// deployed and the token is set.

import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { parseJsonUnknown } from '../json-boundary'
import {
  AcceptanceVerdictCallbackBody,
  authenticateVerdictCallback,
  backfillVerdictIntoVerification,
  type KhalaVerificationStore,
} from './acceptance-dispatch'
import {
  type AcceptedOutcomeSettledSummary,
  type KhalaAcceptedOutcome,
} from './khala-accepted-outcome-settlement'
import { KHALA_CODE_VERIFIER_WORKER_ID } from './khala-code-verifier'

// The accepted-outcome settlement sink the callback fires when a verified, executed
// outcome BACKFILLS for the FIRST time. Returns the settled summary so the route can
// surface `settled:true` + the settlement receipt refs in the response. The wiring
// layer (`index.ts`) builds this from `settleVerifiedAcceptedOutcome` +
// `summarizeAcceptedOutcomeSettlement` behind the loop-arming flag + the M3 owner gate;
// a test passes a mock. Effect (fail-soft): it must never throw into the route.
export type AcceptedOutcomeSettlementSink = (
  outcome: KhalaAcceptedOutcome,
) => Effect.Effect<AcceptedOutcomeSettledSummary>

export type AcceptanceVerdictCallbackDeps = Readonly<{
  // Gateway flag (INFERENCE_GATEWAY_ENABLED). Default OFF => 404.
  enabled: boolean
  // The configured runner-callback bearer token (ACCEPTANCE_VERDICT_CALLBACK_TOKEN).
  // Undefined => the callback is closed (every verdict rejected). Never logged.
  callbackToken: string | undefined
  // The verification verdict store the backfill upserts into (D1 in prod, in-memory
  // in tests). The public receipt read projects from this store.
  store: KhalaVerificationStore
  nowIso: () => string
  // The accepted-outcome settlement sink (#6011). OPTIONAL: absent => no settlement is
  // attempted (the honest default before the loop is armed). When present, it fires
  // ONLY on the FIRST backfill of a VERIFIED + EXECUTED outcome (a redelivered/idempotent
  // callback never re-fires it, so settlement is at-most-once per accepted outcome). The
  // sink itself fail-closes by default (owner gate OFF) and is fail-soft.
  settlement?: AcceptedOutcomeSettlementSink | undefined
}>

const safeJsonParse = (text: string): unknown => {
  try {
    return parseJsonUnknown(text)
  } catch {
    return null
  }
}

const decodeBody = (
  value: unknown,
): AcceptanceVerdictCallbackBody | undefined => {
  try {
    return S.decodeUnknownSync(AcceptanceVerdictCallbackBody)(value)
  } catch {
    return undefined
  }
}

export const handleAcceptanceVerdictCallback = (
  request: Request,
  deps: AcceptanceVerdictCallbackDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    // AUTHENTICATE the runner. Fail-closed: absent configured token, missing/malformed
    // header, or mismatch all reject 401. Reject BEFORE reading the body so a forged
    // verdict never reaches the store.
    const authed = authenticateVerdictCallback({
      authorizationHeader: request.headers.get('authorization'),
      configuredToken: deps.callbackToken,
    })
    if (!authed) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse(
        { error: 'unauthorized' },
        { headers, status: 401 },
      )
    }

    const text = yield* Effect.promise(() => request.text().catch(() => ''))
    if (text === '') {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }
    const parsed = safeJsonParse(text)
    const body = decodeBody(parsed)
    if (body === undefined) {
      // Malformed, or a verdict whose `executed` is not true (we only backfill from a
      // REAL run — the schema requires `executed: true`).
      return noStoreJsonResponse(
        { error: 'invalid_acceptance_verdict' },
        { status: 400 },
      )
    }

    const outcome = yield* backfillVerdictIntoVerification(
      { nowIso: deps.nowIso, store: deps.store },
      body,
    )

    // ACCEPTED-OUTCOME SETTLEMENT (#6011, EPIC #6017). Fire the settlement sink ONLY on
    // the FIRST backfill of a VERIFIED + EXECUTED accepted outcome — so a verify is what
    // pays the worker + validator, and a redelivered/idempotent callback never re-fires
    // it (at-most-once per accepted outcome). The sink itself fail-closes by default
    // (owner real-settlement gate OFF) and is fail-soft; we additionally swallow here so
    // a settlement error never regresses the already-backfilled receipt response.
    let settled: AcceptedOutcomeSettledSummary | null = null
    if (
      deps.settlement !== undefined &&
      outcome.backfilled &&
      outcome.record.verified &&
      outcome.record.executed
    ) {
      settled = yield* deps
        .settlement({
          executed: outcome.record.executed,
          requestId: outcome.record.requestId,
          scalarReward: outcome.record.scalarReward,
          servedModel: body.servedModel,
          // The VALIDATOR is the independent verifier that ran the headless acceptance
          // suite (a distinct party from the producer). The SERVING WORKER that produced
          // the accepted artifact is echoed back on `body.worker` (the dispatch carried
          // the serving adapter id through to the runner).
          validatorRef: KHALA_CODE_VERIFIER_WORKER_ID,
          verificationReceiptRef: outcome.record.verificationReceiptRef,
          verified: outcome.record.verified,
          workerRef: body.worker,
        })
        .pipe(
          Effect.orElseSucceed(
            (): AcceptedOutcomeSettledSummary => ({
              settled: false,
              settledParties: [],
              settlementReceiptRefs: [],
            }),
          ),
        )
    }

    return noStoreJsonResponse(
      {
        backfilled: outcome.backfilled,
        failedChecks: outcome.record.failedChecks,
        passedChecks: outcome.record.passedChecks,
        requestId: outcome.record.requestId,
        scalarReward: outcome.record.scalarReward,
        // Settlement surface (#6011): `settled:true` + the per-party settlement receipt
        // refs when the worker + validator were actually paid; the honest inert default
        // (`settled:false`, empty refs) until the loop + owner gate are armed.
        ...(settled === null
          ? {}
          : {
              settled: settled.settled,
              settledParties: settled.settledParties,
              settlementReceiptRefs: settled.settlementReceiptRefs,
            }),
        verificationReceiptRef: outcome.record.verificationReceiptRef,
        verified: outcome.record.verified,
        verdict: outcome.record.verification,
      },
      { status: 200 },
    )
  })
