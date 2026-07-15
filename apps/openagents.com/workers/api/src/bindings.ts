import type { WorkerBindings } from '@openagentsinc/sync-worker'
import { Effect, Layer, Option, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import type { OpenAgentsWorkerConfigEnv } from './config'

export type OpenAgentsWorkerEnv = WorkerBindings & OpenAgentsWorkerConfigEnv

export class OpenAgentsRuntimeEnvironment extends Context.Service<
  OpenAgentsRuntimeEnvironment,
  OpenAgentsWorkerEnv
>()('@openagentsinc/OpenAgentsRuntimeEnvironment') {}

export class OpenAgentsDatabase extends Context.Service<
  OpenAgentsDatabase,
  D1Database
>()('@openagentsinc/OpenAgentsDatabase') {
  static readonly layer = Layer.effect(
    OpenAgentsDatabase,
    Effect.map(OpenAgentsRuntimeEnvironment, env => env.OPENAGENTS_DB),
  )
}

export class ArtifactOperationError extends S.TaggedErrorClass<ArtifactOperationError>()(
  'ArtifactOperationError',
  {
    error: S.Defect(),
    operation: S.String,
  },
) {}

export type ArtifactObject = Readonly<{
  body: ReadableStream
  text: Effect.Effect<string, ArtifactOperationError>
}>

export type ThreadFileArtifactsShape = Readonly<{
  get: (
    key: string,
  ) => Effect.Effect<Option.Option<ArtifactObject>, ArtifactOperationError>
  put: (
    key: string,
    value: R2PutValue,
    options?: R2PutOptions,
  ) => Effect.Effect<void, ArtifactOperationError>
}>

const artifactOperationError = (operation: string, error: unknown) =>
  new ArtifactOperationError({ error, operation })

export const makeThreadFileArtifacts = (
  bucket: R2Bucket,
): ThreadFileArtifactsShape => ({
  get: key =>
    Effect.tryPromise({
      catch: error => artifactOperationError('get', error),
      try: () => bucket.get(key),
    }).pipe(
      Effect.map(object =>
        object === null
          ? Option.none<ArtifactObject>()
          : Option.some<ArtifactObject>({
              body: object.body,
              text: Effect.tryPromise({
                catch: error => artifactOperationError('text', error),
                try: () => object.text(),
              }),
            }),
      ),
    ),
  put: (key, value, options) =>
    Effect.tryPromise({
      catch: error => artifactOperationError('put', error),
      try: () => bucket.put(key, value, options),
    }).pipe(Effect.asVoid),
})

export class ThreadFileArtifacts extends Context.Service<
  ThreadFileArtifacts,
  ThreadFileArtifactsShape
>()('@openagentsinc/ThreadFileArtifacts') {
  static readonly layer = (bucket: R2Bucket) =>
    Layer.succeed(ThreadFileArtifacts, makeThreadFileArtifacts(bucket))
}
