import { SyncScope } from '@openagentsinc/sync-schema'
import { SyncOutboxStore, type WorkerBindings } from '@openagentsinc/sync-worker'
import { Effect, Layer, Schema as S } from 'effect'
import { Worker, WorkerEnvironment } from 'effect-cf'
import * as Context from 'effect/Context'

import { logWorkerRouteError, unwrapEffectTryPromiseCause } from './observability'

type DatabaseEnv = Pick<WorkerBindings, 'OPENAGENTS_DB'>
type SyncRoomEnv = Pick<WorkerBindings, 'SYNC_ROOM'>

export type OpenAgentsWorkerExecutionContext = Pick<
  ExecutionContext,
  'waitUntil'
>

export type OpenAgentsWorkerRequestShape = Readonly<{
  ctx: ExecutionContext
  env: WorkerBindings
  request: Request
  url: URL
}>

export class OpenAgentsWorkerRequest extends Context.Service<
  OpenAgentsWorkerRequest,
  OpenAgentsWorkerRequestShape
>()('@openagentsinc/OpenAgentsWorkerRequest') {}

// CFG-7 (#8522): the RUNNER_EVENTS Cloudflare Queue lane was deleted. It had
// no producer call sites and no wrangler consumer — dead wiring from the
// pre-SHC runner-events design. New queue lanes ride the oa-infra Postgres
// JobQueue (src/oa-job-queue-producer.ts + apps/oa-queue-worker).

export class SyncRoomNotificationError extends S.TaggedErrorClass<SyncRoomNotificationError>()(
  'SyncRoomNotificationError',
  {
    scope: S.String,
    error: S.Defect,
  },
) {}

export type OpenAgentsSyncRoomNotificationsShape = Readonly<{
  notifyScopes: (
    scopes: ReadonlyArray<SyncScope>,
  ) => Effect.Effect<void, SyncRoomNotificationError>
  notifyScopesPromise: (scopes: ReadonlyArray<SyncScope>) => Promise<void>
  roomForScope: (scope: SyncScope) => Pick<DurableObjectStub, 'fetch'>
}>

export class OpenAgentsSyncRoomNotifications extends Context.Service<
  OpenAgentsSyncRoomNotifications,
  OpenAgentsSyncRoomNotificationsShape
>()('@openagentsinc/OpenAgentsSyncRoomNotifications') {
  static layer = (namespace: DurableObjectNamespace) =>
    Layer.succeed(
      OpenAgentsSyncRoomNotifications,
      makeOpenAgentsSyncRoomNotifications(namespace),
    )
}

export type OpenAgentsWorkerContextShape = Readonly<{
  waitUntil: (promise: Promise<unknown>) => Effect.Effect<void>
}>

export class OpenAgentsWorkerContext extends Context.Service<
  OpenAgentsWorkerContext,
  OpenAgentsWorkerContextShape
>()('@openagentsinc/OpenAgentsWorkerContext') {
  static layer = (ctx: OpenAgentsWorkerExecutionContext) =>
    Layer.succeed(OpenAgentsWorkerContext, makeOpenAgentsWorkerContext(ctx))
}

export const openAgentsDatabase = (env: DatabaseEnv): D1Database =>
  env.OPENAGENTS_DB

export const syncOutboxStoreLayer = (env: DatabaseEnv) =>
  SyncOutboxStore.layer(openAgentsDatabase(env))

export const syncRoomNotifications = (env: SyncRoomEnv) =>
  makeOpenAgentsSyncRoomNotifications(env.SYNC_ROOM)

export const WorkerRequestLayer = (input: {
  ctx: ExecutionContext
  env: WorkerBindings
  request: Request
}) =>
  Layer.mergeAll(
    Layer.succeed(OpenAgentsWorkerRequest, {
      ...input,
      url: new URL(input.request.url),
    }),
    Layer.succeed(Worker.NativeRequest, input.request),
    Layer.succeed(Worker.ExecutionContext, input.ctx),
    Layer.succeed(WorkerEnvironment, input.env),
    OpenAgentsWorkerContext.layer(input.ctx),
    OpenAgentsSyncRoomNotifications.layer(input.env.SYNC_ROOM),
    syncOutboxStoreLayer(input.env),
  )

export const syncScope = (scope: string): SyncScope => SyncScope.make(scope)

export const syncScopes = (
  scopes: ReadonlyArray<string>,
): ReadonlyArray<SyncScope> => [...new Set(scopes)].map(syncScope)

const syncNotificationRequest = (scope: SyncScope): Request =>
  new Request('https://sync.openagents.internal/__sync/notify', {
    headers: { 'x-openagents-sync-scope': scope },
    method: 'POST',
  })

const notifySyncScopePromise = async (
  namespace: DurableObjectNamespace,
  scope: SyncScope,
): Promise<void> => {
  await syncRoomForScope(namespace, scope).fetch(syncNotificationRequest(scope))
}

const notifySyncScopeEffect = (
  namespace: DurableObjectNamespace,
  scope: SyncScope,
): Effect.Effect<void> =>
  Effect.tryPromise(() => notifySyncScopePromise(namespace, scope)).pipe(
    Effect.catch(error => {
      logWorkerRouteError(
        'sync_scope_notify_failed',
        unwrapEffectTryPromiseCause(error),
        { scope },
      )

      return Effect.void
    }),
  )

// Each scope's Durable Object notify fetch is isolated: one scope's
// flaky/failed fetch is logged and skipped without preventing the other
// (unrelated) scopes in this fan-out from being notified. Structured
// concurrency (Effect.forEach) replaces a bare `Promise.all`, whose
// rejection on any single scope would have discarded visibility into
// every sibling scope's notify outcome.
const notifySyncScopesPromise = async (
  namespace: DurableObjectNamespace,
  scopes: ReadonlyArray<SyncScope>,
): Promise<void> => {
  await Effect.runPromise(
    Effect.forEach(
      [...new Set(scopes)],
      scope => notifySyncScopeEffect(namespace, scope),
      { concurrency: 'unbounded' },
    ),
  )
}

export const makeOpenAgentsSyncRoomNotifications = (
  namespace: DurableObjectNamespace,
): OpenAgentsSyncRoomNotificationsShape => ({
  notifyScopes: scopes =>
    Effect.tryPromise({
      catch: error =>
        new SyncRoomNotificationError({
          error,
          scope: scopes.join(','),
        }),
      try: () => notifySyncScopesPromise(namespace, scopes),
    }),
  notifyScopesPromise: scopes => notifySyncScopesPromise(namespace, scopes),
  roomForScope: scope => syncRoomForScope(namespace, scope),
})

const syncRoomForScope = (
  namespace: DurableObjectNamespace,
  scope: SyncScope,
): DurableObjectStub => namespace.getByName(scope)

export const makeOpenAgentsWorkerContext = (
  ctx: OpenAgentsWorkerExecutionContext,
): OpenAgentsWorkerContextShape => ({
  waitUntil: promise =>
    Effect.sync(() => {
      ctx.waitUntil(promise)
    }),
})

export const scheduleBackgroundWork = (
  ctx: OpenAgentsWorkerExecutionContext,
  promise: Promise<unknown>,
): void => {
  ctx.waitUntil(promise)
}
