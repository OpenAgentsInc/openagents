import {
  OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
  OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
  decodeOmegaNostrDeviceLinkChallengeRequest,
  decodeOmegaNostrDeviceLinkRequest,
} from '@openagentsinc/audio-contract'

import { safeJsonRecord } from '../json-boundary'
import type { AuthKvStore } from './auth-kv'
import {
  clientIpFromRequest,
  reserveKvWindowRateLimit,
  stableRateLimitSubject,
} from './kv-window-rate-limit'
import type { OmegaNostrIdentityLinkResult } from './omega-nostr-identity-link-store'
import type {
  OmegaNostrProofConsumption,
  OmegaNostrVerifiedProof,
} from './omega-nostr-session'

export const OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_TTL_SECONDS = 120
export const OMEGA_NOSTR_DEVICE_LINK_DEVICE_HEADER =
  'x-openagents-omega-device-ref'

const CHALLENGE_KEY_PREFIX = 'omega-nostr:device-link:challenge:issued:'
const CONSUMED_KEY_PREFIX = 'omega-nostr:device-link:challenge:consumed:'
const RATE_KEY_PREFIX = 'omega-nostr:device-link:rate'
const MAX_REQUEST_BYTES = 1_024

type AuditEvent =
  | 'challenge_issued'
  | 'challenge_rate_limited'
  | 'challenge_rejected'
  | 'challenge_storage_unavailable'
  | 'link_conflict'
  | 'link_idempotent'
  | 'link_rejected'
  | 'link_succeeded'
  | 'link_storage_unavailable'

type StoredChallenge = Readonly<{
  deviceRef: string
  expiresAtMs: number
  ownerRef: string
  pubkey: string
  schemaVersion: 1
}>

export type OmegaNostrDeviceLinkDependencies<User, Env> = Readonly<{
  audit?: (event: AuditEvent, fields: Readonly<Record<string, string>>) => void
  authStore: (env: Env) => AuthKvStore
  clientIp?: (request: Request) => string
  consumeProof: (
    env: Env,
    proof: OmegaNostrVerifiedProof,
  ) => Promise<OmegaNostrProofConsumption>
  isCanonicalUser: (user: User) => boolean
  linkIdentity: (
    env: Env,
    ownerRef: string,
    pubkey: string,
  ) => Promise<OmegaNostrIdentityLinkResult>
  now?: () => number
  ownerRefFromUser: (user: User) => string
  requireUserBearerSession: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Promise<Readonly<{ user: User }> | undefined>
  verifyProof: (
    request: Request,
    env: Env,
    payload: Uint8Array,
  ) => Promise<
    OmegaNostrVerifiedProof | Readonly<{ _tag: 'Rejected'; response: Response }>
  >
}>

const noStoreJson = (body: unknown, status: number): Response =>
  Response.json(body, {
    status,
    headers: {
      'cache-control': 'no-store',
      'content-type': 'application/json; charset=utf-8',
    },
  })

const randomChallenge = (): string => {
  const bytes = crypto.getRandomValues(new Uint8Array(32))
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replace(/=+$/u, '')
}

const sha256Hex = async (value: string): Promise<string> =>
  stableRateLimitSubject(value)

const audit = <User, Env>(
  dependencies: OmegaNostrDeviceLinkDependencies<User, Env>,
  event: AuditEvent,
  fields: Readonly<Record<string, string>> = {},
): void => {
  try {
    dependencies.audit?.(event, fields)
  } catch {
    // Audit transport failure cannot change an identity decision.
  }
}

const readBoundedPayload = async (
  request: Request,
): Promise<Uint8Array | undefined> => {
  const contentType = request.headers.get('content-type')?.toLowerCase() ?? ''
  const contentLength = Number(request.headers.get('content-length') ?? '0')
  if (
    !contentType.startsWith('application/json') ||
    !Number.isFinite(contentLength) ||
    contentLength > MAX_REQUEST_BYTES
  ) {
    return undefined
  }
  const payload = new Uint8Array(await request.clone().arrayBuffer())
  return payload.byteLength <= MAX_REQUEST_BYTES ? payload : undefined
}

const parseStoredChallenge = (
  value: string | null | undefined,
): StoredChallenge | undefined => {
  const record = safeJsonRecord(value)
  if (
    record?.schemaVersion !== 1 ||
    typeof record.deviceRef !== 'string' ||
    typeof record.expiresAtMs !== 'number' ||
    !Number.isFinite(record.expiresAtMs) ||
    typeof record.ownerRef !== 'string' ||
    typeof record.pubkey !== 'string'
  ) {
    return undefined
  }
  return record as StoredChallenge
}

