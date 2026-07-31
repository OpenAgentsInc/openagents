import { JobRestartPolicy } from '@livekit/protocol'
import {
  SARAH_LIVEKIT_AGENT_NAME,
  decodeSarahLiveKitDispatchMetadata,
} from '@openagentsinc/audio-contract'
import { describe, expect, test, vi } from 'vitest'

import {
  type SarahLiveKitRoomBrokerClients,
  makeSarahLiveKitRoomBroker,
  parseSarahLiveKitRoomBrokerConfig,
} from './sarah-livekit-room-broker'

describe('Sarah LiveKit room broker configuration', () => {
  test('accepts only exact WSS and server credential shapes', () => {
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: 'wss://livekit.openagents.com',
        SARAH_LIVEKIT_API_KEY: `API${'A'.repeat(12)}`,
        SARAH_LIVEKIT_API_SECRET: 'b'.repeat(48),
      }),
    ).toEqual({
      livekitUrl: 'wss://livekit.openagents.com',
      apiKey: `API${'A'.repeat(12)}`,
      apiSecret: 'b'.repeat(48),
    })
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: 'https://livekit.openagents.com',
        SARAH_LIVEKIT_API_KEY: `API${'A'.repeat(12)}`,
        SARAH_LIVEKIT_API_SECRET: 'b'.repeat(48),
      }),
    ).toBeUndefined()
  })

  test('does not accept client or worker credentials in the URL', () => {
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: 'wss://secret@livekit.openagents.com',
        SARAH_LIVEKIT_API_KEY: `API${'A'.repeat(12)}`,
        SARAH_LIVEKIT_API_SECRET: 'b'.repeat(48),
      }),
    ).toBeUndefined()
  })

  test('creates an explicit no-restart dispatch and a microphone-only client grant', async () => {
    const createRoom = vi.fn(async () => undefined)
    const deleteRoom = vi.fn(async () => undefined)
    let dispatchOptions:
      Parameters<SarahLiveKitRoomBrokerClients['createDispatch']>[2] | undefined
    const createDispatch = vi.fn(
      async (
        _roomRef: string,
        _agentName: string,
        options: Parameters<SarahLiveKitRoomBrokerClients['createDispatch']>[2],
      ) => {
        dispatchOptions = options
        return { id: 'dispatch:one' }
      },
    )
    const listDispatch = vi.fn(async () => [])
    const deleteDispatch = vi.fn(async () => undefined)
    const broker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: 'wss://livekit.openagents.com',
        apiKey: `API${'A'.repeat(12)}`,
        apiSecret: 'b'.repeat(48),
      },
      () => 2_000_000_000_000,
      {
        createRoom,
        deleteRoom,
        createDispatch,
        listDispatch,
        deleteDispatch,
      },
    )
    const provision = await broker.provision({
      idempotencyKey: 'sarah-livekit:session:one:1',
      workerControlToken: `oa_sarah_lk_${'C'.repeat(43)}`,
      ownerUserId: 'owner:one',
      deviceRef: 'device:one',
      threadRef: 'thread:one',
      sessionRef: 'session:one',
      generation: 1,
      capabilityProfile: 'omega_editor',
      admissionRef: 'admission:one',
      admissionDigest: 'd'.repeat(64),
      roomContext: { kind: 'private' },
      publishAllowed: true,
      subscribeAllowed: true,
      expiresAtMs: 2_000_000_600_000,
    })

    expect(createRoom).toHaveBeenCalledWith(
      expect.objectContaining({ maxParticipants: 2 }),
    )
    expect(createDispatch).toHaveBeenCalledWith(
      provision.roomRef,
      SARAH_LIVEKIT_AGENT_NAME,
      expect.objectContaining({
        restartPolicy: JobRestartPolicy.JRP_NEVER,
      }),
    )
    expect(dispatchOptions).toBeDefined()
    if (dispatchOptions === undefined) {
      throw new Error('The explicit dispatch options were not observed')
    }
    const dispatch = decodeSarahLiveKitDispatchMetadata(
      JSON.parse(dispatchOptions.metadata),
    )
    expect(dispatch).toMatchObject({
      sessionRef: 'session:one',
      generation: 1,
      sarahParticipantRef: 'principal.sarah',
      controlToken: `oa_sarah_lk_${'C'.repeat(43)}`,
    })
    expect(provision.grantClaims).toEqual(
      expect.objectContaining({
        roomJoin: true,
        canPublishData: false,
        canUpdateOwnMetadata: false,
        canPublishSources: ['microphone'],
        roomAdmin: false,
        roomCreate: false,
        roomList: false,
      }),
    )
    await broker.cleanup(provision)
    expect(deleteDispatch).toHaveBeenCalledWith(
      provision.dispatchRef,
      provision.roomRef,
    )
    expect(deleteRoom).toHaveBeenCalledWith(provision.roomRef)
  })
})
