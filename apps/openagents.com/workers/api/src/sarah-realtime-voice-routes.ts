import {
  SARAH_VOICE_ACCOUNTING_RECONCILIATION_PATH,
  SARAH_VOICE_ACCOUNTING_RECONCILIATION_PROTOCOL_VERSION,
  SARAH_VOICE_ADMISSION_PATH,
  SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
  SARAH_VOICE_ALPHA_COHORT_REF,
  SARAH_VOICE_COHORT_REVOCATION_PATH,
  SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
  SARAH_VOICE_CONNECT_PATH,
  SARAH_VOICE_MODEL,
  SARAH_VOICE_NOSTR_AUTH_METHOD,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
  SARAH_VOICE_SETTLEMENT_PATH,
  SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
  SARAH_VOICE_STAGING_OWNER_COHORT_REF,
  decodeSarahVoiceAccountingReconciliationRequest,
  decodeSarahVoiceAdmissionRequest,
  decodeSarahVoiceCohortRevocationRequest,
  decodeSarahVoiceSessionRequest,
} from '@openagentsinc/audio-contract'
import {
  type SarahRealtimeVoiceStore,
  SarahVoiceAdmissionRejectedError,
  SarahVoiceConcurrentSessionError,
  SarahVoiceDuplicateParticipantError,
  SarahVoiceInsufficientCreditError,
  SarahVoiceSessionRejectedError,
  type SarahVoiceUsage,
} from '@openagentsinc/khala-sync-server'

export const SARAH_REALTIME_VOICE_DEVICE_HEADER =
  'x-openagents-omega-device-ref'
export const SARAH_REALTIME_VOICE_SESSION_PATH = SARAH_VOICE_SESSION_PATH
export const SARAH_REALTIME_VOICE_ADMISSION_PATH = SARAH_VOICE_ADMISSION_PATH
export const SARAH_REALTIME_VOICE_SETTLEMENT_PATH = SARAH_VOICE_SETTLEMENT_PATH
export const SARAH_REALTIME_VOICE_COHORT_REVOCATION_PATH =
  SARAH_VOICE_COHORT_REVOCATION_PATH
export const SARAH_REALTIME_VOICE_ACCOUNTING_RECONCILIATION_PATH =
  SARAH_VOICE_ACCOUNTING_RECONCILIATION_PATH
export const SARAH_REALTIME_VOICE_SESSION_HEADER =
  'x-openagents-sarah-voice-session'
export const SARAH_REALTIME_VOICE_TICKET_HEADER =
  'x-openagents-sarah-voice-ticket'
export const SARAH_LIVEKIT_ACCEPTANCE_EVIDENCE_HEADER =
  'x-openagents-sarah-livekit-acceptance'

type UserBearerSessionBoundary<User, Bindings> = (
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => Promise<Readonly<{ user: User }> | undefined>

type NostrAuthentication<User> = Readonly<{
  _tag: 'Authenticated'
  pubkey: string
  user: User
}>

type NostrVerifiedProof = Readonly<{
  _tag: 'Verified'
  eventId: string
  isOwner: boolean
  pubkey: string
  pubkeyDigest: string
}>

type NostrSessionAuthenticate<User, Bindings> = (
  request: Request,
  env: Bindings,
  verified: NostrVerifiedProof,
) => Promise<
  NostrAuthentication<User> | Readonly<{ _tag: 'Rejected'; response: Response }>
>

type NostrProofVerify<Bindings> = (
  request: Request,
  env: Bindings,
  payload: Uint8Array,
) => Promise<
  NostrVerifiedProof | Readonly<{ _tag: 'Rejected'; response: Response }>
>

type NostrSessionMint<User, Bindings> = (
  env: Bindings,
  authenticated: NostrAuthentication<User>,
) => Promise<
  | Readonly<{
      _tag: 'Issued'
      accessToken: string
      expiresIn: number
      user: User
    }>
  | Readonly<{ _tag: 'Rejected'; response: Response }>
>

export type SarahRealtimeVoiceRouteConfig = Readonly<{
  enabled: boolean
  creditMsatPerMillionTokens: number
  maxSessionSeconds: number
  reservationMsat: number
}>

export type SarahVoiceLiveKitCommunityAccess = Readonly<{
  communityRef: string
  channelRef: string
  membershipRevision: string
  memberPubkey: string
  role: 'member' | 'moderator'
  publishAllowed: boolean
  subscribeAllowed: boolean
}>

export type SarahVoiceLiveKitProvision = Readonly<{
  livekitUrl: string
  roomRef: string
  roomEpoch: number
  participantRef: string
  sarahParticipantRef: string
  participantGrant: string
  joinExpiresAtMs: number
  dispatchRef: string
  sarahPresenceLeaseRef: string
  grantClaims: Readonly<{
    roomRef: string
    participantRef: string
    expiresAtMs: number
    roomJoin: true
    canPublish: boolean
    canSubscribe: boolean
    canPublishData: false
    canUpdateOwnMetadata: false
    canPublishSources: readonly ['microphone']
    roomAdmin: false
    roomCreate: false
    roomList: false
  }>
}>

export type SarahVoiceLiveKitProvisionInput = Readonly<{
  idempotencyKey: string
  ownerUserId: string
  deviceRef: string
  threadRef: string
  sessionRef: string
  generation: number
  capabilityProfile: string
  admissionRef: string
  admissionDigest: string
  roomContext:
    | Readonly<{ kind: 'private' }>
    | (SarahVoiceLiveKitCommunityAccess & Readonly<{ kind: 'community' }>)
  publishAllowed: boolean
  subscribeAllowed: boolean
  expiresAtMs: number
}>

export type SarahVoiceLiveKitRoomBroker = Readonly<{
  workerControlTokenDigest: (input: SarahVoiceLiveKitProvisionInput) => string
  sessionTicket: (input: SarahVoiceLiveKitProvisionInput) => string
  provision: (
    input: SarahVoiceLiveKitProvisionInput,
  ) => Promise<SarahVoiceLiveKitProvision>
  cleanup: (provision: SarahVoiceLiveKitProvision) => Promise<void>
  cleanupByIdempotencyKey: (idempotencyKey: string) => Promise<void>
  cleanupRoom: (
    room: Readonly<{
      sessionRef: string
      generation: number
      roomRef: string
      roomEpoch: number
      dispatchRef: string
      sarahPresenceLeaseRef: string
    }>,
  ) => Promise<void>
  interrupt?: (
    input: Readonly<{
      sessionRef: string
      generation: number
      roomRef: string
      roomEpoch: number
      sarahParticipantRef: string
      interruptSequence: number
    }>,
  ) => Promise<void>
  grantParticipant?: (
    input: Readonly<{
      roomRef: string
      participantRef: string
      expiresAtMs: number
      publishAllowed: boolean
      subscribeAllowed: boolean
    }>,
  ) => Promise<Readonly<{ participantGrant: string; joinExpiresAtMs: number }>>
}>

export type SarahVoiceLiveKitLifecycleDependencies<Bindings> = Readonly<{
  broker: SarahVoiceLiveKitRoomBroker
  creditMsatPerMillionTokens: number
  now?: (() => number) | undefined
  openStore: (
    env: Bindings,
  ) => Promise<
    Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
  >
}>

/**
 * Record the first admission of a dispatched participant into its bound room.
 *
 * Called from the worker event route the moment the worker reports the
 * provider admitted its session, which the agent only sends after the
 * dispatched participant is present in the LiveKit room. The store refuses a
 * second admission of the same identity with
 * `SarahVoiceDuplicateParticipantError`.
 *
 * It takes only the store seam, not the whole LiveKit lifecycle dependency, so
 * the worker route can call it without holding a room broker.
 */
export const recordSarahLiveKitParticipantJoin = async <Bindings>(
  dependencies: Readonly<{
    now?: (() => number) | undefined
    openStore: (
      env: Bindings,
    ) => Promise<
      Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
    >
  }>,
  env: Bindings,
  input: Omit<
    Parameters<SarahRealtimeVoiceStore['recordLiveKitParticipantJoin']>[0],
    'nowIso'
  >,
): Promise<void> => {
  const opened = await dependencies.openStore(env)
  try {
    await opened.store.recordLiveKitParticipantJoin({
      ...input,
      nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
    })
  } finally {
    await opened.close()
  }
}

export const recordSarahLiveKitProviderUsage = async <Bindings>(
  dependencies: SarahVoiceLiveKitLifecycleDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    sessionRef: string
    generation: number
    usage: Omit<SarahVoiceUsage, 'chargeMsat' | 'observedAt'>
  }>,
) => {
  const opened = await dependencies.openStore(env)
  try {
    return await opened.store.recordUsage({
      sessionRef: input.sessionRef,
      generation: input.generation,
      usage: {
        ...input.usage,
        chargeMsat: Math.ceil(
          ((input.usage.inputTokens + input.usage.outputTokens) *
            dependencies.creditMsatPerMillionTokens) /
            1_000_000,
        ),
        observedAt: new Date((dependencies.now ?? Date.now)()).toISOString(),
      },
    })
  } finally {
    await opened.close()
  }
}

