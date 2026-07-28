import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import { describe, expect, test, vi } from 'vitest'

import { type AuthKvStore, makeMemoryAuthKvStore } from './auth-kv'
import {
  MOBILE_DEVICE_LINK_PATH,
  MOBILE_DEVICE_LINK_PROOF_HEADER,
  MOBILE_DEVICE_LINK_SCHEMA,
  handleMobileDeviceLinkRequest,
} from './mobile-device-link'
import type { MobileDeviceLinkState } from './mobile-device-link-store'

const now = new Date('2026-07-28T21:00:00.000Z')
const url = `https://openagents.com${MOBILE_DEVICE_LINK_PATH}`
const ctx = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as unknown as ExecutionContext

const proof = (
  secret: Uint8Array,
  payload: Uint8Array,
  input: Readonly<{
    createdAt?: Date
    requestUrl?: string
  }> = {},
): string => {
  const event = finalizeEvent(
    {
      content: '',
      created_at: Math.floor((input.createdAt ?? now).getTime() / 1_000),
      kind: 27_235,
      tags: [
        ['u', input.requestUrl ?? url],
        ['method', 'POST'],
        ['payload', hashPayloadBytes(payload)],
      ],
    },
    secret,
  )
  return `Nostr ${btoa(JSON.stringify(event))}`
}

const requestFixture = (
  secret: Uint8Array,
  input: Readonly<{
    accessToken?: string
    body?: string
    proofBody?: string
    proofCreatedAt?: Date
    proofUrl?: string
  }> = {},
): Request => {
  const pubkey = getPublicKey(secret)
  const body =
    input.body ??
    JSON.stringify({
      schema: MOBILE_DEVICE_LINK_SCHEMA,
      pubkey,
      deviceRef: `omega-mobile-${pubkey.slice(0, 24)}`,
    })
  const proofBody = input.proofBody ?? body
  return new Request(url, {
    body,
    headers: {
      authorization: `Bearer ${input.accessToken ?? 'openauth-access'}`,
      'content-type': 'application/json',
      [MOBILE_DEVICE_LINK_PROOF_HEADER]: proof(
        secret,
        new TextEncoder().encode(proofBody),
        {
          ...(input.proofCreatedAt === undefined
            ? {}
            : { createdAt: input.proofCreatedAt }),
          ...(input.proofUrl === undefined
            ? {}
            : { requestUrl: input.proofUrl }),
        },
      ),
    },
    method: 'POST',
  })
}

const makeDependencies = (
  input: Readonly<{
    linkState?: 'linked' | 'already_linked' | 'conflict'
    storageFailure?: 'link' | 'replay' | 'session'
  }> = {},
) => {
  type Session = Readonly<{ user: Readonly<{ userId: string }> }>
  type LinkInput = Readonly<{
    deviceRef: string
    nowIso: string
    ownerRef: string
    pubkey: string
  }>
  const store = makeMemoryAuthKvStore()
  const audit = vi.fn()
  const link = vi.fn(
    async (_linkInput: LinkInput): Promise<MobileDeviceLinkState> => {
      if (input.storageFailure === 'link')
        throw new Error('storage unavailable')
      return input.linkState ?? 'linked'
    },
  )
  const requireUserBearerSession = vi.fn(
    async (
      _request: Request,
      _env: unknown,
      _ctx: ExecutionContext,
    ): Promise<Session | undefined> => {
      if (input.storageFailure === 'session') {
        throw new Error('session store unavailable')
      }
      return {
        user: { userId: 'github:canonical-owner' },
      }
    },
  )
  const authStore: (env: unknown) => Pick<AuthKvStore, 'putIfAbsent'> =
    input.storageFailure === 'replay'
      ? () => ({
          putIfAbsent: vi.fn(async () => {
            throw new Error('replay store unavailable')
          }),
        })
      : () => store

  return {
    audit,
    dependencies: {
      audit,
      authStore,
      link: async (_env: unknown, linkInput: LinkInput) => link(linkInput),
      now: () => now,
      requireUserBearerSession,
      userIdFromSession: (session: Session): string => session.user.userId,
    },
    link,
    requireUserBearerSession,
  }
}

