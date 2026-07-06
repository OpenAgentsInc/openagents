/**
 * CFG-7 (#8522): pump behavior against the in-memory oa-infra JobQueue —
 * the same interface the Postgres backend passes conformance for, so what
 * is proven here holds for the deployed FOR UPDATE SKIP LOCKED backend.
 */
import { describe, expect, test } from 'bun:test'
import { Effect } from 'effect'
import { JobQueue } from '@openagentsinc/oa-infra/job-queue'
import * as JobQueueMemory from '@openagentsinc/oa-infra/job-queue-memory'

import {
  OA_QUEUE_DELIVER_PATH,
  drainAllTopicsOnce,
  drainTopicOnce,
  makeHttpDeliver,
  type DeliverFn,
  type FetchLike,
} from './pump.ts'
import { SMOKE_TOPIC, TOPICS, type TopicConfig } from './topics.ts'

const eventLedgerTopic: TopicConfig = {
  batch: 1,
  delivery: 'http',
  retryDelayMs: 0,
  topic: 'openagents-event-ledger-ingest',
  visibilityMs: 60_000,
}

const codexMetadataTopic: TopicConfig = {
  batch: 25,
  delivery: 'http',
  retryDelayMs: 0,
  topic: 'openagents-pylon-codex-raw-event-metadata',
  visibilityMs: 60_000,
}

const smokeTopic: TopicConfig = TOPICS.find(
  topic => topic.topic === SMOKE_TOPIC,
)!

const withQueue = <A>(
  effect: Effect.Effect<A, unknown, JobQueue>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(Effect.provide(JobQueueMemory.layerMemory())) as Effect.Effect<
      A,
      never,
      never
    >,
  )