export const finalizeSarahLiveKitRoom = async <Bindings>(
  dependencies: SarahVoiceLiveKitLifecycleDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{ sessionRef: string; generation: number }>,
): Promise<boolean> => {
  const opened = await dependencies.openStore(env)
  try {
    const cleanup = await opened.store.readLiveKitCleanup(input)
    if (cleanup === undefined) return false
    const nowIso = new Date((dependencies.now ?? Date.now)()).toISOString()
    try {
      await dependencies.broker.cleanupRoom(cleanup)
      await opened.store.markLiveKitCleanup({
        ...input,
        state: 'cleaned',
        nowIso,
      })
    } catch (error) {
      await opened.store.markLiveKitCleanup({
        ...input,
        state: 'cleanup_failed',
        nowIso,
      })
      throw error
    }
    return true
  } finally {
    await opened.close()
  }
}

export const reconcileSarahLiveKitTerminalRooms = async <Bindings>(
  dependencies: SarahVoiceLiveKitLifecycleDependencies<Bindings>,
  env: Bindings,
): Promise<
  Readonly<{ cleaned: number; failed: number; abandoned: number }>
> => {
  const opened = await dependencies.openStore(env)
  let cleaned = 0
  let failed = 0
  let abandoned = 0
  try {
    const nowMs = (dependencies.now ?? Date.now)()
    const nowIso = new Date(nowMs).toISOString()
    const cleanups = await opened.store.claimLiveKitCleanups({
      staleBeforeIso: new Date(nowMs - 15_000).toISOString(),
      nowIso,
      limit: 100,
    })
    for (const cleanup of cleanups) {
      try {
        // eslint-disable-next-line no-await-in-loop
        await dependencies.broker.cleanupRoom(cleanup)
        // eslint-disable-next-line no-await-in-loop
        await opened.store.markLiveKitCleanup({
          sessionRef: cleanup.sessionRef,
          generation: cleanup.generation,
          state: 'cleaned',
          nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
        })
        cleaned += 1
      } catch {
        // eslint-disable-next-line no-await-in-loop
        const outcome = await opened.store.markLiveKitCleanup({
          sessionRef: cleanup.sessionRef,
          generation: cleanup.generation,
          state: 'cleanup_failed',
          nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
        })
        // A spent attempt budget retires the row instead of re-queueing it.
        if (outcome.state === 'cleanup_abandoned') abandoned += 1
        else failed += 1
      }
    }
    return { cleaned, failed, abandoned }
  } finally {
    await opened.close()
  }
}

export const reconcileSarahLiveKitProvisioningIntents = async <Bindings>(
  dependencies: SarahVoiceLiveKitLifecycleDependencies<Bindings>,
  env: Bindings,
): Promise<
  Readonly<{ cleaned: number; failed: number; abandoned: number }>
> => {
  const opened = await dependencies.openStore(env)
  let cleaned = 0
  let failed = 0
  let abandoned = 0
  try {
    const nowMs = (dependencies.now ?? Date.now)()
    const provisioningOwnerRef = `sarah-livekit-reconciler:${crypto.randomUUID()}`
    const intents = await opened.store.claimLiveKitProvisioningIntents({
      staleBeforeIso: new Date(
        nowMs - SARAH_VOICE_ADMISSION_LIFETIME_MS,
      ).toISOString(),
      nowIso: new Date(nowMs).toISOString(),
      provisioningOwnerRef,
      limit: 100,
    })
    for (const intent of intents) {
      let state: 'cleaned' | 'cleanup_failed' = 'cleaned'
      try {
        // eslint-disable-next-line no-await-in-loop
        await opened.store.settleLiveKitProvisioningIntent({
          sessionRef: intent.sessionRef,
          generation: intent.generation,
          provisioningOwnerRef: intent.provisioningOwnerRef,
          closeReason: 'livekit_provisioning_reconcile',
          nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
        })
      } catch {
        // eslint-disable-next-line no-await-in-loop
        const outcome = await opened.store.markLiveKitProvisioningIntent({
          sessionRef: intent.sessionRef,
          generation: intent.generation,
          provisioningOwnerRef: intent.provisioningOwnerRef,
          state: 'cleanup_failed',
          nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
        })
        // A spent attempt budget retires the intent instead of re-queueing it.
        if (outcome.state === 'cleanup_abandoned') abandoned += 1
        else failed += 1
        continue
      }
      try {
        // eslint-disable-next-line no-await-in-loop
        await dependencies.broker.cleanupByIdempotencyKey(intent.idempotencyKey)
      } catch {
        state = 'cleanup_failed'
      }
      // eslint-disable-next-line no-await-in-loop
      const outcome = await opened.store.markLiveKitProvisioningIntent({
        sessionRef: intent.sessionRef,
        generation: intent.generation,
        provisioningOwnerRef: intent.provisioningOwnerRef,
        state,
        nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
      })
      if (state === 'cleaned') cleaned += 1
      else if (outcome.state === 'cleanup_abandoned') abandoned += 1
      else failed += 1
    }
    return { cleaned, failed, abandoned }
  } finally {
    await opened.close()
  }
}

export type SarahRealtimeVoiceOperatorRouteDependencies<Bindings> = Readonly<{
  now?: (() => number) | undefined
  openStore: (
    env: Bindings,
  ) => Promise<
    Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
  >
  requireOperator: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ actorRef: string }> | undefined>
}>

export type SarahRealtimeVoiceRouteDependencies<User, Bindings> = Readonly<{
  audit?: (
    event:
      | 'staging_owner_entitlement_applied'
      | 'staging_owner_entitlement_inactive'
      | 'storage_unavailable',
    fields: Readonly<Record<string, string>>,
  ) => void
  config: (env: Bindings) => SarahRealtimeVoiceRouteConfig | undefined
  openStore: (
    env: Bindings,
  ) => Promise<
    Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
  >
  authenticateNostrSession?:
    NostrSessionAuthenticate<User, Bindings> | undefined
  consumeNostrChallenge?: (
    env: Bindings,
    input: Readonly<{
      challenge: string
      deviceRef: string
      ownerRef: string
      pubkey: string
    }>,
  ) => Promise<'Consumed' | 'Invalid' | 'Unavailable'>
  mintNostrSession?: NostrSessionMint<User, Bindings> | undefined
  liveKitRoomBroker?: SarahVoiceLiveKitRoomBroker | undefined
  resolveLiveKitCommunityAccess?: (
    env: Bindings,
    input: Readonly<{
      ownerUserId: string
      communityRef: string
      channelRef: string
    }>,
  ) => Promise<SarahVoiceLiveKitCommunityAccess | undefined>
  bootstrapLiveKitCommunityRoom?: (
    env: Bindings,
    input: Readonly<{
      ownerUserId: string
      presenceLeaseRef: string
      communityAccess: SarahVoiceLiveKitCommunityAccess
    }>,
  ) => Promise<void>
  requireUserBearerSession: UserBearerSessionBoundary<User, Bindings>
  stagingOwnerEntitlementEnabled?: (env: Bindings) => boolean
  liveKitNewAdmissionsEnabled?: (env: Bindings) => boolean
  waitForLiveKitWorkerClaim?: (
    store: SarahRealtimeVoiceStore,
    input: Readonly<{ sessionRef: string; generation: number }>,
  ) => Promise<boolean>
  userIdFromSession: (session: Readonly<{ user: User }>) => string
  verifyNostrProof?: NostrProofVerify<Bindings> | undefined
  now?: (() => number) | undefined
}>

const noStoreJson = (body: unknown, status: number): Response =>
  Response.json(body, {
    status,
    headers: {
      'cache-control': 'no-store',
      'content-type': 'application/json; charset=utf-8',
    },
  })

const base64Url = (bytes: Uint8Array): string => {
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/u, '')
}

export const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const makeTicket = (): string =>
  base64Url(crypto.getRandomValues(new Uint8Array(32)))

const SARAH_VOICE_ADMISSION_LIFETIME_MS = 120_000
const SARAH_LIVEKIT_PROVISIONING_LEASE_MS = 30_000

const makeAdmissionRef = (): string =>
  `sarah_voice_admission:${base64Url(crypto.getRandomValues(new Uint8Array(32)))}`

const admissionTermsDigest = async (
  terms: Readonly<{
    clientProfile: string
    admissionCohortRef: string
    creditMode: string
    creditRateMsatPerMillionTokens: number
    requiredHoldMsat: number
    spendableRemainingCreditMsat: number | null
    maxDurationSeconds: number
    capabilityBoundary: ReturnType<typeof capabilityBoundaryForProfile>
    transportKind: 'custom_wss_v1' | 'livekit_room_v1'
    roomContext:
      | Readonly<{ kind: 'private' }>
      | (SarahVoiceLiveKitCommunityAccess & Readonly<{ kind: 'community' }>)
  }>,
): Promise<string> => sha256Hex(JSON.stringify(terms))

