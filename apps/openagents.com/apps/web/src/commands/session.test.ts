import { Effect } from 'effect'
import { KeyValueStore } from 'effect/unstable/persistence'
import { describe, expect, test } from 'vitest'

import { SESSION_STORAGE_KEY } from '../constant'
import { clearSessionFromStore } from './session'

describe('session commands', () => {
  test('clears the auth session from the configured store', async () => {
    const encoded = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* KeyValueStore.KeyValueStore
        yield* store.set(SESSION_STORAGE_KEY, 'stored-session')
        yield* clearSessionFromStore
        return yield* store.get(SESSION_STORAGE_KEY)
      }).pipe(Effect.provide(KeyValueStore.layerMemory)),
    )

    expect(encoded).toBeUndefined()
  })
})
