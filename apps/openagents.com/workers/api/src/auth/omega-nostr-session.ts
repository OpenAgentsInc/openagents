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
 * Self-provisioning hooks. Supplying them is what turns this endpoint from an
 * owner-only backdoor into a per-install free-tier door; omitting them leaves
 * the historical owner-only behavior byte-for-byte intact (used by the existing
 * tests and by any deployment that has not armed the flag).
 */
export type OmegaNostrSelfProvisionHooks<User, Env> = Readonly<{
  /** Kill switch, read per request so it flips without a redeploy of code. */
  enabled: (env: Env) => boolean
  /** Find-or-create the user keyed on THIS pubkey. Never the admin account. */
  provision: (env: Env, pubkey: string) => Promise<User | undefined>
  /**
   * Charge the abuse buckets. `creating` tells the limiter whether this mint
   * consumes the tight account-CREATION budget or only the looser mint budget.
   */
  reserve: (
    env: Env,
    input: Readonly<{ creating: boolean; ipAddress: string; pubkey: string }>,
  ) => Promise<OmegaNostrSelfProvisionReservation>
  /** Does a `users` row already exist for this pubkey? */
  userExists: (env: Env, pubkey: string) => Promise<boolean>
}>

export const makeOmegaNostrSessionHandler =
  <User, Env>(dependencies: {
    authStore: (env: Env) => AuthKvStore
    clientIp?: (request: Request) => string
    expectedOwnerPubkey: (env: Env) => string | undefined
    now?: () => Date
    resolveOwner: (env: Env) => Promise<User | undefined>
    selfProvision?: OmegaNostrSelfProvisionHooks<User, Env>
  }) =>
  async (request: Request, env: Env): Promise<Response> => {
    if (request.method !== 'POST') {
      return new Response(null, {
        headers: { allow: 'POST' },
        status: 405,
      })
    }

    const configuredOwnerPubkey = dependencies
      .expectedOwnerPubkey(env)
      ?.trim()
      .toLowerCase()
    const expectedOwnerPubkey = configuredOwnerPubkey?.match(/^[0-9a-f]{64}$/)
      ? configuredOwnerPubkey
      : undefined
    // Omega self-provisioning (2026-07-28): armed per request, so the owner can
    // disarm it
    // on a live Cloud Run revision (env var flip) without shipping code.
    const selfProvisionArmed =
      dependencies.selfProvision !== undefined &&
      dependencies.selfProvision.enabled(env)

    // Unchanged from the owner-only era: with NO owner key configured and NO
    // self-provisioning armed, this endpoint can mint nothing at all.
    if (expectedOwnerPubkey === undefined && !selfProvisionArmed) {
      return noStoreJson({ error: 'omega_nostr_auth_unavailable' }, 503)
    }

    const authorization = request.headers.get('authorization')
    if (!authorization?.startsWith('Nostr ')) {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }

    try {
      const event = await unpackEventFromToken(authorization)
      const nowSeconds = Math.floor(
        (dependencies.now?.() ?? new Date()).getTime() / 1_000,
      )
      // SECURITY BOUNDARY — unchanged. Every NIP-98 check that was here before
      // is here now, in the same conjunction: kind + signature
      // (`verifyHttpAuthEvent`), empty content, `u` tag bound to THIS url,
      // method tag, empty-payload hash, integer `created_at`, and clock skew.
      // The ONLY removed term is the `event.pubkey === expectedOwnerPubkey`
      // equality, which was an allowlist of exactly one install, not a
      // cryptographic control.
      if (
        event.content !== '' ||
        !verifyHttpAuthEvent(event) ||
        !validateEventUrlTag(event, request.url) ||
        !validateEventMethodTag(event, request.method) ||
        !validateEventPayloadTag(event, new Uint8Array()) ||
        !Number.isInteger(event.created_at) ||
        Math.abs(nowSeconds - event.created_at) > MAX_CLOCK_SKEW_SECONDS
      ) {
        return noStoreJson({ error: 'unauthorized' }, 401)
      }

      const pubkey = event.pubkey.toLowerCase()
      const isOwner =
        expectedOwnerPubkey !== undefined && pubkey === expectedOwnerPubkey

      // A non-owner key with self-provisioning disarmed gets the historical
      // 401. `403` would leak that the deployment recognises the key shape.
      if (!isOwner && !selfProvisionArmed) {
        return noStoreJson({ error: 'unauthorized' }, 401)
      }

      const store = dependencies.authStore(env)
      const proofKey = `${PROOF_KEY_PREFIX}${event.id.toLowerCase()}`
      if ((await store.get(proofKey)) !== null) {
        return noStoreJson({ error: 'omega_nostr_proof_replayed' }, 409)
      }

      let user: User | undefined
      if (isOwner) {
        // Preserved owner path: still resolves the configured admin identity,
        // still exempt from the self-provision abuse buckets.
        user = await dependencies.resolveOwner(env)
        if (user === undefined) {
          return noStoreJson({ error: 'omega_nostr_owner_unavailable' }, 503)
        }
      } else {
        const hooks = dependencies.selfProvision
        if (hooks === undefined) {
          return noStoreJson({ error: 'unauthorized' }, 401)
        }
        const ipAddress = (dependencies.clientIp ?? clientIpFromRequest)(
          request,
        )
        // Rate limiting runs AFTER full NIP-98 verification so unsigned noise
        // can never burn the shared global creation budget (that would be a
        // trivial denial of service against every new install).
        const creating = !(await hooks.userExists(env, pubkey))
        const reservation = await hooks.reserve(env, {
          creating,
          ipAddress,
          pubkey,
        })
        if (reservation._tag === 'RateLimited') {
          return new Response(
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
          )
        }
        user = await hooks.provision(env, pubkey)
        if (user === undefined) {
          return noStoreJson(
            { error: 'omega_nostr_self_provision_unavailable' },
            503,
          )
        }
      }

      await store.put(proofKey, 'consumed', {
        expirationTtl: MAX_CLOCK_SKEW_SECONDS * 2,
      })
      const accessToken = randomToken()
      await store.put(
        await sessionKey(accessToken),
        JSON.stringify({
          schemaVersion: 1,
          user,
        } satisfies StoredOmegaNostrSession<User>),
        { expirationTtl: OMEGA_NOSTR_SESSION_TTL_SECONDS },
      )

      return noStoreJson({
        accessToken,
        expiresIn: OMEGA_NOSTR_SESSION_TTL_SECONDS,
        user,
      })
    } catch {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
  }
