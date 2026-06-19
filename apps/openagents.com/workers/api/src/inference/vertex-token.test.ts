import { Effect } from 'effect'
import { beforeAll, describe, expect, test } from 'vitest'

import { type InferenceAdapterError } from './provider-adapter'
import {
  type ServiceAccountKey,
  makeServiceAccountTokenProvider,
  parseServiceAccountKey,
  tokenProviderFromSecret,
} from './vertex-token'

const run = <A>(effect: Effect.Effect<A, InferenceAdapterError>): Promise<A> =>
  Effect.runPromise(effect)

const runError = <A>(
  effect: Effect.Effect<A, InferenceAdapterError>,
): Promise<InferenceAdapterError> => Effect.runPromise(Effect.flip(effect))

// Encode a buffer as PEM PKCS#8 (matches a GCP SA key's `private_key`).
const toPem = (buffer: ArrayBuffer): string => {
  const bytes = new Uint8Array(buffer)
  let binary = ''
  for (const b of bytes) {
    binary += String.fromCharCode(b)
  }
  const base64 = btoa(binary)
  const lines = base64.match(/.{1,64}/gu) ?? []
  return `-----BEGIN PRIVATE KEY-----\n${lines.join('\n')}\n-----END PRIVATE KEY-----\n`
}

let testKey: ServiceAccountKey
let testKeyJson: string

beforeAll(async () => {
  // Generate a real RSA key so the RS256 Web Crypto signing path is exercised.
  const pair = await crypto.subtle.generateKey(
    {
      hash: 'SHA-256',
      modulusLength: 2048,
      name: 'RSASSA-PKCS1-v1_5',
      publicExponent: new Uint8Array([1, 0, 1]),
    },
    true,
    ['sign', 'verify'],
  )
  const pkcs8 = await crypto.subtle.exportKey('pkcs8', pair.privateKey)
  testKey = {
    client_email: 'vertex-sa@openagentsgemini.iam.gserviceaccount.com',
    private_key: toPem(pkcs8),
    project_id: 'openagentsgemini',
    token_uri: 'https://oauth2.googleapis.com/token',
  }
  testKeyJson = JSON.stringify({
    client_email: testKey.client_email,
    private_key: testKey.private_key,
    project_id: testKey.project_id,
    token_uri: testKey.token_uri,
    type: 'service_account',
  })
})

// Mock token endpoint: records the posted assertion, returns an access token.
const recordingTokenFetch = (
  body: unknown,
  status = 200,
): { fetchImpl: typeof fetch; calls: Array<{ url: string; body: string }> } => {
  const calls: Array<{ url: string; body: string }> = []
  const fetchImpl = (async (url: unknown, init?: RequestInit) => {
    calls.push({ body: String(init?.body ?? ''), url: String(url) })
    return new Response(JSON.stringify(body), {
      headers: { 'content-type': 'application/json' },
      status,
    })
  }) as unknown as typeof fetch
  return { calls, fetchImpl }
}

describe('parseServiceAccountKey', () => {
  test('parses a valid SA key JSON', async () => {
    const key = await run(parseServiceAccountKey(testKeyJson))
    expect(key.client_email).toBe(testKey.client_email)
    expect(key.project_id).toBe('openagentsgemini')
  })

  test('rejects invalid JSON', async () => {
    const error = await runError(parseServiceAccountKey('not json'))
    expect(error.reason).toContain('not valid JSON')
  })

  test('rejects a key missing client_email', async () => {
    const error = await runError(
      parseServiceAccountKey(JSON.stringify({ private_key: 'x' })),
    )
    expect(error.reason).toContain('client_email')
  })
})

describe('makeServiceAccountTokenProvider', () => {
  test('signs a JWT and exchanges it for an access token', async () => {
    const { calls, fetchImpl } = recordingTokenFetch({
      access_token: 'ya29.minted-token',
      expires_in: 3600,
    })
    const provider = makeServiceAccountTokenProvider(testKey, {
      fetchImpl,
      nowSeconds: () => 1_700_000_000,
    })
    const token = await run(provider())
    expect(token).toBe('ya29.minted-token')

    expect(calls).toHaveLength(1)
    expect(calls[0]!.url).toBe('https://oauth2.googleapis.com/token')
    // Body is form-encoded JWT-bearer grant with a signed assertion.
    const params = new URLSearchParams(calls[0]!.body)
    expect(params.get('grant_type')).toBe(
      'urn:ietf:params:oauth:grant-type:jwt-bearer',
    )
    const assertion = params.get('assertion') ?? ''
    const segments = assertion.split('.')
    expect(segments).toHaveLength(3) // header.claims.signature
    const claims = JSON.parse(
      atob(segments[1]!.replace(/-/gu, '+').replace(/_/gu, '/')),
    ) as Record<string, unknown>
    expect(claims['iss']).toBe(testKey.client_email)
    expect(claims['aud']).toBe('https://oauth2.googleapis.com/token')
    expect(claims['scope']).toBe(
      'https://www.googleapis.com/auth/cloud-platform',
    )
    expect(claims['iat']).toBe(1_700_000_000)
    expect(claims['exp']).toBe(1_700_003_600)
  })

  test('caches the token across calls until near expiry', async () => {
    const { calls, fetchImpl } = recordingTokenFetch({
      access_token: 'ya29.cached',
      expires_in: 3600,
    })
    let now = 1_700_000_000
    const provider = makeServiceAccountTokenProvider(testKey, {
      fetchImpl,
      nowSeconds: () => now,
    })
    await run(provider())
    now += 100 // still well within TTL
    await run(provider())
    expect(calls).toHaveLength(1) // second call served from cache
  })

  test('maps a 429 from the token endpoint to a retryable error', async () => {
    const { fetchImpl } = recordingTokenFetch({ error: 'rate' }, 429)
    const provider = makeServiceAccountTokenProvider(testKey, {
      fetchImpl,
      nowSeconds: () => 1_700_000_000,
    })
    const error = await runError(provider())
    expect(error.retryable).toBe(true)
    expect(error.reason).toContain('HTTP 429')
  })

  test('fails non-retryably when access_token is missing', async () => {
    const { fetchImpl } = recordingTokenFetch({ token_type: 'Bearer' })
    const provider = makeServiceAccountTokenProvider(testKey, {
      fetchImpl,
      nowSeconds: () => 1_700_000_000,
    })
    const error = await runError(provider())
    expect(error.retryable).toBe(false)
    expect(error.reason).toContain('missing access_token')
  })
})

describe('tokenProviderFromSecret', () => {
  test('returns undefined for an absent/empty secret (adapter stays inert)', () => {
    expect(tokenProviderFromSecret(undefined)).toBeUndefined()
    expect(tokenProviderFromSecret('   ')).toBeUndefined()
  })

  test('mints a token end-to-end from the raw secret JSON', async () => {
    const { fetchImpl } = recordingTokenFetch({
      access_token: 'ya29.from-secret',
      expires_in: 3600,
    })
    const provider = tokenProviderFromSecret(testKeyJson, {
      fetchImpl,
      nowSeconds: () => 1_700_000_000,
    })
    expect(provider).toBeDefined()
    const token = await run(provider!())
    expect(token).toBe('ya29.from-secret')
  })

  test('surfaces a malformed secret as a typed error on first use', async () => {
    const provider = tokenProviderFromSecret('{bad json')
    expect(provider).toBeDefined()
    const error = await runError(provider!())
    expect(error.reason).toContain('not valid JSON')
  })
})
