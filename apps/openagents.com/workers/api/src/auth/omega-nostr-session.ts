import {
  unpackEventFromToken,
  validateEventMethodTag,
  validateEventPayloadTag,
  validateEventUrlTag,
  verifyHttpAuthEvent,
} from 'nostr-effect/nip98'

import {
  type PostgresFailureClass,
  postgresFailureClass,
  postgresFailureCode,
  retryTransientPostgres,
} from '../postgres-transient-failure'
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

/**
 * The step that failed. Carried in the log and in the 503 body so an operator
 * never has to guess WHICH storage read or write went down.
 */
export type OmegaNostrStoragePhase =
  | 'resolve_linked'
  | 'consume_proof'
  | 'authenticate'
  | 'mint_session'

/**
 * The typed 503 codes for an auth-storage failure, one per named cause.
 *
 * INCIDENT 2026-07-31 (P0). All four of these used to be the single string
 * `omega_nostr_auth_storage_unavailable`, emitted from four different `catch {}`
 * blocks that discarded the error entirely. The owner saw a red banner and the
 * server logs carried nothing but a pubkey digest — the failure was
 * undiagnosable by construction. A code that collapses several causes into one
 * string is part of the defect, so each cause now has its own code:
 *
 * - `..._unreachable` — the connection was never established (the Cloud SQL
 *   Auth Proxy on a cold Cloud Run instance). Transient, self-healing.
 * - `..._connection_lost` — an established connection went away mid-operation.
 * - `..._rejected` — Postgres answered and refused. NOT self-healing; this one
 *   means a real defect (missing relation, permission, constraint).
 * - `..._unavailable` — retained as the fallback for an unclassified cause, so
 *   already-shipped clients keep the code they were written against.
 *
 * Every one of them is a 503 and every one is retryable EXCEPT `_rejected`,
 * which is reported honestly rather than inviting a client to hammer it.
 */
export const omegaNostrStorageErrorCode = (
  failureClass: PostgresFailureClass,
): string => {
  switch (failureClass) {
    case 'connect_unavailable':
      return 'omega_nostr_auth_storage_unreachable'
    case 'connection_lost':
      return 'omega_nostr_auth_storage_connection_lost'
    case 'server_error':
      return 'omega_nostr_auth_storage_rejected'
    case 'unknown':
      return 'omega_nostr_auth_storage_unavailable'
  }
}

const STORAGE_RETRY_AFTER_SECONDS = 2

const storageUnavailableResponse = (error: unknown): Response => {
  const failureClass = postgresFailureClass(error)
  const retryable = failureClass !== 'server_error'
  const body = {
    cause: failureClass,
    error: omegaNostrStorageErrorCode(failureClass),
    retryable,
  }
  return new Response(JSON.stringify(body), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'application/json; charset=utf-8',
      ...(retryable
        ? { 'retry-after': String(STORAGE_RETRY_AFTER_SECONDS) }
        : {}),
    },
    status: 503,
  })
}

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
  | 'storage_retry'
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

export type OmegaNostrVerifiedProof = Readonly<{
  _tag: 'Verified'
  eventId: string
  isOwner: boolean
  pubkey: string
  pubkeyDigest: string
}>