const parseRequest = async (request: Request) => {
  const contentLength = Number(request.headers.get('content-length') ?? '0')
  if (!Number.isFinite(contentLength) || contentLength > 8_192) return undefined
  try {
    const text = await request.text()
    const payload = new TextEncoder().encode(text)
    if (payload.byteLength > 8_192) return undefined
    return {
      body: decodeSarahVoiceSessionRequest(JSON.parse(text) as unknown),
      payload,
    }
  } catch {
    return undefined
  }
}

const parseAdmissionRequest = async (request: Request) => {
  const contentLength = Number(request.headers.get('content-length') ?? '0')
  if (!Number.isFinite(contentLength) || contentLength > 8_192) return undefined
  try {
    const text = await request.text()
    const payload = new TextEncoder().encode(text)
    if (payload.byteLength > 8_192) return undefined
    return {
      body: decodeSarahVoiceAdmissionRequest(JSON.parse(text) as unknown),
      payload,
    }
  } catch {
    return undefined
  }
}

const parseBoundedJson = async <A>(
  request: Request,
  decode: (value: unknown) => A,
): Promise<A | undefined> => {
  const contentLength = Number(request.headers.get('content-length') ?? '0')
  if (!Number.isFinite(contentLength) || contentLength > 8_192) return undefined
  try {
    const text = await request.text()
    if (new TextEncoder().encode(text).byteLength > 8_192) return undefined
    return decode(JSON.parse(text) as unknown)
  } catch {
    return undefined
  }
}

const gatewayUrlForRequest = (request: Request): string | undefined => {
  try {
    const source = new URL(request.url)
    if (
      !['http:', 'https:'].includes(source.protocol) ||
      source.username !== '' ||
      source.password !== ''
    ) {
      return undefined
    }
    source.protocol = source.protocol === 'https:' ? 'wss:' : 'ws:'
    source.pathname = SARAH_VOICE_CONNECT_PATH
    source.search = ''
    source.hash = ''
    return source.toString()
  } catch {
    return undefined
  }
}

const validLiveKitProvision = (
  provision: SarahVoiceLiveKitProvision,
  expected: Readonly<{
    nowMs: number
    maximumExpiresAtMs: number
    publishAllowed: boolean
    subscribeAllowed: boolean
  }>,
): boolean => {
  let url: URL
  try {
    url = new URL(provision.livekitUrl)
  } catch {
    return false
  }
  const validRef = (value: string): boolean =>
    value.trim() === value && value.length > 0 && value.length <= 256
  const grantClaimKeys = Object.keys(provision.grantClaims).sort().join(',')
  return (
    url.protocol === 'wss:' &&
    url.username === '' &&
    url.password === '' &&
    provision.livekitUrl.trim() === provision.livekitUrl &&
    provision.livekitUrl.length <= 2_048 &&
    Number.isSafeInteger(provision.roomEpoch) &&
    provision.roomEpoch >= 1 &&
    Number.isSafeInteger(provision.joinExpiresAtMs) &&
    provision.joinExpiresAtMs > expected.nowMs &&
    provision.joinExpiresAtMs <= expected.maximumExpiresAtMs &&
    provision.participantGrant.length > 0 &&
    provision.participantGrant.length <= 4_096 &&
    validRef(provision.roomRef) &&
    validRef(provision.participantRef) &&
    validRef(provision.sarahParticipantRef) &&
    provision.participantRef !== provision.sarahParticipantRef &&
    validRef(provision.dispatchRef) &&
    validRef(provision.sarahPresenceLeaseRef) &&
    provision.grantClaims.roomRef === provision.roomRef &&
    provision.grantClaims.participantRef === provision.participantRef &&
    provision.grantClaims.expiresAtMs === provision.joinExpiresAtMs &&
    grantClaimKeys ===
      [
        'canPublish',
        'canPublishData',
        'canPublishSources',
        'canSubscribe',
        'canUpdateOwnMetadata',
        'expiresAtMs',
        'participantRef',
        'roomAdmin',
        'roomCreate',
        'roomJoin',
        'roomList',
        'roomRef',
      ]
        .sort()
        .join(',') &&
    provision.grantClaims.roomJoin === true &&
    provision.grantClaims.canPublish === expected.publishAllowed &&
    provision.grantClaims.canSubscribe === expected.subscribeAllowed &&
    provision.grantClaims.canPublishData === false &&
    provision.grantClaims.canUpdateOwnMetadata === false &&
    provision.grantClaims.canPublishSources.length === 1 &&
    provision.grantClaims.canPublishSources[0] === 'microphone' &&
    provision.grantClaims.roomAdmin === false &&
    provision.grantClaims.roomCreate === false &&
    provision.grantClaims.roomList === false
  )
}

export const waitForSarahLiveKitWorkerClaim = async (
  store: SarahRealtimeVoiceStore,
  input: Readonly<{ sessionRef: string; generation: number }>,
  wait: () => Promise<void> = () =>
    new Promise(resolve => {
      setTimeout(resolve, 100)
    }),
  maximumChecks = 50,
): Promise<boolean> => {
  for (let check = 0; check < maximumChecks; check += 1) {
    // eslint-disable-next-line no-await-in-loop
    const readiness = await store.readLiveKitWorkerReadiness(input)
    if (readiness === 'claimed') return true
    if (readiness === 'closed') return false
    if (check + 1 < maximumChecks) {
      // eslint-disable-next-line no-await-in-loop
      await wait()
    }
  }
  return false
}

export const parseSarahRealtimeVoiceRouteConfig = (
  env: Readonly<{
    SARAH_REALTIME_VOICE_ENABLED?: string | undefined
    SARAH_REALTIME_RESERVATION_MSAT?: string | undefined
    SARAH_REALTIME_MAX_SESSION_SECONDS?: string | undefined
    SARAH_REALTIME_CREDIT_MSAT_PER_MILLION_TOKENS?: string | undefined
  }>,
): SarahRealtimeVoiceRouteConfig | undefined => {
  const enabled = ['1', 'true', 'on'].includes(
    env.SARAH_REALTIME_VOICE_ENABLED?.trim().toLowerCase() ?? '',
  )
  if (!enabled) {
    return {
      enabled: false,
      creditMsatPerMillionTokens: 0,
      maxSessionSeconds: 600,
      reservationMsat: 0,
    }
  }
  const reservationMsat = Number(env.SARAH_REALTIME_RESERVATION_MSAT)
  const creditMsatPerMillionTokens = Number(
    env.SARAH_REALTIME_CREDIT_MSAT_PER_MILLION_TOKENS,
  )
  const maxSessionSeconds = Number(
    env.SARAH_REALTIME_MAX_SESSION_SECONDS ?? '600',
  )
  if (
    !Number.isSafeInteger(reservationMsat) ||
    reservationMsat <= 0 ||
    !Number.isSafeInteger(creditMsatPerMillionTokens) ||
    creditMsatPerMillionTokens <= 0 ||
    !Number.isSafeInteger(maxSessionSeconds) ||
    maxSessionSeconds < 60 ||
    maxSessionSeconds > 900
  ) {
    return undefined
  }
  return {
    enabled,
    creditMsatPerMillionTokens,
    maxSessionSeconds,
    reservationMsat,
  }
}

const capabilityBoundaryForProfile = (
  clientProfile: 'omega_editor' | 'mobile_voice_only' | 'mobile_command_center',
  roomContextKind: 'private' | 'community' = 'private',
) => {
  const fixedBoundary = {
    directShell: false as const,
    directGit: false as const,
    payment: false as const,
    credentialAccess: false as const,
    deviceControl: false as const,
  }
  if (roomContextKind === 'community') {
    return { commands: [], confirmationRequired: [], ...fixedBoundary }
  }
  if (clientProfile === 'omega_editor') {
    return {
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
      ...fixedBoundary,
    }
  }
  if (clientProfile === 'mobile_command_center') {
    return {
      commands: ['start_agent_thread'],
      confirmationRequired: [],
      ...fixedBoundary,
    }
  }
  return { commands: [], confirmationRequired: [], ...fixedBoundary }
}

