import { BrowserKeyValueStore } from '@effect/platform-browser'
import { Effect } from 'effect'
import { KeyValueStore } from 'effect/unstable/persistence'

import { SESSION_STORAGE_KEY } from '../constant'

// Pure session-store primitives with NO dependency on the top-level message
// union. The `ClearSession` Command (commands/session.ts) and the loggedOut
// submodel's homepage-hero logout both build on these. Keeping them message-free
// avoids a module cycle: the loggedOut submodel imports these directly, while
// `commands/session.ts` (which imports top-level Succeeded/FailedClearSession)
// only re-exports them.

export const sessionStoreLayer = BrowserKeyValueStore.layerLocalStorage

export const clearSessionFromStore: Effect.Effect<
  void,
  KeyValueStore.KeyValueStoreError,
  KeyValueStore.KeyValueStore
> = Effect.gen(function* () {
  const store = yield* KeyValueStore.KeyValueStore
  yield* store.remove(SESSION_STORAGE_KEY)
})
