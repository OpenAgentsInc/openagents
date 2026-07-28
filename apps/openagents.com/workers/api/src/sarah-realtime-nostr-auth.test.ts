import {
  SARAH_VOICE_NOSTR_CHALLENGE_PATH,
  SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
} from '@openagentsinc/audio-contract'
import { describe, expect, test } from 'vitest'

import { makeMemoryAuthKvStore } from './auth/auth-kv'
import {
  SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS,
  makeSarahVoiceNostrChallengeService,
} from './sarah-realtime-nostr-auth'

const url = `https://openagents.com${SARAH_VOICE_NOSTR_CHALLENGE_PATH}`
const nowMs = Date.UTC(2026, 6, 28, 12, 0, 0)
const pubkey = 'a'.repeat(64)

const request = (
  deviceRef = 'omega-1',
  headers: Record<string, string> = {},
): Request =>
  new Request(url, {
    body: JSON.stringify({
      schema: SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
      deviceRef,
      pubkey,
    }),
    headers: { 'content-type': 'application/json', ...headers },
    method: 'POST',
  })

describe('Sarah voice NIP-98 challenge service', () => {
  test('issues an expiring challenge and consumes it once for its device', async () => {
    const store = makeMemoryAuthKvStore()
    const service = makeSarahVoiceNostrChallengeService({
      authStore: () => store,
      enabled: () => true,
      now: () => nowMs,
      resolveOwnerRef: async () => 'user-1',
    })
    const response = await service.issue(request(), {})
    const body = (await response.json()) as {
      challenge: string
      expiresAtMs: number
      schema: string
      ownerRef: string
    }

    expect(response.status).toBe(201)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.schema).toBe(SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION)
    expect(body.challenge).toMatch(/^[A-Za-z0-9_-]{32,256}$/u)
    expect(body.ownerRef).toBe('user-1')
    expect(body.expiresAtMs).toBe(
      nowMs + SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS * 1_000,
    )
    await expect(
      service.consume(
        {},
        {
          challenge: body.challenge,
          deviceRef: 'omega-1',
          ownerRef: 'user-1',
          pubkey,
        },
      ),
    ).resolves.toBe('Consumed')
    await expect(
      service.consume(
        {},
        {
          challenge: body.challenge,
          deviceRef: 'omega-1',
          ownerRef: 'user-1',
          pubkey,
        },
      ),
    ).resolves.toBe('Invalid')
  })

  test('rejects a device mismatch, expiry, malformed input, and disabled issue', async () => {
    let clock = nowMs
    const store = makeMemoryAuthKvStore()
    const service = makeSarahVoiceNostrChallengeService({
      authStore: () => store,
      enabled: () => true,
      now: () => clock,
      resolveOwnerRef: async () => 'user-1',
    })
    const issued = (await (await service.issue(request(), {})).json()) as {
      challenge: string
    }

    await expect(
      service.consume(
        {},
        {
          challenge: issued.challenge,
          deviceRef: 'omega-2',
          ownerRef: 'user-1',
          pubkey,
        },
      ),
    ).resolves.toBe('Invalid')
    clock += SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS * 1_000 + 1
    await expect(
      service.consume(
        {},
        {
          challenge: issued.challenge,
          deviceRef: 'omega-1',
          ownerRef: 'user-1',
          pubkey,
        },
      ),
    ).resolves.toBe('Invalid')
    await expect(
      service.consume(
        {},
        {
          challenge: 'short',
          deviceRef: 'omega-1',
          ownerRef: 'user-1',
          pubkey,
        },
      ),
    ).resolves.toBe('Invalid')

    const disabled = makeSarahVoiceNostrChallengeService({
      authStore: () => store,
      enabled: () => false,
      resolveOwnerRef: async () => undefined,
    })
    expect((await disabled.issue(request(), {})).status).toBe(503)
    expect(
      (
        await service.issue(
          new Request(url, {
            body: '{}',
            method: 'POST',
          }),
          {},
        )
      ).status,
    ).toBe(400)
  })

  test('rate-limits challenge allocation by client address', async () => {
    const store = makeMemoryAuthKvStore()
    const service = makeSarahVoiceNostrChallengeService({
      authStore: () => store,
      enabled: () => true,
      now: () => nowMs,
      resolveOwnerRef: async () => 'user-1',
    })
    const statuses: Array<number> = []
    for (let index = 0; index < 31; index += 1) {
      statuses.push(
        (
          await service.issue(
            request('omega-1', { 'x-real-ip': '198.51.100.5' }),
            {},
          )
        ).status,
      )
    }
    expect(statuses.slice(0, 30).every(status => status === 201)).toBe(true)
    expect(statuses[30]).toBe(429)
  })
})
