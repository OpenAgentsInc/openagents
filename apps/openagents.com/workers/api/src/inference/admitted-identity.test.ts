import { describe, expect, test } from 'vitest'

import {
  ADMITTED_IDENTITY_QUERY,
  ADMITTED_IDENTITY_STATE,
  type AdmittedIdentityDatabase,
  makeAdmittedIdentityLookup,
} from './admitted-identity'

const OWNER_ID = `nostr:${'e'.repeat(64)}`

const fakeDatabase = (
  rowFor: (bindings: ReadonlyArray<unknown>) => unknown,
  seen?: Array<{ query: string; bindings: ReadonlyArray<unknown> }>,
): AdmittedIdentityDatabase => ({
  prepare: query => ({
    bind: (...bindings) => ({
      first: async () => {
        seen?.push({ bindings, query })
        const row = rowFor(bindings)
        return row as never
      },
    }),
  }),
})

describe('makeAdmittedIdentityLookup', () => {
  test('admits an identity with an active, unrevoked membership row', async () => {
    const lookup = makeAdmittedIdentityLookup(
      fakeDatabase(() => ({ admitted: 1 })),
    )
    await expect(lookup(OWNER_ID)).resolves.toBe(true)
  })

  test('does not admit an identity with no membership row', async () => {
    const lookup = makeAdmittedIdentityLookup(fakeDatabase(() => null))
    await expect(lookup(OWNER_ID)).resolves.toBe(false)
  })

  test('binds the bare user id and the active state, in that order', async () => {
    // The membership table keys on `owner_user_id` (`nostr:<pubkey>`), NOT the
    // `openauth:`-prefixed account ref. Binding the wrong form would match zero
    // rows and silently re-create the conflation this module exists to remove.
    const seen: Array<{ query: string; bindings: ReadonlyArray<unknown> }> = []
    const lookup = makeAdmittedIdentityLookup(
      fakeDatabase(() => ({ admitted: 1 }), seen),
    )

    await lookup(OWNER_ID)
    expect(seen[0]?.bindings).toEqual([OWNER_ID, ADMITTED_IDENTITY_STATE])
    expect(seen[0]?.query).toBe(ADMITTED_IDENTITY_QUERY)
  })

  test('the query excludes revoked rows and non-active states', () => {
    // A revoked admission must stop exempting. Asserted on the query text
    // because the predicate is the whole safety property.
    expect(ADMITTED_IDENTITY_QUERY).toContain('revoked_at IS NULL')
    expect(ADMITTED_IDENTITY_QUERY).toContain('state = ?')
    expect(ADMITTED_IDENTITY_STATE).toBe('active')
  })

  test('FAILS CLOSED: a throwing read resolves to NOT admitted', async () => {
    const lookup = makeAdmittedIdentityLookup(
      fakeDatabase(() => {
        throw new Error('database unavailable')
      }),
    )
    await expect(lookup(OWNER_ID)).resolves.toBe(false)
  })
})