export const handleSarahRealtimeVoiceAdmissionRequest = async <User, Bindings>(
  dependencies: SarahRealtimeVoiceRouteDependencies<User, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson({ error: 'method_not_allowed' }, 405)
  }
  const nostrAuthorization = request.headers
    .get('authorization')
    ?.startsWith('Nostr ')
  let session: Readonly<{ user: User }> | undefined
  let parsed: Awaited<ReturnType<typeof parseAdmissionRequest>>
  let issuedNostrAuth:
    Readonly<{ accessToken: string; expiresIn: number }> | undefined
  if (nostrAuthorization) {
    parsed = await parseAdmissionRequest(request.clone())
    if (parsed === undefined) {
      return noStoreJson(
        { error: 'invalid_sarah_voice_admission_request' },
        400,
      )
    }
    if (parsed.body.auth === undefined) {
      return noStoreJson({ error: 'sarah_voice_auth_challenge_required' }, 401)
    }
    const authenticateNostrSession = dependencies.authenticateNostrSession
    const consumeNostrChallenge = dependencies.consumeNostrChallenge
    const mintNostrSession = dependencies.mintNostrSession
    const verifyNostrProof = dependencies.verifyNostrProof
    if (
      authenticateNostrSession === undefined ||
      consumeNostrChallenge === undefined ||
      mintNostrSession === undefined ||
      verifyNostrProof === undefined
    ) {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
    const verified = await verifyNostrProof(request, env, parsed.payload)
    if (verified._tag === 'Rejected') return verified.response
    const challenge = await consumeNostrChallenge(env, {
      challenge: parsed.body.auth.challenge,
      deviceRef: parsed.body.identity.deviceRef,
      ownerRef: parsed.body.identity.ownerRef,
      pubkey: verified.pubkey,
    })
    if (challenge === 'Unavailable') {
      return noStoreJson(
        { error: 'sarah_voice_auth_challenge_unavailable' },
        503,
      )
    }
    if (challenge === 'Invalid') {
      return noStoreJson({ error: 'sarah_voice_auth_challenge_invalid' }, 409)
    }
    const authenticated = await authenticateNostrSession(request, env, verified)
    if (authenticated._tag === 'Rejected') return authenticated.response
    const issued = await mintNostrSession(env, authenticated)
    if (issued._tag === 'Rejected') return issued.response
    session = { user: issued.user }
    issuedNostrAuth = {
      accessToken: issued.accessToken,
      expiresIn: issued.expiresIn,
    }
  } else {
    session = await dependencies.requireUserBearerSession(request, env, ctx)
    if (session === undefined)
      return noStoreJson({ error: 'unauthorized' }, 401)
    parsed = await parseAdmissionRequest(request)
    if (parsed === undefined || parsed.body.auth !== undefined) {
      return noStoreJson(
        { error: 'invalid_sarah_voice_admission_request' },
        400,
      )
    }
  }
  const body = parsed.body
  const userId = dependencies.userIdFromSession(session)
  const deviceRef = request.headers
    .get(SARAH_REALTIME_VOICE_DEVICE_HEADER)
    ?.trim()
  if (
    body.identity.ownerRef !== userId ||
    deviceRef === undefined ||
    deviceRef !== body.identity.deviceRef
  ) {
    return noStoreJson({ error: 'sarah_voice_identity_mismatch' }, 403)
  }
  const config = dependencies.config(env)
  if (config === undefined) {
    return noStoreJson({ error: 'sarah_voice_configuration_invalid' }, 503)
  }
  if (!config.enabled) {
    return noStoreJson({ error: 'sarah_voice_unavailable' }, 503)
  }

  const nowMs = (dependencies.now ?? Date.now)()
  const nowIso = new Date(nowMs).toISOString()
  const clientProfile = body.clientProfile ?? 'omega_editor'
  const requestedTransport = body.requestedTransport ?? 'custom_wss_v1'
  const requestedRoomContext = body.roomContext ?? { kind: 'private' as const }
  if (
    requestedTransport === 'livekit_room_v1' &&
    clientProfile !== 'omega_editor'
  ) {
    return noStoreJson(
      { error: 'sarah_voice_livekit_client_profile_not_supported' },
      400,
    )
  }
  if (
    requestedTransport === 'livekit_room_v1' &&
    dependencies.liveKitNewAdmissionsEnabled?.(env) !== true
  ) {
    return noStoreJson(
      { error: 'sarah_voice_livekit_admissions_disabled' },
      503,
    )
  }
  if (
    requestedTransport === 'custom_wss_v1' &&
    body.roomContext !== undefined
  ) {
    return noStoreJson({ error: 'sarah_voice_room_context_not_supported' }, 400)
  }
  let opened:
    | Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
    | undefined
  try {
    opened = await dependencies.openStore(env)
    let admissionRoomContext:
      | Readonly<{ kind: 'private' }>
      | (SarahVoiceLiveKitCommunityAccess & Readonly<{ kind: 'community' }>) = {
      kind: 'private',
    }
    if (
      requestedTransport === 'livekit_room_v1' &&
      requestedRoomContext.kind === 'community'
    ) {
      const access = await dependencies.resolveLiveKitCommunityAccess?.(env, {
        ownerUserId: userId,
        communityRef: requestedRoomContext.communityRef,
        channelRef: requestedRoomContext.channelRef,
      })
      if (
        access === undefined ||
        access.communityRef !== requestedRoomContext.communityRef ||
        access.channelRef !== requestedRoomContext.channelRef ||
        !access.subscribeAllowed
      ) {
        return noStoreJson(
          { error: 'sarah_voice_community_membership_inactive' },
          403,
        )
      }
      admissionRoomContext = { kind: 'community', ...access }
    }
    const stagingEntitlement =
      dependencies.stagingOwnerEntitlementEnabled?.(env) === true
        ? await opened.store.readActiveStagingOwnerEntitlement({
            entitlementRef: 'sarah_voice_entitlement:staging_owner_v1',
            nowIso,
            ownerUserId: userId,
          })
        : undefined
    if (stagingEntitlement !== undefined) {
      const terms = {
        clientProfile,
        admissionCohortRef: SARAH_VOICE_STAGING_OWNER_COHORT_REF,
        creditMode: 'staging_owner_entitlement' as const,
        creditRateMsatPerMillionTokens: config.creditMsatPerMillionTokens,
        requiredHoldMsat: 0,
        spendableRemainingCreditMsat: null,
        maxDurationSeconds: config.maxSessionSeconds,
        capabilityBoundary: capabilityBoundaryForProfile(
          clientProfile,
          admissionRoomContext.kind,
        ),
      }
      const admissionRef = makeAdmissionRef()
      const admissionExpiresAtMs = nowMs + SARAH_VOICE_ADMISSION_LIFETIME_MS
      await opened.store.issueAdmission({
        admissionRef,
        ownerUserId: userId,
        deviceRef: body.identity.deviceRef,
        threadRef: body.identity.threadRef,
        sessionRef: body.identity.sessionRef,
        generation: body.identity.generation,
        disclosureRef: body.disclosureRef,
        clientProfile,
        admissionCohortRef: terms.admissionCohortRef,
        creditMode: terms.creditMode,
        creditRateMsatPerMillionTokens: terms.creditRateMsatPerMillionTokens,
        termsDigest: await admissionTermsDigest({
          ...terms,
          transportKind: requestedTransport,
          roomContext: admissionRoomContext,
        }),
        spendableRemainingCreditMsat: null,
        nowIso,
        expiresAt: new Date(admissionExpiresAtMs).toISOString(),
      })
      return noStoreJson(
        {
          schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
          admitted: true,
          ...terms,
          admissionRef,
          admissionExpiresAtMs,
          ...(issuedNostrAuth === undefined
            ? {}
            : {
                auth: {
                  method: SARAH_VOICE_NOSTR_AUTH_METHOD,
                  ...issuedNostrAuth,
                },
              }),
        },
        200,
      )
    }

    const [membership, spendableRemainingCreditMsat] = await Promise.all([
      opened.store.readActiveAlphaMembership({
        ownerUserId: userId,
        cohortRef: SARAH_VOICE_ALPHA_COHORT_REF,
        nowIso,
      }),
      opened.store.readSpendableCredit({
        ownerUserId: userId,
        ownerActorRef: `agent:${userId}`,
      }),
    ])
    const admitted =
      membership !== undefined &&
      spendableRemainingCreditMsat >= config.reservationMsat
    const terms = {
      clientProfile,
      admissionCohortRef: SARAH_VOICE_ALPHA_COHORT_REF,
      creditMode: 'metered' as const,
      creditRateMsatPerMillionTokens: config.creditMsatPerMillionTokens,
      requiredHoldMsat: config.reservationMsat,
      spendableRemainingCreditMsat,
      maxDurationSeconds: config.maxSessionSeconds,
      capabilityBoundary: capabilityBoundaryForProfile(
        clientProfile,
        admissionRoomContext.kind,
      ),
    }
    let admissionBinding:
      | Readonly<{ admissionRef: string; admissionExpiresAtMs: number }>
      | undefined
    if (admitted) {
      const admissionRef = makeAdmissionRef()
      const admissionExpiresAtMs = nowMs + SARAH_VOICE_ADMISSION_LIFETIME_MS
      await opened.store.issueAdmission({
        admissionRef,
        ownerUserId: userId,
        deviceRef: body.identity.deviceRef,
        threadRef: body.identity.threadRef,
        sessionRef: body.identity.sessionRef,
        generation: body.identity.generation,
        disclosureRef: body.disclosureRef,
        clientProfile,
        admissionCohortRef: terms.admissionCohortRef,
        creditMode: terms.creditMode,
        creditRateMsatPerMillionTokens: terms.creditRateMsatPerMillionTokens,
        termsDigest: await admissionTermsDigest({
          ...terms,
          transportKind: requestedTransport,
          roomContext: admissionRoomContext,
        }),
        spendableRemainingCreditMsat,
        nowIso,
        expiresAt: new Date(admissionExpiresAtMs).toISOString(),
      })
      admissionBinding = { admissionRef, admissionExpiresAtMs }
    }
    return noStoreJson(
      {
        schema: SARAH_VOICE_ADMISSION_PROTOCOL_VERSION,
        admitted,
        ...terms,
        ...admissionBinding,
        ...(admitted
          ? {}
          : {
              refusalReason:
                membership === undefined
                  ? 'cohort_inactive'
                  : 'insufficient_credit',
            }),
        ...(issuedNostrAuth === undefined
          ? {}
          : {
              auth: {
                method: SARAH_VOICE_NOSTR_AUTH_METHOD,
                ...issuedNostrAuth,
              },
            }),
      },
      200,
    )
  } catch (error) {
    dependencies.audit?.('storage_unavailable', {
      errorMessage:
        error instanceof Error ? error.message.slice(0, 256) : 'unknown error',
      errorName: error instanceof Error ? error.name : typeof error,
    })
    return noStoreJson({ error: 'sarah_voice_storage_unavailable' }, 503)
  } finally {
    try {
      await opened?.close()
    } catch {
      // The response is read-only, so connection release failure cannot change it.
    }
  }
}

