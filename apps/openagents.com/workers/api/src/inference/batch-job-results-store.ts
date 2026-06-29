import { Effect } from 'effect'

import type { InferenceResult } from './provider-adapter'

export type BatchJobStoredResult = Readonly<{
  index: number
  ok: true
  requestedModel: string
  servedModel: string
  content: string
  finishReason: string
  usage: InferenceResult['usage']
}>

export type BatchJobStoredError = Readonly<{
  index: number
  ok: false
  requestedModel: string
  error: string
}>

export type BatchJobResultRow = BatchJobStoredResult | BatchJobStoredError

export type BatchJobResultsStore = Readonly<{
  writeResults: (
    jobId: string,
    rows: ReadonlyArray<BatchJobResultRow>,
  ) => Effect.Effect<string>
  readResults: (key: string) => Effect.Effect<string | null>
}>

export const batchJobResultsR2Key = (jobId: string): string =>
  `inference/batch-jobs/${jobId}/results.jsonl`

const encodeJsonl = (rows: ReadonlyArray<BatchJobResultRow>): string =>
  rows.map(row => JSON.stringify(row)).join('\n')

export const makeR2BatchJobResultsStore = (
  bucket: R2Bucket,
): BatchJobResultsStore => ({
  readResults: key =>
    Effect.tryPromise(async () => {
      const object = await bucket.get(key)
      return object === null ? null : await object.text()
    }).pipe(Effect.orDie),
  writeResults: (jobId, rows) =>
    Effect.tryPromise(async () => {
      const key = batchJobResultsR2Key(jobId)
      await bucket.put(key, encodeJsonl(rows), {
        httpMetadata: { contentType: 'application/x-ndjson; charset=utf-8' },
      })
      return key
    }).pipe(Effect.orDie),
})
