import { describe, expect, it } from 'vitest'
import { registerThrowawaySmokeAgent } from './predeploy-khala-sync-live-seam-smoke.mjs'

const jsonResponse = (status: number, body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

describe('predeploy khala-sync live-seam smoke self-registration', () => {
  it('registers a throwaway agent and returns its token and user id', async () => {
    const requests: Array<{ url: string; body: unknown }> = []
    const fetchImpl = (async (input: unknown, init?: RequestInit) => {
      requests.push({
        url: String(input),
        body: JSON.parse(String(init?.body)),
      })
      return jsonResponse(200, {
        credential: { token: 'oa_agent_test_token' },
        user: { id: 'user_smoke_1' },
      })
    }) as typeof fetch

    const result = await registerThrowawaySmokeAgent(
      fetchImpl,
      'https://openagents-staging.openagents.workers.dev',
      'run1',
    )

    expect(result).toEqual({
      token: 'oa_agent_test_token',
      userId: 'user_smoke_1',
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]!.url).toBe(
      'https://openagents-staging.openagents.workers.dev/api/agents/register',
    )
    const body = requests[0]!.body as {
      externalId: string
      slug: string
      metadata: { authority: string }
    }
    // Fresh externalId/slug per run — the registry rejects reuse.
    expect(body.externalId).toBe('predeploy.khala_sync.live_seam.run1')
    expect(body.slug).toContain('khala-seam-smoke-')
    expect(body.metadata.authority).toBe(
      'staging_predeploy_khala_sync_live_seam_smoke',
    )
  })

  it('throws on a non-2xx registration response', async () => {
    const fetchImpl = (async () => jsonResponse(503, {})) as typeof fetch
    await expect(
      registerThrowawaySmokeAgent(fetchImpl, 'https://staging.test', 'run2'),
    ).rejects.toThrow('HTTP 503')
  })

  it('throws when the response carries no token or no user id', async () => {
    const noToken = (async () =>
      jsonResponse(200, { user: { id: 'user_x' } })) as typeof fetch
    await expect(
      registerThrowawaySmokeAgent(noToken, 'https://staging.test', 'run3'),
    ).rejects.toThrow('no credential token')

    const noUser = (async () =>
      jsonResponse(200, {
        credential: { token: 'oa_agent_x' },
      })) as typeof fetch
    await expect(
      registerThrowawaySmokeAgent(noUser, 'https://staging.test', 'run4'),
    ).rejects.toThrow('no user id')
  })
})