export const handleSarahRealtimeVoiceSettlementRequest = async <User, Bindings>(
  dependencies: SarahRealtimeVoiceRouteDependencies<User, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return noStoreJson({ error: 'method_not_allowed' }, 405)
  }
  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return noStoreJson({ error: 'unauthorized' }, 401)
  const sessionRef = request.headers
    .get(SARAH_REALTIME_VOICE_SESSION_HEADER)
    ?.trim()
  if (
    sessionRef === undefined ||
    sessionRef.length === 0 ||
    sessionRef.length > 256
  ) {
    return noStoreJson({ error: 'invalid_sarah_voice_settlement_request' }, 400)
  }
  let opened:
    | Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
    | undefined
  try {
    opened = await dependencies.openStore(env)
    const settlement = await opened.store.readSettlement({
      sessionRef,
      ownerUserId: dependencies.userIdFromSession(session),
    })
    if (settlement === undefined) {
      return noStoreJson({ error: 'sarah_voice_settlement_not_found' }, 404)
    }
    if (settlement.state === 'accounting_uncertain') {
      const failureEvidenceRequested =
        request.headers.get(SARAH_LIVEKIT_ACCEPTANCE_EVIDENCE_HEADER) ===
        'live-observation-v1'
      return noStoreJson(
        {
          error: 'sarah_voice_accounting_uncertain',
          sessionRef: settlement.sessionRef,
          state: settlement.state,
          creditMode: settlement.creditMode,
          recordedChargeMsat: settlement.recordedChargeMsat,
          reservedCreditMsat: settlement.reservedMsat,
          holdPreserved: settlement.holdPreserved,
          reason: settlement.reason,
          ...(settlement.failureEvidence === undefined ||
          !failureEvidenceRequested
            ? {}
            : { failureEvidence: settlement.failureEvidence }),
        },
        409,
      )
    }
    const acceptanceEvidence = (
      settlement as typeof settlement & {
        acceptanceEvidence?: unknown
      }
    ).acceptanceEvidence
    const acceptanceEvidenceRequested =
      request.headers.get(SARAH_LIVEKIT_ACCEPTANCE_EVIDENCE_HEADER) ===
      'live-observation-v1'
    return noStoreJson(
      {
        schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
        sessionRef: settlement.sessionRef,
        state: settlement.state,
        creditMode: settlement.creditMode,
        finalChargeMsat: settlement.finalChargeMsat,
        spendableRemainingCreditMsat: settlement.spendableRemainingCreditMsat,
        receiptRef: settlement.settlementReceiptRef,
        ...(acceptanceEvidence === undefined || !acceptanceEvidenceRequested
          ? {}
          : { acceptanceEvidence }),
      },
      200,
    )
  } catch {
    return noStoreJson({ error: 'sarah_voice_storage_unavailable' }, 503)
  } finally {
    try {
      await opened?.close()
    } catch {
      // A release failure cannot change the settled receipt returned above.
    }
  }
}

export const handleSarahRealtimeVoiceAccountingReconciliationRequest = async <
  Bindings,
>(
  dependencies: SarahRealtimeVoiceOperatorRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson({ error: 'method_not_allowed' }, 405)
  }
  const operator = await dependencies.requireOperator(request, env, ctx)
  if (operator === undefined) return noStoreJson({ error: 'forbidden' }, 403)
  const body = await parseBoundedJson(
    request,
    decodeSarahVoiceAccountingReconciliationRequest,
  )
  if (body === undefined) {
    return noStoreJson(
      { error: 'invalid_sarah_voice_accounting_reconciliation' },
      400,
    )
  }
  const providerEvidenceRefs = [...body.providerEvidenceRefs].sort()
  if (new Set(providerEvidenceRefs).size !== providerEvidenceRefs.length) {
    return noStoreJson(
      { error: 'invalid_sarah_voice_accounting_reconciliation' },
      400,
    )
  }
  const usage = body.usage
    .map(item =>
      item.kind === 'response'
        ? {
            usageKind: item.kind,
            providerResponseRef: `response:${item.providerResponseRef}`,
            providerStatus: item.status,
            inputTokens: item.inputTokens,
            outputTokens: item.outputTokens,
            cachedInputTokens: item.cachedInputTokens,
            audioInputTokens: item.audioInputTokens,
            audioOutputTokens: item.audioOutputTokens,
          }
        : {
            usageKind: item.kind,
            providerResponseRef: `transcription:${item.providerTranscriptionRef}`,
            inputTokens: item.inputTokens,
            outputTokens: item.outputTokens,
            cachedInputTokens: item.cachedInputTokens,
            audioInputTokens: item.audioInputTokens,
            audioOutputTokens: item.audioOutputTokens,
          },
    )
    .sort((left, right) =>
      left.providerResponseRef.localeCompare(right.providerResponseRef),
    )
  if (
    usage.some(item => item.providerResponseRef.length > 256) ||
    new Set(usage.map(item => item.providerResponseRef)).size !== usage.length
  ) {
    return noStoreJson(
      { error: 'invalid_sarah_voice_accounting_reconciliation' },
      400,
    )
  }
  const reconciliationPayloadDigest = await sha256Hex(
    JSON.stringify({
      reconciliationRef: body.reconciliationRef,
      sessionRef: body.sessionRef,
      generation: body.generation,
      providerSessionRefDigest: body.providerSessionRefDigest,
      providerEvidenceRefs,
      usage,
      reason: body.reason,
    }),
  )
  let opened:
    | Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
    | undefined
  try {
    opened = await dependencies.openStore(env)
    const result = await opened.store.reconcileLiveKitAccounting({
      reconciliationRef: body.reconciliationRef,
      reconciliationPayloadDigest,
      sessionRef: body.sessionRef,
      generation: body.generation,
      providerSessionRefDigest: body.providerSessionRefDigest,
      operatorActorRef: operator.actorRef,
      reason: body.reason,
      providerEvidenceRefs,
      usage,
      nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
    })
    return noStoreJson(
      {
        schema: SARAH_VOICE_ACCOUNTING_RECONCILIATION_PROTOCOL_VERSION,
        reconciliationRef: result.reconciliationRef,
        reconciliationReceiptRef: result.reconciliationReceiptRef,
        sessionRef: result.sessionRef,
        state: result.state,
        finalChargeMsat: result.finalChargeMsat,
        settlementReceiptRef: result.settlementReceiptRef,
        replayed: result.replayed,
      },
      200,
    )
  } catch (error) {
    return error instanceof SarahVoiceSessionRejectedError
      ? noStoreJson(
          { error: 'sarah_voice_accounting_reconciliation_conflict' },
          409,
        )
      : noStoreJson({ error: 'sarah_voice_storage_unavailable' }, 503)
  } finally {
    try {
      await opened?.close()
    } catch {
      // The committed reconciliation is unaffected by pool release failure.
    }
  }
}

