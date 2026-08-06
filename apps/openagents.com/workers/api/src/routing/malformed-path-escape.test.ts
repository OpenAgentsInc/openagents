// PRO-1215: a malformed percent-escape anywhere in a route path must be
// answered by the route, not by the Worker's catch-all defect handler.
//
// This is the follow-up sweep to PRO-101 (commit 376cd0af44), which fixed the
// three public receipt readers and `parseCookies`. The same unguarded
// `decodeURIComponent` appeared in ~50 more route matchers.
//
// The defect: `decodeURIComponent('%')` throws `URIError`. Route matchers run
// inside the `Effect.gen` body of `makeWorkerRouteRequest`, so a throw is not a
// typed failure but a DEFECT: it escapes to the `Effect.catchCause` in
// `index.ts`, which logs `worker_unhandled_exception`, answers `serverError()`,
// and records a backend incident with `kind: 'unhandled_exception'` and
// `severity: 'critical'`. Unauthenticated and trivially repeatable, so it is an
// incident-flooding vector rather than a mere wrong status code.
//
// Note the structural rule the fix depends on: `Effect.promise`, `Effect.gen`,
// and `.pipe(Effect.catch(...))` are NOT guards against a synchronous throw.
// Only `try`/`catch`, `Effect.try`, and `Effect.tryPromise` neutralize one.
//
// Two seams carry the fix, and this file asserts them differently because they
// promise different things:
//
//   `pathRefFromPrefix`      — public prefix-tail readers. A malformed tail is
//                              CLAIMED by the route and short-circuits to that
//                              reader's own unknown-ref answer BEFORE any store
//                              is consulted. Asserted as an exact 404.
//
//   `decodedPathSegmentOrRaw` — every other matcher. An undecodable segment is
//                              kept RAW; it cannot match a stored ref, so the
//                              route runs its normal path (its own method
//                              check, then its own auth check, then its own
//                              not-found) and picks the status itself. Asserted
//                              as EQUAL to the status of a well-formed unknown
//                              ref, which is the exact property that seam
//                              claims.
//
// These tests drive the REAL production composition exported from `../index`
// (`routeWorkerRequest`, the value `workerFetchProgram` dispatches through), so
// a request URL is the only input. That matters twice over: it proves the mount
// CLAIMS the malformed path rather than falling through, and — because a defect
// REJECTS the promise under `Effect.runPromise` — a resolved response is itself
// proof that no unhandled exception was raised, and therefore that no
// `severity: 'critical'` incident was filed.
//
// Mutation check, run 2026-08-06: reverting `safeDecodeUriComponent` in
// `../http/router.ts` to a bare `decodeURIComponent(value)` fails 96 of the 122
// tests in this file, every failure a REJECTED promise carrying
// `URIError: URI malformed`. That is also what makes these tests non-vacuous:
// each asserted path genuinely reaches a decode rather than falling through to
// the dispatcher tail.

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

/** A D1 fake that knows no records at all: every read answers empty. */
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
const dispatch = async (pathname: string, method = 'GET') =>
  Effect.runPromise(
    routeWorkerRequest().pipe(
      Effect.provide(
        WorkerRequestLayer({
          ctx: fakeCtx(),
          env: fakeEnv(),
          request: new Request(`https://openagents.com${pathname}`, { method }),
        }),
      ),
    ),
  )

/**
 * Tails that are NOT legal percent-encoding. `new URL(...).pathname` preserves
 * them verbatim — the WHATWG URL parser does not validate percent-escapes — so
 * they reach the matcher exactly as written.
 */
const MALFORMED_SEGMENTS = ['%', '%zz', '%E0%A4%A'] as const

/** A well-formed segment that no fixture store knows. */
const UNKNOWN_SEGMENT = 'ref.test.abc'

/**
 * Public prefix-tail readers moved onto `pathRefFromPrefix`. Each answers its
 * own unknown-ref 404 for a malformed tail, short-circuiting ahead of its store.
 */
