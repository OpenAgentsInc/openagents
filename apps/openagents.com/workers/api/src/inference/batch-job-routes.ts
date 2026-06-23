import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { compactRandomId } from '../runtime-primitives'
import {
  type BatchJobExecutableItem,
  BatchJobQueueMessage,
} from './batch-job-consumer'
import { settleBatchJobCharge } from './batch-job-metering'
import { makeD1BatchJobStore } from './batch-job-store'
import { projectBatchJobCloseoutReceipt } from './batch-job-closeout-receipts'
// public-projection-staleness
import { estimateRequestCost } from './cost-estimate'

// The PRODUCER seam for the async batch-job consumer (Khala, #6028 / EPIC
// #6017). The submit route charges + persists a `pending` row on the request
// path, then hands the executable items to this enqueue function so the work
// runs OFF the request path (the queue handler in `index.ts` decodes the
// `BatchJobQueueMessage` and calls `executeBatchJob`). The job id is the unit of
// idempotency: re-delivery of the same message is a safe no-op in the consumer,
// so a duplicate enqueue never re-runs or re-charges the job. INERT when absent:
// if no `enqueueBatchJob` seam is wired, the submit route persists the pending
// row and returns the receipt exactly as before — nothing is queued and the
// route's behaviour is unchanged (the consumer also stays flag-gated off).
export type EnqueueBatchJob = (
  message: BatchJobQueueMessage,
) => Effect.Effect<void>

export type BatchJobRoutesDeps = Readonly<{
  authenticate: (
    request: Request,
  ) => Promise<{ accountRef: string } | undefined>
  db: D1Database
  enabled: boolean
  nowIso: () => string
  // Optional producer. When provided, the submit route enqueues the executable
  // batch-job message after the `pending` row is persisted so the consumer can
  // run it detached. Undefined => no queue dispatch (inert producer): the job is
  // accepted + persisted but never executed, preserving the pre-producer
  // behaviour for the flag-off path.
  enqueueBatchJob?: EnqueueBatchJob | undefined
}>

// One submitted dataset row. Carries BOTH the token counts the route prices the
// upfront charge on (backward-compatible with the original token-only schema)
// AND the optional executable payload (`messages` + sampling params) the
// consumer needs to actually RUN the item against the gateway. `messages` is
// optional so the existing token-only submit shape keeps validating; only rows
// that carry `messages` become executable queue items.
const BatchJobItemSchema = S.Struct({
  completionTokens: S.Number,
  model: S.String,
  promptTokens: S.Number,
  // The executable prompt. When present the row is enqueued for real execution;
  // when absent the row only contributes to the upfront price estimate (the
  // original token-only behaviour) and is skipped by the consumer.
  messages: S.optionalKey(
    S.Array(
      S.Struct({
        role: S.String,
        content: S.String,
      }),
    ),
  ),
  // Standard sampling params (temperature, top_p, max_tokens, ...), forwarded
  // verbatim to the adapter by the consumer. Optional.
  passthroughParams: S.optionalKey(S.Record(S.String, S.Unknown)),
})

const BatchJobSubmitSchema = S.Struct({
  dataset: S.Array(BatchJobItemSchema),
})

const decodeBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(BatchJobSubmitSchema)(value)
  } catch {
    return undefined
  }
}

import { parseJsonUnknown } from '../json-boundary'

const safeJsonParse = (text: string): unknown => {
  try {
    return parseJsonUnknown(text)
  } catch {
    return null
  }
}

