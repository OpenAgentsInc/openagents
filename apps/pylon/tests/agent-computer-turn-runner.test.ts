import { describe, expect, test } from 'bun:test'

import {
  AGENT_COMPUTER_DEFAULT_PROVIDER,
  AGENT_COMPUTER_RECEIPT_LANE,
  KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH,
  KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
  chatCompletionUsage,
  chatCompletionText,
  chatCompletionsRequest,
  runModelTurnReceipt,
  usageIngestBody,
  type InferenceConfig,
} from '../deploy/agent-computer/turn-runner.ts'

const inference: InferenceConfig = {
  baseUrl: 'https://openagents-monolith-staging-ezxz4mgdsq-uc.a.run.app',
  agentToken: 'agent-secret-token-should-never-be-serialized',
  ownerUserId: 'github:300914913',
  model: 'gemini-2.5-flash',
  backendProfile: 'omega-hosted-gemini',
  pylonRef: 'pylon.agent-computer.proof',
}

describe('agent-computer turn-runner: usage + text parsing', () => {
  test('extracts exact usage from an OpenAI-shaped completion (prompt/completion_tokens)', () => {
    const usage = chatCompletionUsage({
      usage: { prompt_tokens: 41, completion_tokens: 17, total_tokens: 58 },
    })
    expect(usage).toEqual({ inputTokens: 41, outputTokens: 17, totalTokens: 58 })
  })

  test('falls back to input_tokens/output_tokens and clamps total to the sum', () => {
    const usage = chatCompletionUsage({
      usage: { input_tokens: 10, output_tokens: 5, total_tokens: 3 },
    })
    // total can never be below the exact input+output sum.
    expect(usage).toEqual({ inputTokens: 10, outputTokens: 5, totalTokens: 15 })
  })

  test('extracts assistant text from choices[0].message.content', () => {
    expect(
      chatCompletionText({ choices: [{ message: { content: 'a staged proof note' } }] }),
    ).toBe('a staged proof note')
    expect(chatCompletionText({})).toBe('')
  })
})

describe('agent-computer turn-runner: request contracts', () => {
  test('chat/completions request mirrors runWithHostedKhalaGateway', () => {
    const req = chatCompletionsRequest(inference, 'describe the change')
    expect(req.url).toBe(`${inference.baseUrl}/v1/chat/completions`)
    expect(req.headers.Authorization).toBe(`Bearer ${inference.agentToken}`)
    expect(req.headers['x-openagents-client']).toBe('khala-code-mobile')
    expect(req.headers['x-openagents-demand-kind']).toBe('external')
    expect(req.headers['x-openagents-demand-source']).toBe(
      'khala_mobile_org_cloud_runtime',
    )
    const body = JSON.parse(req.body)
    expect(body).toEqual({
      messages: [{ content: 'describe the change', role: 'user' }],
      model: inference.model,
      stream: false,
    })
  })

  test('usage ingest body matches the KhalaCloudRuntimeUsageIngestBody schema', () => {
    const body = usageIngestBody({
      inference,
      threadId: 'scope.thread.proof',
      turnId: 'turn-microvm-1',
      observedAt: '2026-07-07T00:00:00.000Z',
      usage: { inputTokens: 41, outputTokens: 17, totalTokens: 58 },
      usageRef: 'usage.hosted_khala.abc',
      runtimeEventId: 'evt-1',
    })
    expect(body).toMatchObject({
      schemaVersion: KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
      ownerUserId: 'github:300914913',
      threadId: 'scope.thread.proof',
      turnId: 'turn-microvm-1',
      lane: AGENT_COMPUTER_RECEIPT_LANE,
      provider: AGENT_COMPUTER_DEFAULT_PROVIDER,
      model: inference.model,
      backendProfile: 'omega-hosted-gemini',
      pylonRef: 'pylon.agent-computer.proof',
      runtimeEventId: 'evt-1',
      observedAt: '2026-07-07T00:00:00.000Z',
    })
    expect(body.usage).toEqual({
      usageRef: 'usage.hosted_khala.abc',
      inputTokens: 41,
      outputTokens: 17,
      reasoningTokens: 0,
      cacheReadInputTokens: 0,
      cacheWriteInputTokens: 0,
      totalTokens: 58,
    })
    // The `lane` literal MUST be one the ingest route accepts.
    expect(['codex_app_server', 'claude_pylon', 'hosted_khala']).toContain(
      body.lane,
    )
  })
})

