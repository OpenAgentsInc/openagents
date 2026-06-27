import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BackendExceptionSignalInput,
  type KhalaTraceReviewFacts,
  backendExceptionSignalFromWorkerError,
  buildKhalaTraceReviewReport,
  handleOperatorKhalaTraceReview,
  insertBackendExceptionSignal,
} from './khala-trace-review-routes'

const run = (effect: Effect.Effect<Response>): Promise<Response> =>
  Effect.runPromise(effect)

const facts: KhalaTraceReviewFacts = {
  backendSignalBuckets: [],
  backendSignalSummary: {
    agentCrashCount: 0,
    gatewayTimeoutCount: 0,
    signalCount: 0,
    unhandledExceptionCount: 0,
  },
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

  test('backend exception, timeout, and crash signals feed trace-review triage without raw payloads', () => {
    const report = buildKhalaTraceReviewReport({
      facts: {
        ...facts,
        backendSignalBuckets: [
          {
            count: 2,
            evidenceRefs: [
              'backend_exception_signal.public.sig_unhandled_checkout',
            ],
            latestObservedAt: '2026-06-26T20:30:00.000Z',
            signalKind: 'unhandled_exception',
            surface: '/api/v1/chat/completions',
          },
          {
            count: 1,
            evidenceRefs: ['backend_exception_signal.public.sig_gateway_504'],
            latestObservedAt: '2026-06-26T20:35:00.000Z',
            signalKind: 'gateway_timeout',
            surface: 'inference.gateway',
          },
          {
            count: 1,
            evidenceRefs: ['backend_exception_signal.public.sig_agent_crash'],
            latestObservedAt: '2026-06-26T20:40:00.000Z',
            signalKind: 'agent_crash',
            surface: 'artanis.scheduled_runner',
          },
        ],
        backendSignalSummary: {
          agentCrashCount: 1,
          gatewayTimeoutCount: 1,
          signalCount: 4,
          unhandledExceptionCount: 2,
        },
      },
      generatedAt: '2026-06-26T21:00:00.000Z',
      window: {
        hours: 24,
        since: '2026-06-25T21:00:00.000Z',
        until: '2026-06-26T21:00:00.000Z',
      },
    })

    expect(report.sourceTables).toContain('backend_exception_signals')
    expect(report.aggregates.backendSignals).toEqual({
      agentCrashCount: 1,
      gatewayTimeoutCount: 1,
      signalCount: 4,
      unhandledExceptionCount: 2,
    })
    expect(report.failureModes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          failureRef:
            'failure.khala_trace_review.backend_unhandled_exception.api_v1_chat_completions',
          label:
            'Unhandled backend exceptions on /api/v1/chat/completions',
          severity: 'critical',
        }),
        expect.objectContaining({
          failureRef:
            'failure.khala_trace_review.backend_gateway_timeout.inference_gateway',
          label: 'Gateway timeout signals on inference.gateway',
          severity: 'critical',
        }),
        expect.objectContaining({
          failureRef:
            'failure.khala_trace_review.backend_agent_crash.artanis_scheduled_runner',
          label: 'Silent agent crash signals on artanis.scheduled_runner',
          severity: 'critical',
        }),
      ]),
    )
    expect(report.triageItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          priority: 'high',
          triageRef:
            'triage.khala_trace_review.backend_gateway_timeout.inference_gateway',
        }),
      ]),
    )
    expect(JSON.stringify(report)).not.toContain('Error:')
    expect(JSON.stringify(report)).not.toContain('stack')
  })

  test('backend exception signal insert rejects secret-shaped sanitized messages', async () => {
    const input: BackendExceptionSignalInput = {
      fingerprint: 'fp_secret',
      observedAt: '2026-06-26T20:30:00.000Z',
      outcome: 'exception',
      sanitizedMessage: 'provider returned sk-abcdef0123456789ABCDEF',
      signalKind: 'unhandled_exception',
      signalRef: 'backend_exception_signal.public.secret',
      source: 'worker_tail',
      surface: '/api/private',
    }
    const db = {
      prepare: () => {
        throw new Error('prepare should not run when the backstop rejects')
      },
    } as unknown as D1Database

    await expect(insertBackendExceptionSignal(db, input)).rejects.toMatchObject({
      reason: 'backend exception signal contained private-data-shaped material',
    })
  })

  test('worker error capture stores bounded signal metadata without raw error text', () => {
    const signal = backendExceptionSignalFromWorkerError({
      error: new TypeError(
        'raw stack detail with private prompt and sk-abcdef0123456789ABCDEF',
      ),
      observedAt: '2026-06-26T20:30:00.000Z',
      request: new Request('https://openagents.com/api/v1/chat/completions', {
        headers: { 'cf-ray': 'abc123-DFW' },
      }),
      signalRef: 'backend_exception_signal.public.worker_catch.test',
    })

    expect(signal).toMatchObject({
      errorClass: 'TypeError',
      fingerprint: 'worker_catch:/api/v1/chat/completions:TypeError',
      outcome: 'exception',
      requestRef: 'abc123-DFW',
      signalKind: 'unhandled_exception',
      source: 'worker_catch',
      statusCode: 500,
      surface: '/api/v1/chat/completions',
    })
    expect(signal.sanitizedMessage).not.toContain('private prompt')
    expect(signal.sanitizedMessage).not.toContain('sk-')
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
