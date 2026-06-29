// GCP service-account access-token minting for the Vertex Anthropic adapter
// (EPIC #5474, #5480).
//
// A Cloudflare Worker cannot use gcloud Application Default Credentials, so we
// mint a short-lived GCP OAuth2 access token from a service-account JSON key.
// The key is stored as the Worker secret VERTEX_SA_KEY (a standard GCP SA key
// JSON: { client_email, private_key, token_uri, ... }) and is NEVER committed.
// Flow (standard Google "JWT bearer" service-account grant):
//   1. Build a JWT { iss: client_email, scope, aud: token_uri, iat, exp }.
//   2. Sign it RS256 with the SA private key via Web Crypto (crypto.subtle).
//   3. POST grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer &
//      assertion=<jwt> to token_uri.
//   4. Read access_token from the JSON response.
// Tokens are cached in-memory and reused until shortly before expiry.
//
// Required Worker secret (deploy-time):
//   VERTEX_SA_KEY = <full service-account key JSON for a principal granted
//                    roles/aiplatform.user on project openagentsgemini>
// Optional:
//   VERTEX_PROJECT_ID (defaults to the SA key's project / "openagentsgemini")
//   VERTEX_LOCATION   (defaults to "global")
//
// This module is pure transport/crypto: it does not import the adapter, so it
// can be unit-tested in isolation with a mocked fetch + a generated test key.

import { Effect } from 'effect'

import { parseJsonRecord } from '../json-boundary'
import { currentEpochSeconds } from '../runtime-primitives'
import { InferenceAdapterError } from './provider-adapter'
import { VERTEX_ANTHROPIC_ADAPTER_ID } from './vertex-anthropic-adapter'

const VERTEX_SCOPE = 'https://www.googleapis.com/auth/cloud-platform'
const DEFAULT_TOKEN_URI = 'https://oauth2.googleapis.com/token'
// Mint tokens valid for 1h (the GCP max); refresh 60s before expiry.
const TOKEN_TTL_SECONDS = 3600
const REFRESH_SKEW_SECONDS = 60

// The fields we read from a GCP service-account key JSON.
export type ServiceAccountKey = Readonly<{
  client_email: string
  private_key: string
  token_uri?: string | undefined
  project_id?: string | undefined
}>

const tokenError = (reason: string, retryable: boolean): InferenceAdapterError =>
  new InferenceAdapterError({
    adapterId: VERTEX_ANTHROPIC_ADAPTER_ID,
    retryable,
    reason,
  })

// Parse + validate the SA key JSON. Returns undefined-friendly typed failure on
// any structural problem so an unconfigured/malformed secret never throws.
export const parseServiceAccountKey = (
  raw: string,
): Effect.Effect<ServiceAccountKey, InferenceAdapterError> =>
  Effect.gen(function* () {
    const parsed = parseJsonRecord(raw)
    if (parsed === undefined) {
      return yield* Effect.fail(
        tokenError('VERTEX_SA_KEY is not valid JSON.', false),
      )
    }
    const clientEmail = parsed['client_email']
    const privateKey = parsed['private_key']
    if (typeof clientEmail !== 'string' || clientEmail === '') {
      return yield* Effect.fail(
        tokenError('VERTEX_SA_KEY missing client_email.', false),
      )
    }
    if (typeof privateKey !== 'string' || privateKey === '') {
      return yield* Effect.fail(
        tokenError('VERTEX_SA_KEY missing private_key.', false),
      )
    }
    return {
      client_email: clientEmail,
      private_key: privateKey,
      project_id:
        typeof parsed['project_id'] === 'string'
          ? (parsed['project_id'] as string)
          : undefined,
      token_uri:
        typeof parsed['token_uri'] === 'string'
          ? (parsed['token_uri'] as string)
          : undefined,
    } satisfies ServiceAccountKey
  })

// base64url without padding, from a byte array.
const base64UrlEncodeBytes = (bytes: Uint8Array): string => {
  let binary = ''
  for (const byte of bytes) {
    binary += String.fromCharCode(byte)
  }
  return btoa(binary).replace(/\+/gu, '-').replace(/\//gu, '_').replace(/=+$/u, '')
}

const base64UrlEncodeString = (value: string): string =>
  base64UrlEncodeBytes(new TextEncoder().encode(value))

// Decode a PEM PKCS#8 private key body into an ArrayBuffer for importKey.
const pemToArrayBuffer = (pem: string): ArrayBuffer => {
  const normalized = pem
    .replace(/-----BEGIN PRIVATE KEY-----/u, '')
    .replace(/-----END PRIVATE KEY-----/u, '')
    .replace(/\s+/gu, '')
  const binary = atob(normalized)
  const buffer = new ArrayBuffer(binary.length)
  const view = new Uint8Array(buffer)
  for (let i = 0; i < binary.length; i += 1) {
    view[i] = binary.charCodeAt(i)
  }
  return buffer
}

// Sign a string with the SA private key using RS256 (RSASSA-PKCS1-v1_5 +
// SHA-256) via Web Crypto. Available in the Workers runtime.
const signRs256 = (
  privateKeyPem: string,
  data: string,
): Effect.Effect<string, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: error =>
      tokenError(
        `Failed to sign Vertex JWT: ${
          error instanceof Error ? error.message : String(error)
        }`,
        false,
      ),
    try: async () => {
      const key = await crypto.subtle.importKey(
        'pkcs8',
        pemToArrayBuffer(privateKeyPem),
        { hash: 'SHA-256', name: 'RSASSA-PKCS1-v1_5' },
        false,
        ['sign'],
      )
      const signature = await crypto.subtle.sign(
        'RSASSA-PKCS1-v1_5',
        key,
        new TextEncoder().encode(data),
      )
      return base64UrlEncodeBytes(new Uint8Array(signature))
    },
  })

