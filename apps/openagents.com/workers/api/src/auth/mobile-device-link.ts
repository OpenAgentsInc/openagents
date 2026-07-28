import { Schema as S } from 'effect'
import {
  unpackEventFromToken,
  validateEventMethodTag,
  validateEventPayloadTag,
  validateEventUrlTag,
  verifyHttpAuthEvent,
} from 'nostr-effect/nip98'

import type { AuthKvStore } from './auth-kv'
import { readBearerToken } from './bearer-token'
import type { MobileDeviceLinkState } from './mobile-device-link-store'
import { OMEGA_NOSTR_SESSION_TOKEN_PREFIX } from './omega-nostr-session'

export const MOBILE_DEVICE_LINK_PATH = '/api/mobile/auth/device-link'
export const MOBILE_DEVICE_LINK_SCHEMA = 'openagents.mobile.device-link.v1'
export const MOBILE_DEVICE_LINK_PROOF_HEADER =
  'x-openagents-nostr-authorization'

const MAX_CLOCK_SKEW_SECONDS = 60
const PROOF_TTL_SECONDS = MAX_CLOCK_SKEW_SECONDS * 2
const MAX_PROOF_HEADER_BYTES = 8_192
const REQUIRED_NIP98_TAGS = ['u', 'method', 'payload'] as const

const DeviceLinkRequest = S.Struct({
  schema: S.Literal(MOBILE_DEVICE_LINK_SCHEMA),
  pubkey: S.String.check(S.isPattern(/^[0-9a-f]{64}$/u)),
  deviceRef: S.String.check(S.isPattern(/^omega-mobile-[0-9a-f]{24}$/u)),
})

type DeviceLinkRequest = typeof DeviceLinkRequest.Type

export type MobileDeviceLinkAuditEvent =
  | 'link_conflict'
  | 'link_created'
  | 'link_existing'
  | 'proof_rejected'
  | 'proof_replayed'
  | 'session_rejected'
  | 'storage_unavailable'

export type MobileDeviceLinkDependencies<Session, Env> = Readonly<{
  audit?: (
    event: MobileDeviceLinkAuditEvent,
    fields: Readonly<Record<string, string>>,
  ) => void
  authStore: (env: Env) => Pick<AuthKvStore, 'putIfAbsent'>
  link: (
    env: Env,
    input: Readonly<{
      deviceRef: string
      nowIso: string
      ownerRef: string
      pubkey: string
    }>,
  ) => Promise<MobileDeviceLinkState>
  now?: () => Date
  requireUserBearerSession: (
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  userIdFromSession: (session: Session) => string
}>

const noStoreJson = (body: unknown, status: number): Response =>
  new Response(JSON.stringify(body), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'application/json; charset=utf-8',
    },
    status,
  })

const audit = <Session, Env>(
  dependencies: MobileDeviceLinkDependencies<Session, Env>,
  event: MobileDeviceLinkAuditEvent,
  fields: Readonly<Record<string, string>>,
): void => {
  try {
    dependencies.audit?.(event, fields)
  } catch {
    // Audit transport failure cannot change an identity-link decision.
  }
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

const exactRequiredTags = (
  tags: ReadonlyArray<ReadonlyArray<string>>,
): boolean =>
  REQUIRED_NIP98_TAGS.every(
    name => tags.filter(tag => tag[0] === name).length === 1,
  )

const parseBody = (bytes: Uint8Array): DeviceLinkRequest | undefined => {
  try {
    const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes)
    return S.decodeUnknownSync(DeviceLinkRequest)(JSON.parse(text), {
      onExcessProperty: 'error',
    })
  } catch {
    return undefined
  }
}

type VerifiedProof = Readonly<{
  eventId: string
  pubkey: string
  pubkeyDigest: string
}>

