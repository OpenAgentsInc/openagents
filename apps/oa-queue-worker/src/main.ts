/**
 * CFG-7 (#8522): oa-queue-worker — Cloud Run pump for the oa-infra Postgres
 * JobQueue (epic #8515, Cloudflare→GCP consolidation).
 *
 * Replaces the four retired Cloudflare Queues consumers: leases jobs from
 * `oa_infra_jobs` (FOR UPDATE SKIP LOCKED) per topic and POSTs each one to
 * the app's admin-bearer internal delivery route, where the original queue
 * consumer logic runs unchanged. Ack on 2xx; nack (delayed retry, then
 * dead-letter via max_attempts) otherwise.
 *
 * Env:
 *   OA_INFRA_DATABASE_URL      direct Postgres URL (oa-infra OaInfraSql)
 *   OA_QUEUE_DELIVERY_URL      app origin (default https://openagents.com)
 *   OA_QUEUE_DELIVERY_TOKEN    admin bearer for the delivery route
 *   OA_QUEUE_POLL_MS           idle poll interval (default 1000)
 *   PORT                       health server port (Cloud Run default 8080)
 */
import { Config, Duration, Effect, Layer } from 'effect'
import { OaInfraSql } from '@openagentsinc/oa-infra/sql'
import * as JobQueuePostgres from '@openagentsinc/oa-infra/job-queue-postgres'

import { drainAllTopicsOnce, makeHttpDeliver, type PumpLog } from './pump.ts'
import { TOPICS } from './topics.ts'

const log: PumpLog = (event, fields) => {
  console.log(JSON.stringify({ event, ...fields, at: new Date().toISOString() }))
}

const state = {
  cycles: 0,
  lastCycleAt: '',
  processed: 0,
  startedAt: new Date().toISOString(),
}

const program = Effect.gen(function* () {
  const deliveryUrl = yield* Config.string('OA_QUEUE_DELIVERY_URL').pipe(
    Config.withDefault('https://openagents.com'),
  )
  const deliveryToken = yield* Config.string('OA_QUEUE_DELIVERY_TOKEN')
  const pollMs = yield* Config.string('OA_QUEUE_POLL_MS').pipe(
    Config.withDefault('1000'),
    Config.map(value => {
      const parsed = Number.parseInt(value, 10)
      return Number.isFinite(parsed) && parsed >= 100 ? parsed : 1000
    }),
  )

  const deliver = makeHttpDeliver({ baseUrl: deliveryUrl, token: deliveryToken })

  log('oa_queue_worker_started', {
    deliveryUrl,
    pollMs,
    topics: TOPICS.map(topic => topic.topic).join(','),
  })

  while (true) {
    const processed = yield* drainAllTopicsOnce(TOPICS, deliver, log)
    state.cycles += 1
    state.lastCycleAt = new Date().toISOString()
    state.processed += processed
    if (processed === 0) {
      yield* Effect.sleep(Duration.millis(pollMs))
    }
  }
})

const port = Number.parseInt(process.env['PORT'] ?? '8080', 10)

Bun.serve({
  fetch: () =>
    Response.json({
      cycles: state.cycles,
      lastCycleAt: state.lastCycleAt,
      ok: true,
      processed: state.processed,
      service: 'oa-queue-worker',
      startedAt: state.startedAt,
    }),
  port,
})

log('oa_queue_worker_health_listening', { port })

void Effect.runPromise(
  Effect.scoped(
    program.pipe(
      Effect.provide(
        JobQueuePostgres.layerPostgres.pipe(
          Layer.provideMerge(OaInfraSql.layerConfig),
        ),
      ),
    ),
  ),
).catch((error: unknown) => {
  console.error('oa_queue_worker_fatal', error)
  process.exit(1)
})