// Build + sign the assertion JWT.
const buildSignedJwt = (
  key: ServiceAccountKey,
  nowSeconds: number,
): Effect.Effect<string, InferenceAdapterError> =>
  Effect.gen(function* () {
    const tokenUri = key.token_uri ?? DEFAULT_TOKEN_URI
    const header = { alg: 'RS256', typ: 'JWT' }
    const claims = {
      aud: tokenUri,
      exp: nowSeconds + TOKEN_TTL_SECONDS,
      iat: nowSeconds,
      iss: key.client_email,
      scope: VERTEX_SCOPE,
    }
    const signingInput = `${base64UrlEncodeString(
      JSON.stringify(header),
    )}.${base64UrlEncodeString(JSON.stringify(claims))}`
    const signature = yield* signRs256(key.private_key, signingInput)
    return `${signingInput}.${signature}`
  })

// Options for building a token provider. `fetchImpl` and `nowSeconds` are
// injected for tests; `cache` lets a long-lived adapter reuse a token across
// requests.
export type ServiceAccountTokenProviderOptions = Readonly<{
  fetchImpl?: typeof fetch | undefined
  nowSeconds?: (() => number) | undefined
}>

type CachedToken = Readonly<{ token: string; expiresAtSeconds: number }>

// Build a token provider from a parsed SA key. Returns an Effect-producing
// function suitable as the adapter's `tokenProvider`. Caches the minted token
// in closure state and refreshes shortly before expiry.
export const makeServiceAccountTokenProvider = (
  key: ServiceAccountKey,
  options: ServiceAccountTokenProviderOptions = {},
): (() => Effect.Effect<string, InferenceAdapterError>) => {
  const fetchImpl = options.fetchImpl ?? fetch
  const nowSeconds = options.nowSeconds ?? currentEpochSeconds
  const tokenUri = key.token_uri ?? DEFAULT_TOKEN_URI
  let cached: CachedToken | undefined

  return () =>
    Effect.gen(function* () {
      const now = nowSeconds()
      if (cached !== undefined && cached.expiresAtSeconds - REFRESH_SKEW_SECONDS > now) {
        return cached.token
      }

      const assertion = yield* buildSignedJwt(key, now)
      const response = yield* Effect.tryPromise({
        catch: error =>
          tokenError(
            `GCP token endpoint transport error: ${
              error instanceof Error ? error.message : String(error)
            }`,
            true,
          ),
        try: () =>
          fetchImpl(tokenUri, {
            body: new URLSearchParams({
              assertion,
              grant_type: 'urn:ietf:params:oauth:grant-type:jwt-bearer',
            }).toString(),
            headers: { 'content-type': 'application/x-www-form-urlencoded' },
            method: 'POST',
          }),
      })

      if (!response.ok) {
        const detail = yield* Effect.tryPromise({
          catch: () => tokenError('', false),
          try: () => response.text(),
        }).pipe(Effect.catch(() => Effect.succeed('')))
        return yield* Effect.fail(
          tokenError(
            `GCP token endpoint returned HTTP ${response.status}${
              detail === '' ? '' : `: ${detail.slice(0, 300)}`
            }`,
            response.status === 429 || response.status >= 500,
          ),
        )
      }

      const json = yield* Effect.tryPromise({
        catch: () =>
          tokenError('GCP token response was not valid JSON.', false),
        try: () => response.json() as Promise<Record<string, unknown>>,
      })
      const accessToken = json['access_token']
      if (typeof accessToken !== 'string' || accessToken === '') {
        return yield* Effect.fail(
          tokenError('GCP token response missing access_token.', false),
        )
      }
      const expiresIn =
        typeof json['expires_in'] === 'number'
          ? (json['expires_in'] as number)
          : TOKEN_TTL_SECONDS
      cached = { expiresAtSeconds: now + expiresIn, token: accessToken }
      return accessToken
    })
}

// Convenience: build a token provider straight from the raw VERTEX_SA_KEY
// secret string. Returns undefined when the secret is absent so the adapter can
// stay inert; a malformed secret surfaces as a typed error on first use.
export const tokenProviderFromSecret = (
  secret: string | undefined,
  options: ServiceAccountTokenProviderOptions = {},
): (() => Effect.Effect<string, InferenceAdapterError>) | undefined => {
  if (secret === undefined || secret.trim() === '') {
    return undefined
  }
  // Defer parse failures to first use so construction never throws.
  return () =>
    Effect.gen(function* () {
      const key = yield* parseServiceAccountKey(secret)
      const provider = makeServiceAccountTokenProvider(key, options)
      return yield* provider()
    })
}
