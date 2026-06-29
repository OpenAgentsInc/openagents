import { Effect, Schema as S } from 'effect'

import { parseJsonUnknown } from '../json-boundary'
import type { InferenceResult, InferenceUsage } from './provider-adapter'

export const BatchJobResultItem = S.Struct({
  index: S.Number,
  model: S.String,
  status: S.Literals(['succeeded', 'failed']),
  content: S.optionalKey(S.String),
  errorReason: S.optionalKey(S.String),
  finishReason: S.optionalKey(S.String),
  servedModel: S.optionalKey(S.String),
  usage: S.optionalKey(
    S.Struct({
      completionTokens: S.Number,
      promptTokens: S.Number,
      totalTokens: S.Number,
      cachedPromptTokens: S.optionalKey(S.Number),
    }),
  ),
})
export type BatchJobResultItem = S.Schema.Type<typeof BatchJobResultItem>

export const BatchJobResultsPayload = S.Struct({
  schemaVersion: S.Literal('openagents.inference.batch_job.results.v1'),
  jobId: S.String,
  results: S.Array(BatchJobResultItem),
})
export type BatchJobResultsPayload = S.Schema.Type<
  typeof BatchJobResultsPayload
>

export type BatchJobResultStore = Readonly<{
  putResults: (payload: BatchJobResultsPayload) => Effect.Effect<string>
  getResults: (key: string) => Effect.Effect<BatchJobResultsPayload | null>
}>

export const batchJobResultsR2Key = (jobId: string): string =>
  `inference/batch-jobs/${jobId}/results.json`

export const succeededBatchJobResultItem = (input: {
  index: number
  model: string
  result: InferenceResult
}): BatchJobResultItem => ({
  content: input.result.content,
  finishReason: input.result.finishReason,
  index: input.index,
  model: input.model,
  servedModel: input.result.servedModel,
  status: 'succeeded',
  usage: usageToResultUsage(input.result.usage),
})

export const failedBatchJobResultItem = (input: {
  errorReason: string
  index: number
  model: string
}): BatchJobResultItem => ({
  errorReason: input.errorReason,
  index: input.index,
  model: input.model,
  status: 'failed',
})

const usageToResultUsage = (usage: InferenceUsage) => ({
  completionTokens: usage.completionTokens,
  promptTokens: usage.promptTokens,
  totalTokens: usage.totalTokens,
  ...(usage.cachedPromptTokens === undefined
    ? {}
    : { cachedPromptTokens: usage.cachedPromptTokens }),
})

export const makeR2BatchJobResultStore = (
  bucket: R2Bucket,
): BatchJobResultStore => ({
  putResults: payload =>
    Effect.tryPromise(async () => {
      const key = batchJobResultsR2Key(payload.jobId)
      await bucket.put(key, JSON.stringify(payload), {
        httpMetadata: { contentType: 'application/json' },
      })
      return key
    }).pipe(Effect.orDie),
  getResults: key =>
    Effect.tryPromise(async () => {
      const object = await bucket.get(key)
      if (object === null) {
        return null
      }
      const text = await object.text()
      return S.decodeUnknownSync(BatchJobResultsPayload)(parseJsonUnknown(text))
    }).pipe(Effect.orDie),
})
