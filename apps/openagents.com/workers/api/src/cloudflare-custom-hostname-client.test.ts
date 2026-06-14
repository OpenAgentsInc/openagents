import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type CloudflareCustomHostnameFetch,
  makeCloudflareCustomHostnameClient,
  readCloudflareCustomHostnameConfig,
} from './cloudflare-custom-hostname-client'

// ---------------------------------------------------------------------------
// Fake fetch: no real network. Each test wires the exact Response it wants.
// ---------------------------------------------------------------------------

type FetchCall = Readonly<{
  url: string
  method: string
  headers: Record<string, string>
  body: unknown
}>

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

// Build a fake fetch from a sequence of responders. Records each call so tests
// can assert URL/method/auth without any network access.
const makeFakeFetch = (
  responders: ReadonlyArray<(call: FetchCall) => Response | Promise<Response>>,
) => {
  const calls: Array<FetchCall> = []
  let index = 0

  const fetchImpl: CloudflareCustomHostnameFetch = async (input, init) => {
    const headers: Record<string, string> = {}

    if (init?.headers !== undefined) {
      for (const [key, value] of Object.entries(
        init.headers as Record<string, string>,
      )) {
        headers[key.toLowerCase()] = value
      }
    }

    const rawBody = init?.body
    const call: FetchCall = {
      url: String(input),
      method: init?.method ?? 'GET',
      headers,
      body: typeof rawBody === 'string' ? JSON.parse(rawBody) : rawBody,
    }

    calls.push(call)

    const responder = responders[Math.min(index, responders.length - 1)]
    index += 1

    if (responder === undefined) {
      throw new Error('fake fetch: no responder configured')
    }

    return responder(call)
  }

  return { fetchImpl, calls: () => calls }
}

const config = {
  apiToken: 'test-token-not-real',
  zoneId: 'zone_test_123',
}

