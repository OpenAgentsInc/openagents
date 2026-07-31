import {
  SARAH_VOICE_ADMISSION_PATH,
  SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
  SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
  SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
} from '@openagentsinc/audio-contract'
import {
  type SarahRealtimeVoiceStore,
  SarahVoiceAdmissionRejectedError,
  SarahVoiceInsufficientCreditError,
} from '@openagentsinc/khala-sync-server'
import { describe, expect, test, vi } from 'vitest'

import {
  SARAH_REALTIME_VOICE_DEVICE_HEADER,
  SARAH_REALTIME_VOICE_SESSION_HEADER,
  finalizeSarahLiveKitRoom,
  handleSarahRealtimeVoiceAdmissionRequest,
  handleSarahRealtimeVoiceCohortRevocationRequest,
  handleSarahRealtimeVoiceSessionRequest,
  handleSarahRealtimeVoiceSettlementRequest,
  reconcileSarahLiveKitProvisioningIntents,
  reconcileSarahLiveKitTerminalRooms,
  recordSarahLiveKitParticipantJoin,
  recordSarahLiveKitProviderUsage,
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
    body: JSON.stringify(
      typeof body === 'object' &&
        body !== null &&
        (!('clientProfile' in body) || body.clientProfile === 'omega_editor')
        ? { ...body, admissionRef: 'sarah_voice_admission:test' }
        : body,
    ),
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
    transportKind: input.transportKind ?? 'custom_wss_v1',
    creditMode: input.creditMode,
    entitlementRef: input.entitlementRef,
    admissionCohortRef: input.admissionCohortRef,
    state: 'reserved' as const,
    reservedMsat: input.reservedMsat,
    chargedMsat: 0,
    ticketExpiresAt: input.ticketExpiresAt,
    sessionExpiresAt: input.sessionExpiresAt,
    settlementReceiptRef: null,
    admissionExpiresAt:
      input.admissionBinding === undefined
        ? undefined
        : '2026-07-28T12:02:00.000Z',
    admissionTermsDigest: input.admissionBinding?.termsDigest,
    replayed: false,
  })),
  readActiveStagingOwnerEntitlement: SarahRealtimeVoiceStore['readActiveStagingOwnerEntitlement'] = vi.fn(
    async () => undefined,
  ),
) => {
  const close = vi.fn(async () => undefined)
  const bindLiveKitRoom = vi.fn(async () => undefined)
  const prepareLiveKitProvisioningIntent = vi.fn(async () => undefined)
  const markLiveKitProvisioningIntent = vi.fn(async () => undefined)
  const settle = vi.fn(async () => undefined)
  const issueAdmission = vi.fn(
    async (
      input: Parameters<SarahRealtimeVoiceStore['issueAdmission']>[0],
    ) => ({
      admissionRef: input.admissionRef,
      admissionExpiresAt: input.expiresAt,
    }),
  )
  const store = {
    bindLiveKitRoom,
    prepareLiveKitProvisioningIntent,
    markLiveKitProvisioningIntent,
    reserve,
    issueAdmission,
    connect: vi.fn(),
    recordUsage: vi.fn(),
    readActiveAlphaMembership: vi.fn(async input => ({
      membershipRef: `sarah_voice_alpha:${input.ownerUserId}`,
      cohortRef: input.cohortRef,
      ownerUserId: input.ownerUserId,
    })),
    readActiveStagingOwnerEntitlement,
    readSettlement: vi.fn(),
    readSpendableCredit: vi.fn(async () => 100_000),
    revokeAlphaCohort: vi.fn(),
    settle,
    sweepExpired: vi.fn(),
  } as unknown as SarahRealtimeVoiceStore
  return {
    close,
    dependencies: {
      config: () => ({
        enabled: true,
        creditMsatPerMillionTokens: 100_000,
        maxSessionSeconds: 600,
        reservationMsat: 25_000,
      }),
      openStore: async () => ({ store, close }),
      requireUserBearerSession: async () => ({
        user: { userId: 'user-1' },
      }),
      userIdFromSession: (session: { user: { userId: string } }) =>
        session.user.userId,
      liveKitNewAdmissionsEnabled: () => true,
      now: () => Date.UTC(2026, 6, 28, 12, 0, 0),
    },
    reserve,
    readActiveStagingOwnerEntitlement,
    bindLiveKitRoom,
    prepareLiveKitProvisioningIntent,
    markLiveKitProvisioningIntent,
    issueAdmission,
    settle,
    store,
  }
}

