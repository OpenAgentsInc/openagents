import {
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH,
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_PATH,
  OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
} from '@openagentsinc/audio-contract'
import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import { describe, expect, test, vi } from 'vitest'

import { makeMemoryAuthKvStore } from './auth-kv'
import {
  OMEGA_NOSTR_DEVICE_LINK_DEVICE_HEADER,
  makeOmegaNostrDeviceLinkService,
} from './omega-nostr-device-link'
import { makeOmegaNostrSessionService } from './omega-nostr-session'

const now = new Date('2026-07-28T18:00:00.000Z')
const baseUrl = 'https://staging.openagents.com'
const challengeUrl = `${baseUrl}${OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PATH}`
const linkUrl = `${baseUrl}${OMEGA_NOSTR_DEVICE_LINK_PATH}`
const context = {} as ExecutionContext

type User = Readonly<{
  provider: 'github' | 'email' | 'nostr'
  userId: string
}>

const nip98Authorization = (
  secret: Uint8Array,
  payload: Uint8Array,
  requestUrl = linkUrl,
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
        ['payload', hashPayloadBytes(payload)],
      ],
    },
    secret,
  )
  return `Nostr ${btoa(JSON.stringify(event))}`
}

const makeHarness = (
  overrides: {
    linkIdentity?: () => Promise<
      'Linked' | 'AlreadyLinked' | 'Conflict' | 'Unavailable'
    >
    user?: User | undefined
  } = {},
) => {
  const store = makeMemoryAuthKvStore()
  const user =
    'user' in overrides
      ? overrides.user
      : ({ provider: 'github', userId: 'github:owner' } as const)
  const linkIdentity =
    overrides.linkIdentity ?? vi.fn(async () => 'Linked' as const)
  const nostr = makeOmegaNostrSessionService<User, object>({
    authStore: () => store,
    expectedOwnerPubkey: () => undefined,
    now: () => now,
    resolveOwner: async () => undefined,
  })
  const service = makeOmegaNostrDeviceLinkService<User, object>({
    authStore: () => store,
    consumeProof: nostr.consumeProof,
    isCanonicalUser: value => value.provider !== 'nostr',
    linkIdentity,
    now: () => now.getTime(),
    ownerRefFromUser: value => value.userId,
    requireUserBearerSession: async request =>
      request.headers.get('authorization') === 'Bearer signed-in'
        ? user === undefined
          ? undefined
          : { user }
        : undefined,
    verifyProof: nostr.verifyProof,
  })
  return { linkIdentity, service }
}

const issueChallenge = async (
  service: ReturnType<typeof makeHarness>['service'],
  pubkey: string,
  deviceRef = 'mobile-device-1',
) =>
  service.issueChallenge(
    new Request(challengeUrl, {
      body: JSON.stringify({
        schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
        deviceRef,
        pubkey,
      }),
      headers: {
        authorization: 'Bearer signed-in',
        'content-type': 'application/json',
      },
      method: 'POST',
    }),
    {},
    context,
  )

const linkRequest = async (
  service: ReturnType<typeof makeHarness>['service'],
  secret: Uint8Array,
  challenge: string,
  overrides: {
    deviceHeader?: string
    deviceRef?: string
    ownerRef?: string
    proofPayload?: Uint8Array
  } = {},
) => {
  const deviceRef = overrides.deviceRef ?? 'mobile-device-1'
  const body = JSON.stringify({
    schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
    challenge,
    ownerRef: overrides.ownerRef ?? 'github:owner',
    deviceRef,
  })
  const payload = new TextEncoder().encode(body)
  return service.link(
    new Request(linkUrl, {
      body,
      headers: {
        authorization: nip98Authorization(
          secret,
          overrides.proofPayload ?? payload,
        ),
        'content-type': 'application/json',
        [OMEGA_NOSTR_DEVICE_LINK_DEVICE_HEADER]:
          overrides.deviceHeader ?? deviceRef,
      },
      method: 'POST',
    }),
    {},
  )
}

describe('Omega Nostr device link', () => {
  test('links a proof-bound key and rejects an exact replay', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const { linkIdentity, service } = makeHarness()
    const issued = await issueChallenge(service, pubkey)
    expect(issued.status).toBe(201)
    const challenge = String(
      ((await issued.json()) as { challenge: string }).challenge,
    )

    const first = await linkRequest(service, secret, challenge)
    expect(first.status).toBe(200)
    await expect(first.json()).resolves.toEqual({
      schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
      linked: true,
      ownerRef: 'github:owner',
    })
    expect(linkIdentity).toHaveBeenCalledWith({}, 'github:owner', pubkey)

    const replay = await linkRequest(service, secret, challenge)
    expect(replay.status).toBe(409)
  })

  test('rejects missing bearer and a Nostr-native bearer account', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)
    const missing = makeHarness({ user: undefined })
    expect((await issueChallenge(missing.service, pubkey)).status).toBe(401)

    const native = makeHarness({
      user: { provider: 'nostr', userId: `nostr:${pubkey}` },
    })
    expect((await issueChallenge(native.service, pubkey)).status).toBe(403)
  })

  test('binds the signed body, owner, device, and signer to one challenge', async () => {
    const secret = generateSecretKey()
    const pubkey = getPublicKey(secret)

    for (const run of [
      (service: ReturnType<typeof makeHarness>['service'], challenge: string) =>
        linkRequest(service, secret, challenge, {
          proofPayload: new TextEncoder().encode('{}'),
        }),
      (service: ReturnType<typeof makeHarness>['service'], challenge: string) =>
        linkRequest(service, secret, challenge, {
          ownerRef: 'github:attacker',
        }),
      (service: ReturnType<typeof makeHarness>['service'], challenge: string) =>
        linkRequest(service, secret, challenge, {
          deviceHeader: 'another-device',
        }),
      (service: ReturnType<typeof makeHarness>['service'], challenge: string) =>
        linkRequest(service, generateSecretKey(), challenge),
    ]) {
      const { service } = makeHarness()
      const issued = await issueChallenge(service, pubkey)
      const challenge = String(
        ((await issued.json()) as { challenge: string }).challenge,
      )
      expect((await run(service, challenge)).status).toBe(401)
    }
  })

  test('does not overwrite a foreign or tombstoned identity binding', async () => {
    for (const result of ['Conflict', 'AlreadyLinked'] as const) {
      const secret = generateSecretKey()
      const { service } = makeHarness({
        linkIdentity: vi.fn(async () => result),
      })
      const issued = await issueChallenge(service, getPublicKey(secret))
      const challenge = String(
        ((await issued.json()) as { challenge: string }).challenge,
      )
      const response = await linkRequest(service, secret, challenge)
      expect(response.status).toBe(result === 'Conflict' ? 409 : 200)
    }
  })

  test('rate-limits repeated challenge issuance for one pubkey', async () => {
    const secret = generateSecretKey()
    const { service } = makeHarness()
    const statuses = []
    for (let index = 0; index < 6; index += 1) {
      statuses.push(
        (await issueChallenge(service, getPublicKey(secret))).status,
      )
    }
    expect(statuses).toEqual([201, 201, 201, 201, 201, 429])
  })
})
