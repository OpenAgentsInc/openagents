import {
  unpackEventFromToken,
  validateEventMethodTag,
  validateEventPayloadTag,
  validateEventUrlTag,
  verifyHttpAuthEvent,
} from 'nostr-effect/nip98'

import type { AuthKvStore } from './auth-kv'
import { clientIpFromRequest } from './kv-window-rate-limit'
import type { OmegaNostrSelfProvisionReservation } from './omega-nostr-self-provision'

export const OMEGA_NOSTR_SESSION_PATH = '/api/omega/auth/session'
export const OMEGA_NOSTR_SESSION_TOKEN_PREFIX = 'oa_omega_'
export const OMEGA_NOSTR_SESSION_TTL_SECONDS = 15 * 60

const SESSION_KEY_PREFIX = 'omega-nostr:session:'
const PROOF_KEY_PREFIX = 'omega-nostr:proof:'
const MAX_CLOCK_SKEW_SECONDS = 60
const REQUIRED_NIP98_TAGS = ['u', 'method', 'payload'] as const

type StoredOmegaNostrSession<User> = Readonly<{
  schemaVersion: 1
  user: User
}>

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )
  return Array.from(new Uint8Array(digest), byte =>
    byte.toString(16).padStart(2, '0'),
  ).join('')
}

const sessionKey = async (token: string): Promise<string> =>
  `${SESSION_KEY_PREFIX}${await sha256Hex(token)}`

const randomToken = (): string => {
  const bytes = crypto.getRandomValues(new Uint8Array(32))
  const encoded = btoa(String.fromCharCode(...bytes))
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')
  return `${OMEGA_NOSTR_SESSION_TOKEN_PREFIX}${encoded}`
}

const noStoreJson = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'application/json; charset=utf-8',
    },
    status,
  })

export const readOmegaNostrSession = async <User>(
  store: AuthKvStore,
  token: string,
): Promise<User | undefined> => {
  if (!token.startsWith(OMEGA_NOSTR_SESSION_TOKEN_PREFIX)) {
    return undefined
  }
  const stored = await store.get(await sessionKey(token), 'json')
  if (
    typeof stored !== 'object' ||
    stored === null ||
    !('schemaVersion' in stored) ||
    stored.schemaVersion !== 1 ||
    !('user' in stored)
  ) {
    return undefined
  }
  return (stored as StoredOmegaNostrSession<User>).user
}

/**
 * Self-provisioning hooks. Supplying them preserves the current per-install
 * account rule. Omitting them keeps the configured-owner-only rule.
 */
export type OmegaNostrSelfProvisionHooks<User, Env> = Readonly<{
  enabled: (env: Env) => boolean
  provision: (env: Env, pubkey: string) => Promise<User | undefined>
  reserve: (
    env: Env,
    input: Readonly<{ creating: boolean; ipAddress: string; pubkey: string }>,
  ) => Promise<OmegaNostrSelfProvisionReservation>
  userExists: (env: Env, pubkey: string) => Promise<boolean>
}>

export type OmegaNostrSessionAuditEvent =
  | 'proof_accepted'
  | 'proof_rejected'
  | 'proof_replayed'
  | 'session_issued'
  | 'storage_unavailable'

export type OmegaNostrSessionIssue<User> =
  | Readonly<{
      _tag: 'Issued'
      accessToken: string
      expiresIn: number
      pubkey: string
      user: User
    }>
  | Readonly<{ _tag: 'Rejected'; response: Response }>

export type OmegaNostrSessionDependencies<User, Env> = Readonly<{
  audit?: (
    event: OmegaNostrSessionAuditEvent,
    fields: Readonly<Record<string, string | boolean>>,
  ) => void
  authStore: (env: Env) => AuthKvStore
  clientIp?: (request: Request) => string
  expectedOwnerPubkey: (env: Env) => string | undefined
  linkOwner?: (env: Env, user: User, pubkey: string) => Promise<boolean>
  now?: () => Date
  resolveOwner: (env: Env) => Promise<User | undefined>
  selfProvision?: OmegaNostrSelfProvisionHooks<User, Env>
}>

const audit = <User, Env>(
  dependencies: OmegaNostrSessionDependencies<User, Env>,
  event: OmegaNostrSessionAuditEvent,
  fields: Readonly<Record<string, string | boolean>>,
): void => {
  try {
    dependencies.audit?.(event, fields)
  } catch {
    // Audit transport failure cannot disclose or change an auth decision.
  }
}

const exactRequiredTags = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
): boolean =>
  REQUIRED_NIP98_TAGS.every(
    name => tags.filter(tag => tag[0] === name).length === 1,
  )