describe('agent-computer turn-runner: runModelTurnReceipt (mock fetch)', () => {
  test('happy path: inference then exact usage receipt, token never serialized', async () => {
    const calls: Array<{ url: string; init: RequestInit }> = []
    const fetchImpl = (async (url: string | URL | Request, init?: RequestInit) => {
      const u = String(url)
      calls.push({ url: u, init: init ?? {} })
      if (u.endsWith('/v1/chat/completions')) {
        return new Response(
          JSON.stringify({
            choices: [{ message: { content: 'Added AGENT_COMPUTER_TURN.md.' } }],
            usage: { prompt_tokens: 41, completion_tokens: 17, total_tokens: 58 },
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      }
      if (u.endsWith(KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH)) {
        return new Response(
          JSON.stringify({
            insertedTokenUsage: true,
            tokenUsageEventRef: 'event.inference.served-tokens.khala-cloud-runtime.deadbeef',
            tokensServedDelta: 58,
          }),
          { status: 200, headers: { 'content-type': 'application/json' } },
        )
      }
      return new Response('not found', { status: 404 })
    }) as unknown as typeof globalThis.fetch

    const result = await runModelTurnReceipt(
      {
        inference,
        threadId: 'scope.thread.proof',
        turnId: 'turn-microvm-1',
        instructions: 'describe the change',
        runtimeEventId: 'evt-1',
      },
      fetchImpl,
    )

    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.usage).toEqual({ inputTokens: 41, outputTokens: 17, totalTokens: 58 })
      expect(result.insertedTokenUsage).toBe(true)
      expect(result.tokenUsageEventRef).toContain('served-tokens.khala-cloud-runtime')
      expect(result.tokensServedDelta).toBe(58)
      expect(result.text).toBe('Added AGENT_COMPUTER_TURN.md.')
    }

    // Both legs authenticated by the agent bearer; the token is in the header
    // but never in a serialized result field.
    expect(calls).toHaveLength(2)
    expect(JSON.stringify(result)).not.toContain(inference.agentToken)
    const ingestCall = calls.find((c) => c.url.endsWith(KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH))
    expect(ingestCall).toBeDefined()
    expect((ingestCall!.init.headers as Record<string, string>).Authorization).toBe(
      `Bearer ${inference.agentToken}`,
    )
  })

  test('no receipt when the gateway omits exact usage', async () => {
    const fetchImpl = (async () =>
      new Response(JSON.stringify({ choices: [{ message: { content: 'x' } }], usage: {} }), {
        status: 200,
      })) as unknown as typeof globalThis.fetch
    const result = await runModelTurnReceipt(
      { inference, threadId: 't', turnId: 'u', instructions: 'x' },
      fetchImpl,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.stage).toBe('no_exact_usage')
  })

  test('fail-soft on a gateway HTTP error (never throws)', async () => {
    const fetchImpl = (async () =>
      new Response('unauthorized', { status: 401 })) as unknown as typeof globalThis.fetch
    const result = await runModelTurnReceipt(
      { inference, threadId: 't', turnId: 'u', instructions: 'x' },
      fetchImpl,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.stage).toBe('inference')
      expect(result.status).toBe(401)
    }
  })

  test('surfaces a usage-receipt rejection reason without throwing', async () => {
    const fetchImpl = (async (url: string | URL | Request) => {
      const u = String(url)
      if (u.endsWith('/v1/chat/completions')) {
        return new Response(
          JSON.stringify({ usage: { prompt_tokens: 5, completion_tokens: 5 } }),
          { status: 200 },
        )
      }
      return new Response(JSON.stringify({ reason: 'owner mismatch' }), { status: 403 })
    }) as unknown as typeof globalThis.fetch
    const result = await runModelTurnReceipt(
      { inference, threadId: 't', turnId: 'u', instructions: 'x' },
      fetchImpl,
    )
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.stage).toBe('usage_receipt')
      expect(result.status).toBe(403)
      expect(result.error).toBe('owner mismatch')
    }
  })
})
