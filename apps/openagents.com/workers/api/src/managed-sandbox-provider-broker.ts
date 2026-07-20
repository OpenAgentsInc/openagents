import type { ManagedSandboxResource } from '@openagentsinc/managed-sandbox-contract'
import { Effect, Schema as S } from 'effect'

import type { OpenAgentsWorkerEnv } from './bindings'
import { tokenProviderFromSecret } from './inference/vertex-token'
import { parseJsonUnknown } from './json-boundary'
import {
  BoxV1FacadeError,
  type BoxV1NativeStore,
} from './managed-sandbox-box-v1-routes'

const CAPABILITY_SCHEMA_VERSION =
  'openagents.managed_sandbox_provider_capability.v1'
const TOKEN_MAX_LIFETIME_MS = 15 * 60 * 1_000
const MAX_PROVIDER_REQUEST_BYTES = 2 * 1024 * 1024
const OPENAI_PATH =
  '/api/internal/managed-sandbox/providers/openai/v1/responses'
const ANTHROPIC_PATH =
  '/api/internal/managed-sandbox/providers/anthropic/v1/messages'
const DEFAULT_CODEX_MODEL = 'gpt-5.6'
const DEFAULT_CLAUDE_MODEL = 'claude-sonnet-4-6'
const DEFAULT_CLAUDE_LOCATION = 'us-east5'

const NonNegativeInteger = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)
const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThan(0))

const ProviderCapabilityClaimsSchema = S.Struct({
  schemaVersion: S.Literal(CAPABILITY_SCHEMA_VERSION),
  actorRef: S.String,
  ownerRef: S.String,
  tenantRef: S.String,
  sandboxRef: S.String,
  turnRef: S.String,
  resourceGeneration: PositiveInteger,
  capabilityRef: S.String,
  provider: S.Literals(['codex', 'claude']),
  requestedModelRef: S.String,
  providerModel: S.String,
  issuedAtMs: NonNegativeInteger,
  expiresAtMs: PositiveInteger,
  nonce: S.String,
})

export type ManagedSandboxProviderCapabilityClaims =
  typeof ProviderCapabilityClaimsSchema.Type

const encoder = new TextEncoder()

const base64UrlEncode = (bytes: Uint8Array): string => {
  let binary = ''
  for (const byte of bytes) binary += String.fromCharCode(byte)
  return btoa(binary)
    .replace(/\+/gu, '-')
    .replace(/\//gu, '_')
    .replace(/=+$/gu, '')
}

const base64UrlDecode = (value: string): Uint8Array => {
  const normalized = value.replace(/-/gu, '+').replace(/_/gu, '/')
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, '=')
  const binary = atob(padded)
  return Uint8Array.from(binary, character => character.charCodeAt(0))
}

const hmacKey = (secret: string) =>
  crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign', 'verify'],
  )

export const managedSandboxProviderModel = (
  env: OpenAgentsWorkerEnv,
  provider: 'codex' | 'claude',
  requestedModelRef: string,
): string => {
  if (provider === 'codex') {
    return requestedModelRef === 'model.codex.default'
      ? env.OA_MANAGED_SANDBOX_CODEX_MODEL?.trim() || DEFAULT_CODEX_MODEL
      : requestedModelRef
  }
  return requestedModelRef === 'model.claude.default'
    ? env.OA_MANAGED_SANDBOX_CLAUDE_MODEL?.trim() || DEFAULT_CLAUDE_MODEL
    : requestedModelRef
}

