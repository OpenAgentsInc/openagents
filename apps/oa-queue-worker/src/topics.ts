/**
 * CFG-7 (#8522): topic configuration for the oa-queue-worker pump.
 *
 * Topics mirror the retired Cloudflare queue names 1:1; batch sizes mirror
 * the retired wrangler `consumers` config (batch 1 / 1 / 25, max_retries 3 —
 * dead-lettering itself is enforced by the producer's `max_attempts = 4`,
 * see workers/api src/oa-job-queue-producer.ts).
 *
 * The retired `openagents-autopilot-runner-events` queue is intentionally
 * absent: it had no producer call sites and no consumer (dead lane, removed).
 */
export type TopicDeliveryMode = 'http' | 'ack-local'

export type TopicConfig = Readonly<{
  topic: string
  /** Max jobs leased per drain cycle (mirrors wrangler max_batch_size). */
  batch: number
  /** Lease invisibility window while a delivery is in flight. */
  visibilityMs: number
  /** Delay before a nacked job becomes leasable again. */
  retryDelayMs: number
  /**
   * 'http'      — POST to the app's /api/internal/queue/deliver route.
   * 'ack-local' — ack without delivery (the operator smoke topic, used to
   *               prove the live lease/ack loop with zero app dependency).
   */
  delivery: TopicDeliveryMode
}>

export const SMOKE_TOPIC = 'oa-queue-worker-smoke'

export const TOPICS: ReadonlyArray<TopicConfig> = [
  {
    topic: 'openagents-adjutant-enrichment-jobs',
    batch: 1,
    // Enrichment jobs run research + model calls in the delivery request —
    // keep the lease comfortably longer than the app route can run.
    visibilityMs: 300_000,
    retryDelayMs: 60_000,
    delivery: 'http',
  },
  {
    topic: 'openagents-event-ledger-ingest',
    batch: 1,
    visibilityMs: 60_000,
    retryDelayMs: 30_000,
    delivery: 'http',
  },
  {
    topic: 'openagents-pylon-codex-raw-event-metadata',
    batch: 25,
    visibilityMs: 60_000,
    retryDelayMs: 30_000,
    delivery: 'http',
  },
  {
    topic: SMOKE_TOPIC,
    batch: 10,
    visibilityMs: 30_000,
    retryDelayMs: 5_000,
    delivery: 'ack-local',
  },
]
