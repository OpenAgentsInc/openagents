// Mount-level wiring test for the public privacy-receipt read (#9307).
//
// The defect this closes: `GET /api/public/inference/privacy-receipts/
// {receiptRef}` was registered in the exact-route table with the LITERAL text
// `:receiptRef`. `routeExact` compares a path with `===`, so the only request
// that ever reached `handlePublicPrivacyReceiptRead` was a request for that
// literal text, and every real receipt ref fell through the whole dispatcher
// to 404 — including the `receiptUrl` the purchase and confidential-compute
// writes hand back to callers.
//
// The existing `inference-privacy-receipt-routes.test.ts` could not see it: it
// calls `handlePublicPrivacyReceiptRead` DIRECTLY, so the handler passed while
// no request could reach it. That is the same shape as #9306. This test
// therefore drives the REAL production route composition exported from
// `../index` (`routeWorkerRequest`, the value `workerFetchProgram` dispatches
// through) against a fake env, so a request URL is the only input and "did the
// mount match" is the thing under test.
//
// Mutation check, run 2026-08-03: restoring the literal `:receiptRef`
// exact-route entry and unwiring the parameterised mount fails 4 of the 5
// tests below. The request falls through the ENTIRE dispatcher without
// reaching the handler — in production that is the 404 recorded on #9307; in
// this fake env it surfaces as the document fallback's missing-config error,
// which is the same fact seen from a different side. The one test that passes
// either way is the `:receiptRef` literal case, and it is labelled as
// documentation rather than as a tripwire.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type Env, routeWorkerRequest } from '../index'
import { WorkerRequestLayer } from '../runtime'

const RECEIPT_REF = 'receipt.inference.privacy_entitlement.purchase_9307'
const CONFIDENTIAL_RECEIPT_REF =
  'receipt.inference.confidential_compute.execution_9307'

const entitlementRow = {
  account_ref: 'agent:user_9307',
  capture_excluded: 1,
  created_at: '2026-08-03T00:00:00.000Z',
  entitlement_ref: 'entitlement.inference.paid_privacy.abc0123456789def',
  privacy_tier: 'paid_privacy',
  purchase_ref: 'purchase_9307',
  reason_ref: 'reason.inference.privacy.account_entitlement',
  receipt_ref: RECEIPT_REF,
  updated_at: '2026-08-03T00:00:00.000Z',
}

const confidentialRow = {
  account_ref: 'agent:user_9307',
  capture_excluded: 1,
  created_at: '2026-08-03T00:00:00.000Z',
  execution_ref: 'execution_9307',
  reason_ref: 'reason.inference.privacy.confidential_compute',
  receipt_ref: CONFIDENTIAL_RECEIPT_REF,
  request_ref: 'request_9307',
  updated_at: '2026-08-03T00:00:00.000Z',
}

/**
 * A D1 fake shaped for the REAL `readPublicPrivacyReceipt` reads: it answers
 * the two receipt-by-ref SELECTs for the known refs and null for everything
 * else. Every other statement answers empty rather than throwing, because the
 * production dispatcher runs a chain of path matchers ahead of this route.
 */
const fakeReceiptD1 = () => {
  const statement = (sql: string, args: ReadonlyArray<unknown>) => ({
    all: async () => ({ results: [], success: true }),
    bind: (...bound: ReadonlyArray<unknown>) => statement(sql, bound),
    first: async () => {
      if (
        sql.includes('FROM inference_privacy_entitlement_receipts') &&
        args[0] === RECEIPT_REF
      ) {
        return entitlementRow
      }
      if (
        sql.includes(
          'FROM inference_confidential_compute_execution_receipts',
        ) &&
        args[0] === CONFIDENTIAL_RECEIPT_REF
      ) {
        return confidentialRow
      }
      return null
    },
    raw: async () => [],
    run: async () => ({ meta: {}, results: [], success: true }),
  })
  return {
    batch: async () => [],
    prepare: (sql: string) => statement(sql, []),
  }
}

const fakeEnv = (): Env =>
  ({
    ASSETS: {
      fetch: async () => new Response('asset', { status: 200 }),
    },
    OPENAGENTS_DB: fakeReceiptD1(),
  }) as unknown as Env

const fakeCtx = (): ExecutionContext =>
  ({
    passThroughOnException: () => {},
    props: {},
    waitUntil: () => {},
  }) as unknown as ExecutionContext

/** Drive the production composition for one request URL. */
const dispatch = async (pathname: string, method = 'GET') => {
  const env = fakeEnv()
  const request = new Request(`https://openagents.com${pathname}`, { method })
  return Effect.runPromise(
    routeWorkerRequest().pipe(
      Effect.provide(WorkerRequestLayer({ ctx: fakeCtx(), env, request })),
    ),
  )
}

describe('public privacy-receipt route mount (#9307)', () => {
  test('a REAL receipt ref reaches the handler through the production dispatcher', async () => {
    const response = await dispatch(
      `/api/public/inference/privacy-receipts/${encodeURIComponent(RECEIPT_REF)}`,
    )

    // Before #9307 this was a 404: the request never reached the handler.
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      receipt: {
        receipt: { receiptRef: string; schemaVersion: string }
        sourceRefs: ReadonlyArray<string>
      }
    }
    expect(body.receipt.receipt.receiptRef).toBe(RECEIPT_REF)
    expect(body.receipt.receipt.schemaVersion).toBe(
      'openagents.inference.privacy_entitlement_receipt.v1',
    )
    expect(body.receipt.sourceRefs).toContain(
      `route:/api/public/inference/privacy-receipts/${RECEIPT_REF}`,
    )
  })

  test('the confidential-compute receipt family dereferences at the same path', async () => {
    const response = await dispatch(
      `/api/public/inference/privacy-receipts/${encodeURIComponent(
        CONFIDENTIAL_RECEIPT_REF,
      )}`,
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      receipt: { receipt: { receiptRef: string; schemaVersion: string } }
    }
    expect(body.receipt.receipt.receiptRef).toBe(CONFIDENTIAL_RECEIPT_REF)
    expect(body.receipt.receipt.schemaVersion).toBe(
      'openagents.inference.confidential_compute_execution_receipt.v1',
    )
  })

  test('an unknown ref is an honest 404, not a fabricated receipt', async () => {
    const response = await dispatch(
      '/api/public/inference/privacy-receipts/receipt.inference.privacy_entitlement.not_a_real_ref',
    )

    expect(response.status).toBe(404)
    expect(await response.json()).toEqual({ error: 'not_found' })
  })

  test('the literal `:receiptRef` path is no longer a special case', async () => {
    // Documentation, not a tripwire: this path 404s before and after the fix
    // (the store does not know a ref called `:receiptRef` either way). It
    // records that the literal text stopped being the ONE path that reached
    // the handler.
    const response = await dispatch(
      '/api/public/inference/privacy-receipts/:receiptRef',
    )

    expect(response.status).toBe(404)
  })

  test('a non-GET method on a real ref is refused by the handler, not the router', async () => {
    // Proves the mount claims the path for every method and hands it to the
    // handler, rather than falling through to an unrelated route.
    const response = await dispatch(
      `/api/public/inference/privacy-receipts/${encodeURIComponent(RECEIPT_REF)}`,
      'POST',
    )

    expect(response.status).toBe(405)
    expect(await response.json()).toEqual({ error: 'method_not_allowed' })
  })
})
