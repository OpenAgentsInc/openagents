import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OpenAgentsWorkerConfigEnv,
  decodeOpenAgentsWorkerConfig,
} from './config'
import { makeExaClient } from './exa'

const minimalEnv = (
  overrides: Partial<OpenAgentsWorkerConfigEnv> = {},
): OpenAgentsWorkerConfigEnv => ({
  GITHUB_CLIENT_ID: 'github-client',
  GITHUB_CLIENT_SECRET: 'github-secret',
  OPENAGENTS_APP_URL: 'https://openagents.com',
  OPENAUTH_CLIENT_ID: 'openauth-client',
  OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
  ...overrides,
})

type CapturedFetch = Readonly<{
  body: unknown
  headers: Headers
  method: string
  url: string
}>

const jsonResponse = (value: unknown, status = 200): Response =>
  new Response(JSON.stringify(value), {
    headers: { 'content-type': 'application/json' },
    status,
  })

const makeJsonFetch = (
  response: Response,
): Readonly<{
  captured: Array<CapturedFetch>
  fetcher: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
}> => {
  const captured: Array<CapturedFetch> = []

  return {
    captured,
    fetcher: async (input, init) => {
      captured.push({
        body:
          typeof init?.body === 'string'
            ? JSON.parse(init.body)
            : (init?.body ?? null),
        headers: new Headers(init?.headers),
        method: init?.method ?? 'GET',
        url: String(input),
      })

      return response
    },
  }
}

const configuredClient = async (
  response: Response,
  overrides: Partial<OpenAgentsWorkerConfigEnv> = {},
) => {
  const config = await Effect.runPromise(
    decodeOpenAgentsWorkerConfig(
      minimalEnv({
        EXA_API_KEY: 'exa-test-secret',
        ...overrides,
      }),
    ),
  )
  const fake = makeJsonFetch(response)

  return { client: makeExaClient(config.exa, fake.fetcher), fake }
}

