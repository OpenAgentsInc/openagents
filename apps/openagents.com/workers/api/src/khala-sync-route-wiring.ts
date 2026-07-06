// Khala Sync per-route dependency wiring (ST-3, #8509).
//
// Every `/api/sync/*` route in `index.ts`'s exact-route table used to build
// its `handleKhalaSync*` dependency object as an INLINE closure literal.
// That inline wiring was structurally untestable: the route tests inject a
// fake `authenticate`, so nothing ever proved WHICH request/env the real
// `authenticateRequestActor` was wired to see. That is exactly where the
// 2026-07-06 production bug lived — the connect route's closure
// authenticated the RAW inbound request instead of the token-normalized one
// the handler passes (`withBearerFromQueryToken`), silently 401ing every
// header-less WebSocket bearer client (mobile "Loading threads" forever;
// docs/fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md §R3,
// docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md).
//
// This module extracts that wiring into importable factories so a test can
// assert against the REAL closures with a fake env
// (`khala-sync-route-wiring.test.ts` drives the real
// `authenticateRequestActor` through `khalaSyncRouteWiring` exported from
// `./index`). BEHAVIOR IS BYTE-FOR-BYTE IDENTICAL to the previous inline
// literals — this is a refactor for testability, not a behavior change.
//
// The two authenticate shapes are the load-bearing contract here:
//
// - CONNECT (`makeConnectDeps`): `authenticate(authRequest)` takes the
//   request AS AN ARGUMENT and authenticates THAT request. The handler
//   promotes a `?token=` query bearer into an `Authorization` header first
//   (the only auth channel a WebSocket client has) and passes the
//   normalized request in. There is deliberately no raw request in scope
//   inside this factory, so the original bug (closing over the raw
//   request) is now structurally impossible to reintroduce HERE — and the
//   wiring test pins the route-table composition too.
// - LOG / BOOTSTRAP / PUSH / CVR-PULL (`make*Deps(request, ...)`): plain
//   HTTP routes whose callers CAN set headers, so their zero-arg
//   `authenticate` closes over the route's own inbound request and reads
//   its `Authorization` header/cookies directly. Connect is the one
//   special WS case.
//
// `authenticateRequestActor` / `resolveKhalaSyncActorUserId` stay defined in
// `index.ts` (they compose many index-local seams); they are INJECTED here
// once at module wiring time so this file has no import cycle with index.

import type { KhalaSyncBootstrapDependencies } from './khala-sync-bootstrap-routes'
import type { KhalaSyncConnectDependencies } from './khala-sync-connect-routes'
import {
  isKhalaSyncCvrEnabled,
  type KhalaSyncCvrPullDependencies,
} from './khala-sync-cvr-routes'
import type { KhalaSyncHubNamespaceLike } from './khala-sync-hub-do'
import type { KhalaSyncLogDependencies } from './khala-sync-log-routes'
import type { KhalaSyncPushDependencies } from './khala-sync-push-routes'
import { makeKhalaSyncScopeReadResolver } from './khala-sync-scope-auth'
import { openAgentsDatabase } from './runtime'

/**
 * The minimal structural slice of `OpenAgentsWorkerEnv` the sync route
 * family's wiring reads. Kept structural (not the full Worker env) so the
 * wiring test can hand in a small fake env and still exercise the REAL
 * factories.
 */
export type KhalaSyncRouteWiringEnv = Readonly<{
  OPENAGENTS_DB: D1Database
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
  KHALA_SYNC_HUB?: unknown
  KHALA_SYNC_CVR?: string | undefined
}>

export type KhalaSyncRouteWiring<WorkerEnv> = Readonly<{
  /**
   * `/api/sync/connect` (WS): `authenticate` authenticates its ARGUMENT —
   * the handler passes the token-normalized request
   * (`withBearerFromQueryToken`), never the raw inbound one.
   */
  makeConnectDeps: (
    env: WorkerEnv,
    ctx: ExecutionContext,
  ) => KhalaSyncConnectDependencies
  /** `/api/sync/log`: header-read auth closing over the route's request. */
  makeLogDeps: (
    request: Request,
    env: WorkerEnv,
    ctx: ExecutionContext,
  ) => KhalaSyncLogDependencies
  /** `/api/sync/bootstrap`: header-read auth closing over the route's request. */
  makeBootstrapDeps: (
    request: Request,
    env: WorkerEnv,
    ctx: ExecutionContext,
  ) => KhalaSyncBootstrapDependencies
  /** `/api/sync/push`: header-read auth closing over the route's request. */
  makePushDeps: (
    request: Request,
    env: WorkerEnv,
    ctx: ExecutionContext,
  ) => KhalaSyncPushDependencies
  /** `/api/sync/cvr-pull` (KS-7.2, flag-gated): header-read auth. */
  makeCvrPullDeps: (
    request: Request,
    env: WorkerEnv,
    ctx: ExecutionContext,
  ) => KhalaSyncCvrPullDependencies
}>