export const makeOmegaNostrDeviceLinkService = <User, Env>(
  dependencies: OmegaNostrDeviceLinkDependencies<User, Env>,
) => {
  const issueChallenge = async (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> => {
    if (request.method !== 'POST') {
      return new Response(null, { headers: { allow: 'POST' }, status: 405 })
    }

    const session = await dependencies.requireUserBearerSession(
      request,
      env,
      ctx,
    )
    if (session === undefined) {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
    if (!dependencies.isCanonicalUser(session.user)) {
      audit(dependencies, 'challenge_rejected', {
        reason: 'canonical_account_required',
      })
      return noStoreJson({ error: 'canonical_account_required' }, 403)
    }

    const payload = await readBoundedPayload(request).catch(() => undefined)
    if (payload === undefined) {
      return noStoreJson({ error: 'invalid_nostr_device_link_challenge' }, 400)
    }

    let deviceRef: string
    let pubkey: string
    try {
      const decoded = decodeOmegaNostrDeviceLinkChallengeRequest(
        JSON.parse(new TextDecoder().decode(payload)) as unknown,
      )
      deviceRef = decoded.deviceRef
      pubkey = decoded.pubkey
    } catch {
      return noStoreJson({ error: 'invalid_nostr_device_link_challenge' }, 400)
    }

    const ownerRef = dependencies.ownerRefFromUser(session.user)
    const [ownerDigest, pubkeyDigest, deviceDigest, ipDigest] =
      await Promise.all([
        sha256Hex(ownerRef),
        sha256Hex(pubkey),
        sha256Hex(deviceRef),
        sha256Hex((dependencies.clientIp ?? clientIpFromRequest)(request)),
      ])
    const nowMs = (dependencies.now ?? Date.now)()

    try {
      const store = dependencies.authStore(env)
      const rate = await reserveKvWindowRateLimit(
        store,
        RATE_KEY_PREFIX,
        [
          {
            limit: 10,
            scope: 'account',
            subject: ownerDigest,
            windowSeconds: 3_600,
          },
          {
            limit: 5,
            scope: 'pubkey',
            subject: pubkeyDigest,
            windowSeconds: 3_600,
          },
          {
            limit: 30,
            scope: 'ip',
            subject: ipDigest,
            windowSeconds: 3_600,
          },
        ],
        nowMs,
      )
      if (rate._tag === 'RateLimited') {
        audit(dependencies, 'challenge_rate_limited', {
          ownerDigest,
          pubkeyDigest,
          scope: rate.scope,
        })
        return new Response(
          JSON.stringify({
            error: 'nostr_device_link_rate_limited',
            retryAfterSeconds: rate.retryAfterSeconds,
          }),
          {
            status: 429,
            headers: {
              'cache-control': 'no-store',
              'content-type': 'application/json; charset=utf-8',
              'retry-after': String(rate.retryAfterSeconds),
            },
          },
        )
      }

      const challenge = randomChallenge()
      const expiresAtMs =
        nowMs + OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_TTL_SECONDS * 1_000
      await store.put(
        `${CHALLENGE_KEY_PREFIX}${await sha256Hex(challenge)}`,
        JSON.stringify({
          deviceRef,
          expiresAtMs,
          ownerRef,
          pubkey,
          schemaVersion: 1,
        } satisfies StoredChallenge),
        { expirationTtl: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_TTL_SECONDS },
      )
      audit(dependencies, 'challenge_issued', {
        deviceDigest,
        ownerDigest,
        pubkeyDigest,
      })
      return noStoreJson(
        {
          schema: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_PROTOCOL_VERSION,
          challenge,
          expiresAtMs,
          ownerRef,
        },
        201,
      )
    } catch {
      audit(dependencies, 'challenge_storage_unavailable', {
        ownerDigest,
        pubkeyDigest,
      })
      return noStoreJson({ error: 'nostr_device_link_unavailable' }, 503)
    }
  }

  const link = async (request: Request, env: Env): Promise<Response> => {
    if (request.method !== 'POST') {
      return new Response(null, { headers: { allow: 'POST' }, status: 405 })
    }

    const payload = await readBoundedPayload(request).catch(() => undefined)
    if (payload === undefined) {
      return noStoreJson({ error: 'invalid_nostr_device_link' }, 400)
    }

    let challenge: string
    let deviceRef: string
    let ownerRef: string
    try {
      const decoded = decodeOmegaNostrDeviceLinkRequest(
        JSON.parse(new TextDecoder().decode(payload)) as unknown,
      )
      challenge = decoded.challenge
      deviceRef = decoded.deviceRef
      ownerRef = decoded.ownerRef
    } catch {
      return noStoreJson({ error: 'invalid_nostr_device_link' }, 400)
    }

    if (
      request.headers.get(OMEGA_NOSTR_DEVICE_LINK_DEVICE_HEADER) !== deviceRef
    ) {
      audit(dependencies, 'link_rejected', { reason: 'device_mismatch' })
      return noStoreJson({ error: 'unauthorized' }, 401)
    }

    const verified = await dependencies.verifyProof(request, env, payload)
    if (verified._tag === 'Rejected') return verified.response

    const [challengeDigest, ownerDigest, deviceDigest] = await Promise.all([
      sha256Hex(challenge),
      sha256Hex(ownerRef),
      sha256Hex(deviceRef),
    ])
    const nowMs = (dependencies.now ?? Date.now)()

    try {
      const store = dependencies.authStore(env)
      const issuedKey = `${CHALLENGE_KEY_PREFIX}${challengeDigest}`
      const stored = parseStoredChallenge(await store.get(issuedKey, 'text'))
      const challengeWasConsumed =
        (await store.get(
          `${CONSUMED_KEY_PREFIX}${challengeDigest}`,
          'text',
        )) !== null
      if (challengeWasConsumed) {
        audit(dependencies, 'link_rejected', {
          ownerDigest,
          pubkeyDigest: verified.pubkeyDigest,
          reason: 'challenge_replayed',
        })
        return noStoreJson({ error: 'nostr_device_link_replayed' }, 409)
      }
      if (
        stored === undefined ||
        stored.expiresAtMs <= nowMs ||
        stored.ownerRef !== ownerRef ||
        stored.deviceRef !== deviceRef ||
        stored.pubkey !== verified.pubkey
      ) {
        audit(dependencies, 'link_rejected', {
          ownerDigest,
          pubkeyDigest: verified.pubkeyDigest,
          reason: 'invalid_or_expired_challenge',
        })
        return noStoreJson({ error: 'unauthorized' }, 401)
      }

      const consumedProof = await dependencies.consumeProof(env, verified)
      if (consumedProof._tag === 'Rejected') return consumedProof.response
      const consumedChallenge = await store.putIfAbsent(
        `${CONSUMED_KEY_PREFIX}${challengeDigest}`,
        'consumed',
        { expirationTtl: OMEGA_NOSTR_DEVICE_LINK_CHALLENGE_TTL_SECONDS },
      )
      if (!consumedChallenge) {
        audit(dependencies, 'link_rejected', {
          ownerDigest,
          pubkeyDigest: verified.pubkeyDigest,
          reason: 'challenge_replayed',
        })
        return noStoreJson({ error: 'nostr_device_link_replayed' }, 409)
      }
      await store.delete(issuedKey).catch(() => undefined)

      const result = await dependencies.linkIdentity(
        env,
        ownerRef,
        verified.pubkey,
      )
      if (result === 'Conflict') {
        audit(dependencies, 'link_conflict', {
          ownerDigest,
          pubkeyDigest: verified.pubkeyDigest,
        })
        return noStoreJson({ error: 'nostr_identity_link_conflict' }, 409)
      }
      if (result === 'Unavailable') {
        audit(dependencies, 'link_storage_unavailable', {
          ownerDigest,
          pubkeyDigest: verified.pubkeyDigest,
        })
        return noStoreJson({ error: 'nostr_device_link_unavailable' }, 503)
      }

      audit(
        dependencies,
        result === 'Linked' ? 'link_succeeded' : 'link_idempotent',
        {
          deviceDigest,
          ownerDigest,
          pubkeyDigest: verified.pubkeyDigest,
        },
      )
      return noStoreJson(
        {
          schema: OMEGA_NOSTR_DEVICE_LINK_PROTOCOL_VERSION,
          linked: true,
          ownerRef,
        },
        200,
      )
    } catch {
      audit(dependencies, 'link_storage_unavailable', {
        ownerDigest,
        pubkeyDigest: verified.pubkeyDigest,
      })
      return noStoreJson({ error: 'nostr_device_link_unavailable' }, 503)
    }
  }

  return { issueChallenge, link }
}