describe('cloudflare-custom-hostname-client', () => {
  test('createCustomHostname: POSTs and parses the CF envelope', async () => {
    const fake = makeFakeFetch([
      () =>
        jsonResponse({
          success: true,
          errors: [],
          result: {
            id: 'cf_hostname_abc',
            hostname: 'brand.example.com',
            status: 'pending',
            ssl: { status: 'pending_validation' },
          },
        }),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const result = await Effect.runPromise(
      client.createCustomHostname({
        hostname: 'brand.example.com',
        verificationToken: 'verify_tok_1',
      }),
    )

    expect(result.id).toBe('cf_hostname_abc')
    expect(result.hostname).toBe('brand.example.com')
    // pending_validation collapses to our 'pending'.
    expect(result.status).toBe('pending')

    const calls = fake.calls()
    expect(calls).toHaveLength(1)
    expect(calls[0]?.method).toBe('POST')
    expect(calls[0]?.url).toBe(
      'https://api.cloudflare.com/client/v4/zones/zone_test_123/custom_hostnames',
    )
    expect(calls[0]?.headers.authorization).toBe('Bearer test-token-not-real')
    // The verification token is forwarded as custom_metadata.
    expect(
      (calls[0]?.body as { custom_metadata?: { verification_token?: string } })
        ?.custom_metadata?.verification_token,
    ).toBe('verify_tok_1')
  })

  test('createCustomHostname: active ssl status maps to active', async () => {
    const fake = makeFakeFetch([
      () =>
        jsonResponse({
          success: true,
          result: {
            id: 'cf_active',
            hostname: 'brand.example.com',
            status: 'active',
            ssl: { status: 'active' },
          },
        }),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const result = await Effect.runPromise(
      client.createCustomHostname({
        hostname: 'brand.example.com',
        verificationToken: 'verify_tok_1',
      }),
    )

    expect(result.status).toBe('active')
  })

  test('getStatus: GETs by id and maps a failed ssl status to failed', async () => {
    const fake = makeFakeFetch([
      () =>
        jsonResponse({
          success: true,
          result: {
            id: 'cf_hostname_abc',
            hostname: 'brand.example.com',
            status: 'active',
            ssl: { status: 'validation_failed' },
          },
        }),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const result = await Effect.runPromise(client.getStatus('cf_hostname_abc'))

    expect(result.status).toBe('failed')

    const calls = fake.calls()
    expect(calls[0]?.method).toBe('GET')
    expect(calls[0]?.url).toBe(
      'https://api.cloudflare.com/client/v4/zones/zone_test_123/custom_hostnames/cf_hostname_abc',
    )
  })

  test('deleteCustomHostname: DELETEs by id and succeeds on a success envelope', async () => {
    const fake = makeFakeFetch([
      () =>
        jsonResponse({
          success: true,
          result: { id: 'cf_hostname_abc' },
        }),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    await Effect.runPromise(client.deleteCustomHostname('cf_hostname_abc'))

    const calls = fake.calls()
    expect(calls[0]?.method).toBe('DELETE')
    expect(calls[0]?.url).toBe(
      'https://api.cloudflare.com/client/v4/zones/zone_test_123/custom_hostnames/cf_hostname_abc',
    )
  })

  test('error mapping: 4xx HTTP -> CustomHostnameClientError', async () => {
    const fake = makeFakeFetch([
      () =>
        jsonResponse(
          {
            success: false,
            errors: [{ code: 1436, message: 'workers.api.error.email_validation' }],
            result: null,
          },
          400,
        ),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const error = await Effect.runPromise(
      client
        .createCustomHostname({
          hostname: 'brand.example.com',
          verificationToken: 'verify_tok_1',
        })
        .pipe(Effect.flip),
    )

    expect(error._tag).toBe('CustomHostnameClientError')
    expect(error.operation).toBe('createCustomHostname')
  })

  test('error mapping: 5xx HTTP -> CustomHostnameClientError', async () => {
    const fake = makeFakeFetch([
      () => jsonResponse({ success: false, result: null }, 503),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const error = await Effect.runPromise(
      client.getStatus('cf_hostname_abc').pipe(Effect.flip),
    )

    expect(error._tag).toBe('CustomHostnameClientError')
    expect(error.operation).toBe('getStatus')
  })

  test('error mapping: success=false on a 200 -> CustomHostnameClientError', async () => {
    const fake = makeFakeFetch([
      () =>
        jsonResponse({
          success: false,
          errors: [{ code: 1000, message: 'invalid token' }],
          result: null,
        }),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const error = await Effect.runPromise(
      client.getStatus('cf_hostname_abc').pipe(Effect.flip),
    )

    expect(error._tag).toBe('CustomHostnameClientError')
  })

  test('error mapping: network/transport failure -> CustomHostnameClientError', async () => {
    const fetchImpl: CloudflareCustomHostnameFetch = async () => {
      throw new Error('connect ECONNREFUSED')
    }

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl,
    })

    const error = await Effect.runPromise(
      client
        .createCustomHostname({
          hostname: 'brand.example.com',
          verificationToken: 'verify_tok_1',
        })
        .pipe(Effect.flip),
    )

    expect(error._tag).toBe('CustomHostnameClientError')
    expect(error.operation).toBe('createCustomHostname')
  })

  test('error mapping: malformed result shape -> CustomHostnameClientError', async () => {
    const fake = makeFakeFetch([
      // success=true but result is missing required `id`/`hostname`.
      () => jsonResponse({ success: true, result: { unexpected: true } }),
    ])

    const client = makeCloudflareCustomHostnameClient({
      ...config,
      fetchImpl: fake.fetchImpl,
    })

    const error = await Effect.runPromise(
      client.getStatus('cf_hostname_abc').pipe(Effect.flip),
    )

    expect(error._tag).toBe('CustomHostnameClientError')
  })

  // -------------------------------------------------------------------------
  // Config reader: feature cleanly disabled until BOTH secrets are present.
  // -------------------------------------------------------------------------

  test('config reader: returns undefined when both env vars are unset', () => {
    expect(readCloudflareCustomHostnameConfig({})).toBeUndefined()
  })

  test('config reader: returns undefined when only the token is set', () => {
    expect(
      readCloudflareCustomHostnameConfig({ CLOUDFLARE_API_TOKEN: 'tok' }),
    ).toBeUndefined()
  })

  test('config reader: returns undefined when only the zone id is set', () => {
    expect(
      readCloudflareCustomHostnameConfig({ CLOUDFLARE_ZONE_ID: 'zone' }),
    ).toBeUndefined()
  })

  test('config reader: treats blank/whitespace values as unset', () => {
    expect(
      readCloudflareCustomHostnameConfig({
        CLOUDFLARE_API_TOKEN: '   ',
        CLOUDFLARE_ZONE_ID: 'zone',
      }),
    ).toBeUndefined()
  })

  test('config reader: returns the config when both env vars are set', () => {
    const result = readCloudflareCustomHostnameConfig({
      CLOUDFLARE_API_TOKEN: 'tok',
      CLOUDFLARE_ZONE_ID: 'zone',
    })

    expect(result).toEqual({ apiToken: 'tok', zoneId: 'zone' })
  })
})
