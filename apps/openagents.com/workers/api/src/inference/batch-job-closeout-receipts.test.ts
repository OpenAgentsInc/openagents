import { expect, test } from 'vitest'

import {
  type BatchJobCloseoutReceipt,
  projectBatchJobCloseoutReceipt,
} from './batch-job-closeout-receipts'

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
  }

  const projection = projectBatchJobCloseoutReceipt(
    receipt,
    '2026-06-20T12:05:00.000Z',
  )

  expect(projection.receipt).toEqual(receipt)
  expect(projection.authorityBoundary).toContain('Public proof only')
  expect(projection.staleness.composition).toBe('live_at_read')
})
