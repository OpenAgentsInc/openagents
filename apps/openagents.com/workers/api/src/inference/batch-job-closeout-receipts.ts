import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from '../public-projection-staleness'
import {
  type KhalaTelemetryRecord,
  KhalaTelemetryRecord as KhalaTelemetryRecordSchema,
  buildKhalaTelemetryRecord,
} from './khala-telemetry'

// ---------------------------------------------------------------------------
// Batch-job closeout receipt (Khala async lane, #6028 / EPIC #6017).
//
// Book P0-3 (#6086): this is the TERMINAL, dereferenceable receipt for a DETACHED
// async inference job — the async-lane analogue of the terminal `openagents` block
// the interactive stream attaches at stream close. A completed batch job is
// auditable here: its item tally, its charge, AND a Khala telemetry block whose
// `requestClass` is `batch` (distinguishing detached work from an interactive
// stream) and whose `batchWaitMs` / `queueWaitMs` disclose how long the job sat
// before the consumer ran it. Honest `not_measured` where a wait could not be
// computed (e.g. a job submitted before batch-wait timing existed).
//
// PUBLIC-SAFE (INVARIANTS): no prompt, completion, account ref, amount,
// destination, or payment material — only item COUNTS, public refs, the msat
// total already disclosed by the charge receipt, durations, and neutral
// classifiers. The telemetry block reuses the canonical, public-safe
// `openagents.khala.telemetry.v1` schema (no parallel vocabulary).
// ---------------------------------------------------------------------------

export const BatchJobCloseoutReceiptSchema = S.Struct({
  schemaVersion: S.Literal('openagents.inference.batch_job.closeout.v1'),
  receiptRef: S.String,
  jobId: S.String,
  chargeReceiptRef: S.String,
  totalItems: S.Number,
  successfulItems: S.Number,
  failedItems: S.Number,
  totalCostMsat: S.Number,
  completedAtIso: S.String,
  resultsR2Key: S.String,
  // Book P0-3: the TERMINAL `openagents` telemetry record for the detached job —
  // the async-lane counterpart of the interactive stream's stream-close receipt.
  // This receipt IS the dereferenceable artifact, so it carries the FULL canonical
  // record (not just the at-a-glance block): `requestClass: batch`, the measured
  // edge `queueWaitMs` (0), and the real `batchWaitMs` (or `not_measured`). Reuses
  // the canonical `openagents.khala.telemetry.v1` schema (no parallel vocabulary).
  openagents: KhalaTelemetryRecordSchema,
})

export type BatchJobCloseoutReceipt = S.Schema.Type<
  typeof BatchJobCloseoutReceiptSchema
>

// ---------------------------------------------------------------------------
// Terminal telemetry block for a completed batch job.
// ---------------------------------------------------------------------------

// Compute the batch WAIT (ms) between when the job was enqueued and when the
// consumer started processing it. `undefined` (→ honest `not_measured` in the
// telemetry builder) unless BOTH timestamps exist and parse to finite, ordered
// instants. Never fabricated.
export const computeBatchWaitMs = (
  enqueuedAtIso: string | null,
  startedAtIso: string | null,
): number | undefined => {
  if (enqueuedAtIso === null || startedAtIso === null) {
    return undefined
  }
  const enqueued = Date.parse(enqueuedAtIso)
  const started = Date.parse(startedAtIso)
  if (!Number.isFinite(enqueued) || !Number.isFinite(started)) {
    return undefined
  }
  const delta = started - enqueued
  return delta >= 0 ? delta : undefined
}

export type BatchJobTelemetryInput = Readonly<{
  jobId: string
  servedModel: string
  // The batch wait (ms) from enqueue to consumer start; undefined → not_measured.
  batchWaitMs?: number | undefined
}>

// Build the terminal `openagents` telemetry record for a completed batch job. The
// request class is ALWAYS `batch` (the detached async lane), distinguishing it
// from an interactive stream's `interactive_stream`. A batch job never blocks the
// edge request path, so `queueWaitMs` is a measured `0`; `batchWaitMs` is the real
// in-queue wait (or the honest `not_measured` sentinel + a `blockerRef`). Token
// counts are `not_measured` at the closeout summary (the per-item provider usage
// is metered per item, not re-aggregated here) and verification is `none` — a
// batch job is a detached completion, not a verified accepted outcome.
export const buildBatchJobTelemetryRecord = (
  input: BatchJobTelemetryInput,
): KhalaTelemetryRecord =>
  buildKhalaTelemetryRecord({
    executedVerdict: 'not_executed',
    provider: 'inference.batch_job',
    // A batch job never blocks the edge request path, so its EDGE queue wait is a
    // measured zero. The meaningful wait is the time it sat in the async queue,
    // recorded as `batchWaitMs`.
    queueWaitMs: 0,
    ...(input.batchWaitMs === undefined
      ? {}
      : { batchWaitMs: input.batchWaitMs }),
    requestClass: 'batch',
    requestId: input.jobId,
    requestedModel: input.servedModel,
    route: 'batch',
    servedModel: input.servedModel,
    settlementState: 'not_applicable',
    verificationClass: 'none',
    blockerRefs:
      input.batchWaitMs === undefined
        ? ['batch_wait_not_measured', 'batch_token_usage_per_item']
        : ['batch_token_usage_per_item'],
  })

export const projectBatchJobCloseoutReceipt = (
  receipt: BatchJobCloseoutReceipt,
  generatedAt: string,
): Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  receipt: BatchJobCloseoutReceipt
  staleness: PublicProjectionStalenessContract
}> => ({
  authorityBoundary:
    'Public proof only. This receipt read grants no spend, refund, payout, checkout, settlement, provider, or registry authority.',
  caveatRefs: [
    'caveat.public.no_private_payment_material',
    'caveat.public.no_account_or_amount_projection',
  ],
  generatedAt,
  receipt,
  staleness: liveAtReadStaleness([]),
})
