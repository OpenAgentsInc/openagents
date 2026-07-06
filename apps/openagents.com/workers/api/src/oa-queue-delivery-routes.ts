// CFG-7 (#8522): internal delivery route for the Postgres JobQueue.
//
// The four Cloudflare Queues were evacuated to the oa-infra Postgres
// JobQueue (`oa_infra_jobs`). Producers enqueue with a single INSERT
// (src/oa-job-queue-producer.ts); the Cloud Run pump `apps/oa-queue-worker`
// leases jobs (FOR UPDATE SKIP LOCKED) and delivers each one HERE, because
// the original queue-handler code needs bindings that only exist in this
// app runtime (D1, the EVENT_LEDGER_OWNER Durable Object, provider
// secrets). The handler logic itself is unchanged from the old Worker
// `queue()` export — this route is the same schemaVersion dispatch behind
// an admin bearer instead of a Cloudflare Queues consumer.
//
// Contract with the pump:
//   POST { topic, jobId, attempts, payload }  (payload = raw job payload,
//   a JSON string of the original queue message)
//   200 { ok: true }            -> pump acks the job (removed)
//   4xx/5xx { ok: false, ... }  -> pump nacks (retry, then dead-letter)
//
// AUTH: same admin bearer mechanism as the other internal routes
// (`requireAdminApiToken`, injected as `requireOperator`).

import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'

type HttpResponse = globalThis.Response

export const OA_QUEUE_DELIVER_PATH = '/api/internal/queue/deliver'
export const OA_QUEUE_DELIVER_ROUTE_REF =
  'route.internal.oa_queue.deliver.v0_1'

export class OaQueueDeliverRequest extends S.Class<OaQueueDeliverRequest>(
  'OaQueueDeliverRequest',
)({
  topic: S.String,
  jobId: S.String,
  attempts: S.Number,
  /** Raw `oa_infra_jobs.payload` — a JSON string of the queue message. */
  payload: S.String,
}) {}

export type OaQueueDeliverDependencies = Readonly<{
  requireOperator: () => Promise<boolean>
  /**
   * Runs the original queue-consumer logic for one decoded message body.
   * Must throw (reject) on failure so the pump nacks the job.
   */
  dispatch: (body: unknown) => Promise<void>
}>

const safeErrorMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 300)
}

export const handleOaQueueDeliver = (
  request: Request,
  deps: OaQueueDeliverDependencies,
): Effect.Effect<HttpResponse> =>
  Effect.promise(async () => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    if (!(await deps.requireOperator())) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    let delivery: OaQueueDeliverRequest
    try {
      delivery = S.decodeUnknownSync(OaQueueDeliverRequest)(
        await request.json(),
      )
    } catch (error) {
      return noStoreJsonResponse(
        { error: 'bad_request', reason: safeErrorMessage(error) },
        { status: 400 },
      )
    }

    let body: unknown
    try {
      body = JSON.parse(delivery.payload)
    } catch (error) {
      // An unparseable payload can never succeed on retry; report it as a
      // permanent 422 so the operator sees it in dead letters quickly (the
      // pump still nacks toward the dead-letter state).
      return noStoreJsonResponse(
        {
          error: 'payload_not_json',
          reason: safeErrorMessage(error),
          routeRef: OA_QUEUE_DELIVER_ROUTE_REF,
        },
        { status: 422 },
      )
    }

    try {
      await deps.dispatch(body)
    } catch (error) {
      return noStoreJsonResponse(
        {
          error: 'dispatch_failed',
          ok: false,
          reason: safeErrorMessage(error),
          routeRef: OA_QUEUE_DELIVER_ROUTE_REF,
          topic: delivery.topic,
        },
        { status: 500 },
      )
    }

    return noStoreJsonResponse({
      jobId: delivery.jobId,
      ok: true,
      routeRef: OA_QUEUE_DELIVER_ROUTE_REF,
      topic: delivery.topic,
    })
  })
