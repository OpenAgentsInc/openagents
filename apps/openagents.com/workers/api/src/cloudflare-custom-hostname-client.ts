import { Effect, Schema as S } from 'effect'

import {
  type CloudflareCustomHostname,
  CloudflareCustomHostname as CloudflareCustomHostnameSchema,
  type CloudflareCustomHostnameStatus,
  type CreateCustomHostnameInput,
  type CustomHostnameClient,
  CustomHostnameClientError,
} from './tenant-hostname-provisioning'

// ===========================================================================
// LIVE Cloudflare-for-SaaS custom-hostname client (OpenAgents #4988)
// ===========================================================================
//
// This is the LIVE implementation of the `CustomHostnameClient` interface that
// `tenant-hostname-provisioning.ts` injects. It speaks the Cloudflare for SaaS
// "Custom Hostnames" REST API:
//
//   POST   /zones/{zoneId}/custom_hostnames           -> create (idempotent)
//   GET    /zones/{zoneId}/custom_hostnames/{id}       -> status
//   DELETE /zones/{zoneId}/custom_hostnames/{id}       -> delete
//
// It is built to be UNIT-TESTABLE with an injected `fetchImpl`, so the test
// suite drives it with a fake `fetch` and performs NO real network calls and
// requires NO real credentials. The provisioning service stays the same; only
// the injected client changes between test (fake) and live (this module).
//
// ---------------------------------------------------------------------------
// COORDINATOR WIRING (OWNER-GATED) — do NOT enable without owner sign-off
// ---------------------------------------------------------------------------
//
// Constructing the LIVE client from env, in the Worker entry/coordinator:
//
//   import {
//     makeCloudflareCustomHostnameClient,
//     readCloudflareCustomHostnameConfig,
//   } from './cloudflare-custom-hostname-client'
//   import { makeTenantHostnameProvisioning } from './tenant-hostname-provisioning'
//
//   const cfConfig = readCloudflareCustomHostnameConfig(env)
//   if (cfConfig !== undefined) {
//     const liveClient = makeCloudflareCustomHostnameClient({
//       apiToken: cfConfig.apiToken,
//       zoneId: cfConfig.zoneId,
//       // fetchImpl defaults to globalThis.fetch; pass one only to override.
//     })
//     const provisioning = makeTenantHostnameProvisioning(
//       openAgentsDatabase(env),
//       liveClient,
//     )
//     // ... mount provisioning.provision / provisioning.reconcile on an
//     //     OPERATOR/ADMIN-gated route in index.ts (not edited here).
//   }
//   // When cfConfig === undefined the feature is cleanly DISABLED: do not
//   // construct the live client, do not mount the route, no live call happens.
//
// What stays OWNER-GATED (the build is done; these are operator actions):
//   1. CLOUDFLARE_API_TOKEN — a real Cloudflare API token scoped to
//      "Zone -> SSL and Certificates -> Edit" on the fallback-origin zone.
//      Provisioned as a Worker secret by the owner. Never hardcoded; never
//      required at test time.
//   2. CLOUDFLARE_ZONE_ID — the zone id of the fallback-origin zone that owns
//      the SaaS custom hostnames. Provisioned as a Worker var by the owner.
//   3. Mounting provisioning.provision / provisioning.reconcile behind the
//      operator/admin gate in index.ts (a shared file NOT edited by this lane).
//
// Until BOTH CLOUDFLARE_API_TOKEN and CLOUDFLARE_ZONE_ID are set,
// `readCloudflareCustomHostnameConfig` returns undefined and NO live Cloudflare
// call is ever made.

const CLOUDFLARE_API_BASE = 'https://api.cloudflare.com/client/v4'

// The Cloudflare for SaaS custom-hostname SSL status values we care about. The
// raw API exposes a wider set of ssl statuses; we collapse everything that is
// not clearly active/failed down to 'pending' so the provisioning state machine
// stays on its three-value contract. (Mapping lives in `mapCloudflareStatus`.)
const ACTIVE_SSL_STATUSES: ReadonlySet<string> = new Set(['active'])
const FAILED_SSL_STATUSES: ReadonlySet<string> = new Set([
  'expired',
  'deleted',
  'validation_failed',
  'issuance_failed',
  'failed',
])

