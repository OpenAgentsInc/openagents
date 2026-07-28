import {
  SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
  decodeSarahVoiceNostrChallengeRequest,
} from '@openagentsinc/audio-contract'

import type { AuthKvStore } from './auth/auth-kv'
import {
  clientIpFromRequest,
  reserveKvWindowRateLimit,
  stableRateLimitSubject,
} from './auth/kv-window-rate-limit'
import { safeJsonRecord } from './json-boundary'

export const SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS = 120

const CHALLENGE_KEY_PREFIX = 'sarah-voice:nostr-challenge:issued:'
const CONSUMED_KEY_PREFIX = 'sarah-voice:nostr-challenge:consumed:'
const RATE_KEY_PREFIX = 'sarah-voice:nostr-challenge-rate'
const MAX_REQUEST_BYTES = 1_024

type SarahVoiceNostrChallengeAuditEvent =
  | 'challenge_consumed'
  | 'challenge_issued'
  | 'challenge_rejected'
  | 'challenge_storage_unavailable'

export type SarahVoiceNostrChallengeDependencies<Env> = Readonly<{
  audit?: (
    event: SarahVoiceNostrChallengeAuditEvent,
    fields: Readonly<Record<string, string>>,
  ) => void
  authStore: (env: Env) => AuthKvStore
  clientIp?: (request: Request) => string
  enabled: (env: Env) => boolean
  now?: () => number
  resolveOwnerRef: (env: Env, pubkey: string) => Promise<string | undefined>
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

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )
  return Array.from(new Uint8Array(digest), byte =>
    byte.toString(16).padStart(2, '0'),
  ).join('')
}

const audit = <Env>(
  dependencies: SarahVoiceNostrChallengeDependencies<Env>,
  event: SarahVoiceNostrChallengeAuditEvent,
  fields: Readonly<Record<string, string>> = {},
): void => {
  try {
    dependencies.audit?.(event, fields)
  } catch {
    // Audit transport failure cannot change challenge admission.
  }
}

export const makeSarahVoiceNostrChallengeService = <Env>(
  dependencies: SarahVoiceNostrChallengeDependencies<Env>,
) => {
  const issue = async (request: Request, env: Env): Promise<Response> => {
    if (request.method !== 'POST') {
      return new Response(null, {
        headers: { allow: 'POST' },
        status: 405,
      })
    }
    if (!dependencies.enabled(env)) {
      return noStoreJson({ error: 'sarah_voice_unavailable' }, 503)
    }

    const contentLength = Number(request.headers.get('content-length') ?? '0')
    if (!Number.isFinite(contentLength) || contentLength > MAX_REQUEST_BYTES) {
      return noStoreJson({ error: 'invalid_sarah_voice_auth_challenge' }, 400)
    }

    let deviceRef: string
    let pubkey: string
    try {
      const text = await request.text()
      if (new TextEncoder().encode(text).byteLength > MAX_REQUEST_BYTES) {
        return noStoreJson({ error: 'invalid_sarah_voice_auth_challenge' }, 400)
      }
      const decoded = decodeSarahVoiceNostrChallengeRequest(
        JSON.parse(text) as unknown,
      )
      deviceRef = decoded.deviceRef
      pubkey = decoded.pubkey.toLowerCase()
    } catch {
      return noStoreJson({ error: 'invalid_sarah_voice_auth_challenge' }, 400)
    }

    const nowMs = (dependencies.now ?? Date.now)()
    try {
      const store = dependencies.authStore(env)
      const ipAddress = (dependencies.clientIp ?? clientIpFromRequest)(request)
      const ipSubject = await stableRateLimitSubject(ipAddress)
      const rate = await reserveKvWindowRateLimit(
        store,
        RATE_KEY_PREFIX,
        [
          {
            limit: 30,
            scope: 'ip',
            subject: ipSubject,
            windowSeconds: 60,
          },
          {
            limit: 2_000,
            scope: 'global',
            subject: 'all',
            windowSeconds: 60,
          },
        ],
        nowMs,
      )
      if (rate._tag === 'RateLimited') {
        return new Response(
          JSON.stringify({
            error: 'sarah_voice_auth_challenge_rate_limited',
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

      const ownerRef = await dependencies.resolveOwnerRef(env, pubkey)
      if (ownerRef === undefined) {
        audit(dependencies, 'challenge_rejected', {
          reason: 'identity_not_authorized',
        })
        return noStoreJson({ error: 'unauthorized' }, 401)
      }

      const challenge = randomChallenge()
      const digest = await sha256Hex(challenge)
      const expiresAtMs =
        nowMs + SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS * 1_000
      await store.put(
        `${CHALLENGE_KEY_PREFIX}${digest}`,
        JSON.stringify({
          deviceRef,
          expiresAtMs,
          ownerRef,
          pubkey,
          schemaVersion: 1,
        }),
        { expirationTtl: SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS },
      )
      audit(dependencies, 'challenge_issued')
      return noStoreJson(
        {
          schema: SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
          challenge,
          expiresAtMs,
          ownerRef,
        },
        201,
      )
    } catch {
      audit(dependencies, 'challenge_storage_unavailable')
      return noStoreJson(
        { error: 'sarah_voice_auth_challenge_unavailable' },
        503,
      )
    }
  }

  const consume = async (
    env: Env,
    input: Readonly<{
      challenge: string
      deviceRef: string
      ownerRef: string
      pubkey: string
    }>,
  ): Promise<'Consumed' | 'Invalid' | 'Unavailable'> => {
    if (!/^[A-Za-z0-9_-]{32,256}$/u.test(input.challenge)) return 'Invalid'
    const nowMs = (dependencies.now ?? Date.now)()
    try {
      const store = dependencies.authStore(env)
      const digest = await sha256Hex(input.challenge)
      const issuedKey = `${CHALLENGE_KEY_PREFIX}${digest}`
      const stored = safeJsonRecord(await store.get(issuedKey, 'text'))
      if (
        stored?.schemaVersion !== 1 ||
        stored.deviceRef !== input.deviceRef ||
        stored.ownerRef !== input.ownerRef ||
        stored.pubkey !== input.pubkey.toLowerCase() ||
        typeof stored.expiresAtMs !== 'number' ||
        !Number.isFinite(stored.expiresAtMs) ||
        stored.expiresAtMs <= nowMs
      ) {
        audit(dependencies, 'challenge_rejected', {
          reason: 'invalid_or_expired',
        })
        return 'Invalid'
      }
      const consumed = await store.putIfAbsent(
        `${CONSUMED_KEY_PREFIX}${digest}`,
        'consumed',
        {
          expirationTtl: SARAH_VOICE_NOSTR_CHALLENGE_TTL_SECONDS,
        },
      )
      if (!consumed) {
        audit(dependencies, 'challenge_rejected', { reason: 'replayed' })
        return 'Invalid'
      }
      await store.delete(issuedKey).catch(() => undefined)
      audit(dependencies, 'challenge_consumed')
      return 'Consumed'
    } catch {
      audit(dependencies, 'challenge_storage_unavailable')
      return 'Unavailable'
    }
  }

  return { consume, issue }
}
