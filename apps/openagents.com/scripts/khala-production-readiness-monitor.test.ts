import { describe, expect, test, vi } from 'vitest'

const monitor = await import('./khala-production-readiness-monitor.mjs')

const json = (body: unknown, init?: ResponseInit) => Response.json(body, init)

describe('Khala production readiness monitor', () => {
  test('parses options and redacts secret-shaped output', () => {
    const parsed = monitor.parseArgs([
      '--base-url',
      'https://staging.openagents.com',
      '--model',
      'openagents/khala',
      '--forbid-public-model',
      'raw-model',
      '--forbid-public-model-pattern',
      'private-lane',
    ])

    expect(parsed.baseUrl).toBe('https://staging.openagents.com')
    expect(parsed.forbiddenPublicModelIds).toContain('raw-model')
    expect(parsed.forbiddenPublicModelPatterns).toContain('private-lane')
    expect(
      monitor.redactSecrets(
        'Bearer oa_agent_secret sk-123456789 OPENAGENTS_AGENT_TOKEN=secret',
      ),
    ).toBe(
      'Bearer <redacted> sk-<redacted> OPENAGENTS_AGENT_TOKEN=<redacted>',
    )
  })

  test('passes when readiness is ready and the public catalog is exactly Khala', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await monitor.runKhalaProductionReadinessMonitor({
      fetchImpl,
    })

    expect(output.ok).toBe(true)
    expect(output.authority).toMatchObject({
      bearerTokenAllowed: false,
      chatCompletionAllowed: false,
      inferenceSpendAllowed: false,
      mutationAllowed: false,
    })
    expect(output.catalog.modelIds).toEqual(['openagents/khala'])
    expect(fetchImpl).toHaveBeenCalledTimes(2)
    expect(
      fetchImpl.mock.calls.map(([input]) =>
        new URL(input instanceof Request ? input.url : String(input)).pathname,
      ),
    ).toEqual(['/v1/gateway/readiness', '/v1/models'])
  })

  test('blocks when readiness is not ready', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 0, status: 'degraded' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [{ id: 'openagents/khala' }] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await monitor.runKhalaProductionReadinessMonitor({
      fetchImpl,
    })

    expect(output.ok).toBe(false)
    expect(output.checks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: 'readiness_status_ready',
          passed: false,
        }),
        expect.objectContaining({
          name: 'readiness_has_servable_model',
          passed: false,
        }),
      ]),
    )
  })

  test('blocks when Khala is missing from the public catalog', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({ data: [] })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await monitor.runKhalaProductionReadinessMonitor({
      fetchImpl,
    })

    expect(output.ok).toBe(false)
    expect(output.catalog.modelIds).toEqual([])
    expect(output.checks).toContainEqual(
      expect.objectContaining({
        name: 'models_public_surface_exactly_khala',
        passed: false,
      }),
    )
  })

  test('blocks on extra public model selection even when Khala is present', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 2, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({
          data: [
            { id: 'openagents/khala' },
            { id: 'openagents/khala-mini' },
          ],
        })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await monitor.runKhalaProductionReadinessMonitor({
      fetchImpl,
    })

    expect(output.ok).toBe(false)
    expect(output.catalog.leaks).toEqual([
      {
        modelId: 'openagents/khala-mini',
        reasons: [
          'exact_forbidden_id',
          'pattern:khala-mini',
        ],
      },
    ])
    expect(output.checks).toContainEqual(
      expect.objectContaining({
        name: 'models_public_surface_exactly_khala',
        passed: false,
      }),
    )
  })

  test('blocks raw provider model leakage', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))

      if (url.pathname === '/v1/gateway/readiness') {
        return json({ servableModelCount: 1, status: 'ready' })
      }

      if (url.pathname === '/v1/models') {
        return json({
          data: [{ id: 'accounts/fireworks/models/deepseek-v4-flash' }],
        })
      }

      return json({ error: 'unexpected' }, { status: 500 })
    })

    const output = await monitor.runKhalaProductionReadinessMonitor({
      fetchImpl,
    })

    expect(output.ok).toBe(false)
    expect(output.catalog.leaks).toEqual([
      {
        modelId: 'accounts/fireworks/models/deepseek-v4-flash',
        reasons: [
          'exact_forbidden_id',
          'pattern:accounts/',
          'pattern:deepseek',
          'pattern:fireworks',
        ],
      },
    ])
  })

  test('safe output redacts fetch errors before scheduler logs them', async () => {
    const fetchImpl = vi.fn(async (input: string | URL | Request) => {
      const url = new URL(input instanceof Request ? input.url : String(input))
      if (url.pathname === '/v1/gateway/readiness') {
        throw new Error('Authorization: Bearer oa_agent_secret sk-123456789')
      }
      return json({ data: [{ id: 'openagents/khala' }] })
    })

    const output = await monitor.runKhalaProductionReadinessMonitor({
      fetchImpl,
    })

    expect(output.ok).toBe(false)
    expect(JSON.stringify(monitor.safeOutput(output))).not.toContain(
      'oa_agent_secret',
    )
    expect(JSON.stringify(monitor.safeOutput(output))).not.toContain(
      'sk-123456789',
    )
    expect(monitor.safeOutput(output).checks).toContainEqual(
      expect.objectContaining({
        details: expect.objectContaining({
          error: 'Authorization: Bearer <redacted> sk-<redacted>',
        }),
        name: 'readiness_endpoint_200',
        passed: false,
      }),
    )
  })
})