export const handleSarahRealtimeVoiceCohortRevocationRequest = async <Bindings>(
  dependencies: SarahRealtimeVoiceOperatorRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson({ error: 'method_not_allowed' }, 405)
  }
  const operator = await dependencies.requireOperator(request, env, ctx)
  if (operator === undefined) return noStoreJson({ error: 'forbidden' }, 403)
  const body = await parseBoundedJson(
    request,
    decodeSarahVoiceCohortRevocationRequest,
  )
  if (body === undefined) {
    return noStoreJson({ error: 'invalid_sarah_voice_cohort_revocation' }, 400)
  }
  let opened:
    | Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
    | undefined
  try {
    opened = await dependencies.openStore(env)
    const revokedCount = await opened.store.revokeAlphaCohort({
      cohortRef: body.cohortRef,
      actorRef: operator.actorRef,
      reason: body.reason,
      nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
    })
    return noStoreJson(
      {
        schema: SARAH_VOICE_COHORT_REVOCATION_PROTOCOL_VERSION,
        cohortRef: SARAH_VOICE_ALPHA_COHORT_REF,
        state: revokedCount === 0 ? 'already_revoked' : 'revoked',
        revokedCount,
      },
      200,
    )
  } catch {
    return noStoreJson({ error: 'sarah_voice_storage_unavailable' }, 503)
  } finally {
    try {
      await opened?.close()
    } catch {
      // The committed cohort revocation is unaffected by pool release failure.
    }
  }
}