describe('oa-queue-worker pump (CFG-7 #8522)', () => {
  test('delivers leased jobs and acks on success', async () => {
    const delivered: Array<{ payload: string; topic: string }> = []
    const deliver: DeliverFn = async job => {
      delivered.push({ payload: job.payload, topic: job.topic })
      return { ok: true, status: 200 }
    }

    const outcome = await withQueue(
      Effect.gen(function* () {
        const queue = yield* JobQueue
        yield* queue.enqueue(eventLedgerTopic.topic, '{"a":1}')
        yield* queue.enqueue(eventLedgerTopic.topic, '{"a":2}')

        // batch 1 mirrors the retired wrangler consumer config.
        const first = yield* drainTopicOnce(eventLedgerTopic, deliver)
        const second = yield* drainTopicOnce(eventLedgerTopic, deliver)
        const empty = yield* drainTopicOnce(eventLedgerTopic, deliver)
        return { empty, first, second }
      }),
    )

    expect(outcome.first).toEqual({ acked: 1, leased: 1, nacked: 0 })
    expect(outcome.second).toEqual({ acked: 1, leased: 1, nacked: 0 })
    expect(outcome.empty).toEqual({ acked: 0, leased: 0, nacked: 0 })
    expect(delivered).toEqual([
      { payload: '{"a":1}', topic: eventLedgerTopic.topic },
      { payload: '{"a":2}', topic: eventLedgerTopic.topic },
    ])
  })

  test('leases up to the configured batch (25 for codex metadata)', async () => {
    const deliver: DeliverFn = async () => ({ ok: true, status: 200 })

    const outcome = await withQueue(
      Effect.gen(function* () {
        const queue = yield* JobQueue
        for (let index = 0; index < 30; index++) {
          yield* queue.enqueue(codexMetadataTopic.topic, `{"i":${index}}`)
        }
        return yield* drainTopicOnce(codexMetadataTopic, deliver)
      }),
    )

    expect(outcome).toEqual({ acked: 25, leased: 25, nacked: 0 })
  })

  test('nacks failed deliveries; exhausted jobs dead-letter with the failure reason', async () => {
    const deliver: DeliverFn = async () => ({
      ok: false,
      reason: 'http_500: dispatch_failed',
      status: 500,
    })

    const result = await withQueue(
      Effect.gen(function* () {
        const queue = yield* JobQueue
        yield* queue.enqueue(eventLedgerTopic.topic, '{"doomed":true}', {
          maxAttempts: 2,
        })

        const first = yield* drainTopicOnce(eventLedgerTopic, deliver)
        const second = yield* drainTopicOnce(eventLedgerTopic, deliver)
        const third = yield* drainTopicOnce(eventLedgerTopic, deliver)
        const dead = yield* queue.deadLetters(eventLedgerTopic.topic)
        return { dead, first, second, third }
      }),
    )

    expect(result.first).toEqual({ acked: 0, leased: 1, nacked: 1 })
    expect(result.second).toEqual({ acked: 0, leased: 1, nacked: 1 })
    // Exhausted after 2 attempts — nothing left to lease.
    expect(result.third).toEqual({ acked: 0, leased: 0, nacked: 0 })
    expect(result.dead).toHaveLength(1)
    expect(result.dead[0]).toMatchObject({
      lastError: 'http_500: dispatch_failed',
      payload: '{"doomed":true}',
    })
  })

  test('a deliver function that throws is treated as a nack, not a pump crash', async () => {
    const deliver: DeliverFn = async () => {
      throw new Error('network down')
    }

    const result = await withQueue(
      Effect.gen(function* () {
        const queue = yield* JobQueue
        yield* queue.enqueue(eventLedgerTopic.topic, '{"x":1}')
        const outcome = yield* drainTopicOnce(eventLedgerTopic, deliver)
        const dead = yield* queue.deadLetters(eventLedgerTopic.topic)
        return { dead, outcome }
      }),
    )

    expect(result.outcome).toEqual({ acked: 0, leased: 1, nacked: 1 })
    expect(result.dead).toHaveLength(0)
  })

  test('the smoke topic acks locally without delivery', async () => {
    const deliver: DeliverFn = async () => {
      throw new Error('smoke jobs must never be delivered over HTTP')
    }

    const outcome = await withQueue(
      Effect.gen(function* () {
        const queue = yield* JobQueue
        yield* queue.enqueue(SMOKE_TOPIC, '{"smoke":true}')
        return yield* drainTopicOnce(smokeTopic, deliver)
      }),
    )

    expect(outcome).toEqual({ acked: 1, leased: 1, nacked: 0 })
  })

  test('drainAllTopicsOnce covers every configured topic and reports processed count', async () => {
    const deliver: DeliverFn = async () => ({ ok: true, status: 200 })

    const processed = await withQueue(
      Effect.gen(function* () {
        const queue = yield* JobQueue
        yield* queue.enqueue('openagents-adjutant-enrichment-jobs', '{"j":1}')
        yield* queue.enqueue('openagents-event-ledger-ingest', '{"j":2}')
        yield* queue.enqueue(
          'openagents-pylon-codex-raw-event-metadata',
          '{"j":3}',
        )
        yield* queue.enqueue(SMOKE_TOPIC, '{"j":4}')
        return yield* drainAllTopicsOnce(TOPICS, deliver)
      }),
    )

    expect(processed).toBe(4)
  })

  test('makeHttpDeliver posts the delivery envelope with the bearer token', async () => {
    const calls: Array<{ body: unknown; headers: Record<string, string>; url: string }> = []
    const fetchImpl: FetchLike = async (url, init) => {
      calls.push({ body: JSON.parse(init.body), headers: init.headers, url })
      return { ok: true, status: 200, text: async () => '{"ok":true}' }
    }
    const deliver = makeHttpDeliver({
      baseUrl: 'https://openagents.com/',
      fetchImpl,
      token: 'admin-token',
    })

    const result = await deliver({
      attempts: 1,
      id: 'job_1',
      payload: '{"schemaVersion":"openagents.event_ledger_ingest.v1"}',
      topic: 'openagents-event-ledger-ingest',
    })

    expect(result).toEqual({ ok: true, status: 200 })
    expect(calls).toHaveLength(1)
    expect(calls[0]!.url).toBe(
      `https://openagents.com${OA_QUEUE_DELIVER_PATH}`,
    )
    expect(calls[0]!.headers['authorization']).toBe('Bearer admin-token')
    expect(calls[0]!.body).toEqual({
      attempts: 1,
      jobId: 'job_1',
      payload: '{"schemaVersion":"openagents.event_ledger_ingest.v1"}',
      topic: 'openagents-event-ledger-ingest',
    })
  })

  test('makeHttpDeliver maps non-2xx to a bounded failure reason', async () => {
    const fetchImpl: FetchLike = async () => ({
      ok: false,
      status: 500,
      text: async () => '{"error":"dispatch_failed"}',
    })
    const deliver = makeHttpDeliver({
      baseUrl: 'https://openagents.com',
      fetchImpl,
      token: 'admin-token',
    })

    const result = await deliver({
      attempts: 1,
      id: 'job_2',
      payload: '{}',
      topic: 'openagents-event-ledger-ingest',
    })

    expect(result.ok).toBe(false)
    expect(result.status).toBe(500)
    expect(result.reason).toContain('http_500')
  })
})
