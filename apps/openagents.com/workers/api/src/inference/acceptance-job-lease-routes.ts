// Authenticated job-LEASE + ACK routes for the out-of-Worker acceptance runner (EPIC
// #6017).
//
// The out-of-Worker runner cannot be a Cloudflare Queue consumer (a consumer is a
// Worker; chromium never runs in a Worker), so it PULLS its work here:
//   GET  <lease>  -> 204 (no job) | 200 { leaseId, job } (a claimed AcceptanceJobMessage)
//   POST <ack>    -> 200 { acked } for { leaseId, delivered }
// Both are authenticated with the SAME bearer token the verdict callback verifies
// (`ACCEPTANCE_VERDICT_CALLBACK_TOKEN`) — ONE secret authenticates the whole
// runner<->gateway channel (lease + ack + verdict POST).
//
// FAIL-CLOSED + INERT:
//   - gated by INFERENCE_GATEWAY_ENABLED (404 when off), like the verdict callback;
//   - closed unless the callback token is configured (every request 401);
//   - with no enqueued jobs the lease always returns 204 and the runner idles.
// So prod behaviour is unchanged until the dispatch producer is wired to enqueue into
// the pull queue AND a runner host is armed with the token.

import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { parseJsonUnknown } from '../json-boundary'
import {
  AcceptanceJobMessage,
  authenticateVerdictCallback,
} from './acceptance-dispatch'
import type { AcceptanceJobQueueStore } from './acceptance-job-queue-store'

// Default lease TTL: a runner that crashes mid-job has its job re-leasable after this.
// Generous relative to a headless suite (a few seconds) so a slow-but-live runner is not
// double-leased, bounded so a crash does not strand a job for long.
export const DEFAULT_ACCEPTANCE_LEASE_TTL_MS = 120_000

export type AcceptanceJobLeaseDeps = Readonly<{
  // Gateway flag (INFERENCE_GATEWAY_ENABLED). Default OFF => 404.
  enabled: boolean
  // The shared runner bearer token. Undefined => closed (every request 401). Never logged.
  callbackToken: string | undefined
  store: AcceptanceJobQueueStore
  nowIso: () => string
  // Unique lease-id minter (crypto.randomUUID in prod; deterministic in tests).
  newLeaseId: () => string
  leaseTtlMs?: number | undefined
}>

const unauthorized = () =>
  noStoreJsonResponse(
    { error: 'unauthorized' },
    { headers: new Headers({ 'www-authenticate': 'Bearer' }), status: 401 },
  )

const AckBody = S.Struct({
  leaseId: S.String,
  delivered: S.Boolean,
})

// GET: lease the next claimable acceptance job. 204 when the queue is empty (idle).
export const handleAcceptanceJobLease = (
  request: Request,
  deps: AcceptanceJobLeaseDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }
    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }
    if (
      !authenticateVerdictCallback({
        authorizationHeader: request.headers.get('authorization'),
        configuredToken: deps.callbackToken,
      })
    ) {
      return unauthorized()
    }

    const leased = yield* deps.store.lease({
      leaseTtlMs: deps.leaseTtlMs ?? DEFAULT_ACCEPTANCE_LEASE_TTL_MS,
      newLeaseId: deps.newLeaseId(),
      nowIso: deps.nowIso(),
    })
    if (leased === null) {
      return new Response(null, { status: 204 })
    }
    return noStoreJsonResponse(
      {
        // Encode the typed message back to plain JSON for the wire; the runner re-decodes
        // it through the SAME schema before running.
        job: S.encodeSync(AcceptanceJobMessage)(leased.message),
        leaseId: leased.leaseId,
      },
      { status: 200 },
    )
  })

// POST: ack a leased job as delivered (remove) or retryable (return to pending).
export const handleAcceptanceJobAck = (
  request: Request,
  deps: AcceptanceJobLeaseDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_gateway_disabled' },
        { status: 404 },
      )
    }
    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }
    if (
      !authenticateVerdictCallback({
        authorizationHeader: request.headers.get('authorization'),
        configuredToken: deps.callbackToken,
      })
    ) {
      return unauthorized()
    }

    const text = yield* Effect.promise(() => request.text().catch(() => ''))
    if (text === '') {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }
    const body = (() => {
      try {
        return S.decodeUnknownSync(AckBody)(parseJsonUnknown(text))
      } catch {
        return undefined
      }
    })()
    if (body === undefined) {
      return noStoreJsonResponse({ error: 'invalid_ack' }, { status: 400 })
    }

    yield* deps.store.ack({
      delivered: body.delivered,
      leaseId: body.leaseId,
      nowIso: deps.nowIso(),
    })
    return noStoreJsonResponse({ acked: true }, { status: 200 })
  })
