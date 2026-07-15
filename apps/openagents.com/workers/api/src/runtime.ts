import { SyncOutboxStore, type WorkerBindings } from '@openagentsinc/sync-worker'
import { Effect, Layer } from 'effect'
import * as Context from 'effect/Context'

import { OpenAgentsDatabase, OpenAgentsRuntimeEnvironment } from './bindings'

type DatabaseEnv = Pick<WorkerBindings, 'OPENAGENTS_DB'>

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
    Layer.succeed(OpenAgentsRuntimeEnvironment, input.env),
    OpenAgentsWorkerContext.layer(input.ctx),
    syncOutboxStoreLayer(input.env),
    OpenAgentsDatabase.layer.pipe(
      Layer.provide(Layer.succeed(OpenAgentsRuntimeEnvironment, input.env)),
    ),
  )

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