export const handleSarahRealtimeVoiceSessionRequest = async <User, Bindings>(
  dependencies: SarahRealtimeVoiceRouteDependencies<User, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson({ error: 'method_not_allowed' }, 405)
  }

  const nostrAuthorization = request.headers
    .get('authorization')
    ?.startsWith('Nostr ')
  let session: Readonly<{ user: User }> | undefined
  let parsed: Awaited<ReturnType<typeof parseRequest>>
  let issuedNostrAuth:
    Readonly<{ accessToken: string; expiresIn: number }> | undefined

  if (nostrAuthorization) {
    parsed = await parseRequest(request.clone())
    if (parsed === undefined) {
      return noStoreJson({ error: 'invalid_sarah_voice_request' }, 400)
    }
    if (parsed.body.auth === undefined) {
      return noStoreJson({ error: 'sarah_voice_auth_challenge_required' }, 401)
    }
    const authenticateNostrSession = dependencies.authenticateNostrSession
    const consumeNostrChallenge = dependencies.consumeNostrChallenge
    const mintNostrSession = dependencies.mintNostrSession
    const verifyNostrProof = dependencies.verifyNostrProof
    if (
      authenticateNostrSession === undefined ||
      consumeNostrChallenge === undefined ||
      mintNostrSession === undefined ||
      verifyNostrProof === undefined
    ) {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
    const verified = await verifyNostrProof(request, env, parsed.payload)
    if (verified._tag === 'Rejected') return verified.response
    const challenge = await consumeNostrChallenge(env, {
      challenge: parsed.body.auth.challenge,
      deviceRef: parsed.body.identity.deviceRef,
      ownerRef: parsed.body.identity.ownerRef,
      pubkey: verified.pubkey,
    })
    if (challenge === 'Unavailable') {
      return noStoreJson(
        { error: 'sarah_voice_auth_challenge_unavailable' },
        503,
      )
    }
    if (challenge === 'Invalid') {
      return noStoreJson({ error: 'sarah_voice_auth_challenge_invalid' }, 409)
    }
    const authenticated = await authenticateNostrSession(request, env, verified)
    if (authenticated._tag === 'Rejected') return authenticated.response
    const issued = await mintNostrSession(env, authenticated)
    if (issued._tag === 'Rejected') return issued.response
    session = { user: issued.user }
    issuedNostrAuth = {
      accessToken: issued.accessToken,
      expiresIn: issued.expiresIn,
    }
  } else {
    session = await dependencies.requireUserBearerSession(request, env, ctx)
    if (session === undefined) {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
    parsed = await parseRequest(request)
    if (parsed === undefined) {
      return noStoreJson({ error: 'invalid_sarah_voice_request' }, 400)
    }
    if (parsed.body.auth !== undefined) {
      return noStoreJson({ error: 'invalid_sarah_voice_request' }, 400)
    }
  }
  const body = parsed.body
  const clientProfile = body.clientProfile ?? 'omega_editor'
  const admissionRef = body.admissionRef
  const requestedTransport = body.requestedTransport ?? 'custom_wss_v1'
  const requestedRoomContext = body.roomContext ?? { kind: 'private' as const }
  if (
    requestedTransport === 'livekit_room_v1' &&
    clientProfile !== 'omega_editor'
  ) {
    return noStoreJson(
      { error: 'sarah_voice_livekit_client_profile_not_supported' },
      400,
    )
  }
  if (
    requestedTransport === 'livekit_room_v1' &&
    dependencies.liveKitNewAdmissionsEnabled?.(env) !== true
  ) {
    return noStoreJson(
      { error: 'sarah_voice_livekit_admissions_disabled' },
      503,
    )
  }
  const requiresAdmission =
    clientProfile === 'omega_editor' || requestedTransport === 'livekit_room_v1'
  const userId = dependencies.userIdFromSession(session)
  const deviceRef = request.headers
    .get(SARAH_REALTIME_VOICE_DEVICE_HEADER)
    ?.trim()
  if (
    body.identity.ownerRef !== userId ||
    deviceRef === undefined ||
    deviceRef !== body.identity.deviceRef
  ) {
    return noStoreJson({ error: 'sarah_voice_identity_mismatch' }, 403)
  }
  if (requiresAdmission && admissionRef === undefined) {
    return noStoreJson({ error: 'sarah_voice_admission_required' }, 409)
  }
  if (
    requestedTransport === 'custom_wss_v1' &&
    body.roomContext !== undefined
  ) {
    return noStoreJson({ error: 'sarah_voice_room_context_not_supported' }, 400)
  }
  if (
    requestedTransport === 'livekit_room_v1' &&
    dependencies.liveKitRoomBroker === undefined
  ) {
    return noStoreJson({ error: 'sarah_voice_livekit_unavailable' }, 503)
  }

  const config = dependencies.config(env)
  if (config === undefined) {
    return noStoreJson({ error: 'sarah_voice_configuration_invalid' }, 503)
  }
  if (!config.enabled) {
    return noStoreJson({ error: 'sarah_voice_unavailable' }, 503)
  }
  const gatewayUrl = gatewayUrlForRequest(request)
  if (gatewayUrl === undefined) {
    return noStoreJson({ error: 'sarah_voice_gateway_unavailable' }, 503)
  }

  const nowMs = (dependencies.now ?? Date.now)()
  const nowIso = new Date(nowMs).toISOString()
  let ticketExpiresAtMs =
    nowMs + Math.min(60_000, config.maxSessionSeconds * 1_000)
  let sessionExpiresAtMs = nowMs + config.maxSessionSeconds * 1_000
  let ticket = makeTicket()
  let opened:
    | Readonly<{ store: SarahRealtimeVoiceStore; close: () => Promise<void> }>
    | undefined
  try {
    opened = await dependencies.openStore(env)
    const stagingEntitlementEnabled =
      dependencies.stagingOwnerEntitlementEnabled?.(env) === true
    const entitlement = stagingEntitlementEnabled
      ? await opened.store.readActiveStagingOwnerEntitlement({
          entitlementRef: 'sarah_voice_entitlement:staging_owner_v1',
          nowIso,
          ownerUserId: userId,
        })
      : undefined
    if (stagingEntitlementEnabled) {
      dependencies.audit?.(
        entitlement === undefined
          ? 'staging_owner_entitlement_inactive'
          : 'staging_owner_entitlement_applied',
        {
          entitlementRef: 'sarah_voice_entitlement:staging_owner_v1',
          ownerDigest: await sha256Hex(userId),
        },
      )
    }
    const alphaMembership =
      entitlement === undefined
        ? await opened.store.readActiveAlphaMembership({
            ownerUserId: userId,
            cohortRef: SARAH_VOICE_ALPHA_COHORT_REF,
            nowIso,
          })
        : undefined
    if (entitlement === undefined && alphaMembership === undefined) {
      throw new SarahVoiceSessionRejectedError(
        'The Sarah voice alpha membership is not active',
      )
    }
    const creditMode =
      entitlement === undefined ? 'metered' : 'staging_owner_entitlement'
    const reservedMsat = entitlement === undefined ? config.reservationMsat : 0
    const admissionCohortRef =
      entitlement === undefined
        ? SARAH_VOICE_ALPHA_COHORT_REF
        : SARAH_VOICE_STAGING_OWNER_COHORT_REF
    const spendableRemainingCreditMsat =
      requiresAdmission && entitlement === undefined
        ? await opened.store.readSpendableCredit({
            ownerUserId: userId,
            ownerActorRef: `agent:${userId}`,
          })
        : null
    let liveKitRoomContext:
      | Readonly<{ kind: 'private' }>
      | (SarahVoiceLiveKitCommunityAccess & Readonly<{ kind: 'community' }>) = {
      kind: 'private',
    }
    if (
      requestedTransport === 'livekit_room_v1' &&
      requestedRoomContext.kind === 'community'
    ) {
      const access = await dependencies.resolveLiveKitCommunityAccess?.(env, {
        ownerUserId: userId,
        communityRef: requestedRoomContext.communityRef,
        channelRef: requestedRoomContext.channelRef,
      })
      if (
        access === undefined ||
        access.communityRef !== requestedRoomContext.communityRef ||
        access.channelRef !== requestedRoomContext.channelRef ||
        !access.subscribeAllowed
      ) {
        throw new SarahVoiceSessionRejectedError(
          'The Sarah voice community room membership is not active',
        )
      }
      liveKitRoomContext = { kind: 'community', ...access }
    }
    const capabilityBoundary = capabilityBoundaryForProfile(
      clientProfile,
      liveKitRoomContext.kind,
    )
    const currentTerms = {
      clientProfile,
      admissionCohortRef,
      creditMode,
      creditRateMsatPerMillionTokens: config.creditMsatPerMillionTokens,
      requiredHoldMsat: reservedMsat,
      spendableRemainingCreditMsat,
      maxDurationSeconds: config.maxSessionSeconds,
      capabilityBoundary,
    }
    const currentAdmissionDigest =
      requiresAdmission && admissionRef !== undefined
        ? await admissionTermsDigest({
            ...currentTerms,
            transportKind: requestedTransport,
            roomContext: liveKitRoomContext,
          })
        : null
    if (
      requiresAdmission &&
      (admissionRef === undefined || currentAdmissionDigest === null)
    ) {
      throw new SarahVoiceAdmissionRejectedError(
        'The Sarah voice admission binding is incomplete',
      )
    }
    const exactAdmissionBinding =
      admissionRef !== undefined && currentAdmissionDigest !== null
        ? {
            admissionRef,
            termsDigest: currentAdmissionDigest,
            creditRateMsatPerMillionTokens:
              currentTerms.creditRateMsatPerMillionTokens,
            spendableRemainingCreditMsat,
          }
        : undefined
    let liveKitProvisionInput: SarahVoiceLiveKitProvisionInput | undefined
    if (
      requestedTransport === 'livekit_room_v1' &&
      admissionRef !== undefined &&
      currentAdmissionDigest !== null
    ) {
      const broker = dependencies.liveKitRoomBroker
      if (broker === undefined) {
        throw new SarahVoiceAdmissionRejectedError(
          'The LiveKit voice broker is unavailable',
        )
      }
      liveKitProvisionInput = {
        idempotencyKey: `sarah-livekit:${body.identity.sessionRef}:${body.identity.generation}`,
        ownerUserId: userId,
        deviceRef: body.identity.deviceRef,
        threadRef: body.identity.threadRef,
        sessionRef: body.identity.sessionRef,
        generation: body.identity.generation,
        capabilityProfile: clientProfile,
        admissionRef,
        admissionDigest: currentAdmissionDigest,
        roomContext: liveKitRoomContext,
        publishAllowed:
          liveKitRoomContext.kind === 'private'
            ? true
            : liveKitRoomContext.publishAllowed,
        subscribeAllowed:
          liveKitRoomContext.kind === 'private'
            ? true
            : liveKitRoomContext.subscribeAllowed,
        expiresAtMs: sessionExpiresAtMs,
      }
      ticket = broker.sessionTicket(liveKitProvisionInput)
    }
    const reserved = await opened.store.reserve({
      deviceRef: body.identity.deviceRef,
      disclosureRef: body.disclosureRef,
      clientProfile,
      transportKind: requestedTransport,
      generation: body.identity.generation,
      nowIso,
      ownerActorRef: `agent:${userId}`,
      ownerUserId: userId,
      reservationRef: `sarah:voice:reserve:${userId}:${body.identity.sessionRef}`,
      creditMode,
      entitlementRef: entitlement?.entitlementRef ?? null,
      admissionCohortRef,
      creditRateMsatPerMillionTokens:
        currentTerms.creditRateMsatPerMillionTokens,
      reservedMsat,
      sessionExpiresAt: new Date(sessionExpiresAtMs).toISOString(),
      sessionRef: body.identity.sessionRef,
      threadRef: body.identity.threadRef,
      ticketDigest: await sha256Hex(ticket),
      ticketExpiresAt: new Date(ticketExpiresAtMs).toISOString(),
      ...(requiresAdmission && exactAdmissionBinding !== undefined
        ? { admissionBinding: exactAdmissionBinding }
        : {}),
    })
    ticketExpiresAtMs = Date.parse(reserved.ticketExpiresAt)
    sessionExpiresAtMs = Date.parse(reserved.sessionExpiresAt)
    if (
      !Number.isSafeInteger(ticketExpiresAtMs) ||
      !Number.isSafeInteger(sessionExpiresAtMs) ||
      ticketExpiresAtMs <= nowMs ||
      sessionExpiresAtMs <= nowMs
    ) {
      throw new SarahVoiceSessionRejectedError(
        'The Sarah voice replay expiry was invalid',
      )
    }
    if (
      liveKitProvisionInput !== undefined &&
      reserved.admissionTermsDigest !== undefined
    ) {
      liveKitProvisionInput = {
        ...liveKitProvisionInput,
        admissionDigest: reserved.admissionTermsDigest,
        expiresAtMs: sessionExpiresAtMs,
      }
    } else if (liveKitProvisionInput !== undefined) {
      liveKitProvisionInput = {
        ...liveKitProvisionInput,
        expiresAtMs: sessionExpiresAtMs,
      }
    }
    const admissionExpiresAtMs =
      requiresAdmission && reserved.admissionExpiresAt !== undefined
        ? Date.parse(reserved.admissionExpiresAt)
        : undefined
    if (
      requiresAdmission &&
      (admissionExpiresAtMs === undefined ||
        !Number.isSafeInteger(admissionExpiresAtMs) ||
        admissionExpiresAtMs <= nowMs)
    ) {
      throw new SarahVoiceAdmissionRejectedError(
        'The consumed Sarah voice admission expiry was invalid',
      )
    }
    let liveKitProvision: SarahVoiceLiveKitProvision | undefined
    if (requestedTransport === 'livekit_room_v1') {
      const broker = dependencies.liveKitRoomBroker
      if (
        broker === undefined ||
        admissionRef === undefined ||
        admissionExpiresAtMs === undefined ||
        liveKitProvisionInput === undefined
      ) {
        throw new SarahVoiceAdmissionRejectedError(
          'The LiveKit voice admission binding is incomplete',
        )
      }
      const liveKitIdempotencyKey = `sarah-livekit:${body.identity.sessionRef}:${body.identity.generation}`
      const provisioningOwnerRef = `sarah-livekit-issuer:${crypto.randomUUID()}`
      // Re-requesting a generation whose participant is already in the room
      // would mint a second grant for one participant identity, so two clients
      // would race the same room and session. Refuse before provisioning:
      // nothing here may disturb the room the admitted participant is in.
      // Re-requesting before the participant arrives stays the ordinary
      // same-generation reconnect.
      if (
        await opened.store.readLiveKitOwnerParticipantAdmitted({
          sessionRef: body.identity.sessionRef,
          generation: body.identity.generation,
          nowIso,
        })
      ) {
        return noStoreJson({ error: 'duplicate_participant_refused' }, 409)
      }
      try {
        const provisionInput = liveKitProvisionInput
        const publishAllowed = provisionInput.publishAllowed
        const subscribeAllowed = provisionInput.subscribeAllowed
        const liveKitWorkerControlTokenDigest =
          broker.workerControlTokenDigest(provisionInput)
        await opened.store.prepareLiveKitProvisioningIntent({
          sessionRef: body.identity.sessionRef,
          ownerUserId: userId,
          deviceRef: body.identity.deviceRef,
          threadRef: body.identity.threadRef,
          generation: body.identity.generation,
          capabilityProfile: clientProfile,
          admissionRef,
          admissionDigest: provisionInput.admissionDigest,
          idempotencyKey: liveKitIdempotencyKey,
          workerControlTokenDigest: liveKitWorkerControlTokenDigest,
          roomContext: liveKitRoomContext,
          nowIso,
        })
        const claimedProvisioning =
          await opened.store.claimLiveKitProvisioningIntent({
            sessionRef: body.identity.sessionRef,
            generation: body.identity.generation,
            provisioningOwnerRef,
            staleBeforeIso: new Date(
              nowMs - SARAH_LIVEKIT_PROVISIONING_LEASE_MS,
            ).toISOString(),
            nowIso,
          })
        if (!claimedProvisioning) {
          return noStoreJson(
            { error: 'sarah_voice_livekit_issuance_in_progress' },
            503,
          )
        }
        liveKitProvision = await broker.provision(provisionInput)
        if (
          !validLiveKitProvision(liveKitProvision, {
            nowMs,
            maximumExpiresAtMs: Math.min(
              sessionExpiresAtMs,
              admissionExpiresAtMs,
            ),
            publishAllowed,
            subscribeAllowed,
          })
        ) {
          throw new Error('invalid_livekit_provision')
        }
        await opened.store.bindLiveKitRoom({
          sessionRef: body.identity.sessionRef,
          ownerUserId: userId,
          deviceRef: body.identity.deviceRef,
          threadRef: body.identity.threadRef,
          generation: body.identity.generation,
          provisioningOwnerRef,
          capabilityProfile: clientProfile,
          admissionRef,
          admissionDigest: provisionInput.admissionDigest,
          roomContext: liveKitRoomContext,
          roomRef: liveKitProvision.roomRef,
          roomEpoch: liveKitProvision.roomEpoch,
          participantRef: liveKitProvision.participantRef,
          sarahParticipantRef: liveKitProvision.sarahParticipantRef,
          participantGrantDigest: await sha256Hex(
            liveKitProvision.participantGrant,
          ),
          joinExpiresAt: new Date(
            liveKitProvision.joinExpiresAtMs,
          ).toISOString(),
          dispatchRef: liveKitProvision.dispatchRef,
          sarahPresenceLeaseRef: liveKitProvision.sarahPresenceLeaseRef,
          workerControlTokenDigest: liveKitWorkerControlTokenDigest,
          publishAllowed,
          subscribeAllowed,
          nowIso,
        })
        if (liveKitRoomContext.kind === 'community') {
          const bootstrap = dependencies.bootstrapLiveKitCommunityRoom
          if (bootstrap === undefined) {
            throw new Error('livekit_community_authority_unavailable')
          }
          await bootstrap(env, {
            ownerUserId: userId,
            presenceLeaseRef: liveKitProvision.sarahPresenceLeaseRef,
            communityAccess: {
              communityRef: liveKitRoomContext.communityRef,
              channelRef: liveKitRoomContext.channelRef,
              membershipRevision: liveKitRoomContext.membershipRevision,
              memberPubkey: liveKitRoomContext.memberPubkey,
              role: liveKitRoomContext.role,
              publishAllowed: liveKitRoomContext.publishAllowed,
              subscribeAllowed: liveKitRoomContext.subscribeAllowed,
            },
          })
        }
        const workerClaimed = await (
          dependencies.waitForLiveKitWorkerClaim ??
          waitForSarahLiveKitWorkerClaim
        )(opened.store, {
          sessionRef: body.identity.sessionRef,
          generation: body.identity.generation,
        })
        if (!workerClaimed) {
          throw new Error('livekit_worker_not_ready')
        }
        await opened.store.completeLiveKitProvisioningIntent({
          sessionRef: body.identity.sessionRef,
          generation: body.identity.generation,
          provisioningOwnerRef,
          nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
        })
      } catch (error) {
        if (error instanceof SarahVoiceDuplicateParticipantError) {
          // Lost the race to a request that bound this generation first. The
          // room and its accounting belong to the admitted participant, so this
          // path must release its provisioning claim and touch nothing else —
          // cleaning up here would evict a live participant.
          try {
            await opened.store.completeLiveKitProvisioningIntent({
              sessionRef: body.identity.sessionRef,
              generation: body.identity.generation,
              provisioningOwnerRef,
              nowIso: new Date((dependencies.now ?? Date.now)()).toISOString(),
            })
          } catch {
            // The stale-claim window releases it if this could not.
          }
          return noStoreJson({ error: 'duplicate_participant_refused' }, 409)
        }
        console.error('Sarah LiveKit provisioning failed', {
          sessionRef: body.identity.sessionRef,
          generation: body.identity.generation,
          reason:
            error instanceof Error
              ? error.message.slice(0, 256)
              : 'unknown_error',
        })
        let accountingTerminal = false
        try {
          await opened.store.settleLiveKitProvisioningIntent({
            sessionRef: body.identity.sessionRef,
            generation: body.identity.generation,
            provisioningOwnerRef,
            closeReason: 'livekit_provision_failed',
            nowIso,
          })
          accountingTerminal = true
        } catch {
          // The provisioning intent remains available for reconciliation.
        }
        let cleanupState: 'cleaned' | 'cleanup_failed' = 'cleaned'
        if (accountingTerminal) {
          try {
            if (liveKitProvision !== undefined) {
              await broker.cleanup(liveKitProvision)
            } else {
              await broker.cleanupByIdempotencyKey(liveKitIdempotencyKey)
            }
          } catch {
            cleanupState = 'cleanup_failed'
          }
        } else {
          cleanupState = 'cleanup_failed'
        }
        try {
          await opened.store.markLiveKitProvisioningIntent({
            sessionRef: body.identity.sessionRef,
            generation: body.identity.generation,
            provisioningOwnerRef,
            state: cleanupState,
            nowIso,
          })
        } catch {
          // The idempotent broker key still permits later reconciliation.
        }
        return noStoreJson({ error: 'sarah_voice_livekit_unavailable' }, 503)
      }
    }
    return noStoreJson(
      {
        schema: SARAH_VOICE_PROTOCOL_VERSION,
        sessionRef: body.identity.sessionRef,
        model: SARAH_VOICE_MODEL,
        ...(body.requestedTransport === undefined
          ? {}
          : {
              transport:
                liveKitProvision === undefined
                  ? { kind: 'custom_wss_v1' as const }
                  : {
                      kind: 'livekit_room_v1',
                      livekitUrl: liveKitProvision.livekitUrl,
                      roomRef: liveKitProvision.roomRef,
                      roomEpoch: liveKitProvision.roomEpoch,
                      participantRef: liveKitProvision.participantRef,
                      sarahParticipantRef: liveKitProvision.sarahParticipantRef,
                      participantGrant: liveKitProvision.participantGrant,
                      joinExpiresAtMs: liveKitProvision.joinExpiresAtMs,
                      dispatchRef: liveKitProvision.dispatchRef,
                      sarahPresenceLeaseRef:
                        liveKitProvision.sarahPresenceLeaseRef,
                      permissions: {
                        roomJoin: true,
                        canPublish:
                          liveKitRoomContext.kind === 'private'
                            ? true
                            : liveKitRoomContext.publishAllowed,
                        canSubscribe: true,
                        canPublishData: false,
                        canUpdateOwnMetadata: false,
                        canPublishSources: ['microphone'],
                        roomAdmin: false,
                        roomCreate: false,
                        roomList: false,
                      },
                    },
            }),
        gatewayUrl,
        ticket,
        ticketExpiresAtMs,
        sessionExpiresAtMs,
        reservedCreditMsat: reservedMsat,
        maxDurationSeconds: config.maxSessionSeconds,
        clientProfile,
        ...(requiresAdmission && admissionRef !== undefined
          ? {
              admissionRef,
              admissionExpiresAtMs,
              admissionCohortRef,
              creditMode,
              creditRateMsatPerMillionTokens: config.creditMsatPerMillionTokens,
              spendableRemainingCreditMsat,
              capabilityBoundary,
            }
          : {}),
        inputAudio: {
          codec: 'pcm_s16le',
          sampleRateHz: 24_000,
          channels: 1,
        },
        outputAudio: {
          codec: 'pcm_s16le',
          sampleRateHz: 24_000,
          channels: 1,
        },
        ...(issuedNostrAuth === undefined
          ? {}
          : {
              auth: {
                method: SARAH_VOICE_NOSTR_AUTH_METHOD,
                ...issuedNostrAuth,
              },
            }),
      },
      201,
    )
  } catch (error) {
    if (error instanceof SarahVoiceAdmissionRejectedError) {
      return noStoreJson({ error: 'sarah_voice_admission_invalid' }, 409)
    }
    if (error instanceof SarahVoiceInsufficientCreditError) {
      return noStoreJson({ error: 'insufficient_credit' }, 402)
    }
    if (error instanceof SarahVoiceConcurrentSessionError) {
      return noStoreJson({ error: 'sarah_voice_concurrency_limit' }, 409)
    }
    if (error instanceof SarahVoiceDuplicateParticipantError) {
      return noStoreJson({ error: 'duplicate_participant_refused' }, 409)
    }
    if (error instanceof SarahVoiceSessionRejectedError) {
      return noStoreJson({ error: 'sarah_voice_not_entitled' }, 403)
    }
    dependencies.audit?.('storage_unavailable', {
      errorMessage:
        error instanceof Error ? error.message.slice(0, 256) : 'unknown error',
      errorName: error instanceof Error ? error.name : typeof error,
    })
    return noStoreJson({ error: 'sarah_voice_storage_unavailable' }, 503)
  } finally {
    try {
      await opened?.close()
    } catch {
      // The shared client release cannot change the committed reservation.
    }
  }
}