export type OmegaNostrProofConsumption =
  | Readonly<{ _tag: 'Consumed' }>
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
  resolveLinked?: (env: Env, pubkey: string) => Promise<User | undefined>
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
  /**
   * Name the storage failure in the audit log AND in the response.
   *
   * Replaces four separate `catch {}` blocks that threw the error away. The
   * fields are all public-safe: a failure class, postgres.js's own connection
   * code, the error's class name, and which step failed — never a message,
   * never a row value, never a key.
   */
  const storageFailure = (
    phase: OmegaNostrStoragePhase,
    error: unknown,
    fields: Readonly<Record<string, string | boolean>> = {},
  ): Readonly<{ _tag: 'Rejected'; response: Response }> => {
    audit(dependencies, 'storage_unavailable', {
      ...fields,
      errorName: error instanceof Error ? error.name : typeof error,
      failureClass: postgresFailureClass(error),
      failureCode: postgresFailureCode(error),
      phase,
    })
    return { _tag: 'Rejected', response: storageUnavailableResponse(error) }
  }

  const verifyProof = async (
    request: Request,
    env: Env,
    payload: Uint8Array,
  ): Promise<
    OmegaNostrVerifiedProof | Readonly<{ _tag: 'Rejected'; response: Response }>
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
    return {
      _tag: 'Verified',
      eventId: event.id.toLowerCase(),
      isOwner,
      pubkey,
      pubkeyDigest,
    }
  }

  const verify = async (
    request: Request,
    env: Env,
    payload: Uint8Array,
  ): Promise<
    OmegaNostrVerifiedProof | Readonly<{ _tag: 'Rejected'; response: Response }>
  > => {
    const configuredOwnerPubkey = dependencies
      .expectedOwnerPubkey(env)
      ?.trim()
      .toLowerCase()
    const hasConfiguredOwner =
      configuredOwnerPubkey !== undefined &&
      /^[0-9a-f]{64}$/u.test(configuredOwnerPubkey)
    const selfProvisionArmed =
      dependencies.selfProvision !== undefined &&
      dependencies.selfProvision.enabled(env)
    if (
      !hasConfiguredOwner &&
      !selfProvisionArmed &&
      dependencies.resolveLinked === undefined
    ) {
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'omega_nostr_auth_unavailable' }, 503),
      }
    }

    const verified = await verifyProof(request, env, payload)
    if (verified._tag === 'Rejected') return verified

    let linked: User | undefined
    try {
      // A pure read of the identity binding. It does NOT go through the auth
      // KV store, so it needs its own transient-connect retry: on a cold Cloud
      // Run instance this is the FIRST statement a sign-in attempts, and it is
      // the one that met the not-yet-ready Cloud SQL proxy on 2026-07-31.
      linked = await retryTransientPostgres(
        async () => dependencies.resolveLinked?.(env, verified.pubkey),
        {
          onRetry: info => {
            audit(dependencies, 'storage_retry', {
              attempt: String(info.attempt),
              delayMs: String(info.delayMs),
              failureClass: info.failureClass,
              failureCode: info.failureCode,
              phase: 'resolve_linked',
              pubkeyDigest: verified.pubkeyDigest,
            })
          },
          scope: 'connection',
        },
      )
    } catch (error) {
      return storageFailure('resolve_linked', error, {
        pubkeyDigest: verified.pubkeyDigest,
      })
    }
    if (!verified.isOwner && linked === undefined && !selfProvisionArmed) {
      audit(dependencies, 'proof_rejected', {
        pubkeyDigest: verified.pubkeyDigest,
        reason: 'not_authorized',
      })
      return {
        _tag: 'Rejected',
        response: noStoreJson({ error: 'unauthorized' }, 401),
      }
    }

    return verified
  }

  const consumeProof = async (
    env: Env,
    verified: OmegaNostrVerifiedProof,
  ): Promise<OmegaNostrProofConsumption> => {
    try {
      const consumed = await dependencies
        .authStore(env)
        .putIfAbsent(`${PROOF_KEY_PREFIX}${verified.eventId}`, 'consumed', {
          expirationTtl: MAX_CLOCK_SKEW_SECONDS * 2,
        })
      if (!consumed) {
        audit(dependencies, 'proof_replayed', {
          pubkeyDigest: verified.pubkeyDigest,
        })
        return {
          _tag: 'Rejected',
          response: noStoreJson({ error: 'omega_nostr_proof_replayed' }, 409),
        }
      }
      return { _tag: 'Consumed' }
    } catch (error) {
      return storageFailure('consume_proof', error, {
        pubkeyDigest: verified.pubkeyDigest,
      })
    }
  }

  const authenticateVerified = async (
    request: Request,
    env: Env,
    verified: OmegaNostrVerifiedProof,
  ): Promise<
    | Readonly<{ _tag: 'Authenticated'; pubkey: string; user: User }>
    | Readonly<{ _tag: 'Rejected'; response: Response }>
  > => {
    const { isOwner, pubkey, pubkeyDigest } = verified
    try {
      const consumed = await consumeProof(env, verified)
      if (consumed._tag === 'Rejected') return consumed

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
        user = await dependencies.resolveLinked?.(env, pubkey)
      }

      if (!isOwner && user === undefined) {
        const hooks = dependencies.selfProvision
        if (hooks === undefined || !hooks.enabled(env)) {
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

      if (!isOwner && user === undefined) {
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
    } catch (error) {
      return storageFailure('authenticate', error, { pubkeyDigest })
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
    } catch (error) {
      return storageFailure('mint_session', error, { pubkeyDigest })
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

  return {
    authenticate,
    authenticateVerified,
    consumeProof,
    handle,
    issue,
    mint,
    verify,
    verifyProof,
  }
}

export const makeOmegaNostrSessionHandler = <User, Env>(
  dependencies: OmegaNostrSessionDependencies<User, Env>,
) => makeOmegaNostrSessionService(dependencies).handle
