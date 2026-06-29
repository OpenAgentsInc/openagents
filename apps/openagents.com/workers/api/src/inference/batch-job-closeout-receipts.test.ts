import { expect, test } from 'vitest'

import {
  type BatchJobCloseoutReceipt,
  buildBatchJobTelemetryRecord,
  computeBatchWaitMs,
  projectBatchJobCloseoutReceipt,
} from './batch-job-closeout-receipts'

const telemetryFor = (batchWaitMs?: number) =>
  buildBatchJobTelemetryRecord({
    jobId: 'batch_123',
    servedModel: 'inference.batch_job',
    ...(batchWaitMs === undefined ? {} : { batchWaitMs }),
  })

test('projects public batch job closeout receipt safely', () => {
  const receipt: BatchJobCloseoutReceipt = {
    schemaVersion: 'openagents.inference.batch_job.closeout.v1',
    receiptRef: 'receipt.inference.batch_job.closeout.batch_123',
    jobId: 'batch_123',
    chargeReceiptRef: 'receipt.inference.batch_job_charge.batch_123',
    totalItems: 100,
    successfulItems: 99,
    failedItems: 1,
    totalCostMsat: 50000,
    completedAtIso: '2026-06-20T12:00:00.000Z',
    resultsR2Key: 'batch_jobs/batch_123/results.jsonl',
    openagents: telemetryFor(4200),
  }

  const projection = projectBatchJobCloseoutReceipt(
    receipt,
    '2026-06-20T12:05:00.000Z',
  )

  expect(projection.receipt).toEqual(receipt)
  expect(projection.authorityBoundary).toContain('Public proof only')
  expect(projection.staleness.composition).toBe('live_at_read')
})

// Book P0-3: the terminal batch telemetry record distinguishes the detached job
// (`requestClass: batch`) and discloses the wait honestly.
test('terminal batch telemetry record carries requestClass batch + measured queue/batch wait', () => {
  const record = telemetryFor(4200)
  expect(record.requestClass).toBe('batch')
  // A batch job never blocks the edge request path → measured zero edge wait.
  expect(record.queueWaitMs).toBe(0)
  // The real time the job sat in the async queue.
  expect(record.batchWaitMs).toBe(4200)
  expect(record.verificationClass).toBe('none')
  expect(record.executedVerdict).toBe('not_executed')
  expect(record.settlementState).toBe('not_applicable')
  // Token counts are not aggregated at the closeout summary (metered per item).
  expect(record.promptTokens).toBe('not_measured')
})

test('an unmeasured batch wait records a blocker, not a fabricated number', () => {
  const record = telemetryFor(undefined)
  expect(record.requestClass).toBe('batch')
  // Honest sentinel — never a fake 0 for the batch wait.
  expect(record.batchWaitMs).toBe('not_measured')
  // The edge queue wait is still a measured zero (a batch job never blocks it).
  expect(record.queueWaitMs).toBe(0)
  expect(record.blockerRefs).toContain('batch_wait_not_measured')
})

test('computeBatchWaitMs returns the enqueue->start delta, or undefined when unmeasurable', () => {
  expect(
    computeBatchWaitMs(
      '2026-06-22T00:00:00.000Z',
      '2026-06-22T00:00:03.500Z',
    ),
  ).toBe(3500)
  // Missing either timestamp → not measurable (honest undefined → not_measured).
  expect(computeBatchWaitMs(null, '2026-06-22T00:00:03.500Z')).toBeUndefined()
  expect(computeBatchWaitMs('2026-06-22T00:00:00.000Z', null)).toBeUndefined()
  // A negative delta (clock skew / malformed) is rejected, never a fake number.
  expect(
    computeBatchWaitMs(
      '2026-06-22T00:00:05.000Z',
      '2026-06-22T00:00:00.000Z',
    ),
  ).toBeUndefined()
  // Unparseable timestamps → undefined.
  expect(computeBatchWaitMs('not-a-date', 'also-bad')).toBeUndefined()
})