describe('mobile protected Nostr device link', () => {
  test('links a fresh protected device proof to the canonical mobile session', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const fixture = makeDependencies()
    const response = await handleMobileDeviceLinkRequest(
      fixture.dependencies,
      requestFixture(secret),
      {},
      ctx,
    )

    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      schema: MOBILE_DEVICE_LINK_SCHEMA,
      state: 'linked',
      ownerRef: 'github:canonical-owner',
    })
    expect(fixture.link).toHaveBeenCalledWith({
      deviceRef: `omega-mobile-${pubkey.slice(0, 24)}`,
      nowIso: now.toISOString(),
      ownerRef: 'github:canonical-owner',
      pubkey,
    })
    expect(fixture.audit).toHaveBeenCalledWith(
      'link_created',
      expect.objectContaining({
        ownerRef: 'github:canonical-owner',
        proofRef: expect.stringMatching(/^nip98:[0-9a-f]{64}$/u),
        pubkeyDigest: expect.stringMatching(/^[0-9a-f]{64}$/u),
      }),
    )
  })

  test('returns an idempotent state for the same canonical account', async () => {
    const secret = generateSecretKey()
    const fixture = makeDependencies({ linkState: 'already_linked' })
    const response = await handleMobileDeviceLinkRequest(
      fixture.dependencies,
      requestFixture(secret),
      {},
      ctx,
    )

    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      schema: MOBILE_DEVICE_LINK_SCHEMA,
      state: 'already_linked',
      ownerRef: 'github:canonical-owner',
    })
    expect(fixture.audit).toHaveBeenCalledWith(
      'link_existing',
      expect.any(Object),
    )
  })

  test('requires a canonical OpenAuth bearer and rejects a native Omega bearer', async () => {
    const secret = generateSecretKey()
    for (const request of [
      new Request(url, { method: 'POST' }),
      requestFixture(secret, { accessToken: `oa_omega_${'a'.repeat(43)}` }),
    ]) {
      const fixture = makeDependencies()
      const response = await handleMobileDeviceLinkRequest(
        fixture.dependencies,
        request,
        {},
        ctx,
      )

      expect(response.status).toBe(401)
      expect(await response.json()).toEqual({
        error: 'mobile_session_required',
      })
      expect(fixture.requireUserBearerSession).not.toHaveBeenCalled()
      expect(fixture.link).not.toHaveBeenCalled()
    }
  })

  test('rejects an invalid session before it evaluates the device proof', async () => {
    const secret = generateSecretKey()
    const fixture = makeDependencies()
    fixture.requireUserBearerSession.mockResolvedValueOnce(undefined)
    const response = await handleMobileDeviceLinkRequest(
      fixture.dependencies,
      requestFixture(secret),
      {},
      ctx,
    )

    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({
      error: 'mobile_session_required',
    })
    expect(fixture.link).not.toHaveBeenCalled()
  })

  test('rejects wrong bindings, stale proofs, and noncanonical device refs', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const cases = [
      requestFixture(secret, {
        proofBody: JSON.stringify({ different: true }),
      }),
      requestFixture(secret, {
        proofCreatedAt: new Date(now.getTime() - 61_000),
      }),
      requestFixture(secret, {
        proofUrl: 'https://evil.example/api/mobile/auth/device-link',
      }),
      requestFixture(secret, {
        body: JSON.stringify({
          schema: MOBILE_DEVICE_LINK_SCHEMA,
          pubkey,
          deviceRef: 'omega-mobile-000000000000000000000000',
        }),
      }),
    ]

    for (const request of cases) {
      const fixture = makeDependencies()
      const response = await handleMobileDeviceLinkRequest(
        fixture.dependencies,
        request,
        {},
        ctx,
      )

      expect(response.status).toBe(403)
      expect(await response.json()).toEqual({
        error: 'device_proof_rejected',
      })
      expect(fixture.link).not.toHaveBeenCalled()
    }
  })

  test('atomically consumes one proof under concurrent replay', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const body = JSON.stringify({
      schema: MOBILE_DEVICE_LINK_SCHEMA,
      pubkey,
      deviceRef: `omega-mobile-${pubkey.slice(0, 24)}`,
    })
    const signedProof = proof(secret, new TextEncoder().encode(body))
    const fixture = makeDependencies()
    const makeRequest = () =>
      new Request(url, {
        body,
        headers: {
          authorization: 'Bearer openauth-access',
          [MOBILE_DEVICE_LINK_PROOF_HEADER]: signedProof,
        },
        method: 'POST',
      })
    const responses = await Promise.all(
      Array.from({ length: 8 }, () =>
        handleMobileDeviceLinkRequest(
          fixture.dependencies,
          makeRequest(),
          {},
          ctx,
        ),
      ),
    )

    expect(responses.filter(response => response.status === 200)).toHaveLength(
      1,
    )
    expect(responses.filter(response => response.status === 409)).toHaveLength(
      7,
    )
    expect(fixture.link).toHaveBeenCalledOnce()
    for (const response of responses.filter(item => item.status === 409)) {
      expect(await response.json()).toEqual({
        error: 'device_proof_replayed',
      })
    }
  })

  test('rejects cross-account ownership without disclosing the other owner', async () => {
    const secret = generateSecretKey()
    const fixture = makeDependencies({ linkState: 'conflict' })
    const response = await handleMobileDeviceLinkRequest(
      fixture.dependencies,
      requestFixture(secret),
      {},
      ctx,
    )

    expect(response.status).toBe(409)
    expect(await response.json()).toEqual({ error: 'device_link_conflict' })
    expect(fixture.audit).toHaveBeenCalledWith(
      'link_conflict',
      expect.any(Object),
    )
  })

  test('maps replay-store and identity-store failures to one bounded error', async () => {
    for (const storageFailure of ['session', 'replay', 'link'] as const) {
      const secret = generateSecretKey()
      const fixture = makeDependencies({ storageFailure })
      const response = await handleMobileDeviceLinkRequest(
        fixture.dependencies,
        requestFixture(secret),
        {},
        ctx,
      )

      expect(response.status).toBe(503)
      expect(await response.json()).toEqual({
        error: 'device_link_unavailable',
      })
      expect(fixture.audit).toHaveBeenCalledWith(
        'storage_unavailable',
        expect.any(Object),
      )
    }
  })
})