// Map a raw Cloudflare ssl/hostname status string onto our 3-value contract.
// Unknown / in-flight statuses (pending_validation, pending_issuance,
// pending_deployment, initializing, ...) all resolve to 'pending'.
const mapCloudflareStatus = (
  raw: string | undefined,
): CloudflareCustomHostnameStatus => {
  if (raw !== undefined && ACTIVE_SSL_STATUSES.has(raw)) {
    return 'active'
  }

  if (raw !== undefined && FAILED_SSL_STATUSES.has(raw)) {
    return 'failed'
  }

  return 'pending'
}

// ---------------------------------------------------------------------------
// Raw Cloudflare response schemas (decoded leniently, then normalized).
// ---------------------------------------------------------------------------

// Cloudflare wraps every response in an envelope. `result` is null for some
// error envelopes, so we keep it optional/unknown and validate downstream.
const CloudflareEnvelope = S.Struct({
  success: S.Boolean,
  errors: S.optionalKey(
    S.Array(
      S.Struct({
        code: S.optionalKey(S.Number),
        message: S.optionalKey(S.String),
      }),
    ),
  ),
  result: S.optionalKey(S.Unknown),
})
type CloudflareEnvelope = typeof CloudflareEnvelope.Type

// The custom_hostname object inside `result`. We read `id`, `hostname`, and the
// nested `ssl.status` (preferring it over the top-level hostname `status`).
const CloudflareCustomHostnameResult = S.Struct({
  id: S.String,
  hostname: S.String,
  status: S.optionalKey(S.String),
  ssl: S.optionalKey(
    S.Struct({
      status: S.optionalKey(S.String),
    }),
  ),
})
type CloudflareCustomHostnameResult = typeof CloudflareCustomHostnameResult.Type

const normalizeResult = (
  raw: CloudflareCustomHostnameResult,
): CloudflareCustomHostname => ({
  id: raw.id,
  hostname: raw.hostname,
  // The SSL/certificate status is the live signal for "is this hostname
  // serving?"; fall back to the hostname-level status, then to 'pending'.
  status: mapCloudflareStatus(raw.ssl?.status ?? raw.status),
})

// ---------------------------------------------------------------------------
// Client config + factory
// ---------------------------------------------------------------------------

export type CloudflareCustomHostnameFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

export type CloudflareCustomHostnameClientConfig = Readonly<{
  apiToken: string
  zoneId: string
  // Injectable for unit tests; defaults to the platform fetch in production.
  fetchImpl?: CloudflareCustomHostnameFetch
}>

// Env shape this client reads. Both values are owner-provisioned; absence means
// the feature is disabled (see `readCloudflareCustomHostnameConfig`).
export type CloudflareCustomHostnameEnv = Readonly<{
  CLOUDFLARE_API_TOKEN?: string
  CLOUDFLARE_ZONE_ID?: string
}>

export type CloudflareCustomHostnameConfig = Readonly<{
  apiToken: string
  zoneId: string
}>

// Config reader: pulls apiToken/zoneId from env. Returns undefined when EITHER
// value is unset/blank, so the caller can cleanly leave the feature disabled
// until the owner provisions both secrets. Never throws.
export const readCloudflareCustomHostnameConfig = (
  env: CloudflareCustomHostnameEnv,
): CloudflareCustomHostnameConfig | undefined => {
  const apiToken = env.CLOUDFLARE_API_TOKEN
  const zoneId = env.CLOUDFLARE_ZONE_ID

  if (
    apiToken === undefined ||
    apiToken.trim() === '' ||
    zoneId === undefined ||
    zoneId.trim() === ''
  ) {
    return undefined
  }

  return { apiToken, zoneId }
}

const clientError = (
  operation: string,
  error: unknown,
): CustomHostnameClientError =>
  new CustomHostnameClientError({ operation, error })

