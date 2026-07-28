import {
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
} from '@openagentsinc/audio-contract'
import {
  type SarahRealtimeVoiceStore,
  SarahVoiceInsufficientCreditError,
} from '@openagentsinc/khala-sync-server'
import { describe, expect, test, vi } from 'vitest'

import {
  SARAH_REALTIME_VOICE_DEVICE_HEADER,
  handleSarahRealtimeVoiceSessionRequest,
} from './sarah-realtime-voice-routes'

const identity = {
  ownerRef: 'user-1',
  deviceRef: 'omega-1',
  threadRef: 'thread-1',
  sessionRef: 'voice-1',
  generation: 1,
} as const

const request = (
  body: unknown,
  deviceRef: string = identity.deviceRef,
): Request =>
  new Request(`https://openagents.com${SARAH_VOICE_SESSION_PATH}`, {
    method: 'POST',
    headers: {
      authorization: 'Bearer test',
      'content-type': 'application/json',
      [SARAH_REALTIME_VOICE_DEVICE_HEADER]: deviceRef,
    },
    body: JSON.stringify(body),
  })

const ctx = {
  passThroughOnException: () => undefined,
  waitUntil: () => undefined,
} as unknown as ExecutionContext

const makeDependencies = (
  reserve: SarahRealtimeVoiceStore['reserve'] = vi.fn(async input => ({
    sessionRef: input.sessionRef,
    ownerUserId: input.ownerUserId,
    ownerActorRef: input.ownerActorRef,
    deviceRef: input.deviceRef,
    threadRef: input.threadRef,
    generation: input.generation,
    disclosureRef: input.disclosureRef,
    clientProfile: input.clientProfile,
    state: 'reserved' as const,
    reservedMsat: input.reservedMsat,
    chargedMsat: 0,
    ticketExpiresAt: input.ticketExpiresAt,
    sessionExpiresAt: input.sessionExpiresAt,
    settlementReceiptRef: null,
  })),
) => {
  const close = vi.fn(async () => undefined)
  const store = {
    reserve,
    connect: vi.fn(),
    recordUsage: vi.fn(),
    settle: vi.fn(),
    sweepExpired: vi.fn(),
  } as unknown as SarahRealtimeVoiceStore
  return {
    close,
    dependencies: {
      config: () => ({
        enabled: true,
        maxSessionSeconds: 600,
        reservationMsat: 25_000,
      }),
      openStore: async () => ({ store, close }),
      requireUserBearerSession: async () => ({
        user: { userId: 'user-1' },
      }),
      userIdFromSession: (session: { user: { userId: string } }) =>
        session.user.userId,
      now: () => Date.UTC(2026, 6, 28, 12, 0, 0),
    },
    reserve,
  }
}

