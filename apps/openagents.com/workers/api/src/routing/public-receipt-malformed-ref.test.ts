// PRO-101: a malformed percent-escape in a public receipt ref must read like an
// unknown ref, not like a backend fault.
//
// Reproduced against production 2026-08-05:
//
//   GET /api/public/cloud/receipts/receipt.test.abc      -> 404 (correct)
//   GET /api/public/cloud/receipts/%                     -> 500 (the defect)
//   GET /api/public/inference/receipts/%                 -> 500
//   GET /api/public/inference/privacy-receipts/%         -> 500
//
// Each of the three readers called `decodeURIComponent` unguarded inside its
// path matcher, and `decodeURIComponent('%')` throws `URIError`. The matchers
// run inside the `Effect.gen` body of `makeWorkerRouteRequest`, so the throw
// was a DEFECT rather than a typed failure: it escaped to the
// `Effect.catchCause` in `index.ts`, which logged `worker_unhandled_exception`,
// answered `serverError()`, and recorded a backend incident with
// `kind: 'unhandled_exception'` and `severity: 'critical'`. A caller typing a
// stray `%` into a public URL is not a critical backend fault.
//
// These tests drive the REAL production composition exported from `../index`
// (`routeWorkerRequest`, the value `workerFetchProgram` dispatches through),
// the same approach as `inference-privacy-receipt-route-wiring.test.ts`, so a
// request URL is the only input. That matters twice over: it proves the mount
// CLAIMS the malformed path rather than falling through, and — because a defect
// REJECTS the promise under `Effect.runPromise` — a passing status assertion is
// itself proof that no unhandled exception was raised, and therefore that no
// `severity: 'critical'` incident was filed.
//
// Mutation check, run 2026-08-06: removing the try/catch from
// `safeDecodeUriComponent` fails 23 of the 38 tests across this file and the
// three reader suites, each with a REJECTED promise carrying
// `URIError: URI malformed` thrown from `pathRefFromPrefix` and propagating
// through `worker-routes.ts` route dispatch — the production defect exactly.

import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { type Env, routeWorkerRequest } from '../index'
import { WorkerRequestLayer } from '../runtime'

const emptyStatement = () => ({
  all: async () => ({ results: [], success: true }),
  bind: () => emptyStatement(),
  first: async () => null,
  raw: async () => [],
  run: async () => ({ meta: {}, results: [], success: true }),
})

/** A D1 fake that knows no receipts at all: every read answers empty. */
const emptyD1 = () => ({
  batch: async () => [],
  prepare: () => emptyStatement(),
})

const fakeEnv = (): Env =>
  ({
    ASSETS: {
      fetch: async () => new Response('asset', { status: 200 }),
    },
    OPENAGENTS_DB: emptyD1(),
  }) as unknown as Env

const fakeCtx = (): ExecutionContext =>
  ({
    passThroughOnException: () => {},
    props: {},
    waitUntil: () => {},
  }) as unknown as ExecutionContext

/** Drive the production composition for one raw request path. */
const dispatch = async (pathname: string) =>
  Effect.runPromise(
    routeWorkerRequest().pipe(
      Effect.provide(
        WorkerRequestLayer({
          ctx: fakeCtx(),
          env: fakeEnv(),
          request: new Request(`https://openagents.com${pathname}`),
        }),
      ),
    ),
  )

const READERS = [
  '/api/public/cloud/receipts',
  '/api/public/inference/receipts',
  '/api/public/inference/privacy-receipts',
] as const

/**
 * The one reader whose store is the D1 binding this fixture can fake.
 *
 * The other two resolve their store through `paymentsLedgerDbForEnv`, which
 * requires a live `KHALA_SYNC_DB` Postgres connection string. Without it the
 * ledger refuses and a WELL-FORMED ref 500s here as "ledger unavailable" — an
 * unrelated fixture limit, not PRO-101 — so a dispatcher-level unknown-ref
 * baseline is only meaningful for this one. The other two get that baseline in
 * their own suites against a properly faked store:
 * `public-inference-receipt-routes.test.ts` and
 * `cloud/public-cloud-primitive-receipt-routes.test.ts`.
 *
 * That limit is also what makes the assertion below sharp: in this fixture the
 * ledger-backed readers answer 404 for a malformed ref even though they would
 * answer 500 for a well-formed one, which proves the guard short-circuits
 * BEFORE the store is ever consulted.
 */
const D1_BACKED_READER = '/api/public/inference/privacy-receipts'

/**
 * Tails that are NOT legal percent-encoding. `new URL(...).pathname` preserves
 * them verbatim — the WHATWG URL parser does not validate percent-escapes — so
 * they reach the matcher exactly as written.
 */
const MALFORMED_TAILS = ['%', '%zz', '%E0%A4%A', 'receipt.test.%'] as const

describe('public receipt readers reject a malformed ref as 404 (PRO-101)', () => {
  for (const reader of READERS) {
    describe(reader, () => {
      for (const tail of MALFORMED_TAILS) {
        test(`a malformed ref \`${tail}\` is a 404, not a 500`, async () => {
          const response = await dispatch(`${reader}/${tail}`)

          // The defect answered 500 `internal_server_error` here, and filed a
          // `severity: 'critical'` backend incident alongside it.
          expect(response.status).toBe(404)
          expect(response.status).not.toBe(500)
        })
      }
    })
  }

  test('a malformed ref is answered before the store is consulted', async () => {
    // See D1_BACKED_READER: the two ledger-backed readers cannot reach their
    // store in this fixture, so a well-formed ref 500s. A malformed ref still
    // answers 404 — only possible if the guard short-circuits ahead of the read.
    const ledgerBacked = READERS.filter(reader => reader !== D1_BACKED_READER)

    const outcomes = await Promise.all(
      ledgerBacked.map(async reader => ({
        malformed: (await dispatch(`${reader}/%`)).status,
        reader,
        wellFormed: (await dispatch(`${reader}/receipt.test.abc`)).status,
      })),
    )

    expect(outcomes).toEqual(
      ledgerBacked.map(reader => ({
        malformed: 404,
        reader,
        wellFormed: 500,
      })),
    )
  })

  test('a malformed ref is indistinguishable from an unknown ref to a caller', async () => {
    // The reason 404 is the right code rather than 400: the two cases carry the
    // same meaning to a caller ("no such receipt"), and answering 404 declines
    // to leak that the ref was parsed at all.
    const unknown = await dispatch(`${D1_BACKED_READER}/receipt.test.abc`)
    const malformed = await dispatch(`${D1_BACKED_READER}/%`)

    expect(unknown.status).toBe(404)
    expect(malformed.status).toBe(unknown.status)
    expect(await malformed.json()).toEqual(await unknown.json())
  })
})
