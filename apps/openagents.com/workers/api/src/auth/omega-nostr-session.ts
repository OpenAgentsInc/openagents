import {
  unpackEventFromToken,
  validateEventMethodTag,
  validateEventPayloadTag,
  validateEventUrlTag,
  verifyHttpAuthEvent,
} from 'nostr-effect/nip98'

import type { AuthKvStore } from './auth-kv'

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

export const makeOmegaNostrSessionHandler =
  <User, Env>(dependencies: {
    authStore: (env: Env) => AuthKvStore
    expectedOwnerPubkey: (env: Env) => string | undefined
    now?: () => Date
    resolveOwner: (env: Env) => Promise<User | undefined>
  }) =>
  async (request: Request, env: Env): Promise<Response> => {
    if (request.method !== 'POST') {
      return new Response(null, {
        headers: { allow: 'POST' },
        status: 405,
      })
    }

    const expectedOwnerPubkey = dependencies
      .expectedOwnerPubkey(env)
      ?.trim()
      .toLowerCase()
    if (!expectedOwnerPubkey?.match(/^[0-9a-f]{64}$/)) {
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
      if (
        event.content !== '' ||
        event.pubkey.toLowerCase() !== expectedOwnerPubkey ||
        !verifyHttpAuthEvent(event) ||
        !validateEventUrlTag(event, request.url) ||
        !validateEventMethodTag(event, request.method) ||
      !validateEventPayloadTag(event, new Uint8Array()) ||
        !Number.isInteger(event.created_at) ||
        Math.abs(nowSeconds - event.created_at) > MAX_CLOCK_SKEW_SECONDS
      ) {
        return noStoreJson({ error: 'unauthorized' }, 401)
      }

      const store = dependencies.authStore(env)
      const proofKey = `${PROOF_KEY_PREFIX}${event.id.toLowerCase()}`
      if ((await store.get(proofKey)) !== null) {
        return noStoreJson({ error: 'omega_nostr_proof_replayed' }, 409)
      }
      const owner = await dependencies.resolveOwner(env)
      if (owner === undefined) {
        return noStoreJson({ error: 'omega_nostr_owner_unavailable' }, 503)
      }

      await store.put(proofKey, 'consumed', {
        expirationTtl: MAX_CLOCK_SKEW_SECONDS * 2,
      })
      const accessToken = randomToken()
      await store.put(
        await sessionKey(accessToken),
        JSON.stringify({
          schemaVersion: 1,
          user: owner,
        } satisfies StoredOmegaNostrSession<User>),
        { expirationTtl: OMEGA_NOSTR_SESSION_TTL_SECONDS },
      )

      return noStoreJson({
        accessToken,
        expiresIn: OMEGA_NOSTR_SESSION_TTL_SECONDS,
        user: owner,
      })
    } catch {
      return noStoreJson({ error: 'unauthorized' }, 401)
    }
  }