describe('managed Sarah Realtime voice session route', () => {
  test('issues a one-use OpenAgents ticket after reserving credit', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
      }),
      {},
      ctx,
    )
    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      readonly gatewayUrl: string
      readonly ticket: string
      readonly clientProfile: string
    }
    expect(body).toMatchObject({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      sessionRef: 'voice-1',
      model: 'gpt-realtime-2.1',
      reservedCreditMsat: 25_000,
      maxDurationSeconds: 600,
      clientProfile: 'omega_editor',
    })
    expect(body.gatewayUrl).toBe(
      'wss://openagents.com/api/omega/sarah/voice/connect',
    )
    expect(body.ticket).toMatch(/^[A-Za-z0-9_-]{32,}$/u)
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        ownerActorRef: 'agent:user-1',
        clientProfile: 'omega_editor',
        reservedMsat: 25_000,
        ticketDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
      }),
    )
    expect(fixture.close).toHaveBeenCalledOnce()
  })

  test('persists and echoes the voice-only mobile profile', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'openagents.mobile.sarah.voice.v1',
        clientProfile: 'mobile_voice_only',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toMatchObject({
      clientProfile: 'mobile_voice_only',
    })
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        clientProfile: 'mobile_voice_only',
      }),
    )
  })

  test('rejects an unknown client profile before credit reservation', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        clientProfile: 'arbitrary_device_commands',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(400)
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('rejects an Omega device mismatch before storage access', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request(
        {
          schema: SARAH_VOICE_PROTOCOL_VERSION,
          identity,
          disclosureRef: 'disclosure-1',
        },
        'other-device',
      ),
      {},
      ctx,
    )
    expect(response.status).toBe(403)
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('maps an unavailable reservation to payment required', async () => {
    const fixture = makeDependencies(
      vi.fn(async () => {
        throw new SarahVoiceInsufficientCreditError('insufficient')
      }),
    )
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
      }),
      {},
      ctx,
    )
    expect(response.status).toBe(402)
    expect(await response.json()).toEqual({ error: 'insufficient_credit' })
    expect(fixture.close).toHaveBeenCalledOnce()
  })

  test('exchanges a body-bound NIP-98 proof and one-use challenge for the normal ticket and bearer session', async () => {
    const fixture = makeDependencies()
    const body = {
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      disclosureRef: 'disclosure-1',
      auth: {
        method: 'nostr_nip98',
        challenge: 'challenge_abcdefghijklmnopqrstuvwxyz012345',
      },
    } as const
    const rawBody = JSON.stringify(body)
    const verifyNostrProof = vi.fn(
      async (_request: Request, _env: unknown, payload: Uint8Array) => {
        expect(new TextDecoder().decode(payload)).toBe(rawBody)
        return {
          _tag: 'Verified' as const,
          eventId: 'b'.repeat(64),
          isOwner: false,
          pubkey: 'a'.repeat(64),
          pubkeyDigest: 'c'.repeat(64),
        }
      },
    )
    const authenticateNostrSession = vi.fn(async () => ({
      _tag: 'Authenticated' as const,
      pubkey: 'a'.repeat(64),
      user: { userId: 'user-1' },
    }))
    const consumeNostrChallenge = vi.fn(async () => 'Consumed' as const)
    const mintNostrSession = vi.fn(async () => ({
      _tag: 'Issued' as const,
      accessToken: `oa_omega_${'a'.repeat(43)}`,
      expiresIn: 900,
      user: { userId: 'user-1' },
    }))
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        authenticateNostrSession,
        consumeNostrChallenge,
        mintNostrSession,
        verifyNostrProof,
      },
      new Request(`https://openagents.com${SARAH_VOICE_SESSION_PATH}`, {
        body: rawBody,
        headers: {
          authorization: 'Nostr signed-event',
          'content-type': 'application/json',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        method: 'POST',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toMatchObject({
      auth: {
        accessToken: `oa_omega_${'a'.repeat(43)}`,
        expiresIn: 900,
        method: 'nostr_nip98',
      },
      sessionRef: identity.sessionRef,
    })
    expect(authenticateNostrSession).toHaveBeenCalledOnce()
    expect(verifyNostrProof).toHaveBeenCalledOnce()
    expect(consumeNostrChallenge).toHaveBeenCalledWith(
      {},
      {
        challenge: body.auth.challenge,
        deviceRef: identity.deviceRef,
        ownerRef: identity.ownerRef,
        pubkey: 'a'.repeat(64),
      },
    )
    expect(mintNostrSession).toHaveBeenCalledOnce()
    expect(fixture.reserve).toHaveBeenCalledOnce()
  })

  test('rejects a missing or replayed NIP-98 challenge before credit reservation', async () => {
    const fixture = makeDependencies()
    const baseBody = {
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      disclosureRef: 'disclosure-1',
    } as const
    const nostrRequest = (body: unknown) =>
      new Request(`https://openagents.com${SARAH_VOICE_SESSION_PATH}`, {
        body: JSON.stringify(body),
        headers: {
          authorization: 'Nostr signed-event',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        method: 'POST',
      })
    const authenticated = {
      _tag: 'Authenticated' as const,
      pubkey: 'a'.repeat(64),
      user: { userId: 'user-1' },
    }
    const dependencies = {
      ...fixture.dependencies,
      authenticateNostrSession: vi.fn(async () => authenticated),
      consumeNostrChallenge: vi.fn(async () => 'Invalid' as const),
      mintNostrSession: vi.fn(),
      verifyNostrProof: vi.fn(async () => ({
        _tag: 'Verified' as const,
        eventId: 'b'.repeat(64),
        isOwner: false,
        pubkey: 'a'.repeat(64),
        pubkeyDigest: 'c'.repeat(64),
      })),
    }

    expect(
      (
        await handleSarahRealtimeVoiceSessionRequest(
          dependencies,
          nostrRequest(baseBody),
          {},
          ctx,
        )
      ).status,
    ).toBe(401)
    expect(
      (
        await handleSarahRealtimeVoiceSessionRequest(
          dependencies,
          nostrRequest({
            ...baseBody,
            auth: {
              method: 'nostr_nip98',
              challenge: 'challenge_abcdefghijklmnopqrstuvwxyz012345',
            },
          }),
          {},
          ctx,
        )
      ).status,
    ).toBe(409)
    expect(fixture.reserve).not.toHaveBeenCalled()
    expect(dependencies.mintNostrSession).not.toHaveBeenCalled()
    expect(dependencies.authenticateNostrSession).not.toHaveBeenCalled()
  })
})
