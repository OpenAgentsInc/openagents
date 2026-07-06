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

  it('resolves the GCS adapter when the ARTIFACTS_GCS_* config is complete (#8523)', () => {
    const env = {
      ARTIFACTS_GCS_BUCKET: 'openagentsgemini-oa-artifacts',
      ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID: 'GOOG1ETEST',
      ARTIFACTS_GCS_HMAC_SECRET: 'secret',
    }
    const bucket = artifactsBucketForEnv(env)
    // A real adapter, not the rejecting stub: implemented ops are functions
    // that do NOT reject with ArtifactsUnavailableError.
    expect(typeof bucket.get).toBe('function')
    expect(typeof bucket.put).toBe('function')
    // Memoized per config: the same env config yields the same instance.
    expect(artifactsBucketForEnv({ ...env })).toBe(bucket)
    // A different bucket name yields a different adapter.
    expect(
      artifactsBucketForEnv({
        ...env,
        ARTIFACTS_GCS_BUCKET: 'openagentsgemini-oa-artifacts-staging',
      }),
    ).not.toBe(bucket)
  })

  it('an injected ARTIFACTS binding wins over GCS config', () => {
    const injected = { get: () => Promise.resolve(null) } as unknown as R2Bucket
    expect(
      artifactsBucketForEnv({
        ARTIFACTS: injected,
        ARTIFACTS_GCS_BUCKET: 'openagentsgemini-oa-artifacts',
        ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID: 'GOOG1ETEST',
        ARTIFACTS_GCS_HMAC_SECRET: 'secret',
      }),
    ).toBe(injected)
  })

  it('incomplete GCS config (bucket without secrets) still degrades to the stub', async () => {
    const bucket = artifactsBucketForEnv({
      ARTIFACTS_GCS_BUCKET: 'openagentsgemini-oa-artifacts',
    })
    await expect(bucket.get('some/key')).rejects.toBeInstanceOf(
      ArtifactsUnavailableError,
    )
  })
})
