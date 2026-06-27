import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type KhalaTraceReviewFacts,
  buildKhalaTraceReviewReport,
  handleOperatorKhalaTraceReview,
} from './khala-trace-review-routes'

const run = (effect: Effect.Effect<Response>): Promise<Response> =>
  Effect.runPromise(effect)

const facts: KhalaTraceReviewFacts = {
  modelMix: [
    {
      count: 3,
      model: 'openagents/glm-5.2-reap-504b',
      provider: 'hydralisk',
      totalTokens: 1200,
    },
  ],
  notableTraces: [
    {
      createdAt: '2026-06-26T20:05:00.000Z',
      demandKind: 'external',
      demandSource: 'khala_cli',
      reasonRefs: [],
      stepCount: 8,
      traceRef: 'trace.trace_123',
      traceUuid: 'trace_123',
      visibility: 'owner_only',
    },
  ],
  outcomes: [
    {
      count: 2,
      outcome: 'stop',
      totalTokens: 900,
    },
  ],
  rawEventHighlights: [
    {
      assignmentRef: 'assignment.public.khala_coding.chatcmpl_123',
      byteLength: 4096,
      eventCount: 12,
      observedAt: '2026-06-26T20:08:00.000Z',
      rawEventRef: 'raw_event.public.123',
    },
  ],
  rawEventSummary: {
    assignmentCount: 1,
    byteLength: 4096,
    eventCount: 12,
    rowCount: 1,
  },
  tokenByDemandSource: [
    {
      count: 3,
      label: 'khala_cli',
      totalTokens: 1200,
    },
  ],
  tokenSummary: {
    estimatedUsageCount: 1,
    eventCount: 3,
    inputTokens: 800,
    outputTokens: 400,
    reasoningTokens: 50,
    totalTokens: 1200,
    zeroOutputCount: 1,
  },
  traceByDemandSource: [
    {
      count: 2,
      label: 'khala_cli',
      totalTokens: 0,
    },
  ],
  traceSummary: {
    ownerOnlyCount: 1,
    publicCount: 0,
    traceCount: 2,
    trainingConsentCount: 1,
    unlistedCount: 1,
    zeroStepCount: 1,
  },
}

