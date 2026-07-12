import { Schema as S } from 'effect'

import { parseJsonUnknown } from './json-boundary'

export const AUDIO_GRANT_ISSUE_PATH = '/api/desktop/audio/grant'
export const AUDIO_GRANT_REQUEST_SCHEMA = 'openagents.audio.grant.request.v1'
export const AUDIO_GRANT_RESPONSE_SCHEMA = 'openagents.audio.grant.v1'
export const AUDIO_GRANT_TTL_MS = 5 * 60_000
export const AUDIO_GRANT_MAX_TTL_MS = 15 * 60_000
export const AUDIO_GRANT_DEVICE_HEADER = 'x-openagents-desktop-device-ref'

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256))
const Generation = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(2_147_483_647),
)

export const AudioGrantVoiceIdentitySchema = S.Struct({
  ownerRef: Ref,
  deviceRef: Ref,
  threadRef: Ref,
  sessionRef: Ref,
  generation: Generation,
})

export type AudioGrantVoiceIdentity = typeof AudioGrantVoiceIdentitySchema.Type

export const AudioGrantIssueRequestSchema = S.Struct({
  schema: S.Literal(AUDIO_GRANT_REQUEST_SCHEMA),
  identity: AudioGrantVoiceIdentitySchema,
  disclosureRef: Ref,
})

export type AudioGrantIssueRequest = typeof AudioGrantIssueRequestSchema.Type

type UserBearerSessionBoundary<User, Bindings> = (
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) => Promise<Readonly<{ user: User }> | undefined>

export type AudioGrantRouteDependencies<User, Bindings> = Readonly<{
  gatewayUrl: (env: Bindings) => string | undefined
  requireUserBearerSession: UserBearerSessionBoundary<User, Bindings>
  signingSecret: (env: Bindings) => string | undefined
  userIdFromSession: (session: Readonly<{ user: User }>) => string
  now?: (() => number) | undefined
}>

const authenticatedGatewayUrl = (
  value: string | undefined,
): string | undefined => {
  if (value === undefined) return undefined
  try {
    const url = new URL(value.trim())
    return url.protocol === 'wss:' &&
      url.pathname === '/v1/stream' &&
      url.username === '' &&
      url.password === '' &&
      url.search === '' &&
      url.hash === ''
      ? url.toString()
      : undefined
  } catch {
    return undefined
  }
}

const noStoreJson = (body: unknown, status: number): Response =>
  Response.json(body, {
    status,
    headers: {
      'Cache-Control': 'no-store',
      'Content-Type': 'application/json; charset=utf-8',
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

const mintAudioGrant = async (
  identity: AudioGrantVoiceIdentity,
  expiresAtMs: number,
  secret: string,
): Promise<string> => {
  // AUDIO-2 (`apps/openagents-audio/src/auth.ts`) verifies this exact wire:
  // base64url(JSON({ identity, expiresAtMs })) + HMAC-SHA256 signature.
  const encoder = new TextEncoder()
  const body = base64Url(
    encoder.encode(JSON.stringify({ identity, expiresAtMs })),
  )
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const signature = await crypto.subtle.sign('HMAC', key, encoder.encode(body))
  return `${body}.${base64Url(new Uint8Array(signature))}`
}

const parseRequest = async (
  request: Request,
): Promise<AudioGrantIssueRequest | undefined> => {
  const contentLength = Number(request.headers.get('content-length') ?? '0')
  if (!Number.isFinite(contentLength) || contentLength > 8_192) return undefined

  try {
    const text = await request.text()
    if (new TextEncoder().encode(text).byteLength > 8_192) return undefined
    return S.decodeUnknownSync(AudioGrantIssueRequestSchema)(
      parseJsonUnknown(text),
      { onExcessProperty: 'error' },
    )
  } catch {
    return undefined
  }
}

export const handleAudioGrantIssueRequest = async <User, Bindings>(
  dependencies: AudioGrantRouteDependencies<User, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return noStoreJson({ error: 'method_not_allowed' }, 405)
  }

  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) {
    return noStoreJson({ error: 'unauthorized' }, 401)
  }

  const body = await parseRequest(request)
  if (body === undefined) {
    return noStoreJson({ error: 'invalid_audio_grant_request' }, 400)
  }

  const ownerRef = dependencies.userIdFromSession(session)
  const deviceRef = request.headers.get(AUDIO_GRANT_DEVICE_HEADER)?.trim()
  if (
    body.identity.ownerRef !== ownerRef ||
    deviceRef === undefined ||
    deviceRef !== body.identity.deviceRef
  ) {
    return noStoreJson({ error: 'audio_identity_mismatch' }, 403)
  }

  const secret = dependencies.signingSecret(env)?.trim()
  const gatewayUrl = authenticatedGatewayUrl(dependencies.gatewayUrl(env))
  if (secret === undefined || secret.length < 32 || gatewayUrl === undefined) {
    return noStoreJson({ error: 'audio_grant_issuer_unavailable' }, 503)
  }

  const issuedAtMs = (dependencies.now ?? Date.now)()
  if (!Number.isSafeInteger(issuedAtMs) || issuedAtMs < 0) {
    return noStoreJson({ error: 'audio_grant_issuer_unavailable' }, 503)
  }
  const expiresAtMs = issuedAtMs + AUDIO_GRANT_TTL_MS
  if (expiresAtMs - issuedAtMs > AUDIO_GRANT_MAX_TTL_MS) {
    return noStoreJson({ error: 'audio_grant_issuer_unavailable' }, 503)
  }

  const grant = await mintAudioGrant(body.identity, expiresAtMs, secret)
  return noStoreJson(
    {
      schema: AUDIO_GRANT_RESPONSE_SCHEMA,
      disclosureRef: body.disclosureRef,
      expiresAtMs,
      gatewayUrl,
      grant,
    },
    201,
  )
}
