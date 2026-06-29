// Async batch-job consumer (Khala, issue #6028 / EPIC #6017).
//
// The submit route (`batch-job-routes.ts handleBatchJobsSubmit`) accepts a
// dataset, charges an estimate up front, persists a `pending`
// `inference_batch_jobs` row, and returns `202`-style `{ jobId, receiptRef }`.
// THIS module is the piece that runs that work OFF the request path so a
// detached / minutes-long batch never touches the Cloudflare edge timeout (the
// `524` postmortem in
// docs/inference/2026-06-22-long-running-inference-response-strategies.md,
// Strategy 2).
//
// A Queue (or DO/Workflow) delivers a `BatchJobQueueMessage` carrying the job id
// and the executable items. `executeBatchJob`:
//   (a) loads the `pending` job from the store,
//   (b) executes each item against the SAME inference gateway the interactive
//       route uses — the provider-adapter registry via `dispatchWithOverflow`,
//   (c) decrements credits per item through the EXISTING `MeteringHook` seam
//       (never re-implements pricing/ledger; consumes the exported contract),
//   (d) marks the job `completed` (or `failed`) so the dereferenceable closeout
//       receipt the public route reads (`handleBatchJobReceiptRead`, which only
//       projects a receipt for a `completed` job) becomes resolvable.
//
// INERT BY DEFAULT. Nothing here changes prod behaviour until a queue
// producer/consumer is wired AND `INFERENCE_BATCH_JOBS_ENABLED` is on. The
// module is pure orchestration over injected seams (store, registry, metering
// hook, clock) so it is fully exercisable against a fake gateway + fake queue
// message with no live run and no new infrastructure.

import { Effect, Schema as S } from 'effect'

import { workerLogEntry } from '../observability'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type DispatchDeps,
  dispatchWithOverflow,
} from './model-router'
import type { MeteringContext, MeteringHook } from './metering-hook'
import { stubMeteringHook } from './metering-hook'
import type {
  InferenceRequest,
  InferenceResult,
} from './provider-adapter'
import type { BatchJobStore } from './batch-job-store'
import type {
  BatchJobResultRow,
  BatchJobResultsStore,
} from './batch-job-results-store'

// One executable unit of a batch job. The submit route prices on token counts;
// the consumer needs the actual prompt to RUN the item, so the queue message
// carries the messages + sampling params (the dataset rows the customer
// submitted). Mirrors the normalized `InferenceRequest` shape so the consumer
// hands the registry exactly what the interactive route does.
export const BatchJobExecutableItemSchema = S.Struct({
  model: S.String,
  messages: S.Array(
    S.Struct({
      role: S.String,
      content: S.String,
    }),
  ),
  // Standard sampling params forwarded verbatim to the adapter (temperature,
  // top_p, max_tokens, ...). Optional; the consumer defaults absent params to
  // an empty record when building the inference request.
  passthroughParams: S.optionalKey(S.Record(S.String, S.Unknown)),
})
export type BatchJobExecutableItem = S.Schema.Type<
  typeof BatchJobExecutableItemSchema
>

// The queue payload that triggers execution of one submitted batch job. Carries
// the job id (the row the consumer marks done) and the items to run. Kept
// self-contained so the consumer never has to reach back into the request that
// enqueued it.
export class BatchJobQueueMessage extends S.Class<BatchJobQueueMessage>(
  'BatchJobQueueMessage',
)({
  schemaVersion: S.Literal('openagents.inference.batch_job.v1'),
  jobId: S.String,
  items: S.Array(BatchJobExecutableItemSchema),
  // Book P0-3 (#6086): when the producer enqueued this message (ISO 8601), so the
  // consumer can compute the batch WAIT (`startedAt - enqueuedAt`) and make a
  // detached job's time-in-queue auditable in its closeout receipt. Optional so a
  // message published before this field existed still decodes; when absent the
  // receipt honestly reports `not_measured` for `batchWaitMs` rather than a
  // fabricated number.
  enqueuedAtIso: S.optionalKey(S.String),
}) {}

// Result of running a batch job, surfaced to the queue handler for ack/retry
// decisions and to tests. `completed` is true when the job reached a terminal
// `completed` status; `failedItems` counts items whose dispatch errored.
export type BatchJobExecutionOutcome = Readonly<{
  jobId: string
  completed: boolean
  processedItems: number
  failedItems: number
  receiptRef: string
}>

// The dereferenceable closeout receipt ref the public route resolves. Built from
// the same prefix `handleBatchJobReceiptRead` matches on, so a client that holds
// the job id can dereference the receipt once the job is `completed`.
export const batchJobCloseoutReceiptRef = (jobId: string): string =>
  `receipt.inference.batch_job.closeout.${jobId}`

export type BatchJobConsumerDeps = Readonly<{
  // The batch-job store (D1-backed in prod, fake in tests). The consumer loads
  // the pending job and writes status/progress through it.
  store: BatchJobStore
  // The inference gateway dispatch seam — the SAME provider-adapter registry +
  // overflow path the interactive route uses. The consumer never invents a
  // transport; it dispatches through the registry like `/v1/chat/completions`.
  dispatch: DispatchDeps
  // The EXISTING metering hook. The Worker passes the live ledger hook
  // (`makeLedgerMeteringHook`) so each executed item decrements credits exactly
  // as an interactive completion would; tests pass a fake/stub. Defaults to the
  // no-op stub so a flag-off / misconfigured path never charges.
  meteringHook?: MeteringHook | undefined
  // Funding kind for the metering context (card | bitcoin). Defaults to card.
  fundingKind?: MeteringContext['fundingKind'] | undefined
  // Book P0-3 (#6086): the clock the consumer stamps the start-of-processing time
  // with (the END of the batch wait). Defaults to the canonical
  // `currentIsoTimestamp` primitive in prod; tests inject a deterministic clock so
  // `batchWaitMs` is exactly assertable. Injectable so the consumer stays a pure
  // function of its seams.
  nowIso?: (() => string) | undefined
  // Optional results sink. When wired, the consumer persists one JSONL row per
  // item and stores the returned key on the job so the authenticated results
  // route can retrieve completed outputs. Undefined keeps older/inert paths
  // receipt-only.
  resultsStore?: BatchJobResultsStore | undefined
}>

