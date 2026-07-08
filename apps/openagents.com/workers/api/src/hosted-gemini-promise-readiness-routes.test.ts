import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type {
  InferenceReceiptReadStore,
  InferenceReceiptRecord,
} from './inference-receipts'
import type {
  PromiseTransitionReceipt,
  PromiseTransitionReceiptStore,
} from './promise-transition-receipt-routes'
import { makeHostedGeminiPromiseReadinessRoutes } from './hosted-gemini-promise-readiness-routes'

const receiptRef = 'receipt.inference.charge.chatcmpl_hosted_gemini_prod'

const record = (
  input: Partial<InferenceReceiptRecord> = {},
): InferenceReceiptRecord => ({
  contextRef:
    'inference:vertex-gemini:served:models%2Fgemini-3.5-flash:tokens:321:requested:openagents%2Fkhala',
  createdAt: '2026-06-29T11:59:00.000Z',
  payInType: 'adjustment',
  receiptRef,
  state: 'paid',
  stateChangedAt: '2026-06-29T11:59:01.000Z',
  ...input,
})

const transition = (
  input: Partial<PromiseTransitionReceipt> = {},
): PromiseTransitionReceipt => ({
  checkedAt: '2026-06-29T12:01:00.000Z',
  checks: [],
  evidenceRefs: [receiptRef],
  exception: {
    approvedByRef: 'owner:openagents',
    expiresAt: '2026-07-06',
    reasonRef: 'owner_signoff.hosted_gemini.production_receipt',
  },
  fromState: 'yellow',
  promiseId: 'api.hosted_gemini.v1',
  receiptId: 'promise_transition_hosted_gemini_green_1',
  registryVersion: '2026-06-29.1',
  result: 'exception',
  toState: 'green',
  ...input,
})

const stores = (
  records: ReadonlyArray<InferenceReceiptRecord>,
  receipts: ReadonlyArray<PromiseTransitionReceipt>,
) => {
  const inference: InferenceReceiptReadStore = {
    readInferenceReceiptByRef: ref =>
      Promise.resolve(records.find(candidate => candidate.receiptRef === ref) ?? null),
  }
  const transitions: PromiseTransitionReceiptStore = {
    createReceipt: () => Promise.resolve(),
    listReceipts: () => Promise.resolve(receipts),
  }

  return { inference, transitions }
}

const route = async (
  input: ReturnType<typeof stores>,
  url: string,
  init?: RequestInit,
) => {
  const routes = makeHostedGeminiPromiseReadinessRoutes<
    ReturnType<typeof stores>
  >({
    makeInferenceReceiptStore: env => env.inference,
    makeTransitionReceiptStore: env => env.transitions,
    nowIso: () => '2026-06-29T12:02:00.000Z',
  })
  const response = routes.routeHostedGeminiPromiseReadinessRequest(
    new Request(url, init),
    input,
  )

  if (response === undefined) {
    throw new Error('hosted Gemini readiness route did not match')
  }

  return Effect.runPromise(response)
}

describe('Hosted Gemini product-promise readiness route', () => {
  test('serves public-safe readiness for a production receipt and owner transition', async () => {
    const response = await route(
      stores([record()], [transition()]),
      `https://openagents.com/api/public/product-promises/api.hosted_gemini.v1/readiness?receiptRef=${encodeURIComponent(
        receiptRef,
      )}`,
    )
    const body = (await response.json()) as Record<string, any>

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.generatedAt).toBe('2026-06-29T12:02:00.000Z')
    expect(body.maxStalenessSeconds).toBe(0)
    expect(body.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })
    expect(body.readiness).toMatchObject({
      greenGateMet: true,
      promiseId: 'api.hosted_gemini.v1',
      receiptRef,
      requestPath: 'POST /api/v1/chat/completions',
      responseShapeRef: 'response.openai_compatible.chat_completion.success',
      schemaVersion: 'openagents.product_promise.hosted_gemini_readiness.v1',
    })
    expect(body.readiness.blockerRefs).toEqual([])
    expect(JSON.stringify(body)).not.toMatch(
      /bearer\s+[A-Za-z0-9._-]+|\bsk-[A-Za-z0-9_-]{16,}|wallet_key/i,
    )
  })

  test('keeps blockers when the receipt is missing or the method mutates', async () => {
    const missingReceipt = await route(
      stores([], []),
      `https://openagents.com/api/public/product-promises/api.hosted_gemini.v1/readiness?receiptRef=${encodeURIComponent(
        receiptRef,
      )}`,
    )
    const missingParam = await route(
      stores([], []),
      'https://openagents.com/api/public/product-promises/api.hosted_gemini.v1/readiness',
    )
    const mutation = await route(
      stores([], []),
      `https://openagents.com/api/public/product-promises/api.hosted_gemini.v1/readiness?receiptRef=${encodeURIComponent(
        receiptRef,
      )}`,
      { method: 'POST' },
    )
    const body = (await missingReceipt.json()) as Record<string, any>

    expect(missingReceipt.status).toBe(200)
    expect(body.readiness.greenGateMet).toBe(false)
    expect(body.readiness.blockerRefs).toEqual([
      'blocker.product_promises.hosted_gemini_production_receipt_pending',
      'blocker.product_promises.hosted_gemini_owner_upgrade_signoff_pending',
    ])
    expect(missingParam.status).toBe(400)
    expect(mutation.status).toBe(405)
  })
})
