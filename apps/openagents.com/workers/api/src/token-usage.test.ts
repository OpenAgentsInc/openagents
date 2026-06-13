import { describe, expect, test } from 'vitest'

import {
  TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF,
  extractAutopilotTokenUsage,
  resolveTokenUsageAccountAttribution,
  sourceRefForTokenUsageEvent,
} from './token-usage'

describe('Autopilot token usage extraction', () => {
  test('reads OpenCode step-finish token usage', () => {
    expect(
      extractAutopilotTokenUsage({
        model: 'gpt-5-codex',
        part: {
          cost: 0.01,
          tokens: {
            cache: { read: 30, write: 40 },
            input: 100,
            output: 50,
            reasoning: 20,
          },
          type: 'step-finish',
        },
        provider: 'openai',
      }),
    ).toEqual({
      cacheReadTokens: 30,
      cacheWrite1hTokens: 40,
      cacheWrite5mTokens: 0,
      inputTokens: 100,
      model: 'gpt-5-codex',
      outputTokens: 50,
      provider: 'openai',
      reasoningTokens: 20,
      totalTokens: 240,
    })
  })

  test('reads OpenCode inference log field names', () => {
    expect(
      extractAutopilotTokenUsage({
        model: 'gpt-5-codex',
        provider: 'openai',
        tokens: {
          cache_read: 7,
          cache_write_1h: 11,
          cache_write_5m: 13,
          input: 101,
          output: 17,
          reasoning: 19,
        },
      }),
    ).toEqual({
      cacheReadTokens: 7,
      cacheWrite1hTokens: 11,
      cacheWrite5mTokens: 13,
      inputTokens: 101,
      model: 'gpt-5-codex',
      outputTokens: 17,
      provider: 'openai',
      reasoningTokens: 19,
      totalTokens: 168,
    })
  })

  test('reads OpenAI-compatible response usage', () => {
    expect(
      extractAutopilotTokenUsage({
        response: {
          usage: {
            completion_tokens: 34,
            completion_tokens_details: { reasoning_tokens: 12 },
            prompt_tokens: 123,
            prompt_tokens_details: { cached_tokens: 56 },
            total_tokens: 157,
          },
        },
      }),
    ).toMatchObject({
      cacheReadTokens: 56,
      inputTokens: 123,
      outputTokens: 34,
      reasoningTokens: 12,
      totalTokens: 157,
    })
  })

  test('reads Codex JSONL turn-completed usage inside an SHC runner log wrapper', () => {
    const codexEvent = {
      type: 'turn.completed',
      usage: {
        cached_input_tokens: 5,
        input_tokens: 20,
        output_tokens: 8,
        reasoning_output_tokens: 2,
      },
    }

    expect(
      extractAutopilotTokenUsage({
        dataJson: JSON.stringify({
          artifactRefs: [],
          detail: `stdout: ${JSON.stringify(codexEvent)}`,
          receiptRefs: [],
        }),
        source: 'runner',
        summary: 'Codex VM log captured.',
        type: 'runner.log',
      }),
    ).toEqual({
      cacheReadTokens: 5,
      cacheWrite1hTokens: 0,
      cacheWrite5mTokens: 0,
      inputTokens: 20,
      model: null,
      outputTokens: 8,
      provider: null,
      reasoningTokens: 2,
      totalTokens: 30,
    })
  })

  test('reads Codex app-server last-turn token usage without double-counting thread totals', () => {
    expect(
      extractAutopilotTokenUsage({
        tokenUsage: {
          last: {
            cachedInputTokens: 5,
            inputTokens: 20,
            outputTokens: 8,
            reasoningOutputTokens: 2,
            totalTokens: 30,
          },
          total: {
            cachedInputTokens: 50,
            inputTokens: 200,
            outputTokens: 80,
            reasoningOutputTokens: 20,
            totalTokens: 300,
          },
        },
        type: 'thread.token_usage.updated',
      }),
    ).toMatchObject({
      cacheReadTokens: 5,
      inputTokens: 20,
      outputTokens: 8,
      reasoningTokens: 2,
      totalTokens: 30,
    })
  })

  test('reads Anthropic cache write and read usage fields', () => {
    expect(
      extractAutopilotTokenUsage({
        message: {
          model: 'claude-opus-4-5-20251101',
          usage: {
            cache_creation: {
              ephemeral_1h_input_tokens: 11,
              ephemeral_5m_input_tokens: 11543,
            },
            cache_creation_input_tokens: 11543,
            cache_read_input_tokens: 7,
            input_tokens: 3,
            output_tokens: 1,
          },
        },
        provider: 'anthropic',
      }),
    ).toMatchObject({
      cacheReadTokens: 7,
      cacheWrite1hTokens: 11,
      cacheWrite5mTokens: 11543,
      inputTokens: 3,
      outputTokens: 1,
      provider: 'anthropic',
      totalTokens: 11565,
    })
  })

  test('reads Gemini usageMetadata fields', () => {
    expect(
      extractAutopilotTokenUsage({
        modelVersion: 'gemini-3-pro',
        usageMetadata: {
          cachedContentTokenCount: 4,
          candidatesTokenCount: 3,
          promptTokenCount: 10,
          thoughtsTokenCount: 2,
          totalTokenCount: 15,
        },
      }),
    ).toMatchObject({
      cacheReadTokens: 4,
      inputTokens: 6,
      model: 'gemini-3-pro',
      outputTokens: 3,
      reasoningTokens: 2,
      totalTokens: 15,
    })
  })

  test('uses stable sequence source refs for retry idempotency', () => {
    expect(
      sourceRefForTokenUsageEvent({
        artifactRefs: [],
        createdAt: '2026-06-03T00:00:00.000Z',
        externalEventId: null,
        id: 'event_random',
        parentId: 'agent_run_1',
        payloadJson: null,
        sequence: 9,
        source: 'shc',
        status: null,
        summary: 'usage',
        type: 'runner.usage',
      }),
    ).toBe('agent_run_1:9')
  })
})

describe('resolveTokenUsageAccountAttribution', () => {
  test('attributes usage to a run-carried provider-account lease ref', () => {
    expect(
      resolveTokenUsageAccountAttribution('provider-account_chatgpt_codex_a'),
    ).toEqual({
      accountRef: 'provider-account_chatgpt_codex_a',
      attributed: true,
    })
  })

  test('records the typed unattributed sentinel when no lease ref is present', () => {
    for (const value of [null, undefined, '', '   ']) {
      expect(resolveTokenUsageAccountAttribution(value)).toEqual({
        accountRef: TOKEN_USAGE_UNATTRIBUTED_ACCOUNT_REF,
        attributed: false,
      })
    }
  })
})
