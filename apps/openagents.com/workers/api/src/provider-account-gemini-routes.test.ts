import { describe, expect, test, vi } from 'vitest'

import { makeProviderAccountServiceHandlers } from './provider-account-service-routes'

type TokenUsageRow = Record<string, string | number | null>

const env = {
  AUTH_STORAGE: {} as KVNamespace,
  GEMINI_API_KEY: 'test-gemini-key',
  OPENAGENTS_DB: {} as D1Database,
}

const makeExecutionContext = (): Readonly<{
  ctx: ExecutionContext
  promises: Array<Promise<unknown>>
}> => {
  const promises: Array<Promise<unknown>> = []

  return {
    ctx: {
      passThroughOnException: () => undefined,
      waitUntil: (promise: Promise<unknown>) => {
        promises.push(Promise.resolve(promise))
      },
    } as unknown as ExecutionContext,
    promises,
  }
}

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const tokenUsageRowFromBindings = (
  values: ReadonlyArray<unknown>,
): TokenUsageRow => ({
  account_ref: values[8] as string | null,
  actor_team_id: values[7] as string | null,
  actor_user_id: values[6] as string | null,
  anonymized_source_ref: values[9] as string | null,
  backend_profile: values[16] as string | null,
  cache_read_tokens: values[20] as number,
  cache_write_1h_tokens: values[22] as number,
  cache_write_5m_tokens: values[21] as number,
  cost_amount: values[25] as number | null,
  currency: values[26] as string | null,
  id: values[0] as string,
  idempotency_key: values[1] as string,
  ingested_at: values[3] as string,
  input_tokens: values[17] as number,
  leaderboard_eligible: values[27] as number,
  model: values[15] as string,
  observed_at: values[2] as string,
  output_tokens: values[18] as number,
  privacy_opt_out: values[28] as number,
  producer_system: values[4] as string,
  provider: values[14] as string,
  reasoning_tokens: values[19] as number,
  repository_ref: values[13] as string | null,
  run_ref: values[10] as string | null,
  safe_metadata_json: values[29] as string,
  session_ref: values[11] as string | null,
  source_route: values[5] as string,
  task_ref: values[12] as string | null,
  total_tokens: values[23] as number,
  usage_truth: values[24] as string,
})

const makeTokenUsageDb = (): Readonly<{
  db: D1Database
  rows: Array<TokenUsageRow>
}> => {
  const rows: Array<TokenUsageRow> = []
  const prepare = (query: string): D1PreparedStatement => {
    let values: ReadonlyArray<unknown> = []

    function raw<T = unknown[]>(options: {
      columnNames: true
    }): Promise<[Array<string>, ...Array<T>]>
    function raw<T = unknown[]>(options?: {
      columnNames?: false
    }): Promise<Array<T>>
    function raw<T = unknown[]>(options?: {
      columnNames?: boolean
    }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
      return options?.columnNames === true
        ? Promise.resolve([[]])
        : Promise.resolve([])
    }

    const statement: D1PreparedStatement = {
      all: <T = Record<string, unknown>>() => Promise.resolve(makeResult<T>()),
      bind: (...nextValues: ReadonlyArray<unknown>) => {
        values = nextValues

        return statement
      },
      first: <T = Record<string, unknown>>() => Promise.resolve<T | null>(null),
      raw,
      run: <T = Record<string, unknown>>() => {
        if (query.includes('INSERT OR IGNORE INTO token_usage_events')) {
          const row = tokenUsageRowFromBindings(values)

          if (
            !rows.some(
              candidate =>
                candidate.id === row.id ||
                candidate.idempotency_key === row.idempotency_key,
            )
          ) {
            rows.push(row)
          }
        }

        return Promise.resolve(makeResult<T>())
      },
    }

    return statement
  }
  const db = {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare,
    withSession: () =>
      ({
        batch: async <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare,
      }) satisfies D1DatabaseSession,
  } satisfies D1Database

  return { db, rows }
}

const handlers = makeProviderAccountServiceHandlers({
  readConnectedCodexAuthMaterial: () => Promise.resolve(undefined),
  requireProviderServiceActor: request =>
    request.headers.get('authorization') === 'Bearer oa_agent_test'
      ? Promise.resolve({ user: { id: 'agent:test' } })
      : Promise.resolve(undefined),
})