export const mintManagedSandboxProviderCapability = (
  env: OpenAgentsWorkerEnv,
  input: Readonly<{
    actorRef: string
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
    resourceGeneration: number
    capabilityRef: string
    capabilityExpiresAt: string
    provider: 'codex' | 'claude'
    requestedModelRef: string
    nowMs?: number | undefined
  }>,
): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const secret = env.OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY?.trim()
      if (secret === undefined || secret.length < 32) {
        throw new Error('broker_signing_key_unavailable')
      }
      const nowMs = input.nowMs ?? Date.now()
      const capabilityExpiresAtMs = Date.parse(input.capabilityExpiresAt)
      const expiresAtMs = Math.min(
        capabilityExpiresAtMs,
        nowMs + TOKEN_MAX_LIFETIME_MS,
      )
      if (!Number.isSafeInteger(expiresAtMs) || expiresAtMs <= nowMs) {
        throw new Error('broker_capability_expired')
      }
      const nonceBytes = new Uint8Array(18)
      crypto.getRandomValues(nonceBytes)
      const claims = S.decodeUnknownSync(ProviderCapabilityClaimsSchema)({
        schemaVersion: CAPABILITY_SCHEMA_VERSION,
        actorRef: input.actorRef,
        ownerRef: input.ownerRef,
        tenantRef: input.tenantRef,
        sandboxRef: input.sandboxRef,
        turnRef: input.turnRef,
        resourceGeneration: input.resourceGeneration,
        capabilityRef: input.capabilityRef,
        provider: input.provider,
        requestedModelRef: input.requestedModelRef,
        providerModel: managedSandboxProviderModel(
          env,
          input.provider,
          input.requestedModelRef,
        ),
        issuedAtMs: nowMs,
        expiresAtMs,
        nonce: base64UrlEncode(nonceBytes),
      })
      const payload = base64UrlEncode(encoder.encode(JSON.stringify(claims)))
      const key = await hmacKey(secret)
      const signature = await crypto.subtle.sign(
        'HMAC',
        key,
        encoder.encode(payload),
      )
      return `${payload}.${base64UrlEncode(new Uint8Array(signature))}`
    },
    catch: () =>
      new BoxV1FacadeError({
        code: 'upstream_unavailable',
        status: 503,
        message: 'managed-sandbox provider capability broker is unavailable',
        retryable: true,
      }),
  })

const decodeCapability = async (
  secret: string,
  token: string,
  nowMs: number,
): Promise<ManagedSandboxProviderCapabilityClaims | undefined> => {
  const [payload, signature, excess] = token.split('.')
  if (!payload || !signature || excess !== undefined) return undefined
  try {
    const key = await hmacKey(secret)
    const valid = await crypto.subtle.verify(
      'HMAC',
      key,
      Uint8Array.from(base64UrlDecode(signature)).buffer,
      encoder.encode(payload),
    )
    if (!valid) return undefined
    const decoded = S.decodeUnknownSync(ProviderCapabilityClaimsSchema)(
      parseJsonUnknown(new TextDecoder().decode(base64UrlDecode(payload))),
    )
    if (
      decoded.issuedAtMs > nowMs + 30_000 ||
      decoded.expiresAtMs <= nowMs ||
      decoded.expiresAtMs - decoded.issuedAtMs > TOKEN_MAX_LIFETIME_MS
    ) {
      return undefined
    }
    return decoded
  } catch {
    return undefined
  }
}

const bearer = (request: Request): string | undefined => {
  const value = request.headers.get('authorization')
  return value?.startsWith('Bearer ') && value.length > 'Bearer '.length
    ? value.slice('Bearer '.length)
    : undefined
}

const json = (body: unknown, status: number): Response =>
  Response.json(body, {
    status,
    headers: { 'cache-control': 'no-store' },
  })

type ProviderCapabilityDenial =
  | 'owner_scope_mismatch'
  | 'tenant_scope_mismatch'
  | 'sandbox_scope_mismatch'
  | 'resource_generation_mismatch'
  | 'resource_not_accepting_work'
  | 'resource_lifecycle_denied'
  | 'resource_cleanup_complete'
  | 'resource_lease_inactive'
  | 'resource_lease_expired'
  | 'capability_missing'
  | 'capability_kind_denied'
  | 'capability_inactive'
  | 'capability_expired'