describe('Khala trace review report', () => {
  test('builds aggregate failure, intent, and triage refs without raw trace data', () => {
    const report = buildKhalaTraceReviewReport({
      facts,
      generatedAt: '2026-06-26T21:00:00.000Z',
      window: {
        hours: 24,
        since: '2026-06-25T21:00:00.000Z',
        until: '2026-06-26T21:00:00.000Z',
      },
    })

    expect(report.schemaVersion).toBe('openagents.khala.trace_review.v1')
    expect(report.aggregates.tokens.totalTokens).toBe(1200)
    expect(report.modelMix[0]?.model).toBe('openagents/glm-5.2-reap-504b')
    expect(report.failureModes.map(mode => mode.failureRef)).toEqual([
      'failure.khala_trace_review.empty_response',
      'failure.khala_trace_review.estimated_usage',
      'failure.khala_trace_review.empty_trace',
    ])
    expect(report.userIntents[0]).toMatchObject({
      intentRef: 'intent.khala_trace_review.khala_cli',
      label: 'khala_cli',
    })
    expect(report.triageItems.length).toBeGreaterThan(0)
    expect(JSON.stringify(report)).not.toContain('trajectory_json')
    expect(JSON.stringify(report)).not.toContain('raw_payload')
  })

  test('a legitimate serving provider id with an sk-shaped substring does not trip the secret-material backstop', async () => {
    // Regression: `hydralisk-vllm-glm-5p2-reap-504b` contains `sk-vllm-glm-5p2-
    // reap-504b`, which the blunt OpenAI-key heuristic false-positived on,
    // throwing and taking the whole operator report down with a 500. The bounded
    // model/provider identifier fields must be excluded from the backstop scan.
    const reportFacts: KhalaTraceReviewFacts = {
      ...facts,
      modelMix: [
        {
          count: 5,
          model: 'openagents/glm-5.2-reap-504b',
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          totalTokens: 9000,
        },
      ],
    }

    // It builds (does not throw) and preserves the real provider/model ids.
    const report = buildKhalaTraceReviewReport({
      facts: reportFacts,
      generatedAt: '2026-06-26T21:00:00.000Z',
      window: {
        hours: 24,
        since: '2026-06-25T21:00:00.000Z',
        until: '2026-06-26T21:00:00.000Z',
      },
    })
    expect(report.modelMix[0]?.provider).toBe('hydralisk-vllm-glm-5p2-reap-504b')

    // And the route serves it 200 instead of khala_trace_review_unavailable 500.
    const response = await run(
      handleOperatorKhalaTraceReview(
        new Request('https://openagents.com/api/operator/khala/trace-review'),
        {
          nowIso: () => '2026-06-26T21:00:00.000Z',
          requireAdminApiToken: async () => true,
          store: { readFacts: async () => reportFacts },
        },
      ),
    )
    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      modelMix: ReadonlyArray<{ provider: string }>
    }
    expect(body.modelMix[0]?.provider).toBe('hydralisk-vllm-glm-5p2-reap-504b')
  })

  test('a genuine secret in a non-identifier free-text field still trips the backstop (500)', async () => {
    // The backstop must remain active for fields other than the bounded
    // model/provider identifiers: a real OpenAI-key-shaped value in a demand
    // source must still fail closed rather than leak into the operator report.
    const leakyFacts: KhalaTraceReviewFacts = {
      ...facts,
      tokenByDemandSource: [
        {
          count: 1,
          label: 'sk-abcdef0123456789ABCDEF',
          totalTokens: 10,
        },
      ],
    }
    const response = await run(
      handleOperatorKhalaTraceReview(
        new Request('https://openagents.com/api/operator/khala/trace-review'),
        {
          nowIso: () => '2026-06-26T21:00:00.000Z',
          requireAdminApiToken: async () => true,
          store: { readFacts: async () => leakyFacts },
        },
      ),
    )
    expect(response.status).toBe(500)
    expect((await response.json()) as unknown).toEqual({
      error: 'khala_trace_review_unavailable',
    })
  })

  test('operator route requires admin auth', async () => {
    const response = await run(
      handleOperatorKhalaTraceReview(
        new Request('https://openagents.com/api/operator/khala/trace-review'),
        {
          requireAdminApiToken: async () => false,
          store: {
            readFacts: async () => facts,
          },
        },
      ),
    )

    expect(response.status).toBe(401)
  })

  test('operator route returns a bounded report window', async () => {
    const seen: Array<{ hours: number; limit: number }> = []
    const response = await run(
      handleOperatorKhalaTraceReview(
        new Request(
          'https://openagents.com/api/operator/khala/trace-review?hours=48&limit=3',
        ),
        {
          nowIso: () => '2026-06-26T21:00:00.000Z',
          requireAdminApiToken: async () => true,
          store: {
            readFacts: async input => {
              seen.push({ hours: input.window.hours, limit: input.limit })
              return facts
            },
          },
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      backlogFeed: { producedItemCount: number }
      window: { hours: number; since: string; until: string }
    }
    expect(seen).toEqual([{ hours: 48, limit: 3 }])
    expect(body.window).toEqual({
      hours: 48,
      since: '2026-06-24T21:00:00.000Z',
      until: '2026-06-26T21:00:00.000Z',
    })
    expect(body.backlogFeed.producedItemCount).toBeGreaterThan(0)
  })

  test('operator route rejects non-GET methods', async () => {
    const response = await run(
      handleOperatorKhalaTraceReview(
        new Request('https://openagents.com/api/operator/khala/trace-review', {
          method: 'POST',
        }),
        {
          requireAdminApiToken: async () => true,
          store: {
            readFacts: async () => facts,
          },
        },
      ),
    )

    expect(response.status).toBe(405)
    expect(response.headers.get('allow')).toBe('GET')
  })
})
