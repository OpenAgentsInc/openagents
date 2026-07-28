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

const request = (body: unknown, deviceRef = identity.deviceRef): Request =>
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
    const body = await response.json()
    expect(body).toMatchObject({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      sessionRef: 'voice-1',
      model: 'gpt-realtime-2.1',
      reservedCreditMsat: 25_000,
      maxDurationSeconds: 600,
    })
    expect(body.gatewayUrl).toBe(
      'wss://openagents.com/api/omega/sarah/voice/connect',
    )
    expect(body.ticket).toMatch(/^[A-Za-z0-9_-]{32,}$/u)
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        ownerActorRef: 'agent:user-1',
        reservedMsat: 25_000,
        ticketDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
      }),
    )
    expect(fixture.close).toHaveBeenCalledOnce()
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
})
