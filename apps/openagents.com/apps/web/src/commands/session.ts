import { BrowserKeyValueStore } from '@effect/platform-browser'
import { Effect } from 'effect'
import { KeyValueStore } from 'effect/unstable/persistence'
import { Command } from 'foldkit'

import { SESSION_STORAGE_KEY } from '../constant'
import { FailedClearSession, SucceededClearSession } from '../message'

export const sessionStoreLayer = BrowserKeyValueStore.layerLocalStorage

const withSessionStore = <A, E>(
  effect: Effect.Effect<A, E, KeyValueStore.KeyValueStore>,
): Effect.Effect<A, E> => effect.pipe(Effect.provide(sessionStoreLayer))

export const clearSessionFromStore: Effect.Effect<
  void,
  KeyValueStore.KeyValueStoreError,
  KeyValueStore.KeyValueStore
> = Effect.gen(function* () {
  const store = yield* KeyValueStore.KeyValueStore
  yield* store.remove(SESSION_STORAGE_KEY)
})

export const ClearSession = Command.define(
  'ClearSession',
  SucceededClearSession,
  FailedClearSession,
)(
  withSessionStore(clearSessionFromStore).pipe(
    Effect.as(SucceededClearSession()),
    Effect.catch(error =>
      Effect.succeed(FailedClearSession({ error: String(error) })),
    ),
  ),
)
