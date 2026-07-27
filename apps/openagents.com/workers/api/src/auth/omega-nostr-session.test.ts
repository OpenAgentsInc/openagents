import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import { describe, expect, test } from 'vitest'

import { makeMemoryAuthKvStore } from './auth-kv'
import {
  OMEGA_NOSTR_SESSION_PATH,
  makeOmegaNostrSessionHandler,
  readOmegaNostrSession,
} from './omega-nostr-session'

const now = new Date('2026-07-27T17:00:00.000Z')
const url = `https://openagents.com${OMEGA_NOSTR_SESSION_PATH}`

const authorization = (
  secret: Uint8Array,
  requestUrl = url,
  createdAt = now,
): string => {
  const event = finalizeEvent(
    {
      content: '',
      created_at: Math.floor(createdAt.getTime() / 1_000),
      kind: 27_235,
      tags: [
        ['u', requestUrl],
        ['method', 'POST'],
        ['payload', hashPayloadBytes(new Uint8Array())],
      ],
    },
    secret,
  )
  return `Nostr ${btoa(JSON.stringify(event))}`
}

describe('Omega background Nostr session', () => {
  test('mints a short-lived bearer session from the configured owner key', async () => {
    const secret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const owner = { userId: 'github:owner', email: 'owner@example.com' }
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => getPublicKey(secret),
      now: () => now,
      resolveOwner: async () => owner,
    })
    const response = await handler(
      new Request(url, {
        headers: { authorization: authorization(secret) },
        method: 'POST',
      }),
      {},
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      accessToken: string
      expiresIn: number
    }
    expect(body.accessToken).toMatch(/^oa_omega_/)
    expect(body.expiresIn).toBe(900)
    await expect(
      readOmegaNostrSession(store, body.accessToken),
    ).resolves.toEqual(owner)
  })

  test('rejects another key, a wrong URL, a stale proof, and replay', async () => {
    const ownerSecret = generateSecretKey()
    const otherSecret = generateSecretKey()
    const store = makeMemoryAuthKvStore()
    const handler = makeOmegaNostrSessionHandler({
      authStore: () => store,
      expectedOwnerPubkey: () => getPublicKey(ownerSecret),
      now: () => now,
      resolveOwner: async () => ({ userId: 'github:owner' }),
    })
    const run = (auth: string) =>
      handler(
        new Request(url, {
          headers: { authorization: auth },
          method: 'POST',
        }),
        {},
      )

    expect((await run(authorization(otherSecret))).status).toBe(401)
    expect(
      (await run(authorization(ownerSecret, 'https://evil.example/session')))
        .status,
    ).toBe(401)
    expect(
      (
        await run(
          authorization(ownerSecret, url, new Date(now.getTime() - 61_000)),
        )
      ).status,
    ).toBe(401)
    const proof = authorization(ownerSecret)
    expect((await run(proof)).status).toBe(200)
    expect((await run(proof)).status).toBe(409)
  })
})