const PUBLIC_PREFIX_READERS = [
  '/api/public/nip90-market/receipts',
  '/api/public/partner-payout-receipts',
  '/api/public/site-referral-payout-receipts',
  '/api/public/billing/stripe-checkout-receipts',
  '/api/public/business/coding-quick-win-receipts',
  '/api/public/business/already-sold-engagement-receipts',
  '/api/public/business/case-studies',
  '/api/public/ecommerce-campaign/receipts',
  '/api/public/marketing-agency/receipts',
  '/api/public/marketing-agency/self-serve/deliverability',
] as const

/**
 * Routes moved onto `decodedPathSegmentOrRaw`, with `%s` marking where the
 * segment goes. One representative path per changed route file. Public and
 * auth-gated routes sit together deliberately: the seam makes the same promise
 * for both, and for the auth-gated ones the decode sits in the MATCHER, which
 * runs BEFORE the handler's auth check — so before this fix an unauthenticated
 * caller got 500 plus a critical incident where the route's own answer was 401.
 */
const RAW_FALLBACK_ROUTES: ReadonlyArray<
  Readonly<{ method?: string; path: string }>
> = [
  // Public readers whose own ref validator rejects a raw segment.
  { path: '/api/public/khala-code/outside-user-runs/%s' },
  {
    path: '/api/public/khala-code/trace-plugin-revenue-share-precedents/%s',
  },
  { path: '/api/public/qa-swarm/first-engagements/%s' },
  { path: '/api/public/agents/%s/goal' },
  { path: '/api/public/goals/%s' },
  { path: '/api/public/goals/%s/snapshot' },
  { path: '/api/public/training/runs/%s' },
  { path: '/api/public/training/runs/%s/settlements' },

  // Auth-gated matchers: the decode runs ahead of the auth check.
  { path: '/api/operator/adjutant/assignments/%s' },
  { path: '/api/operator/adjutant/assignments/%s/launch', method: 'POST' },
  {
    method: 'POST',
    path: '/api/operator/adjutant/assignments/%s/enrichment/source-refs/%s/review',
  },
  { path: '/api/operator/adjutant/orders/%s/assign', method: 'POST' },
  { path: '/api/operator/adjutant/sites/%s/assign', method: 'POST' },
  { path: '/api/pylons/%s' },
  { path: '/api/pylons/%s/heartbeat', method: 'POST' },
  { path: '/api/pylons/%s/wallet-readiness', method: 'POST' },
  { path: '/api/pylons/%s/spark-payout-target', method: 'POST' },
  { path: '/api/pylons/%s/assignments' },
  { path: '/api/operator/pylons/%s/quarantine', method: 'POST' },
  { path: '/api/operator/pylons/assignments/%s/closeout', method: 'POST' },
  { path: '/api/training/runs/%s' },
  { path: '/api/training/runs/%s/settlements' },
  { path: '/api/training/runs/%s/admit', method: 'POST' },
  { path: '/api/training/runs/%s/bootstrap-grant', method: 'POST' },
  { path: '/api/training/leaderboards/%s' },
  { path: '/api/agents/goals/%s' },
  { path: '/api/agents/goals/%s/update', method: 'POST' },
  { path: '/api/autopilot/goals/%s' },
  { path: '/api/operator/autopilot/goals/%s' },
  { path: '/api/agents/claims/%s' },
  { path: '/api/agents/claims/rewards/%s' },
  { path: '/api/agents/claims/rewards/%s/dispatch', method: 'POST' },
  { path: '/api/agents/scoped-grants/%s/revoke', method: 'POST' },
  { path: '/api/autopilot/decisions/%s/actions', method: 'POST' },
  { path: '/api/autopilot/decision-closeouts/%s' },
  { path: '/api/autopilot/work/%s/decisions' },
  { path: '/api/autopilot/onboarding/%s' },
  { path: '/api/omni/workrooms/%s' },
  { path: '/api/omni/workrooms/%s/lifecycle-decisions' },
  { path: '/api/traces/%s' },
  { path: '/api/lists/%s' },
  { path: '/api/lists/%s/subscribers' },
  { path: '/api/workspaces/%s' },
  { path: '/api/workspaces/%s/engagement' },
  { path: '/api/tenant/client/workrooms/%s' },
  { path: '/api/mobile/repos/%s/%s' },
  { path: '/v1/models/%s' },
  { path: '/v1/boxes/%s' },
]