describe('managed Sarah Realtime voice session route', () => {
  test('requires a server-issued admission for the Omega editor profile', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      new Request(`https://openagents.com${SARAH_VOICE_SESSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Bearer test',
          'content-type': 'application/json',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        body: JSON.stringify({
          schema: SARAH_VOICE_PROTOCOL_VERSION,
          identity,
          disclosureRef: 'disclosure-1',
          clientProfile: 'omega_editor',
        }),
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(409)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_admission_required',
    })
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('maps an expired, replayed, or changed admission to conflict', async () => {
    const fixture = makeDependencies(
      vi.fn(async () => {
        throw new SarahVoiceAdmissionRejectedError('admission changed')
      }),
    )
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        clientProfile: 'omega_editor',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(409)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_admission_invalid',
    })
  })

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
      admissionRef: 'sarah_voice_admission:test',
      admissionExpiresAtMs: Date.UTC(2026, 6, 28, 12, 2, 0),
      admissionCohortRef: 'sarah_voice_cohort:alpha_v1',
      creditMode: 'metered',
      creditRateMsatPerMillionTokens: 100_000,
      spendableRemainingCreditMsat: 100_000,
      capabilityBoundary: {
        commands: [
          'context_read',
          'reveal_range',
          'replace_selection',
          'save_document',
          'start_agent_thread',
        ],
        confirmationRequired: [
          'replace_selection',
          'save_document',
          'start_agent_thread',
        ],
        directShell: false,
        directGit: false,
        payment: false,
        credentialAccess: false,
        deviceControl: false,
      },
    })
    expect(body.gatewayUrl).toBe(
      'wss://openagents.com/api/omega/sarah/voice/connect',
    )
    expect(body).not.toHaveProperty('transport')
    expect(body.ticket).toMatch(/^[A-Za-z0-9_-]{32,}$/u)
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        ownerActorRef: 'agent:user-1',
        clientProfile: 'omega_editor',
        reservedMsat: 25_000,
        ticketDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
        admissionBinding: {
          admissionRef: 'sarah_voice_admission:test',
          spendableRemainingCreditMsat: 100_000,
          termsDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
        },
      }),
    )
    expect(fixture.close).toHaveBeenCalledOnce()
  })

  test('provisions and server-binds an explicitly requested LiveKit room', async () => {
    const fixture = makeDependencies()
    const provision = {
      livekitUrl: 'wss://livekit.openagents.test',
      roomRef: 'sarah-room:voice-1:g1',
      roomEpoch: 1,
      participantRef: 'participant:user-1:voice-1:g1',
      sarahParticipantRef: 'participant:sarah:voice-1:g1',
      participantGrant: 'opaque-livekit-participant-grant',
      joinExpiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
      dispatchRef: 'dispatch:voice-1:g1',
      sarahPresenceLeaseRef: 'sarah-presence:voice-1:g1',
      grantClaims: {
        roomRef: 'sarah-room:voice-1:g1',
        participantRef: 'participant:user-1:voice-1:g1',
        expiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
        roomJoin: true,
        canPublish: true,
        canSubscribe: true,
        canPublishData: false,
        canUpdateOwnMetadata: false,
        canPublishSources: ['microphone'],
        roomAdmin: false,
        roomCreate: false,
        roomList: false,
      },
    } as const
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'livekit-session-ticket'),
      provision: vi.fn(async () => provision),
      cleanup: vi.fn(async () => undefined),
      cleanupByIdempotencyKey: vi.fn(async () => undefined),
      cleanupRoom: vi.fn(async () => undefined),
    }
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        liveKitRoomBroker: broker,
      },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        requestedTransport: 'livekit_room_v1',
        roomContext: { kind: 'private' },
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(201)
    const body = await response.json()
    expect(body).toMatchObject({
      transport: {
        kind: 'livekit_room_v1',
        livekitUrl: provision.livekitUrl,
        roomRef: provision.roomRef,
        roomEpoch: 1,
        participantRef: provision.participantRef,
        sarahParticipantRef: provision.sarahParticipantRef,
        participantGrant: provision.participantGrant,
        dispatchRef: provision.dispatchRef,
        sarahPresenceLeaseRef: provision.sarahPresenceLeaseRef,
        permissions: {
          roomJoin: true,
          canPublish: true,
          canSubscribe: true,
          roomAdmin: false,
          roomCreate: false,
          roomList: false,
        },
      },
    })
    expect(fixture.bindLiveKitRoom).toHaveBeenCalledWith(
      expect.objectContaining({
        ownerUserId: 'user-1',
        deviceRef: 'omega-1',
        threadRef: 'thread-1',
        sessionRef: 'voice-1',
        generation: 1,
        roomRef: provision.roomRef,
        participantGrantDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
        roomContext: { kind: 'private' },
      }),
    )
    const transport = (body as { transport: Record<string, unknown> }).transport
    expect(transport).not.toHaveProperty('email')
    expect(transport).not.toHaveProperty('filesystemPath')
    expect(transport).not.toHaveProperty('openAiApiKey')
    expect(transport).not.toHaveProperty('balanceMsat')
    expect(transport).not.toHaveProperty('commandCapabilities')
    expect(transport).not.toHaveProperty('sarahSigningKey')
  })

  test('does not silently select LiveKit when its broker is unavailable', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        requestedTransport: 'livekit_room_v1',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_livekit_unavailable',
    })
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('blocks new LiveKit dispatches when the emergency admission seam is disabled', async () => {
    const fixture = makeDependencies()
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'livekit-session-ticket'),
      provision: vi.fn(),
      cleanup: vi.fn(),
      cleanupByIdempotencyKey: vi.fn(),
      cleanupRoom: vi.fn(),
    }
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        liveKitRoomBroker: broker,
        liveKitNewAdmissionsEnabled: () => false,
      },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        requestedTransport: 'livekit_room_v1',
        roomContext: { kind: 'private' },
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_livekit_admissions_disabled',
    })
    expect(fixture.reserve).not.toHaveBeenCalled()
    expect(broker.provision).not.toHaveBeenCalled()
  })

  test('keeps new LiveKit admissions disabled when the rollout seam is absent', async () => {
    const fixture = makeDependencies()
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'livekit-session-ticket'),
      provision: vi.fn(),
      cleanup: vi.fn(),
      cleanupByIdempotencyKey: vi.fn(),
      cleanupRoom: vi.fn(),
    }
    const {
      liveKitNewAdmissionsEnabled: _liveKitNewAdmissionsEnabled,
      ...defaultOffDependencies
    } = fixture.dependencies
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...defaultOffDependencies,
        liveKitRoomBroker: broker,
      },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        requestedTransport: 'livekit_room_v1',
        roomContext: { kind: 'private' },
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_livekit_admissions_disabled',
    })
    expect(fixture.reserve).not.toHaveBeenCalled()
    expect(broker.provision).not.toHaveBeenCalled()
  })

  test('rejects deferred mobile profiles before LiveKit admission', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      fixture.dependencies,
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        clientProfile: 'mobile_voice_only',
        requestedTransport: 'livekit_room_v1',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_livekit_client_profile_not_supported',
    })
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('uses authoritative community membership and publish policy', async () => {
    const fixture = makeDependencies()
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'livekit-session-ticket'),
      provision: vi.fn(
        async () =>
          ({
            livekitUrl: 'wss://livekit.openagents.test',
            roomRef: 'community-room:channel-7:g1',
            roomEpoch: 7,
            participantRef: 'participant:user-1:channel-7:g1',
            sarahParticipantRef: 'participant:sarah:channel-7:g1',
            participantGrant: 'opaque-community-grant',
            joinExpiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
            dispatchRef: 'dispatch:channel-7:g1',
            sarahPresenceLeaseRef: 'sarah-presence:channel-7:g1',
            grantClaims: {
              roomRef: 'community-room:channel-7:g1',
              participantRef: 'participant:user-1:channel-7:g1',
              expiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
              roomJoin: true,
              canPublish: false,
              canSubscribe: true,
              canPublishData: false,
              canUpdateOwnMetadata: false,
              canPublishSources: ['microphone'],
              roomAdmin: false,
              roomCreate: false,
              roomList: false,
            },
          }) as const,
      ),
      cleanup: vi.fn(async () => undefined),
      cleanupByIdempotencyKey: vi.fn(async () => undefined),
      cleanupRoom: vi.fn(async () => undefined),
    }
    const resolveLiveKitCommunityAccess = vi.fn(async () => ({
      communityRef: 'community-7',
      channelRef: 'channel-7',
      membershipRevision: 'membership-revision-42',
      publishAllowed: false,
      subscribeAllowed: true,
    }))
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        liveKitRoomBroker: broker,
        resolveLiveKitCommunityAccess,
      },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        requestedTransport: 'livekit_room_v1',
        roomContext: {
          kind: 'community',
          communityRef: 'community-7',
          channelRef: 'channel-7',
        },
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toMatchObject({
      transport: {
        kind: 'livekit_room_v1',
        permissions: { canPublish: false, canSubscribe: true },
      },
    })
    expect(broker.provision).toHaveBeenCalledWith(
      expect.objectContaining({
        roomContext: {
          kind: 'community',
          communityRef: 'community-7',
          channelRef: 'channel-7',
          membershipRevision: 'membership-revision-42',
          publishAllowed: false,
          subscribeAllowed: true,
        },
        publishAllowed: false,
      }),
    )
    expect(fixture.bindLiveKitRoom).toHaveBeenCalledWith(
      expect.objectContaining({
        roomContext: expect.objectContaining({
          membershipRevision: 'membership-revision-42',
        }),
        publishAllowed: false,
      }),
    )
  })

  test('binds a LiveKit admission digest to the current community membership revision', async () => {
    const fixture = makeDependencies()
    let membershipRevision = 'membership-revision-1'
    const dependencies = {
      ...fixture.dependencies,
      resolveLiveKitCommunityAccess: vi.fn(async () => ({
        communityRef: 'community-7',
        channelRef: 'channel-7',
        membershipRevision,
        publishAllowed: false,
        subscribeAllowed: true,
      })),
    }
    const makeAdmissionRequest = () =>
      new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Bearer test',
          'content-type': 'application/json',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        body: JSON.stringify({
          schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
          identity,
          disclosureRef: 'disclosure-1',
          clientProfile: 'omega_editor',
          requestedTransport: 'livekit_room_v1',
          roomContext: {
            kind: 'community',
            communityRef: 'community-7',
            channelRef: 'channel-7',
          },
        }),
      })

    expect(
      (
        await handleSarahRealtimeVoiceAdmissionRequest(
          dependencies,
          makeAdmissionRequest(),
          {},
          ctx,
        )
      ).status,
    ).toBe(200)
    membershipRevision = 'membership-revision-2'
    expect(
      (
        await handleSarahRealtimeVoiceAdmissionRequest(
          dependencies,
          makeAdmissionRequest(),
          {},
          ctx,
        )
      ).status,
    ).toBe(200)

    expect(fixture.issueAdmission.mock.calls[0]?.[0].termsDigest).not.toBe(
      fixture.issueAdmission.mock.calls[1]?.[0].termsDigest,
    )
  })

  test('settles before cleaning up a provision whose binding fails', async () => {
    const fixture = makeDependencies()
    fixture.bindLiveKitRoom.mockRejectedValueOnce(new Error('binding failed'))
    const provision = {
      livekitUrl: 'wss://livekit.openagents.test',
      roomRef: 'sarah-room:binding-failure',
      roomEpoch: 1,
      participantRef: 'participant:user-1:binding-failure',
      sarahParticipantRef: 'participant:sarah:binding-failure',
      participantGrant: 'opaque-failed-binding-grant',
      joinExpiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
      dispatchRef: 'dispatch:binding-failure',
      sarahPresenceLeaseRef: 'sarah-presence:binding-failure',
      grantClaims: {
        roomRef: 'sarah-room:binding-failure',
        participantRef: 'participant:user-1:binding-failure',
        expiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
        roomJoin: true,
        canPublish: true,
        canSubscribe: true,
        canPublishData: false,
        canUpdateOwnMetadata: false,
        canPublishSources: ['microphone'],
        roomAdmin: false,
        roomCreate: false,
        roomList: false,
      },
    } as const
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'livekit-session-ticket'),
      provision: vi.fn(async () => provision),
      cleanup: vi.fn(async () => undefined),
      cleanupByIdempotencyKey: vi.fn(async () => undefined),
      cleanupRoom: vi.fn(async () => undefined),
    }
    const response = await handleSarahRealtimeVoiceSessionRequest(
      { ...fixture.dependencies, liveKitRoomBroker: broker },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
        requestedTransport: 'livekit_room_v1',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(503)
    expect(fixture.settle).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: 'voice-1',
        closeReason: 'livekit_provision_failed',
      }),
    )
    expect(broker.cleanup).toHaveBeenCalledWith(provision)
    expect(fixture.settle.mock.invocationCallOrder[0]).toBeLessThan(
      broker.cleanup.mock.invocationCallOrder[0] ?? 0,
    )
  })

  test('exposes generation-bound worker lifecycle and crash reconciliation seams', async () => {
    const recordLiveKitParticipantJoin = vi.fn(async () => undefined)
    const recordUsage = vi.fn(async () => ({
      chargedMsat: 25,
      reservedMsat: 100,
      creditLimitReached: false,
    }))
    const readLiveKitCleanup = vi.fn(async () => ({
      sessionRef: 'voice-1',
      generation: 1,
      roomRef: 'room-1',
      roomEpoch: 1,
      dispatchRef: 'dispatch-1',
      sarahPresenceLeaseRef: 'presence-1',
    }))
    const markLiveKitCleanup = vi.fn(async () => undefined)
    const claimLiveKitCleanups = vi.fn(async () => [
      {
        sessionRef: 'voice-terminal',
        generation: 2,
        roomRef: 'room-terminal',
        roomEpoch: 1,
        dispatchRef: 'dispatch-terminal',
        sarahPresenceLeaseRef: 'presence-terminal',
        cleanupAttemptedAt: '2026-07-28T12:00:00.000Z',
      },
    ])
    const claimLiveKitProvisioningIntents = vi.fn(async () => [
      {
        sessionRef: 'crashed-voice',
        generation: 3,
        idempotencyKey: 'sarah-livekit:crashed-voice:3',
      },
    ])
    const markLiveKitProvisioningIntent = vi.fn(async () => undefined)
    const settleLiveKitProvisioningIntent = vi.fn(async () => undefined)
    const close = vi.fn(async () => undefined)
    const store = {
      recordLiveKitParticipantJoin,
      recordUsage,
      readLiveKitCleanup,
      claimLiveKitCleanups,
      markLiveKitCleanup,
      claimLiveKitProvisioningIntents,
      markLiveKitProvisioningIntent,
      settleLiveKitProvisioningIntent,
    } as unknown as SarahRealtimeVoiceStore
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'livekit-session-ticket'),
      provision: vi.fn(),
      cleanup: vi.fn(),
      cleanupByIdempotencyKey: vi.fn(async () => undefined),
      cleanupRoom: vi.fn(async () => undefined),
    }
    const dependencies = {
      broker,
      creditMsatPerMillionTokens: 2_000_000,
      now: () => Date.UTC(2026, 6, 28, 12, 0, 0),
      openStore: async () => ({ store, close }),
    }

    await recordSarahLiveKitParticipantJoin(
      dependencies,
      {},
      {
        sessionRef: 'voice-1',
        generation: 1,
        roomRef: 'room-1',
        participantRef: 'participant-1',
        role: 'owner',
      },
    )
    await recordSarahLiveKitProviderUsage(
      dependencies,
      {},
      {
        sessionRef: 'voice-1',
        generation: 1,
        usage: {
          providerResponseRef: 'provider-response-1',
          inputTokens: 1,
          outputTokens: 1,
          cachedInputTokens: 0,
          audioInputTokens: 1,
          audioOutputTokens: 1,
        },
      },
    )
    expect(
      await finalizeSarahLiveKitRoom(
        dependencies,
        {},
        {
          sessionRef: 'voice-1',
          generation: 1,
        },
      ),
    ).toBe(true)
    expect(
      await reconcileSarahLiveKitProvisioningIntents(dependencies, {}),
    ).toEqual({
      cleaned: 1,
      failed: 0,
    })
    expect(await reconcileSarahLiveKitTerminalRooms(dependencies, {})).toEqual({
      cleaned: 1,
      failed: 0,
    })

    expect(recordLiveKitParticipantJoin).toHaveBeenCalledWith(
      expect.objectContaining({
        nowIso: '2026-07-28T12:00:00.000Z',
      }),
    )
    expect(recordUsage).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: 'voice-1',
        generation: 1,
        usage: expect.objectContaining({
          chargeMsat: 4,
          observedAt: '2026-07-28T12:00:00.000Z',
        }),
      }),
    )
    expect(broker.cleanupRoom).toHaveBeenCalledTimes(2)
    expect(markLiveKitCleanup).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionRef: 'voice-terminal',
        state: 'cleaned',
      }),
    )
    expect(broker.cleanupByIdempotencyKey).toHaveBeenCalledWith(
      'sarah-livekit:crashed-voice:3',
    )
    expect(
      settleLiveKitProvisioningIntent.mock.invocationCallOrder[0],
    ).toBeLessThan(
      broker.cleanupByIdempotencyKey.mock.invocationCallOrder[0] ?? 0,
    )
    expect(markLiveKitProvisioningIntent).toHaveBeenCalledWith(
      expect.objectContaining({ state: 'cleaned' }),
    )
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

  test('replays the same unconsumed LiveKit ticket after a lost issuance response', async () => {
    let firstReservation:
      | Awaited<ReturnType<SarahRealtimeVoiceStore['reserve']>>
      | undefined
    const reserve = vi.fn<SarahRealtimeVoiceStore['reserve']>(async input => {
      if (firstReservation === undefined) {
        firstReservation = {
          sessionRef: input.sessionRef,
          ownerUserId: input.ownerUserId,
          ownerActorRef: input.ownerActorRef,
          deviceRef: input.deviceRef,
          threadRef: input.threadRef,
          generation: input.generation,
          disclosureRef: input.disclosureRef,
          clientProfile: input.clientProfile,
          transportKind: input.transportKind ?? 'custom_wss_v1',
          creditMode: input.creditMode,
          entitlementRef: input.entitlementRef,
          admissionCohortRef: input.admissionCohortRef,
          state: 'reserved',
          reservedMsat: input.reservedMsat,
          chargedMsat: 0,
          ticketExpiresAt: input.ticketExpiresAt,
          sessionExpiresAt: input.sessionExpiresAt,
          settlementReceiptRef: null,
          admissionExpiresAt: '2026-07-28T12:02:00.000Z',
          admissionTermsDigest: input.admissionBinding?.termsDigest,
          replayed: false,
        }
        return firstReservation
      }
      return { ...firstReservation, replayed: true }
    })
    const fixture = makeDependencies(reserve)
    const provision = {
      livekitUrl: 'wss://livekit.openagents.test',
      roomRef: 'sarah-room:voice-1:g1',
      roomEpoch: 1,
      participantRef: 'participant:user-1:voice-1:g1',
      sarahParticipantRef: 'participant:sarah:voice-1:g1',
      participantGrant: 'opaque-livekit-participant-grant',
      joinExpiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
      dispatchRef: 'dispatch:voice-1:g1',
      sarahPresenceLeaseRef: 'sarah-presence:voice-1:g1',
      grantClaims: {
        roomRef: 'sarah-room:voice-1:g1',
        participantRef: 'participant:user-1:voice-1:g1',
        expiresAtMs: Date.UTC(2026, 6, 28, 12, 1, 0),
        roomJoin: true,
        canPublish: true,
        canSubscribe: true,
        canPublishData: false,
        canUpdateOwnMetadata: false,
        canPublishSources: ['microphone'],
        roomAdmin: false,
        roomCreate: false,
        roomList: false,
      },
    } as const
    const broker = {
      workerControlTokenDigest: vi.fn(() => 'b'.repeat(64)),
      sessionTicket: vi.fn(() => 'stable-livekit-session-ticket'),
      provision: vi.fn(async () => provision),
      cleanup: vi.fn(async () => undefined),
      cleanupByIdempotencyKey: vi.fn(async () => undefined),
      cleanupRoom: vi.fn(async () => undefined),
    }
    const issue = () =>
      handleSarahRealtimeVoiceSessionRequest(
        { ...fixture.dependencies, liveKitRoomBroker: broker },
        request({
          schema: SARAH_VOICE_PROTOCOL_VERSION,
          identity,
          disclosureRef: 'disclosure-1',
          requestedTransport: 'livekit_room_v1',
          roomContext: { kind: 'private' },
        }),
        {},
        ctx,
      )

    const firstResponse = await issue()
    const replayResponse = await issue()
    expect(firstResponse.status).toBe(201)
    expect(replayResponse.status).toBe(201)
    const firstBody = (await firstResponse.json()) as {
      ticket: string
      ticketExpiresAtMs: number
      sessionExpiresAtMs: number
    }
    const replayBody = (await replayResponse.json()) as typeof firstBody
    expect(replayBody).toEqual(
      expect.objectContaining({
        ticket: firstBody.ticket,
        ticketExpiresAtMs: firstBody.ticketExpiresAtMs,
        sessionExpiresAtMs: firstBody.sessionExpiresAtMs,
      }),
    )
    expect(reserve).toHaveBeenCalledTimes(2)
    expect(reserve.mock.calls[0]?.[0].ticketDigest).toBe(
      reserve.mock.calls[1]?.[0].ticketDigest,
    )
    expect(broker.provision).toHaveBeenCalledTimes(2)
    expect(fixture.settle).not.toHaveBeenCalled()
  })

  test('rejects a revoked alpha member before reserving credit', async () => {
    const fixture = makeDependencies()
    vi.mocked(fixture.store.readActiveAlphaMembership).mockResolvedValue(
      undefined,
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
    expect(response.status).toBe(403)
    expect(await response.json()).toEqual({ error: 'sarah_voice_not_entitled' })
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('audits a bounded storage failure before returning unavailable', async () => {
    const fixture = makeDependencies(
      vi.fn(async () => {
        throw new Error('reservation failed')
      }),
    )
    const audit = vi.fn()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      { ...fixture.dependencies, audit },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_storage_unavailable',
    })
    expect(audit).toHaveBeenCalledWith('storage_unavailable', {
      errorMessage: 'reservation failed',
      errorName: 'Error',
    })
    expect(fixture.close).toHaveBeenCalledOnce()
  })

  test('waives only the staging owner credit hold', async () => {
    const fixture = makeDependencies(
      undefined,
      vi.fn(async input => ({
        entitlementRef: input.entitlementRef,
        ownerUserId: input.ownerUserId,
        expiresAt: input.expiresAt,
      })),
    )
    const audit = vi.fn()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        audit,
        stagingOwnerEntitlementEnabled: () => true,
      },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toMatchObject({ reservedCreditMsat: 0 })
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        creditMode: 'staging_owner_entitlement',
        entitlementRef: 'sarah_voice_entitlement:staging_owner_v1',
        reservedMsat: 0,
      }),
    )
    expect(audit).toHaveBeenCalledWith(
      'staging_owner_entitlement_applied',
      expect.objectContaining({
        entitlementRef: 'sarah_voice_entitlement:staging_owner_v1',
        ownerDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
      }),
    )
  })

  test('keeps an unknown account on the normal credit gate', async () => {
    const fixture = makeDependencies(
      vi.fn(async () => {
        throw new SarahVoiceInsufficientCreditError('insufficient')
      }),
    )
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        stagingOwnerEntitlementEnabled: () => true,
      },
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
    expect(fixture.readActiveStagingOwnerEntitlement).toHaveBeenCalledWith({
      entitlementRef: 'sarah_voice_entitlement:staging_owner_v1',
      nowIso: '2026-07-28T12:00:00.000Z',
      ownerUserId: 'user-1',
    })
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        creditMode: 'metered',
        entitlementRef: null,
        reservedMsat: 25_000,
      }),
    )
  })

  test('does not read the staging entitlement when the server gate is off', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceSessionRequest(
      {
        ...fixture.dependencies,
        stagingOwnerEntitlementEnabled: () => false,
      },
      request({
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        identity,
        disclosureRef: 'disclosure-1',
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(201)
    expect(fixture.readActiveStagingOwnerEntitlement).not.toHaveBeenCalled()
    expect(fixture.reserve).toHaveBeenCalledWith(
      expect.objectContaining({
        creditMode: 'metered',
        entitlementRef: null,
        reservedMsat: 25_000,
      }),
    )
  })

  test('exchanges a body-bound NIP-98 proof and one-use challenge for the normal ticket and bearer session', async () => {
    const fixture = makeDependencies()
    const body = {
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      disclosureRef: 'disclosure-1',
      admissionRef: 'sarah_voice_admission:test',
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

describe('managed Sarah Realtime voice admission and closeout routes', () => {
  const admissionBody = {
    schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
    identity,
    disclosureRef: 'disclosure-1',
    clientProfile: 'omega_editor',
  } as const

  test('returns exact admission economics and capabilities without reserving credit', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceAdmissionRequest(
      fixture.dependencies,
      new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Bearer test',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        body: JSON.stringify(admissionBody),
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
      admitted: true,
      clientProfile: 'omega_editor',
      admissionCohortRef: 'sarah_voice_cohort:alpha_v1',
      creditMode: 'metered',
      creditRateMsatPerMillionTokens: 100_000,
      requiredHoldMsat: 25_000,
      spendableRemainingCreditMsat: 100_000,
      maxDurationSeconds: 600,
      capabilityBoundary: {
        commands: [
          'context_read',
          'reveal_range',
          'replace_selection',
          'save_document',
          'start_agent_thread',
        ],
        confirmationRequired: [
          'replace_selection',
          'save_document',
          'start_agent_thread',
        ],
        directShell: false,
        directGit: false,
        payment: false,
        credentialAccess: false,
        deviceControl: false,
      },
      admissionRef: expect.stringMatching(
        /^sarah_voice_admission:[A-Za-z0-9_-]{32,}$/u,
      ),
      admissionExpiresAtMs: Date.UTC(2026, 6, 28, 12, 2, 0),
    })
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('blocks new LiveKit admissions at the emergency rollback seam', async () => {
    const fixture = makeDependencies()
    const response = await handleSarahRealtimeVoiceAdmissionRequest(
      {
        ...fixture.dependencies,
        liveKitNewAdmissionsEnabled: () => false,
      },
      new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Bearer test',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        body: JSON.stringify({
          ...admissionBody,
          requestedTransport: 'livekit_room_v1',
          roomContext: { kind: 'private' },
        }),
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(503)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_livekit_admissions_disabled',
    })
    expect(fixture.issueAdmission).not.toHaveBeenCalled()
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test.each([
    {
      membershipActive: false,
      spendableMsat: 100_000,
      reason: 'cohort_inactive',
    },
    {
      membershipActive: true,
      spendableMsat: 10_000,
      reason: 'insufficient_credit',
    },
  ] as const)(
    'reports $reason without creating a reservation',
    async fixtureCase => {
      const fixture = makeDependencies()
      vi.mocked(fixture.store.readActiveAlphaMembership).mockResolvedValue(
        fixtureCase.membershipActive
          ? {
              membershipRef: 'sarah_voice_alpha:user-1',
              cohortRef: 'sarah_voice_cohort:alpha_v1',
              ownerUserId: 'user-1',
            }
          : undefined,
      )
      vi.mocked(fixture.store.readSpendableCredit).mockResolvedValue(
        fixtureCase.spendableMsat,
      )
      const response = await handleSarahRealtimeVoiceAdmissionRequest(
        fixture.dependencies,
        new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
          method: 'POST',
          headers: {
            authorization: 'Bearer test',
            [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
          },
          body: JSON.stringify(admissionBody),
        }),
        {},
        ctx,
      )
      expect(await response.json()).toMatchObject({
        admitted: false,
        refusalReason: fixtureCase.reason,
        spendableRemainingCreditMsat: fixtureCase.spendableMsat,
      })
      expect(fixture.reserve).not.toHaveBeenCalled()
    },
  )

  test('authenticates NIP-98 admission and rejects a replayed or expired challenge', async () => {
    const fixture = makeDependencies()
    const body = {
      ...admissionBody,
      auth: {
        method: 'nostr_nip98',
        challenge: 'challenge_abcdefghijklmnopqrstuvwxyz012345',
      },
    } as const
    const dependencies = {
      ...fixture.dependencies,
      authenticateNostrSession: vi.fn(async () => ({
        _tag: 'Authenticated' as const,
        pubkey: 'a'.repeat(64),
        user: { userId: 'user-1' },
      })),
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
    const response = await handleSarahRealtimeVoiceAdmissionRequest(
      dependencies,
      new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Nostr signed-event',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        body: JSON.stringify(body),
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(409)
    expect(await response.json()).toEqual({
      error: 'sarah_voice_auth_challenge_invalid',
    })
    expect(dependencies.authenticateNostrSession).not.toHaveBeenCalled()
    expect(dependencies.mintNostrSession).not.toHaveBeenCalled()
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('authenticates NIP-98 admission and keeps it bound to the request device', async () => {
    const fixture = makeDependencies()
    const nostrBody = {
      ...admissionBody,
      auth: {
        method: 'nostr_nip98',
        challenge: 'challenge_abcdefghijklmnopqrstuvwxyz012345',
      },
    } as const
    const dependencies = {
      ...fixture.dependencies,
      authenticateNostrSession: vi.fn(async () => ({
        _tag: 'Authenticated' as const,
        pubkey: 'a'.repeat(64),
        user: { userId: 'user-1' },
      })),
      consumeNostrChallenge: vi.fn(async () => 'Consumed' as const),
      mintNostrSession: vi.fn(async () => ({
        _tag: 'Issued' as const,
        accessToken: `oa_omega_${'a'.repeat(43)}`,
        expiresIn: 900,
        user: { userId: 'user-1' },
      })),
      verifyNostrProof: vi.fn(
        async (_request: Request, _env: unknown, payload: Uint8Array) => {
          expect(new TextDecoder().decode(payload)).toBe(
            JSON.stringify(nostrBody),
          )
          return {
            _tag: 'Verified' as const,
            eventId: 'b'.repeat(64),
            isOwner: false,
            pubkey: 'a'.repeat(64),
            pubkeyDigest: 'c'.repeat(64),
          }
        },
      ),
    }
    const admitted = await handleSarahRealtimeVoiceAdmissionRequest(
      dependencies,
      new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Nostr signed-event',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: identity.deviceRef,
        },
        body: JSON.stringify(nostrBody),
      }),
      {},
      ctx,
    )
    expect(admitted.status).toBe(200)
    expect(await admitted.json()).toMatchObject({
      admitted: true,
      auth: {
        method: 'nostr_nip98',
        accessToken: `oa_omega_${'a'.repeat(43)}`,
        expiresIn: 900,
      },
    })

    const response = await handleSarahRealtimeVoiceAdmissionRequest(
      dependencies,
      new Request(`https://openagents.com${SARAH_VOICE_ADMISSION_PATH}`, {
        method: 'POST',
        headers: {
          authorization: 'Nostr signed-event',
          [SARAH_REALTIME_VOICE_DEVICE_HEADER]: 'different-device',
        },
        body: JSON.stringify(nostrBody),
      }),
      {},
      ctx,
    )

    expect(response.status).toBe(403)
    expect(fixture.reserve).not.toHaveBeenCalled()
  })

  test('returns owner-scoped settlement evidence', async () => {
    const fixture = makeDependencies()
    vi.mocked(fixture.store.readSettlement).mockResolvedValue({
      sessionRef: identity.sessionRef,
      state: 'settled',
      creditMode: 'metered',
      finalChargeMsat: 750,
      spendableRemainingCreditMsat: 99_250,
      settlementReceiptRef: 'sarah_voice_settlement:voice-1',
    })
    const response = await handleSarahRealtimeVoiceSettlementRequest(
      fixture.dependencies,
      new Request('https://openagents.com/api/omega/sarah/voice/settlement', {
        headers: {
          authorization: 'Bearer test',
          [SARAH_REALTIME_VOICE_SESSION_HEADER]: identity.sessionRef,
        },
      }),
      {},
      ctx,
    )
    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
      sessionRef: identity.sessionRef,
      state: 'settled',
      creditMode: 'metered',
      finalChargeMsat: 750,
      spendableRemainingCreditMsat: 99_250,
      receiptRef: 'sarah_voice_settlement:voice-1',
    })
  })

  test('lets an operator revoke only the fixed alpha cohort', async () => {
    const fixture = makeDependencies()
    vi.mocked(fixture.store.revokeAlphaCohort).mockResolvedValue(1)
    const response = await handleSarahRealtimeVoiceCohortRevocationRequest(
      {
        openStore: fixture.dependencies.openStore,
        requireOperator: async () => ({ actorRef: 'operator:user-admin' }),
        now: fixture.dependencies.now,
      },
      new Request(
        'https://openagents.com/api/operator/omega/sarah/voice/cohort/revoke',
        {
          method: 'POST',
          headers: { authorization: 'Bearer admin' },
          body: JSON.stringify({
            schema: SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
            cohortRef: 'sarah_voice_cohort:alpha_v1',
            reason: 'End alpha access',
          }),
        },
      ),
      {},
      ctx,
    )
    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      schema: SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
      cohortRef: 'sarah_voice_cohort:alpha_v1',
      state: 'revoked',
      revokedCount: 1,
    })
  })
})