const resourceAdmissionDenial = (
  resource: ManagedSandboxResource,
  claims: ManagedSandboxProviderCapabilityClaims,
  nowMs: number,
): ProviderCapabilityDenial | undefined => {
  const capability = resource.capabilities.find(
    candidate => candidate.capabilityRef === claims.capabilityRef,
  )
  if (resource.ownerRef !== claims.ownerRef) return 'owner_scope_mismatch'
  if (resource.tenantRef !== claims.tenantRef) return 'tenant_scope_mismatch'
  if (resource.sandboxRef !== claims.sandboxRef) return 'sandbox_scope_mismatch'
  if (resource.resourceGeneration !== claims.resourceGeneration)
    return 'resource_generation_mismatch'
  if (!resource.facts.acceptingWork) return 'resource_not_accepting_work'
  if (!['ready', 'idle', 'running'].includes(resource.facts.lifecycle))
    return 'resource_lifecycle_denied'
  if (resource.facts.cleanupComplete) return 'resource_cleanup_complete'
  if (resource.lease.state !== 'active') return 'resource_lease_inactive'
  if (Date.parse(resource.lease.expiresAt) <= nowMs)
    return 'resource_lease_expired'
  if (capability === undefined) return 'capability_missing'
  if (capability.kind !== 'agent_turn') return 'capability_kind_denied'
  if (capability.state !== 'active') return 'capability_inactive'
  if (Date.parse(capability.expiresAt) <= nowMs) return 'capability_expired'
  return undefined
}

const upstreamResponse = (response: Response): Response => {
  const headers = new Headers({
    'cache-control': 'no-store',
    'content-type': response.headers.get('content-type') ?? 'application/json',
  })
  const requestId = response.headers.get('x-request-id')
  if (requestId) headers.set('x-request-id', requestId)
  return new Response(response.body, { status: response.status, headers })
}

export type ManagedSandboxProviderBrokerDependencies = Readonly<{
  store: (env: OpenAgentsWorkerEnv) => BoxV1NativeStore
  fetchImpl?: typeof fetch | undefined
  nowMs?: (() => number) | undefined
  vertexAccessToken?: string | undefined
}>

