import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { compactRandomId } from '../runtime-primitives'
import { settleBatchJobCharge } from './batch-job-metering'
import { makeD1BatchJobStore } from './batch-job-store'
import { projectBatchJobCloseoutReceipt } from './batch-job-closeout-receipts'
// public-projection-staleness
import { estimateRequestCost } from './cost-estimate'

export type BatchJobRoutesDeps = Readonly<{
  authenticate: (
    request: Request,
  ) => Promise<{ accountRef: string } | undefined>
  db: D1Database
  enabled: boolean
  nowIso: () => string
}>

const BatchJobItemSchema = S.Struct({
  completionTokens: S.Number,
  model: S.String,
  promptTokens: S.Number,
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

    return noStoreJsonResponse({
      jobId,
      receiptRef: settle.receiptRef,
      status: 'accepted',
      totalCostMsat,
    })
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