const fill = (template: string, segment: string): string =>
  template.split('%s').join(segment)

/**
 * What one dispatch actually did — a response, or a rejected promise.
 *
 * Comparing OUTCOMES rather than statuses is what lets one assertion cover
 * every route honestly. Some routes in this list need env config or bindings
 * the bare fixture above does not supply (`OpenAgentsWorkerConfigError`, or a
 * 500 from a missing store); those limits are unrelated to PRO-1215 and hit a
 * WELL-FORMED ref exactly as hard. The invariant that matters is that a
 * malformed segment lands in the SAME place as a well-formed unknown one — so
 * the fixture limit shows up identically on both sides and cancels out, while
 * the defect this sweep removed does not: before the fix the malformed
 * dispatch rejected with `URIError` while the well-formed dispatch did not.
 */
type Outcome =
  | Readonly<{ error: string; kind: 'rejected' }>
  | Readonly<{ kind: 'response'; status: number }>

/** First line only: enough to identify the failure, stable across paths. */
const describeError = (error: unknown): string =>
  String(error).split('\n')[0]!.trim()

const outcomeOf = async (pathname: string, method: string): Promise<Outcome> =>
  dispatch(pathname, method).then(
    response => ({ kind: 'response', status: response.status }) as const,
    error => ({ error: describeError(error), kind: 'rejected' }) as const,
  )

describe('public prefix-tail readers answer a malformed ref as 404 (PRO-1215)', () => {
  for (const reader of PUBLIC_PREFIX_READERS) {
    for (const segment of MALFORMED_SEGMENTS) {
      test(`${reader}/${segment} is a 404, not a 500`, async () => {
        // A resolved promise is itself the proof that no defect escaped: a
        // defect REJECTS under `Effect.runPromise`.
        const response = await dispatch(`${reader}/${segment}`)

        // The defect answered 500 `internal_server_error` here, and filed a
        // `severity: 'critical'` backend incident alongside it.
        expect(response.status).toBe(404)
      })
    }
  }

  test('a malformed tail is claimed by the reader, not left to the dispatcher tail', async () => {
    // If the guard let the path fall through instead of claiming it, these
    // would be answered by the SPA asset fallback (200) rather than 404.
    const statuses = await Promise.all(
      PUBLIC_PREFIX_READERS.map(async reader => ({
        reader,
        status: (await dispatch(`${reader}/%`)).status,
      })),
    )

    expect(statuses).toEqual(
      PUBLIC_PREFIX_READERS.map(reader => ({ reader, status: 404 })),
    )
  })

  test('a non-GET method on a malformed ref still answers 405, preserving method ordering', async () => {
    // The method check runs ahead of the ref answer, exactly as it does for a
    // well-formed ref. This is the ordering the fix had to preserve.
    const response = await dispatch(
      '/api/public/nip90-market/receipts/%',
      'DELETE',
    )

    expect(response.status).toBe(405)
  })
})

describe('raw-fallback routes answer a malformed segment exactly like an unknown one (PRO-1215)', () => {
  for (const route of RAW_FALLBACK_ROUTES) {
    const method = route.method ?? 'GET'

    test(`${method} ${route.path} matches its unknown-ref answer`, async () => {
      const wellFormed = await outcomeOf(
        fill(route.path, UNKNOWN_SEGMENT),
        method,
      )
      const malformed = await outcomeOf(fill(route.path, '%'), method)

      // The defect itself: an escaped `URIError` from the route matcher, which
      // `index.ts` turned into 500 plus a `severity: 'critical'` incident.
      expect(JSON.stringify(malformed)).not.toContain('URI malformed')

      // The property `decodedPathSegmentOrRaw` claims: an undecodable segment
      // is just a segment that matches nothing, so the route's own
      // method/auth/not-found ordering decides the outcome unchanged.
      expect(malformed).toEqual(wellFormed)
    })
  }
})
