import { Effect } from 'effect'
import { KeyValueStore } from 'effect/unstable/persistence'
import { Command } from 'foldkit'

import { FailedClearSession, SucceededClearSession } from '../message'
import { clearSessionFromStore, sessionStoreLayer } from './session-store'

// Re-export the pure store primitives so existing `commands/session` importers
// keep working. The definitions now live in the message-free `session-store`
// module to avoid a module cycle (see session-store.ts).
export { clearSessionFromStore, sessionStoreLayer }

const withSessionStore = <A, E>(
  effect: Effect.Effect<A, E, KeyValueStore.KeyValueStore>,
): Effect.Effect<A, E> => effect.pipe(Effect.provide(sessionStoreLayer))

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
