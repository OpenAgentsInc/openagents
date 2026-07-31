import {
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  type SarahLiveKitJobEvent,
  decodeSarahLiveKitJobClaimResponse,
} from '@openagentsinc/audio-contract'
import type { SarahRealtimeVoiceStore } from '@openagentsinc/khala-sync-server'
import { describe, expect, test, vi } from 'vitest'

import {
  handleSarahLiveKitWorkerClaim,
  handleSarahLiveKitWorkerEvent,
} from './sarah-livekit-worker-routes'

const token = `oa_sarah_lk_${'A'.repeat(43)}`
const claimLiveKitWorkerJob = vi.fn(async () => ({
  sessionRef: 'session:one',
  generation: 1,
  ownerUserId: 'owner:one',
  capabilityProfile: 'omega_editor' as const,
  roomContext: { kind: 'private' as const },
  sessionExpiresAt: '2033-05-18T03:34:20.000Z',
}))
const authorizeLiveKitWorkerEvent = vi.fn(async () => ({
  roomRef: 'room:one',
  sarahParticipantRef: 'principal.sarah',
  state: 'active' as const,
}))
const recordUsage = vi.fn(async () => ({
  chargedMsat: 1,
  reservedMsat: 10,
  creditLimitReached: false,
}))
const recordLiveKitParticipantJoin = vi.fn(async () => undefined)
const closeLiveKitWorkerJob = vi.fn(async () => ({}))
const store = {
  authorizeLiveKitWorkerEvent,
  claimLiveKitWorkerJob,
  closeLiveKitWorkerJob,
  recordLiveKitParticipantJoin,
  recordUsage,
} as unknown as SarahRealtimeVoiceStore
const cleanup = vi.fn(async () => undefined)
const dependencies = {
  creditMsatPerMillionTokens: () => 100_000,
  now: () => 2_000_000_000_000,
  openStore: async () => ({ store, close: async () => undefined }),
  cleanup,
}

const authorizedRequest = (path: string, body: unknown) =>
  new Request(`https://openagents.com${path}`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(body),
  })

describe('Sarah LiveKit worker routes', () => {
  test('claims the exact server dispatch and returns a private profile without owner refs', async () => {
    const response = await handleSarahLiveKitWorkerClaim(
      dependencies,
      authorizedRequest('/api/internal/sarah/livekit/job/claim', {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        workerRef: 'worker:one',
        jobRef: 'job:one',
        dispatchRef: 'dispatch:one',
        roomSid: 'RM_one',
        dispatch: {
          sessionRef: 'session:one',
          generation: 1,
          roomRef: 'room:one',
          roomEpoch: 1,
          participantRef: 'owner:one',
          sarahParticipantRef: 'principal.sarah',
          sarahPresenceLeaseRef: 'presence:one',
          capabilityProfile: 'omega_editor',
          roomContext: { kind: 'private' },
        },
      }),
      {},
    )
    expect(response.status).toBe(200)
    const body = decodeSarahLiveKitJobClaimResponse(await response.json())
    expect(body.capabilityProfile.kind).toBe('private_owner_v1')
    expect(JSON.stringify(body)).not.toContain('owner:one')
    expect(claimLiveKitWorkerJob).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: 'session:one',
        dispatchRef: 'dispatch:one',
        workerJobRef: 'job:one',
      }),
    )
  })

  test('records response and transcription usage under different idempotency refs', async () => {
    const base = {
      schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
      sessionRef: 'session:one',
      generation: 1,
      jobRef: 'job:one',
      eventRef: 'event:one',
      inputTokens: 7,
      outputTokens: 3,
      cachedInputTokens: 1,
      audioInputTokens: 5,
      audioOutputTokens: 2,
    } as const
    for (const event of [
      {
        ...base,
        _tag: 'response_usage',
        providerResponseRef: 'resp_1',
        status: 'completed',
      },
      {
        ...base,
        _tag: 'transcription_usage',
        providerTranscriptionRef: 'item_1',
      },
    ] satisfies SarahLiveKitJobEvent[]) {
      recordUsage.mockClear()
      const response = await handleSarahLiveKitWorkerEvent(
        dependencies,
        authorizedRequest('/api/internal/sarah/livekit/job/event', event),
        {},
      )
      expect(response.status).toBe(200)
      expect(recordUsage).toHaveBeenCalledWith(
        expect.objectContaining({
          usage: expect.objectContaining({
            usageKind:
              event._tag === 'response_usage' ? 'response' : 'transcription',
            providerResponseRef:
              event._tag === 'response_usage'
                ? 'response:resp_1'
                : 'transcription:item_1',
          }),
        }),
      )
    }
  })

  test('settles and cleans the room exactly once on worker close', async () => {
    const response = await handleSarahLiveKitWorkerEvent(
      dependencies,
      authorizedRequest('/api/internal/sarah/livekit/job/event', {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: 'close',
        sessionRef: 'session:one',
        generation: 1,
        jobRef: 'job:one',
        eventRef: 'close:job:one',
        reason: 'provider_disconnect',
      }),
      {},
    )
    expect(response.status).toBe(200)
    expect(closeLiveKitWorkerJob).toHaveBeenCalledWith(
      expect.objectContaining({
        closeReason: 'livekit_worker_provider_disconnect',
      }),
    )
    expect(cleanup).toHaveBeenCalledWith(
      {},
      {
        sessionRef: 'session:one',
        generation: 1,
      },
    )
  })
})