export type KhalaSyncRouteWiringPrimitives<WorkerEnv, Actor> = Readonly<{
  /** The Worker's standard actor auth (`authenticateRequestActor` in index.ts). */
  authenticateActor: (
    request: Request,
    env: WorkerEnv,
    ctx: ExecutionContext,
  ) => Promise<Actor | undefined>
  /** `resolveKhalaSyncActorUserId` (index.ts): actor → scope-owner userId. */
  resolveActorUserId: (actor: Actor) => string
  /** The module-level production mutator registry (index.ts singleton). */
  mutatorRegistry: KhalaSyncPushDependencies['registry']
}>

export const makeKhalaSyncRouteWiring = <
  WorkerEnv extends KhalaSyncRouteWiringEnv,
  Actor,
>(
  primitives: KhalaSyncRouteWiringPrimitives<WorkerEnv, Actor>,
): KhalaSyncRouteWiring<WorkerEnv> => {
  // One shared closure shape for every sync route: authenticate exactly the
  // request given HERE (each factory decides which request that is — the
  // whole point of this module), then resolve the Khala Sync scope-owner
  // userId. Identical to the previous inline route-table literals.
  const authenticateRequest =
    (request: Request, env: WorkerEnv, ctx: ExecutionContext) =>
    async (): Promise<{ readonly userId: string } | undefined> => {
      const actor = await primitives.authenticateActor(request, env, ctx)
      if (actor === undefined) {
        return undefined
      }
      return { userId: primitives.resolveActorUserId(actor) }
    }

  const resolveScopeRead = (env: WorkerEnv) =>
    makeKhalaSyncScopeReadResolver({
      binding: env.KHALA_SYNC_DB,
      db: openAgentsDatabase(env),
    })

  const hubNamespace = (env: WorkerEnv) =>
    env.KHALA_SYNC_HUB as KhalaSyncHubNamespaceLike | undefined

  return {
    makeConnectDeps: (env, ctx) => ({
      // MUST authenticate the request the ROUTE passes (it has the
      // `?token=` query bearer promoted into an Authorization header —
      // the only auth channel a WebSocket client has), never a closure
      // over the raw inbound request: that closure 401'd every mobile
      // live tail and left the app on an infinite "Loading threads"
      // spinner (2026-07-06 production bug). Pinned by
      // khala-sync-route-wiring.test.ts against the REAL actor auth.
      authenticate: authRequest => authenticateRequest(authRequest, env, ctx)(),
      hubNamespace: hubNamespace(env),
      resolveScopeRead: resolveScopeRead(env),
    }),
    makeLogDeps: (request, env, ctx) => ({
      authenticate: authenticateRequest(request, env, ctx),
      binding: env.KHALA_SYNC_DB,
      hubNamespace: hubNamespace(env),
      resolveScopeRead: resolveScopeRead(env),
    }),
    makeBootstrapDeps: (request, env, ctx) => ({
      authenticate: authenticateRequest(request, env, ctx),
      binding: env.KHALA_SYNC_DB,
      resolveScopeRead: resolveScopeRead(env),
    }),
    makePushDeps: (request, env, ctx) => ({
      authenticate: authenticateRequest(request, env, ctx),
      binding: env.KHALA_SYNC_DB,
      registry: primitives.mutatorRegistry,
    }),
    makeCvrPullDeps: (request, env, ctx) => ({
      enabled: isKhalaSyncCvrEnabled(env.KHALA_SYNC_CVR),
      authenticate: authenticateRequest(request, env, ctx),
      binding: env.KHALA_SYNC_DB,
      resolveScopeRead: resolveScopeRead(env),
    }),
  }
}
