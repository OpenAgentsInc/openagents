import { describe, expect, it } from 'vitest'

import {
  ArtifactsUnavailableError,
  artifactsBucketForEnv,
} from './artifacts-binding'

describe('artifactsBucketForEnv', () => {
  it('returns the real binding when present', () => {
    const bucket = { get: () => Promise.resolve(null) } as unknown as R2Bucket
    expect(artifactsBucketForEnv({ ARTIFACTS: bucket })).toBe(bucket)
  })

  it('falls back to a bucket whose operations reject with the typed error', async () => {
    const bucket = artifactsBucketForEnv({})
    await expect(bucket.get('some/key')).rejects.toBeInstanceOf(
      ArtifactsUnavailableError,
    )
    await expect(bucket.put('some/key', 'value')).rejects.toMatchObject({
      _tag: 'ArtifactsUnavailableError',
      operation: 'put',
    })
  })

  it('rejects asynchronously so caller .catch() handling sees the failure', async () => {
    const bucket = artifactsBucketForEnv({ ARTIFACTS: undefined })
    // The exact shape used by callers like image-generation-routes:
    // a synchronous throw would escape this .catch().
    const result = await bucket.get('generated-images/x').catch(() => undefined)
    expect(result).toBeUndefined()
  })

  it('is not thenable (awaiting the bucket must not hang)', async () => {
    const bucket = artifactsBucketForEnv({})
    const resolved = await Promise.resolve(bucket)
    expect(resolved).toBe(bucket)
  })
})
