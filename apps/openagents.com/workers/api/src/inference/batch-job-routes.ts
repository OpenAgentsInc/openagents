import { Effect, Schema as S } from 'effect'

import { noStoreJsonResponse } from '../http/responses'
import { compactRandomId } from '../runtime-primitives'
import { settleBatchJobCharge } from './batch-job-metering'
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

    return noStoreJsonResponse({
      jobId,
      receiptRef: settle.receiptRef,
      status: 'accepted',
      totalCostMsat,
    })
  })
