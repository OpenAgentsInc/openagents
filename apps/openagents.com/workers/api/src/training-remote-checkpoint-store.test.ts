import { describe, expect, test } from 'vitest'

import {
  MinDurableReplicationFactor,
  evaluateDurableCheckpointSeal,
} from './training-durable-checkpoint-seal'
import {
  TrainingRemoteCheckpointContentType,
  TrainingRemoteCheckpointStoreError,
  TrainingRemoteCheckpointStoreSchemaVersion,
  storeAndVerifyRemoteCheckpointSeal,
  trainingRemoteCheckpointObjectKey,
} from './training-remote-checkpoint-store'

class MemoryR2Bucket {
  readonly objects = new Map<
    string,
    Readonly<{
      body: Uint8Array
      customMetadata: Record<string, string> | undefined
      httpMetadata: R2HTTPMetadata | undefined
      sha256: unknown
    }>
  >()

  async put(key: string, value: unknown, options?: R2PutOptions) {
    const body =
      value instanceof Uint8Array
        ? value
        : typeof value === 'string'
          ? new TextEncoder().encode(value)
          : value instanceof ArrayBuffer
            ? new Uint8Array(value)
            : new Uint8Array(await new Response(value as BodyInit).arrayBuffer())

    this.objects.set(key, {
      body,
      customMetadata: options?.customMetadata,
      httpMetadata:
        options?.httpMetadata instanceof Headers
          ? undefined
          : options?.httpMetadata,
      sha256: options?.sha256,
    })

    return { key } as R2Object
  }

  async get(key: string) {
    const object = this.objects.get(key)
    if (object === undefined) {
      return null
    }

    return {
      arrayBuffer: async () => object.body.slice().buffer as ArrayBuffer,
      body: new Response(object.body.slice().buffer as ArrayBuffer).body,
      key,
      size: object.body.byteLength,
    } as R2ObjectBody
  }
}

class TamperingReadbackBucket extends MemoryR2Bucket {
  override async get(key: string) {
    const object = await super.get(key)
    if (object === null) {
      return null
    }

    return {
      ...object,
      arrayBuffer: async () => new TextEncoder().encode('tampered').buffer,
    } as R2ObjectBody
  }
}

describe('training remote checkpoint store', () => {
  test('writes a content-addressed checkpoint, reads it back, and emits a durable seal receipt', async () => {
    const bucket = new MemoryR2Bucket()
    const result = await storeAndVerifyRemoteCheckpointSeal({
      bucket: bucket as unknown as Pick<R2Bucket, 'get' | 'put'>,
      checkpoint: 'checkpoint bytes for window 7',
      nowIso: '2026-06-29T12:00:00.000Z',
      runRef: 'training.run.r1',
      windowRef: 'training.window.0007',
    })

    expect(result.checkpointDigestRef).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(result.objectKey).toBe(
      trainingRemoteCheckpointObjectKey(result.checkpointDigestRef),
    )
    expect(result.readbackDigestRef).toBe(result.checkpointDigestRef)
    expect(result.seal).toMatchObject({
      checkpointDigestRef: result.checkpointDigestRef,
      replicationFactor: MinDurableReplicationFactor,
      retrievalProofRef: result.receipt.receiptRef,
      retrievalVerified: true,
      storageClass: 'content_addressed_object_store',
      windowRef: 'training.window.0007',
    })
    expect(evaluateDurableCheckpointSeal(result.seal).durable).toBe(true)
    expect(result.receipt.outcome).toBe('seal_on_durable_checkpoint')

    const stored = bucket.objects.get(result.objectKey)
    expect(stored?.sha256).toBe(result.checkpointDigestRef.slice('sha256:'.length))
    expect(stored?.httpMetadata?.contentType).toBe(
      TrainingRemoteCheckpointContentType,
    )
    expect(stored?.customMetadata).toMatchObject({
      checkpointDigestRef: result.checkpointDigestRef,
      runRef: 'training.run.r1',
      schemaVersion: TrainingRemoteCheckpointStoreSchemaVersion,
      visibility: 'operator_only',
      windowRef: 'training.window.0007',
    })
  })

  test('refuses to emit a seal when the remote read-back digest does not match', async () => {
    await expect(
      storeAndVerifyRemoteCheckpointSeal({
        bucket: new TamperingReadbackBucket() as unknown as Pick<
          R2Bucket,
          'get' | 'put'
        >,
        checkpoint: 'checkpoint bytes for window 8',
        nowIso: '2026-06-29T12:05:00.000Z',
        runRef: 'training.run.r1',
        windowRef: 'training.window.0008',
      }),
    ).rejects.toBeInstanceOf(TrainingRemoteCheckpointStoreError)
  })

  test('refuses empty checkpoints before writing to the store', async () => {
    const bucket = new MemoryR2Bucket()

    await expect(
      storeAndVerifyRemoteCheckpointSeal({
        bucket: bucket as unknown as Pick<R2Bucket, 'get' | 'put'>,
        checkpoint: '',
        nowIso: '2026-06-29T12:10:00.000Z',
        runRef: 'training.run.r1',
        windowRef: 'training.window.empty',
      }),
    ).rejects.toBeInstanceOf(TrainingRemoteCheckpointStoreError)

    expect(bucket.objects.size).toBe(0)
  })
})
