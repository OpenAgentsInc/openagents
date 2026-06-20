import { Effect } from 'effect'

export type BatchJobStatus = 'pending' | 'processing' | 'completed' | 'failed'

export type BatchJobRecord = Readonly<{
  jobId: string
  accountRef: string
  status: BatchJobStatus
  chargeReceiptRef: string
  datasetSize: number
  processedItems: number
  failedItems: number
  resultsR2Key: string | null
  createdAt: string
  updatedAt: string
}>

export type BatchJobStore = Readonly<{
  insertBatchJob: (job: Omit<BatchJobRecord, 'updatedAt'>) => Effect.Effect<void>
  updateBatchJobStatus: (
    jobId: string,
    status: BatchJobStatus,
    updates: Partial<Pick<BatchJobRecord, 'processedItems' | 'failedItems' | 'resultsR2Key'>>
  ) => Effect.Effect<void>
  getBatchJob: (jobId: string) => Effect.Effect<BatchJobRecord | null>
}>

export const makeD1BatchJobStore = (db: D1Database, nowIso: () => string): BatchJobStore => ({
  insertBatchJob: job =>
    Effect.tryPromise(() =>
      db
        .prepare(
          `INSERT INTO inference_batch_jobs (
             job_id, account_ref, status, charge_receipt_ref, dataset_size, 
             processed_items, failed_items, results_r2_key, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
        )
        .bind(
          job.jobId,
          job.accountRef,
          job.status,
          job.chargeReceiptRef,
          job.datasetSize,
          job.processedItems,
          job.failedItems,
          job.resultsR2Key,
          job.createdAt,
          nowIso()
        )
        .run()
    ).pipe(Effect.asVoid, Effect.orDie),

  updateBatchJobStatus: (jobId, status, updates) =>
    Effect.tryPromise(() => {
      let query = 'UPDATE inference_batch_jobs SET status = ?, updated_at = ?'
      const bindings: any[] = [status, nowIso()]

      if (updates.processedItems !== undefined) {
        query += ', processed_items = ?'
        bindings.push(updates.processedItems)
      }
      if (updates.failedItems !== undefined) {
        query += ', failed_items = ?'
        bindings.push(updates.failedItems)
      }
      if (updates.resultsR2Key !== undefined) {
        query += ', results_r2_key = ?'
        bindings.push(updates.resultsR2Key)
      }

      query += ' WHERE job_id = ?'
      bindings.push(jobId)

      return db.prepare(query).bind(...bindings).run()
    }).pipe(Effect.asVoid, Effect.orDie),

  getBatchJob: jobId =>
    Effect.tryPromise(() =>
      db
        .prepare(
          `SELECT job_id, account_ref, status, charge_receipt_ref, dataset_size, 
             processed_items, failed_items, results_r2_key, created_at, updated_at
           FROM inference_batch_jobs WHERE job_id = ? LIMIT 1`
        )
        .bind(jobId)
        .first<any>()
    ).pipe(
      Effect.map(row => {
        if (!row) return null
        return {
          jobId: String(row.job_id),
          accountRef: String(row.account_ref),
          status: row.status as BatchJobStatus,
          chargeReceiptRef: String(row.charge_receipt_ref),
          datasetSize: Number(row.dataset_size),
          processedItems: Number(row.processed_items),
          failedItems: Number(row.failed_items),
          resultsR2Key: row.results_r2_key ? String(row.results_r2_key) : null,
          createdAt: String(row.created_at),
          updatedAt: String(row.updated_at),
        }
      }),
      Effect.orDie
    ),
})