describe('ExaClient', () => {
  test('returns a typed disabled state when EXA_API_KEY is missing', async () => {
    const config = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig(minimalEnv()),
    )
    const fake = makeJsonFetch(jsonResponse({ results: [] }))
    const client = makeExaClient(config.exa, fake.fetcher)

    await expect(
      Effect.runPromise(client.search({ query: 'OTEC floating datacenter' })),
    ).rejects.toMatchObject({
      _tag: 'ExaConfigurationDisabled',
    })
    expect(fake.captured).toHaveLength(0)
  })

  test('sends current /search payload shape with nested contents defaults', async () => {
    const { client, fake } = await configuredClient(
      jsonResponse({
        costDollars: {
          search: {
            neural: 0.012,
          },
          total: 0.012,
        },
        requestId: 'req_search',
        results: [
          {
            contents: {
              highlights: ['Ocean thermal energy conversion context.'],
            },
            highlights: ['Top-level OTEC provider highlight.'],
            id: 'https://example.com/otec',
            score: 0.97,
            title: 'OTEC overview',
            url: 'https://example.com/otec',
          },
        ],
        searchType: 'auto',
      }),
    )

    const response = await Effect.runPromise(
      client.search({ query: 'OTEC SWAC floating datacenter' }),
    )

    expect(response.costDollars).toBe(0.012)
    expect(response.results).toHaveLength(1)
    expect(response.results[0]?.highlights).toEqual([
      'Top-level OTEC provider highlight.',
    ])
    expect(fake.captured).toHaveLength(1)
    expect(fake.captured[0]?.url).toBe('https://api.exa.ai/search')
    expect(fake.captured[0]?.method).toBe('POST')
    expect(fake.captured[0]?.headers.get('x-api-key')).toBe('exa-test-secret')
    expect(fake.captured[0]?.body).toMatchObject({
      contents: {
        highlights: true,
        maxAgeHours: 24,
      },
      numResults: 8,
      query: 'OTEC SWAC floating datacenter',
      type: 'auto',
    })
    expect(fake.captured[0]?.body).not.toHaveProperty('highlights')
    expect(fake.captured[0]?.body).not.toHaveProperty('text')
    expect(fake.captured[0]?.body).not.toHaveProperty('livecrawl')
    expect(JSON.stringify(response)).not.toContain('exa-test-secret')
  })

  test('supports people search entities only through the typed category', async () => {
    const { client, fake } = await configuredClient(
      jsonResponse({
        requestId: 'req_people',
        results: [
          {
            contents: {
              highlights: ['Public professional profile highlight.'],
            },
            entities: [{ name: 'Ben', type: 'person' }],
            title: 'Ben public profile',
            url: 'https://example.com/ben',
          },
        ],
      }),
    )

    const response = await Effect.runPromise(
      client.search({
        category: 'people',
        query: 'public professional profile for explicit source ref',
      }),
    )

    expect(response.results[0]?.entities).toEqual([
      { name: 'Ben', type: 'person' },
    ])
    expect(fake.captured[0]?.body).toMatchObject({
      category: 'people',
    })
  })

  test('sends /contents payload with top-level URL targets and nested options', async () => {
    const { client, fake } = await configuredClient(
      jsonResponse({
        requestId: 'req_contents',
        results: [
          {
            highlights: ['Seawater air conditioning context.'],
            title: 'SWAC overview',
            url: 'https://example.com/swac',
          },
        ],
      }),
    )

    const response = await Effect.runPromise(
      client.getContents({
        contents: {
          text: { maxCharacters: 500 },
        },
        urls: ['https://example.com/swac'],
      }),
    )

    expect(response.results[0]?.highlights).toEqual([
      'Seawater air conditioning context.',
    ])
    expect(fake.captured[0]?.url).toBe('https://api.exa.ai/contents')
    expect(fake.captured[0]?.body).toMatchObject({
      contents: {
        maxAgeHours: 24,
        text: { maxCharacters: 500 },
      },
      urls: ['https://example.com/swac'],
    })
    expect(fake.captured[0]?.body).not.toHaveProperty('text')
    expect(fake.captured[0]?.body).not.toHaveProperty('highlights')
  })

  test('classifies HTTP errors without serializing the API key', async () => {
    const { client } = await configuredClient(
      jsonResponse(
        {
          error:
            'rate limited while using bearer sk-provider-secret and exa-test-secret',
        },
        429,
      ),
    )

    await expect(
      Effect.runPromise(client.search({ query: 'OTEC' })),
    ).rejects.toMatchObject({
      _tag: 'ExaProviderHttpError',
      endpoint: '/search',
      status: 429,
    })

    try {
      await Effect.runPromise(client.search({ query: 'OTEC' }))
    } catch (error) {
      const serialized = JSON.stringify(error)
      expect(serialized).not.toContain('sk-provider-secret')
      expect(serialized).not.toContain('exa-test-secret')
    }
  })

  test('classifies invalid JSON and schema mismatch responses', async () => {
    const invalidJsonConfig = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig(
        minimalEnv({
          EXA_API_KEY: 'exa-test-secret',
        }),
      ),
    )
    const invalidJsonClient = makeExaClient(
      invalidJsonConfig.exa,
      async () => new Response('{', { status: 200 }),
    )

    await expect(
      Effect.runPromise(invalidJsonClient.search({ query: 'OTEC' })),
    ).rejects.toMatchObject({
      _tag: 'ExaProviderInvalidJson',
    })

    const { client } = await configuredClient(jsonResponse({ results: [{}] }))

    await expect(
      Effect.runPromise(client.search({ query: 'OTEC' })),
    ).rejects.toMatchObject({
      _tag: 'ExaProviderSchemaError',
    })
  })

  test('classifies abort and timeout-shaped fetch failures', async () => {
    const config = await Effect.runPromise(
      decodeOpenAgentsWorkerConfig(
        minimalEnv({
          EXA_API_KEY: 'exa-test-secret',
          EXA_REQUEST_TIMEOUT_MS: '10',
        }),
      ),
    )
    const client = makeExaClient(config.exa, async () => {
      throw new DOMException('operation timed out', 'TimeoutError')
    })

    await expect(
      Effect.runPromise(client.search({ query: 'OTEC' })),
    ).rejects.toMatchObject({
      _tag: 'ExaProviderTimeout',
      timeoutMs: 10,
    })
  })
})
