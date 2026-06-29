import { describe, expect, test } from 'vitest'

import { MinDurableReplicationFactor } from './training-durable-checkpoint-seal'
import {
  durableCheckpointR2ObjectKey,
  writeReadBackR2DurableCheckpointSeal,
} from './training-durable-checkpoint-seal-r2-store'

class MemoryR2Bucket {
  readonly objects = new Map<string, Uint8Array>()

  async put(
    key: string,
    value: ArrayBuffer | ArrayBufferView | string,
  ): Promise<R2Object> {
    if (typeof value === 'string') {
      this.objects.set(key, new TextEncoder().encode(value))
    } else if (ArrayBuffer.isView(value)) {
      this.objects.set(
        key,
        new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
      )
    } else {
      this.objects.set(key, new Uint8Array(value))
    }
    return { key } as R2Object
  }

  async get(key: string): Promise<R2ObjectBody | null> {
    const stored = this.objects.get(key)
    if (stored === undefined) {
      return null
    }
    return {
      arrayBuffer: async () =>
        stored.buffer.slice(
          stored.byteOffset,
          stored.byteOffset + stored.byteLength,
        ),
      key,
    } as R2ObjectBody
  }
}

class CorruptingReadbackR2Bucket extends MemoryR2Bucket {
  override async get(key: string): Promise<R2ObjectBody | null> {
    const object = await super.get(key)
    if (object === null) {
      return null
    }
    return {
      arrayBuffer: async () => new TextEncoder().encode('corrupted').buffer,
      key,
    } as R2ObjectBody
  }
}

describe('R2 durable checkpoint seal store', () => {
  test('stores checkpoint bytes under a content-addressed R2 key and verifies read-back by rehash', async () => {
    const bucket = new MemoryR2Bucket()
    const result = await writeReadBackR2DurableCheckpointSeal({
      bucket: bucket as unknown as R2Bucket,
      checkpoint: JSON.stringify({ step: 42, weightsDigest: 'fixture' }),
      windowRef: 'training.window.r1.0007',
    })

    expect(result.checkpointDigestRef).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(result.objectKey).toBe(
      durableCheckpointR2ObjectKey({
        checkpointDigestRef: result.checkpointDigestRef,
        windowRef: 'training.window.r1.0007',
      }),
    )
    expect(bucket.objects.has(result.objectKey)).toBe(true)
    expect(result.readBackDigestRef).toBe(result.checkpointDigestRef)
    expect(result.seal).toMatchObject({
      checkpointDigestRef: result.checkpointDigestRef,
      replicationFactor: MinDurableReplicationFactor,
      retrievalVerified: true,
      storageClass: 'content_addressed_object_store',
      windowRef: 'training.window.r1.0007',
    })
    expect(result.seal.retrievalProofRef).toContain(
      'receipt.training.checkpoint_readback.training.window.r1.0007.sha256:',
    )
    expect(result.receipt.outcome).toBe('seal_on_durable_checkpoint')
    expect(result.receipt.checkpointDigestRef).toBe(result.checkpointDigestRef)
  })

  test('fails closed when R2 read-back bytes do not rehash to the stored digest', async () => {
    await expect(
      writeReadBackR2DurableCheckpointSeal({
        bucket: new CorruptingReadbackR2Bucket() as unknown as R2Bucket,
        checkpoint: 'checkpoint fixture',
        windowRef: 'training.window.r1.0008',
      }),
    ).rejects.toThrow(/rehashed bytes did not match/)
  })

  test('fails closed on empty checkpoints before storing a seal descriptor', async () => {
    await expect(
      writeReadBackR2DurableCheckpointSeal({
        bucket: new MemoryR2Bucket() as unknown as R2Bucket,
        checkpoint: new Uint8Array(),
        windowRef: 'training.window.r1.empty',
      }),
    ).rejects.toThrow(/empty checkpoint/)
  })
})
