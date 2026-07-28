import { describe, expect, test } from 'vitest'

import type { IdentityDb } from '../identity-db'
import { makeMobileDeviceLinkStore } from './mobile-device-link-store'

type StoredIdentity = {
  deletedAt: string | null
  deviceRef: string
  ownerRef: string
}

const makeIdentityDb = (
  initial?: Readonly<{
    deletedAt?: string | null
    deviceRef?: string
    ownerRef: string
    pubkey: string
  }>,
): Readonly<{
  batches: Array<unknown>
  db: IdentityDb
  identities: Map<string, StoredIdentity>
}> => {
  const identities = new Map<string, StoredIdentity>()
  if (initial !== undefined) {
    identities.set(initial.pubkey, {
      deletedAt: initial.deletedAt ?? null,
      deviceRef: initial.deviceRef ?? '',
      ownerRef: initial.ownerRef,
    })
  }
  const batches: Array<unknown> = []
  const db: IdentityDb = {
    query: async (_sql, params = []) => {
      const pubkey = String(params[0] ?? '')
      const identity = identities.get(pubkey)
      return identity === undefined
        ? []
        : [
            {
              user_id: identity.ownerRef,
              deleted_at: identity.deletedAt,
            },
          ]
    },
    batch: async statements => {
      batches.push(statements)
      const params = statements[0]?.params ?? []
      const ownerRef = String(params[1] ?? '')
      const pubkey = String(params[2] ?? '')
      const deviceRef = String(params[3] ?? '')
      const current = identities.get(pubkey)
      if (current === undefined) {
        identities.set(pubkey, {
          deletedAt: null,
          deviceRef,
          ownerRef,
        })
      } else if (current.ownerRef === ownerRef) {
        current.deletedAt = null
        current.deviceRef = deviceRef
      }
    },
  }
  return { batches, db, identities }
}

const input = {
  deviceRef: 'omega-mobile-aaaaaaaaaaaaaaaaaaaaaaaa',
  nowIso: '2026-07-28T21:00:00.000Z',
  ownerRef: 'github:canonical-owner',
  pubkey: 'a'.repeat(64),
} as const

describe('mobile protected Nostr device identity store', () => {
  test('creates an unowned identity for the canonical account', async () => {
    const fixture = makeIdentityDb()
    const state = await makeMobileDeviceLinkStore(fixture.db).link(input)

    expect(state).toBe('linked')
    expect(fixture.identities.get(input.pubkey)).toEqual({
      deletedAt: null,
      deviceRef: input.deviceRef,
      ownerRef: input.ownerRef,
    })
    expect(fixture.batches).toHaveLength(1)
  })

  test('keeps and reactivates an identity owned by the same account', async () => {
    const fixture = makeIdentityDb({
      deletedAt: '2026-07-27T00:00:00.000Z',
      ownerRef: input.ownerRef,
      pubkey: input.pubkey,
    })
    const state = await makeMobileDeviceLinkStore(fixture.db).link(input)

    expect(state).toBe('already_linked')
    expect(fixture.identities.get(input.pubkey)?.deletedAt).toBeNull()
    expect(fixture.batches).toHaveLength(1)
  })

  test('rejects another canonical owner without a write', async () => {
    const fixture = makeIdentityDb({
      ownerRef: 'github:other-owner',
      pubkey: input.pubkey,
    })
    const state = await makeMobileDeviceLinkStore(fixture.db).link(input)

    expect(state).toBe('conflict')
    expect(fixture.identities.get(input.pubkey)?.ownerRef).toBe(
      'github:other-owner',
    )
    expect(fixture.batches).toHaveLength(0)
  })
})
