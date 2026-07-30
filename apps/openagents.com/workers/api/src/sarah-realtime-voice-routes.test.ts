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
  SarahVoiceInsufficientCreditError,
} from '@openagentsinc/khala-sync-server'
import { describe, expect, test, vi } from 'vitest'

import {
  SARAH_REALTIME_VOICE_DEVICE_HEADER,
  SARAH_REALTIME_VOICE_SESSION_HEADER,
  handleSarahRealtimeVoiceAdmissionRequest,
  handleSarahRealtimeVoiceCohortRevocationRequest,
  handleSarahRealtimeVoiceSessionRequest,
  handleSarahRealtimeVoiceSettlementRequest,
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
    creditMode: input.creditMode,
    entitlementRef: input.entitlementRef,
    admissionCohortRef: input.admissionCohortRef,
    state: 'reserved' as const,
    reservedMsat: input.reservedMsat,
    chargedMsat: 0,
    ticketExpiresAt: input.ticketExpiresAt,
    sessionExpiresAt: input.sessionExpiresAt,
    settlementReceiptRef: null,
  })),
  readActiveStagingOwnerEntitlement: SarahRealtimeVoiceStore['readActiveStagingOwnerEntitlement'] = vi.fn(
    async () => undefined,
  ),
) => {
  const close = vi.fn(async () => undefined)
  const store = {
    reserve,
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
    settle: vi.fn(),
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
      now: () => Date.UTC(2026, 6, 28, 12, 0, 0),
    },
    reserve,
    readActiveStagingOwnerEntitlement,
    store,
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
    })
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