export const makeOmegaNostrSessionService = <User, Env>(
  dependencies: OmegaNostrSessionDependencies<User, Env>,
) => {
  const verify = async (
    request: Request,
    env: Env,
    payload: Uint8Array,
  ): Promise<
    | Readonly<{
        _tag: 'Verified'
        eventId: string
        isOwner: boolean
        pubkey: string
        pubkeyDigest: string
      }>
    | Readonly<{ _tag: 'Rejected'; response: Response }>
  > => {
    if (request.method !== 'POST') {
      return {
        _tag: 'Rejected',
        response: new Response(null, {
          headers: { allow: 'POST' },
          status: 405,
        }),
      }
    }

    const configuredOwnerPubkey = dependencies
      .expectedOwnerPubkey(env)
      ?.trim()
      .toLowerCase()
    const expectedOwnerPubkey = configuredOwnerPubkey?.match(/^[0-9a-f]{64}$/u)
      ? configuredOwnerPubkey
      : undefined
    const selfProvisionArmed =
      dependencies.selfProvision !== undefined &&
      dependencies.selfProvision.enabled(env)

    if (expectedOwnerPubkey === undefined && !selfProvisionArmed) {
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'omega_nostr_auth_unavailable' }, 503),
      }
    }

    const authorization = request.headers.get('authorization')
    if (!authorization?.startsWith('Nostr ')) {
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'unauthorized' }, 401),
      }
    }

    let event: Awaited<ReturnType<typeof unpackEventFromToken>>
    try {
      event = await unpackEventFromToken(authorization)
      const nowSeconds = Math.floor(
        (dependencies.now?.() ?? new Date()).getTime() / 1_000,
      )
      if (
        event.content !== '' ||
        !verifyHttpAuthEvent(event) ||
        !exactRequiredTags(event.tags) ||
        !validateEventUrlTag(event, request.url) ||
        !validateEventMethodTag(event, request.method) ||
        !validateEventPayloadTag(event, payload) ||
        !Number.isInteger(event.created_at) ||
        Math.abs(nowSeconds - event.created_at) > MAX_CLOCK_SKEW_SECONDS
      ) {
        audit(dependencies, 'proof_rejected', { reason: 'invalid_proof' })
        return {
          _tag: 'Rejected',
          response: noStoreJson({ error: 'unauthorized' }, 401),
        }
      }
    } catch {
      audit(dependencies, 'proof_rejected', { reason: 'invalid_proof' })
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'unauthorized' }, 401),
      }
    }

    const pubkey = event.pubkey.toLowerCase()
    if (!/^[0-9a-f]{64}$/u.test(pubkey)) {
      audit(dependencies, 'proof_rejected', { reason: 'invalid_pubkey' })
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'unauthorized' }, 401),
      }
    }
    const pubkeyDigest = await sha256Hex(pubkey)
    const isOwner =
      expectedOwnerPubkey !== undefined && pubkey === expectedOwnerPubkey

    if (!isOwner && !selfProvisionArmed) {
      audit(dependencies, 'proof_rejected', {
        pubkeyDigest,
        reason: 'not_authorized',
      })
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'unauthorized' }, 401),
      }
    }

    return {
      _tag: 'Verified',
      eventId: event.id.toLowerCase(),
      isOwner,
      pubkey,
      pubkeyDigest,
    }
  }

  const authenticateVerified = async (
    request: Request,
    env: Env,
    verified: Readonly<{
      eventId: string
      isOwner: boolean
      pubkey: string
      pubkeyDigest: string
    }>,
  ): Promise<
    | Readonly<{ _tag: 'Authenticated'; pubkey: string; user: User }>
    | Readonly<{ _tag: 'Rejected'; response: Response }>
  > => {
    const { eventId, isOwner, pubkey, pubkeyDigest } = verified
    try {
      const store = dependencies.authStore(env)
      const proofKey = `${PROOF_KEY_PREFIX}${eventId}`
      const consumed = await store.putIfAbsent(proofKey, 'consumed', {
        expirationTtl: MAX_CLOCK_SKEW_SECONDS * 2,
      })
      if (!consumed) {
        audit(dependencies, 'proof_replayed', { pubkeyDigest })
        return {
          _tag: 'Rejected',
          response: noStoreJson({ error: 'omega_nostr_proof_replayed' }, 409),
        }
      }

      let user: User | undefined

      if (isOwner) {
        user = await dependencies.resolveOwner(env)
        if (user === undefined) {
          return {
            _tag: 'Rejected',
            response: noStoreJson(
              { error: 'omega_nostr_owner_unavailable' },
              503,
            ),
          }
        }
        if (
          dependencies.linkOwner !== undefined &&
          !(await dependencies.linkOwner(env, user, pubkey))
        ) {
          audit(dependencies, 'proof_rejected', {
            pubkeyDigest,
            reason: 'identity_link_conflict',
          })
          return {
            _tag: 'Rejected',
            response: noStoreJson({ error: 'unauthorized' }, 401),
          }
        }
      } else {
        const hooks = dependencies.selfProvision
        if (hooks === undefined) {
          return {
            _tag: 'Rejected',
            response: noStoreJson({ error: 'unauthorized' }, 401),
          }
        }
        const ipAddress = (dependencies.clientIp ?? clientIpFromRequest)(
          request,
        )
        const creating = !(await hooks.userExists(env, pubkey))
        const reservation = await hooks.reserve(env, {
          creating,
          ipAddress,
          pubkey,
        })
        if (reservation._tag === 'RateLimited') {
          return {
            _tag: 'Rejected',
            response: new Response(
              JSON.stringify({
                error: 'omega_nostr_self_provision_rate_limited',
                limit: reservation.limit,
                resetAt: reservation.resetAt,
                retryAfterSeconds: reservation.retryAfterSeconds,
                scope: reservation.scope,
              }),
              {
                headers: {
                  'cache-control': 'no-store',
                  'content-type': 'application/json; charset=utf-8',
                  'retry-after': String(reservation.retryAfterSeconds),
                },
                status: 429,
              },
            ),
          }
        }
      }

      if (!isOwner) {
        user = await dependencies.selfProvision?.provision(env, pubkey)
        if (user === undefined) {
          return {
            _tag: 'Rejected',
            response: noStoreJson(
              { error: 'omega_nostr_self_provision_unavailable' },
              503,
            ),
          }
        }
      }

      audit(dependencies, 'proof_accepted', { isOwner, pubkeyDigest })
      return { _tag: 'Authenticated', pubkey, user: user as User }
    } catch {
      audit(dependencies, 'storage_unavailable', { pubkeyDigest })
      return {
        _tag: 'Rejected',
        response: noStoreJson(
          { error: 'omega_nostr_auth_storage_unavailable' },
          503,
        ),
      }
    }
  }

  const authenticate = async (
    request: Request,
    env: Env,
    payload: Uint8Array,
  ): Promise<
    | Readonly<{ _tag: 'Authenticated'; pubkey: string; user: User }>
    | Readonly<{ _tag: 'Rejected'; response: Response }>
  > => {
    const verified = await verify(request, env, payload)
    return verified._tag === 'Rejected'
      ? verified
      : authenticateVerified(request, env, verified)
  }

  const mint = async (
    env: Env,
    authenticated: Readonly<{ pubkey: string; user: User }>,
  ): Promise<OmegaNostrSessionIssue<User>> => {
    const pubkeyDigest = await sha256Hex(authenticated.pubkey)
    try {
      const accessToken = randomToken()
      await dependencies.authStore(env).put(
        await sessionKey(accessToken),
        JSON.stringify({
          schemaVersion: 1,
          user: authenticated.user,
        } satisfies StoredOmegaNostrSession<User>),
        { expirationTtl: OMEGA_NOSTR_SESSION_TTL_SECONDS },
      )
      audit(dependencies, 'session_issued', { pubkeyDigest })
      return {
        _tag: 'Issued',
        accessToken,
        expiresIn: OMEGA_NOSTR_SESSION_TTL_SECONDS,
        pubkey: authenticated.pubkey,
        user: authenticated.user,
      }
    } catch {
      audit(dependencies, 'storage_unavailable', { pubkeyDigest })
      return {
        _tag: 'Rejected',
        response: noStoreJson(
          { error: 'omega_nostr_auth_storage_unavailable' },
          503,
        ),
      }
    }
  }

  const issue = async (
    request: Request,
    env: Env,
    payload: Uint8Array,
  ): Promise<OmegaNostrSessionIssue<User>> => {
    const authenticated = await authenticate(request, env, payload)
    return authenticated._tag === 'Rejected'
      ? authenticated
      : mint(env, authenticated)
  }

  const handle = async (request: Request, env: Env): Promise<Response> => {
    let payload: Uint8Array
    try {
      payload = new Uint8Array(await request.clone().arrayBuffer())
    } catch {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
    if (payload.byteLength !== 0) {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
    const issued = await issue(request, env, payload)
    return issued._tag === 'Rejected'
      ? issued.response
      : noStoreJson({
          accessToken: issued.accessToken,
          expiresIn: issued.expiresIn,
          user: issued.user,
        })
  }

  return { authenticate, authenticateVerified, handle, issue, mint, verify }
}

export const makeOmegaNostrSessionHandler = <User, Env>(
  dependencies: OmegaNostrSessionDependencies<User, Env>,
) => makeOmegaNostrSessionService(dependencies).handle