// Read + JSON-parse a Cloudflare response into a validated envelope, mapping
// transport, HTTP, JSON, and CF-application errors onto CustomHostnameClientError.
const readEnvelope = (
  operation: string,
  response: Response,
): Effect.Effect<CloudflareEnvelope, CustomHostnameClientError> =>
  Effect.gen(function* () {
    const payload: unknown = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: error => clientError(operation, error),
    })

    // Non-2xx HTTP: surface status + any CF error messages we can read.
    if (!response.ok) {
      return yield* clientError(
        operation,
        new Error(
          `Cloudflare ${operation} failed with HTTP ${response.status}: ${summarize(payload)}`,
        ),
      )
    }

    const envelope = yield* Effect.try({
      try: () => S.decodeUnknownSync(CloudflareEnvelope)(payload),
      catch: error => clientError(operation, error),
    })

    if (!envelope.success) {
      return yield* clientError(
        operation,
        new Error(
          `Cloudflare ${operation} returned success=false: ${summarize(payload)}`,
        ),
      )
    }

    return envelope
  })

const summarize = (payload: unknown): string => {
  try {
    const text = typeof payload === 'string' ? payload : JSON.stringify(payload)

    return (text ?? '').slice(0, 300)
  } catch {
    return '<unserializable payload>'
  }
}

const decodeHostnameResult = (
  operation: string,
  envelope: CloudflareEnvelope,
): Effect.Effect<CloudflareCustomHostname, CustomHostnameClientError> =>
  Effect.try({
    try: () =>
      normalizeResult(
        S.decodeUnknownSync(CloudflareCustomHostnameResult)(envelope.result),
      ),
    catch: error => clientError(operation, error),
  })

export const makeCloudflareCustomHostnameClient = (
  config: CloudflareCustomHostnameClientConfig,
): CustomHostnameClient => {
  const fetchImpl: CloudflareCustomHostnameFetch =
    config.fetchImpl ?? globalThis.fetch.bind(globalThis)

  const zoneBase = `${CLOUDFLARE_API_BASE}/zones/${encodeURIComponent(
    config.zoneId,
  )}/custom_hostnames`

  const authHeaders = (): Record<string, string> => ({
    authorization: `Bearer ${config.apiToken}`,
    'content-type': 'application/json',
    accept: 'application/json',
  })

  const doFetch = (
    operation: string,
    input: RequestInfo | URL,
    init: RequestInit,
  ): Effect.Effect<Response, CustomHostnameClientError> =>
    Effect.tryPromise({
      try: () => fetchImpl(input, init),
      catch: error => clientError(operation, error),
    })

  const createCustomHostname = (
    input: CreateCustomHostnameInput,
  ): Effect.Effect<CloudflareCustomHostname, CustomHostnameClientError> =>
    Effect.gen(function* () {
      const operation = 'createCustomHostname'
      const response = yield* doFetch(operation, zoneBase, {
        method: 'POST',
        headers: authHeaders(),
        body: JSON.stringify({
          hostname: input.hostname,
          // DV certificate over the fallback origin (Cloudflare for SaaS).
          ssl: {
            method: 'http',
            type: 'dv',
            settings: { min_tls_version: '1.2' },
          },
          // Correlate the CF record with our row at reconcile time.
          custom_metadata: { verification_token: input.verificationToken },
        }),
      })

      const envelope = yield* readEnvelope(operation, response)

      return yield* decodeHostnameResult(operation, envelope)
    })

  const getStatus = (
    cloudflareId: string,
  ): Effect.Effect<CloudflareCustomHostname, CustomHostnameClientError> =>
    Effect.gen(function* () {
      const operation = 'getStatus'
      const url = `${zoneBase}/${encodeURIComponent(cloudflareId)}`
      const response = yield* doFetch(operation, url, {
        method: 'GET',
        headers: authHeaders(),
      })

      const envelope = yield* readEnvelope(operation, response)

      return yield* decodeHostnameResult(operation, envelope)
    })

  const deleteCustomHostname = (
    cloudflareId: string,
  ): Effect.Effect<void, CustomHostnameClientError> =>
    Effect.gen(function* () {
      const operation = 'deleteCustomHostname'
      const url = `${zoneBase}/${encodeURIComponent(cloudflareId)}`
      const response = yield* doFetch(operation, url, {
        method: 'DELETE',
        headers: authHeaders(),
      })

      // We only need success confirmation here; the deleted id is echoed back
      // but the provisioning service does not consume it.
      yield* readEnvelope(operation, response)
    })

  return { createCustomHostname, getStatus, deleteCustomHostname }
}

// Re-export the schema for callers that want to decode/validate independently.
export { CloudflareCustomHostnameSchema }