const verifyProof = async (
  request: Request,
  body: DeviceLinkRequest,
  payload: Uint8Array,
  now: Date,
): Promise<VerifiedProof | undefined> => {
  const authorization = request.headers.get(MOBILE_DEVICE_LINK_PROOF_HEADER)
  if (
    authorization === null ||
    !authorization.startsWith('Nostr ') ||
    new TextEncoder().encode(authorization).byteLength > MAX_PROOF_HEADER_BYTES
  ) {
    return undefined
  }

  try {
    const event = await unpackEventFromToken(authorization)
    const nowSeconds = Math.floor(now.getTime() / 1_000)
    const pubkey = event.pubkey.toLowerCase()
    if (
      event.content !== '' ||
      !verifyHttpAuthEvent(event) ||
      !exactRequiredTags(event.tags) ||
      !validateEventUrlTag(event, request.url) ||
      !validateEventMethodTag(event, request.method) ||
      !validateEventPayloadTag(event, payload) ||
      !Number.isInteger(event.created_at) ||
      Math.abs(nowSeconds - event.created_at) > MAX_CLOCK_SKEW_SECONDS ||
      pubkey !== body.pubkey
    ) {
      return undefined
    }

    return {
      eventId: event.id.toLowerCase(),
      pubkey,
      pubkeyDigest: await sha256Hex(pubkey),
    }
  } catch {
    return undefined
  }
}

export const handleMobileDeviceLinkRequest = async <Session, Env>(
  dependencies: MobileDeviceLinkDependencies<Session, Env>,
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return new Response(null, { headers: { allow: 'POST' }, status: 405 })
  }

  const accessToken = readBearerToken(request)
  if (
    accessToken === undefined ||
    accessToken.startsWith(OMEGA_NOSTR_SESSION_TOKEN_PREFIX)
  ) {
    audit(dependencies, 'session_rejected', {
      reason: 'mobile_session_required',
    })
    return noStoreJson({ error: 'mobile_session_required' }, 401)
  }

  let session: Session | undefined
  try {
    session = await dependencies.requireUserBearerSession(request, env, ctx)
  } catch {
    audit(dependencies, 'storage_unavailable', {
      reason: 'session_check_failed',
    })
    return noStoreJson({ error: 'device_link_unavailable' }, 503)
  }
  if (session === undefined) {
    audit(dependencies, 'session_rejected', {
      reason: 'mobile_session_required',
    })
    return noStoreJson({ error: 'mobile_session_required' }, 401)
  }

  let payload: Uint8Array
  try {
    payload = new Uint8Array(await request.arrayBuffer())
  } catch {
    audit(dependencies, 'proof_rejected', { reason: 'invalid_body' })
    return noStoreJson({ error: 'device_proof_rejected' }, 403)
  }
  const body = parseBody(payload)
  if (
    body === undefined ||
    body.deviceRef !== `omega-mobile-${body.pubkey.slice(0, 24)}`
  ) {
    audit(dependencies, 'proof_rejected', { reason: 'invalid_body' })
    return noStoreJson({ error: 'device_proof_rejected' }, 403)
  }

  const now = dependencies.now?.() ?? new Date()
  const proof = await verifyProof(request, body, payload, now)
  if (proof === undefined) {
    audit(dependencies, 'proof_rejected', { reason: 'invalid_proof' })
    return noStoreJson({ error: 'device_proof_rejected' }, 403)
  }

  const ownerRef = dependencies.userIdFromSession(session)
  const auditFields = {
    deviceRef: body.deviceRef,
    ownerRef,
    proofRef: `nip98:${proof.eventId}`,
    pubkeyDigest: proof.pubkeyDigest,
  }

  try {
    const consumed = await dependencies.authStore(env).putIfAbsent(
      `mobile-device-link:proof:${proof.eventId}`,
      JSON.stringify({
        deviceRef: body.deviceRef,
        ownerRef,
        pubkeyDigest: proof.pubkeyDigest,
      }),
      { expirationTtl: PROOF_TTL_SECONDS },
    )
    if (!consumed) {
      audit(dependencies, 'proof_replayed', auditFields)
      return noStoreJson({ error: 'device_proof_replayed' }, 409)
    }

    const state = await dependencies.link(env, {
      deviceRef: body.deviceRef,
      nowIso: now.toISOString(),
      ownerRef,
      pubkey: proof.pubkey,
    })
    if (state === 'conflict') {
      audit(dependencies, 'link_conflict', auditFields)
      return noStoreJson({ error: 'device_link_conflict' }, 409)
    }

    audit(
      dependencies,
      state === 'linked' ? 'link_created' : 'link_existing',
      auditFields,
    )
    return noStoreJson(
      {
        schema: MOBILE_DEVICE_LINK_SCHEMA,
        state,
        ownerRef,
      },
      200,
    )
  } catch {
    audit(dependencies, 'storage_unavailable', auditFields)
    return noStoreJson({ error: 'device_link_unavailable' }, 503)
  }
}
