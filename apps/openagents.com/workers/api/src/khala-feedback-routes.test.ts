import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type KhalaFeedbackRecord,
  type KhalaFeedbackStore,
  handleKhalaFeedbackSubmit,
  handleOperatorKhalaFeedback,
} from './khala-feedback-routes'

const run = (effect: Effect.Effect<Response>): Promise<Response> =>
  Effect.runPromise(effect)

const makeStore = (): KhalaFeedbackStore & {
  readonly records: Array<KhalaFeedbackRecord>
} => {
  const records: Array<KhalaFeedbackRecord> = []

  return {
    records,
    create: async input => {
      const record = {
        clientVersion: input.clientVersion,
        createdAt: input.createdAt,
        feedback: input.feedback,
        feedbackRef: input.feedbackRef,
        source: input.source,
        traceRef: input.traceRef,
        userAgent: input.userAgent,
      }
      records.unshift(record)
      return record
    },
    listRecent: async input =>
      records
        .filter(record =>
          input.traceRef === undefined ? true : record.traceRef === input.traceRef,
        )
        .slice(0, input.limit),
  }
}

const submitRequest = (body: unknown, init: RequestInit = {}): Request =>
  new Request('https://openagents.com/api/khala/feedback', {
    body: JSON.stringify(body),
    headers: {
      'content-type': 'application/json',
      'user-agent': 'khala-test',
      ...(init.headers ?? {}),
    },
    method: 'POST',
    ...init,
  })

describe('khala feedback routes', () => {
  test('stores feedback with optional trace reference', async () => {
    const store = makeStore()
    const response = await run(
      handleKhalaFeedbackSubmit(
        submitRequest({
          clientVersion: '0.1.2',
          feedback: 'the transcript disappeared',
          source: 'khala-cli-interactive',
          traceRef: 'trace_123',
        }),
        {
          makeFeedbackRef: () => 'khala_feedback:test',
          nowIso: () => '2026-06-26T16:20:00.000Z',
          store,
        },
      ),
    )

    expect(response.status).toBe(201)
    expect(await response.json()).toEqual({
      schemaVersion: 'openagents.khala.feedback.submit.v1',
      createdAt: '2026-06-26T16:20:00.000Z',
      feedbackRef: 'khala_feedback:test',
      traceRef: 'trace_123',
    })
    expect(store.records).toEqual([
      {
        clientVersion: '0.1.2',
        createdAt: '2026-06-26T16:20:00.000Z',
        feedback: 'the transcript disappeared',
        feedbackRef: 'khala_feedback:test',
        source: 'khala-cli-interactive',
        traceRef: 'trace_123',
        userAgent: 'khala-test',
      },
    ])
  })

  test('accepts text as an alias for feedback', async () => {
    const store = makeStore()
    const response = await run(
      handleKhalaFeedbackSubmit(submitRequest({ text: 'from headless cli' }), {
        makeFeedbackRef: () => 'khala_feedback:headless',
        nowIso: () => '2026-06-26T16:21:00.000Z',
        store,
      }),
    )

    expect(response.status).toBe(201)
    expect(store.records[0]?.feedback).toBe('from headless cli')
  })

  test('rejects empty feedback without storing', async () => {
    const store = makeStore()
    const response = await run(
      handleKhalaFeedbackSubmit(submitRequest({ feedback: '   ' }), { store }),
    )

    expect(response.status).toBe(400)
    expect(store.records).toEqual([])
  })

  test('rejects non-POST submit requests', async () => {
    const store = makeStore()
    const response = await run(
      handleKhalaFeedbackSubmit(
        new Request('https://openagents.com/api/khala/feedback', {
          method: 'GET',
        }),
        { store },
      ),
    )

    expect(response.status).toBe(405)
  })

  test('lists recent feedback for admins and filters by traceRef', async () => {
    const store = makeStore()
    await store.create({
      clientVersion: '0.1.2',
      createdAt: '2026-06-26T16:21:00.000Z',
      feedback: 'first',
      feedbackRef: 'fb_1',
      source: 'khala-cli',
      traceRef: null,
      userAgent: 'khala-test',
    })
    await store.create({
      clientVersion: '0.1.2',
      createdAt: '2026-06-26T16:22:00.000Z',
      feedback: 'second',
      feedbackRef: 'fb_2',
      source: 'khala-cli-interactive',
      traceRef: 'trace_123',
      userAgent: 'khala-test',
    })

    const response = await run(
      handleOperatorKhalaFeedback(
        new Request(
          'https://openagents.com/api/operator/khala/feedback?traceRef=trace_123',
        ),
        {
          requireAdminApiToken: async () => true,
          store,
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(await response.json()).toEqual({
      schemaVersion: 'openagents.khala.feedback.list.v1',
      feedback: [
        {
          clientVersion: '0.1.2',
          createdAt: '2026-06-26T16:22:00.000Z',
          feedback: 'second',
          feedbackRef: 'fb_2',
          source: 'khala-cli-interactive',
          traceRef: 'trace_123',
          userAgent: 'khala-test',
        },
      ],
    })
  })

  test('blocks operator feedback reads without admin token', async () => {
    const store = makeStore()
    const response = await run(
      handleOperatorKhalaFeedback(
        new Request('https://openagents.com/api/operator/khala/feedback'),
        {
          requireAdminApiToken: async () => false,
          store,
        },
      ),
    )

    expect(response.status).toBe(401)
  })
})