const toInferenceRequest = (
  item: BatchJobExecutableItem,
): InferenceRequest => ({
  model: item.model,
  messages: item.messages,
  // Batch items are never streamed — they run to completion off the request
  // path and the result is persisted, not relayed token-by-token.
  stream: false,
  passthroughParams: item.passthroughParams ?? {},
})

// Execute one submitted batch job end-to-end. Pure orchestration over injected
// seams: load -> per-item dispatch -> per-item meter -> terminal status. Errors
// from individual items are counted (the job still completes with a failure
// tally) so one bad row never aborts the whole batch; an inability to LOAD the
// job is a hard failure the queue handler can retry.
export const executeBatchJob = (
  deps: BatchJobConsumerDeps,
  message: BatchJobQueueMessage,
): Effect.Effect<BatchJobExecutionOutcome> =>
  Effect.gen(function* () {
    const meteringHook = deps.meteringHook ?? stubMeteringHook
    const fundingKind = deps.fundingKind ?? 'card'
    const nowIso = deps.nowIso ?? currentIsoTimestamp
    const receiptRef = batchJobCloseoutReceiptRef(message.jobId)

    const job = yield* deps.store.getBatchJob(message.jobId)

    // No row, or already terminal: nothing to do. Idempotent — a redelivered
    // message for a job already completed/failed is a no-op (never re-charges,
    // since the per-item metering idempotency key would also dedupe).
    if (job === null) {
      yield* Effect.logInfo(
        workerLogEntry('inference.batch_job.consume.missing', {
          jobId: message.jobId,
        }),
      )
      return {
        completed: false,
        failedItems: 0,
        jobId: message.jobId,
        processedItems: 0,
        receiptRef,
      }
    }

    if (job.status === 'completed' || job.status === 'failed') {
      yield* Effect.logInfo(
        workerLogEntry('inference.batch_job.consume.already_terminal', {
          jobId: message.jobId,
          status: job.status,
        }),
      )
      return {
        completed: job.status === 'completed',
        failedItems: job.failedItems,
        jobId: message.jobId,
        processedItems: job.processedItems,
        receiptRef,
      }
    }

    // Stamp the start-of-processing time as the END of the batch wait (book
    // P0-3). `batchWaitMs` (in the closeout receipt) = startedAt - enqueuedAt.
    yield* deps.store.updateBatchJobStatus(message.jobId, 'processing', {
      startedAt: nowIso(),
    })

    let processedItems = 0
    let failedItems = 0
    const resultRows: Array<BatchJobResultRow> = []

    for (let index = 0; index < message.items.length; index += 1) {
      const item = message.items[index]
      if (item === undefined) {
        continue
      }

      const request = toInferenceRequest(item)

      const dispatched = yield* dispatchWithOverflow<{
        adapterId: string
        value: InferenceResult
      }>(
        request,
        (adapter, req) =>
          adapter
            .complete(req)
            .pipe(Effect.map(value => ({ adapterId: adapter.id, value }))),
        deps.dispatch,
      ).pipe(
        Effect.map(served => ({ ok: true as const, served })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )

      processedItems += 1

      if (!dispatched.ok) {
        failedItems += 1
        resultRows.push({
          error: dispatched.reason,
          index,
          ok: false,
          requestedModel: item.model,
        })
        yield* Effect.logInfo(
          workerLogEntry('inference.batch_job.item.failed', {
            index,
            jobId: message.jobId,
            reason: dispatched.reason,
          }),
        )
        continue
      }

      resultRows.push({
        content: dispatched.served.value.content,
        finishReason: dispatched.served.value.finishReason,
        index,
        ok: true,
        requestedModel: item.model,
        servedModel: dispatched.served.value.servedModel,
        usage: dispatched.served.value.usage,
      })

      // Decrement credits through the EXISTING metering hook. The per-item
      // request id is `<jobId>:<index>` so the hook's idempotency key dedupes a
      // redelivered/retried item and never double-charges.
      yield* meteringHook({
        accountRef: job.accountRef,
        adapterId: dispatched.served.adapterId,
        batch: true,
        fundingKind,
        requestId: `${message.jobId}:${index}`,
        requestedModel: item.model,
        servedModel: dispatched.served.value.servedModel,
        streamed: false,
        usage: dispatched.served.value.usage,
      })
    }

    const allFailed =
      message.items.length > 0 && failedItems === message.items.length
    const terminalStatus = allFailed ? 'failed' : 'completed'
    const resultsR2Key =
      resultRows.length > 0 && deps.resultsStore !== undefined
        ? yield* deps.resultsStore.writeResults(message.jobId, resultRows)
        : undefined

    yield* deps.store.updateBatchJobStatus(message.jobId, terminalStatus, {
      failedItems,
      processedItems,
      ...(resultsR2Key === undefined ? {} : { resultsR2Key }),
    })

    yield* Effect.logInfo(
      workerLogEntry('inference.batch_job.consume.done', {
        failedItems,
        jobId: message.jobId,
        processedItems,
        status: terminalStatus,
      }),
    )

    return {
      completed: terminalStatus === 'completed',
      failedItems,
      jobId: message.jobId,
      processedItems,
      receiptRef,
    }
  })