export const makeManagedSandboxProviderBrokerRoutes = (
  dependencies: ManagedSandboxProviderBrokerDependencies,
) => ({
  route: (
    request: Request,
    env: OpenAgentsWorkerEnv,
  ): Effect.Effect<Response, never> | undefined => {
    const path = new URL(request.url).pathname
    const provider =
      path === OPENAI_PATH
        ? ('codex' as const)
        : path === ANTHROPIC_PATH
          ? ('claude' as const)
          : undefined
    if (provider === undefined) return undefined
    return Effect.gen(function* () {
      if (request.method !== 'POST')
        return json({ error: 'method_not_allowed' }, 405)
      const secret = env.OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY?.trim()
      if (secret === undefined || secret.length < 32) {
        return json({ error: 'provider_broker_not_armed' }, 404)
      }
      const token = bearer(request)
      if (token === undefined) return json({ error: 'unauthorized' }, 401)
      const nowMs = dependencies.nowMs?.() ?? Date.now()
      const claims = yield* Effect.promise(() =>
        decodeCapability(secret, token, nowMs),
      )
      if (claims === undefined || claims.provider !== provider) {
        return json({ error: 'unauthorized' }, 401)
      }
      const contentLength = Number(request.headers.get('content-length') ?? '0')
      if (contentLength > MAX_PROVIDER_REQUEST_BYTES) {
        return json({ error: 'request_too_large' }, 413)
      }
      const bytes = new Uint8Array(
        yield* Effect.promise(() => request.arrayBuffer()),
      )
      if (
        bytes.byteLength === 0 ||
        bytes.byteLength > MAX_PROVIDER_REQUEST_BYTES
      ) {
        return json({ error: 'request_out_of_bounds' }, 400)
      }
      const store = dependencies.store(env)
      const resource = yield* store
        .inspect({
          ownerRef: claims.ownerRef,
          tenantRef: claims.tenantRef,
          sandboxRef: claims.sandboxRef,
        })
        .pipe(Effect.option)
      const denial =
        resource._tag === 'Some'
          ? resourceAdmissionDenial(resource.value, claims, nowMs)
          : 'resource_missing'
      if (denial !== undefined) {
        console.warn(
          JSON.stringify({
            event: 'managed_sandbox_provider_capability_denied',
            provider,
            denial,
          }),
        )
        return json({ error: 'capability_revoked' }, 403)
      }
      const turn = yield* store
        .inspectTurn({
          ownerRef: claims.ownerRef,
          tenantRef: claims.tenantRef,
          sandboxRef: claims.sandboxRef,
          turnRef: claims.turnRef,
        })
        .pipe(Effect.option)
      if (
        turn._tag !== 'Some' ||
        turn.value.turn.turnRef !== claims.turnRef ||
        turn.value.turn.ownerRef !== claims.ownerRef ||
        turn.value.turn.tenantRef !== claims.tenantRef ||
        turn.value.turn.sandboxRef !== claims.sandboxRef ||
        turn.value.turn.capabilityRef !== claims.capabilityRef ||
        turn.value.turn.resourceGeneration !== claims.resourceGeneration ||
        turn.value.turn.runtime.provider !== provider ||
        turn.value.turn.runtime.modelRef !== claims.requestedModelRef ||
        !['pending', 'running'].includes(turn.value.turn.status)
      ) {
        return json({ error: 'turn_scope_conflict' }, 409)
      }
      const parsed = yield* Effect.try({
        try: () => parseJsonUnknown(new TextDecoder().decode(bytes)),
        catch: () => undefined,
      }).pipe(Effect.option)
      if (
        parsed._tag !== 'Some' ||
        typeof parsed.value !== 'object' ||
        parsed.value === null ||
        Array.isArray(parsed.value)
      ) {
        return json({ error: 'invalid_provider_request' }, 400)
      }
      const body: Record<string, unknown> = {
        ...(parsed.value as Record<string, unknown>),
        model: claims.providerModel,
      }
      const fetchImpl = dependencies.fetchImpl ?? fetch
      if (provider === 'codex') {
        const apiKey = env.OPENAI_API_KEY?.trim()
        if (!apiKey) return json({ error: 'provider_not_configured' }, 503)
        const response = yield* Effect.tryPromise({
          try: () =>
            fetchImpl('https://api.openai.com/v1/responses', {
              method: 'POST',
              headers: {
                authorization: `Bearer ${apiKey}`,
                'content-type': 'application/json',
              },
              body: JSON.stringify(body),
            }),
          catch: () => undefined,
        }).pipe(Effect.option)
        return response._tag === 'Some'
          ? upstreamResponse(response.value)
          : json({ error: 'provider_unavailable' }, 502)
      }
      let accessToken = dependencies.vertexAccessToken
      if (accessToken === undefined) {
        const tokenProvider = tokenProviderFromSecret(env.VERTEX_SA_KEY)
        if (tokenProvider === undefined) {
          return json({ error: 'provider_not_configured' }, 503)
        }
        const minted = yield* tokenProvider().pipe(Effect.option)
        if (minted._tag !== 'Some') {
          return json({ error: 'provider_unavailable' }, 502)
        }
        accessToken = minted.value
      }
      delete body['model']
      // Claude Agent SDK 0.3.172 negotiates a direct-Anthropic context
      // management beta on retry. Vertex Sonnet 4.6 rejects that SDK-only
      // transport hint as an extra input, so the Vertex adapter removes only
      // the hint while preserving the prompt, messages, tools, and limits.
      delete body['context_management']
      body['anthropic_version'] = 'vertex-2023-10-16'
      const location =
        env.OA_MANAGED_SANDBOX_CLAUDE_LOCATION?.trim() ||
        DEFAULT_CLAUDE_LOCATION
      const method = body['stream'] === true ? 'streamRawPredict' : 'rawPredict'
      const host =
        location === 'global'
          ? 'aiplatform.googleapis.com'
          : `${location}-aiplatform.googleapis.com`
      const project = env.VERTEX_PROJECT_ID?.trim() || 'openagentsgemini'
      const url =
        `https://${host}/v1/projects/${encodeURIComponent(project)}` +
        `/locations/${encodeURIComponent(location)}/publishers/anthropic/models/` +
        `${encodeURIComponent(claims.providerModel)}:${method}`
      const response = yield* Effect.tryPromise({
        try: () =>
          fetchImpl(url, {
            method: 'POST',
            headers: {
              authorization: `Bearer ${accessToken}`,
              'content-type': 'application/json',
            },
            body: JSON.stringify(body),
          }),
        catch: () => undefined,
      }).pipe(Effect.option)
      return response._tag === 'Some'
        ? upstreamResponse(response.value)
        : json({ error: 'provider_unavailable' }, 502)
    })
  },
})

export const managedSandboxProviderBrokerPaths = {
  anthropic: ANTHROPIC_PATH,
  openai: OPENAI_PATH,
} as const