describe('google gemini provider account routes', () => {
  test('rejects Gemini grant resolution without service bearer auth', async () => {
    const response = await handlers.handleGoogleGeminiGrantResolveApi(
      new Request(
        'https://openagents.com/api/provider-accounts/google-gemini/grants/resolve',
        {
          body: JSON.stringify({ grantRef: 'provider-auth-grant_test' }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      env,
    )

    expect(response.status).toBe(401)
    expect(response.headers.get('cache-control')).toBe('no-store')
  })

  test('returns a Probe-compatible redacted Gemini grant', async () => {
    const response = await handlers.handleGoogleGeminiGrantResolveApi(
      new Request(
        'https://openagents.com/api/provider-accounts/google-gemini/grants/resolve',
        {
          body: JSON.stringify({
            grantRef: 'provider-auth-grant_test',
            providerAccountRef: 'provider-account_google_gemini_primary',
            runnerSessionId: 'runner_session_test',
          }),
          headers: {
            authorization: 'Bearer oa_agent_test',
            'content-type': 'application/json',
          },
          method: 'POST',
        },
      ),
      env,
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      grantRef: 'provider-auth-grant_test',
      provider: 'google_gemini',
      providerAccountRef: 'provider-account_google_gemini_primary',
      providerSecretRef:
        'provider-account://google-gemini/worker-secret/GEMINI_API_KEY',
      runnerSessionId: 'runner_session_test',
      status: 'issued',
      materialization: {
        kind: 'probe_gemini_api_key',
        provider: 'google_gemini',
        providerSecretRef:
          'provider-account://google-gemini/worker-secret/GEMINI_API_KEY',
        target: {
          kind: 'env',
          name: 'GOOGLE_GENERATIVE_AI_API_KEY',
        },
        homeIsolation: 'per_run',
        scrubAfterCloseout: true,
      },
    })
    expect(JSON.stringify(body)).not.toContain('test-gemini-key')
  })

  test('brokers Gemini inference with the Worker secret and no credential echo', async () => {
    const upstream = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        'data: {"candidates":[{"content":{"role":"model","parts":[{"text":"ok"}]},"finishReason":"STOP"}]}\n\ndata: [DONE]\n\n',
        {
          headers: { 'content-type': 'text/event-stream' },
          status: 200,
        },
      ),
    )

    try {
      const { ctx, promises } = makeExecutionContext()
      const response = await handlers.handleGoogleGeminiGenerateContentApi(
        new Request(
          'https://openagents.com/api/provider-accounts/google-gemini/models/gemini-2.5-flash:streamGenerateContent?alt=sse',
          {
            body: JSON.stringify({ contents: [] }),
            headers: {
              authorization: 'Bearer oa_agent_test',
              'content-type': 'application/json',
            },
            method: 'POST',
          },
        ),
        env,
        ctx,
        'gemini-2.5-flash',
      )
      const body = await response.text()
      await Promise.all(promises)
      const upstreamHeaders = new Headers(upstream.mock.calls[0]?.[1]?.headers)

      expect(response.status).toBe(200)
      expect(response.headers.get('cache-control')).toBe('no-store')
      expect(response.headers.get('content-type')).toContain(
        'text/event-stream',
      )
      expect(body).toContain('"ok"')
      expect(body).not.toContain('test-gemini-key')
      expect(String(upstream.mock.calls[0]?.[0])).toBe(
        'https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse',
      )
      expect(upstreamHeaders.get('x-goog-api-key')).toBe('test-gemini-key')
    } finally {
      upstream.mockRestore()
    }
  })

  test('records Gemini usageMetadata from brokered successful responses idempotently', async () => {
    const upstream = vi.spyOn(globalThis, 'fetch').mockImplementation(() =>
      Promise.resolve(
        new Response(
          [
            'data: {"candidates":[{"content":{"role":"model","parts":[{"text":"ok"}]},"finishReason":"STOP"}]}',
            '',
            'data: {"usageMetadata":{"promptTokenCount":120,"candidatesTokenCount":35,"thoughtsTokenCount":10,"cachedContentTokenCount":20,"totalTokenCount":165}}',
            '',
            'data: [DONE]',
            '',
          ].join('\n'),
          {
            headers: { 'content-type': 'text/event-stream' },
            status: 200,
          },
        ),
      ),
    )
    const store = makeTokenUsageDb()
    const envWithDb = {
      ...env,
      OPENAGENTS_DB: store.db,
    }

    try {
      for (const index of [0, 1]) {
        const { ctx, promises } = makeExecutionContext()
        const response = await handlers.handleGoogleGeminiGenerateContentApi(
          new Request(
            'https://openagents.com/api/provider-accounts/google-gemini/models/gemini-2.5-flash:streamGenerateContent?alt=sse',
            {
              body: JSON.stringify({ contents: [{ parts: [{ text: 'hi' }] }] }),
              headers: {
                authorization: 'Bearer oa_agent_test',
                'content-type': 'application/json',
                'idempotency-key': 'gemini-route-success-1',
                'x-openagents-run-ref': 'probe-run:gemini-route-1',
              },
              method: 'POST',
            },
          ),
          envWithDb,
          ctx,
          'gemini-2.5-flash',
        )

        expect(response.status).toBe(200)
        await response.text()
        await Promise.all(promises)
        expect(upstream).toHaveBeenCalledTimes(index + 1)
      }

      expect(store.rows).toHaveLength(1)
      expect(store.rows[0]).toMatchObject({
        account_ref: 'provider-account_google_gemini_worker_secret',
        actor_user_id: 'agent:test',
        backend_profile: 'worker_secret_gemini_api_key',
        cache_read_tokens: 20,
        input_tokens: 100,
        model: 'gemini-2.5-flash',
        output_tokens: 35,
        producer_system: 'omega',
        provider: 'google_gemini',
        reasoning_tokens: 10,
        run_ref: 'probe-run:gemini-route-1',
        source_route: 'omega_provider_broker',
        total_tokens: 165,
        usage_truth: 'exact',
      })
      expect(JSON.stringify(store.rows)).not.toContain('test-gemini-key')
      expect(JSON.stringify(store.rows)).not.toContain('contents')
    } finally {
      upstream.mockRestore()
    }
  })

  test('records Gemini usageMetadata from provider failures', async () => {
    const upstream = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          error: { code: 429, message: 'rate limited' },
          usageMetadata: {
            candidatesTokenCount: 0,
            promptTokenCount: 12,
            totalTokenCount: 12,
          },
        }),
        {
          headers: { 'content-type': 'application/json' },
          status: 429,
        },
      ),
    )
    const store = makeTokenUsageDb()
    const { ctx, promises } = makeExecutionContext()

    try {
      const response = await handlers.handleGoogleGeminiGenerateContentApi(
        new Request(
          'https://openagents.com/api/provider-accounts/google-gemini/models/gemini-2.5-pro:streamGenerateContent?alt=sse',
          {
            body: JSON.stringify({ contents: [{ parts: [{ text: 'hi' }] }] }),
            headers: {
              authorization: 'Bearer oa_agent_test',
              'content-type': 'application/json',
              'idempotency-key': 'gemini-route-failure-1',
            },
            method: 'POST',
          },
        ),
        {
          ...env,
          OPENAGENTS_DB: store.db,
        },
        ctx,
        'gemini-2.5-pro',
      )

      expect(response.status).toBe(429)
      await response.text()
      await Promise.all(promises)
      expect(store.rows).toHaveLength(1)
      expect(store.rows[0]).toMatchObject({
        account_ref: 'provider-account_google_gemini_worker_secret',
        input_tokens: 12,
        model: 'gemini-2.5-pro',
        output_tokens: 0,
        source_route: 'omega_provider_broker',
        total_tokens: 12,
      })
      expect(JSON.parse(String(store.rows[0]?.safe_metadata_json))).toEqual({
        providerHttpStatus: 429,
        providerRequestStatus: 'failed',
      })
    } finally {
      upstream.mockRestore()
    }
  })
})