export const handleBatchJobsSubmit = (
  request: Request,
  deps: BatchJobRoutesDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_batch_jobs_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse(
        { error: 'unauthorized' },
        { headers, status: 401 },
      )
    }

    const textResult = yield* Effect.promise(() => request.text().catch(() => ''))
    
    if (textResult === '') {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const body = safeJsonParse(textResult)

    if (body === null) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    const payload = decodeBody(body)
    if (payload === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    let totalCostMsat = 0
    for (const item of payload.dataset) {
      const estimate = estimateRequestCost({
        batch: true,
        completionTokens: item.completionTokens,
        fundingKind: 'card',
        model: item.model,
        promptTokens: item.promptTokens,
      })
      totalCostMsat += estimate.estimatedChargeMsat
    }

    const jobId = compactRandomId('batch')

    const settle = yield* settleBatchJobCharge(deps, {
      accountRef: session.accountRef,
      costMsat: totalCostMsat,
      jobId,
    })

    if (!settle.ok) {
      return noStoreJsonResponse(
        { error: 'insufficient_funds' },
        { status: 402 },
      )
    }

    const store = makeD1BatchJobStore(deps.db, deps.nowIso)
    yield* store.insertBatchJob({
      jobId,
      accountRef: session.accountRef,
      status: 'pending',
      chargeReceiptRef: settle.receiptRef,
      datasetSize: payload.dataset.length,
      processedItems: 0,
      failedItems: 0,
      resultsR2Key: null,
      createdAt: deps.nowIso(),
    })

    // Hand the job to the async consumer OFF the request path. Only rows that
    // carry `messages` are executable; token-only rows are skipped (they priced
    // the charge but have no prompt to run). When no producer is wired, or when
    // no row is executable, nothing is enqueued — the job is still accepted and
    // persisted (pending), preserving the pre-producer behaviour. The job id is
    // the idempotency unit, so a re-enqueue is a safe no-op in the consumer.
    const executableItems: ReadonlyArray<BatchJobExecutableItem> =
      payload.dataset.flatMap(item =>
        item.messages === undefined
          ? []
          : [
              {
                model: item.model,
                messages: item.messages,
                ...(item.passthroughParams === undefined
                  ? {}
                  : { passthroughParams: item.passthroughParams }),
              },
            ],
      )

    if (deps.enqueueBatchJob !== undefined && executableItems.length > 0) {
      yield* deps.enqueueBatchJob(
        new BatchJobQueueMessage({
          schemaVersion: 'openagents.inference.batch_job.v1',
          jobId,
          items: executableItems,
        }),
      )
    }

    // 202 Accepted: the job is durably accepted + charged but runs detached, so
    // the client polls `/v1/inference/batches/:jobId` and dereferences the
    // closeout receipt rather than blocking on the edge request.
    return noStoreJsonResponse(
      {
        jobId,
        receiptRef: settle.receiptRef,
        status: 'accepted',
        totalCostMsat,
      },
      { status: 202 },
    )
  })

export const handleBatchJobReceiptRead = (
  request: Request,
  deps: BatchJobRoutesDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_batch_jobs_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const url = new URL(request.url)
    const match = url.pathname.match(/^\/api\/public\/inference\/batch-job-receipts\/(.+)$/)
    if (!match) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const receiptRef = match[1] ?? ''
    const prefix = 'receipt.inference.batch_job.closeout.'
    if (!receiptRef.startsWith(prefix)) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const jobId = receiptRef.slice(prefix.length)
    const store = makeD1BatchJobStore(deps.db, deps.nowIso)
    const job = yield* store.getBatchJob(jobId)

    if (!job || job.status !== 'completed') {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    // Cost MSat from ledger
    const costMsatRaw = yield* Effect.tryPromise(() =>
      deps.db
        .prepare('SELECT cost_msat FROM pay_ins WHERE public_receipt_ref = ? LIMIT 1')
        .bind(job.chargeReceiptRef)
        .first<{ cost_msat: number }>()
    ).pipe(Effect.orDie)

    const costMsat = costMsatRaw ? costMsatRaw.cost_msat : 0

    const receipt = {
      schemaVersion: 'openagents.inference.batch_job.closeout.v1' as const,
      receiptRef,
      jobId: job.jobId,
      chargeReceiptRef: job.chargeReceiptRef,
      totalItems: job.datasetSize,
      successfulItems: job.processedItems - job.failedItems,
      failedItems: job.failedItems,
      totalCostMsat: costMsat,
      completedAtIso: job.updatedAt,
      resultsR2Key: job.resultsR2Key || '',
    }

    return noStoreJsonResponse(
      projectBatchJobCloseoutReceipt(receipt, deps.nowIso())
    )
  })

export const handleBatchJobStatusRead = (
  request: Request,
  deps: BatchJobRoutesDeps,
) =>
  Effect.gen(function* () {
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'inference_batch_jobs_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse({ error: 'unauthorized' }, { headers, status: 401 })
    }

    const url = new URL(request.url)
    const match = url.pathname.match(/^\/v1\/inference\/batches\/(.+)$/)
    if (!match) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const jobId = match[1] ?? ''
    const store = makeD1BatchJobStore(deps.db, deps.nowIso)
    const job = yield* store.getBatchJob(jobId)

    if (!job || job.accountRef !== session.accountRef) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse({
      jobId: job.jobId,
      status: job.status,
      datasetSize: job.datasetSize,
      processedItems: job.processedItems,
      failedItems: job.failedItems,
      resultsR2Key: job.resultsR2Key,
      createdAt: job.createdAt,
      updatedAt: job.updatedAt,
    })
  })
