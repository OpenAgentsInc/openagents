import { describe, expect, test } from 'vitest'

import {
  buildTrainingWindowRecord,
  transitionTrainingWindowRecord,
  type TrainingWindowSealMetadata,
} from './training-run-window-authority'
import {
  durableTrainingCheckpointR2Key,
  sealTrainingWindowRecordWithDurableCheckpointStore,
  sha256DigestRefForCheckpointBytes,
  storeDurableCheckpointAndBuildSeal,
} from './training-durable-checkpoint-store'
import { verifyDurableCheckpointSealReceipt } from './training-durable-checkpoint-seal-receipt-verifier'
import { selectLastDurableSealWindow } from './training-window-bootstrap'

class MemoryR2Object {
  constructor(readonly bytes: Uint8Array) {}

  async arrayBuffer(): Promise<ArrayBuffer> {
    return this.bytes.slice().buffer
  }
}

class MemoryR2Bucket {
  readonly objects = new Map<string, Uint8Array>()

  async head(key: string): Promise<R2Object | null> {
    return this.objects.has(key) ? ({} as R2Object) : null
  }

  async get(key: string): Promise<R2ObjectBody | null> {
    const bytes = this.objects.get(key)
    return bytes === undefined
      ? null
      : (new MemoryR2Object(bytes) as unknown as R2ObjectBody)
  }

  async put(
    key: string,
    value: ReadableStream | ArrayBuffer | ArrayBufferView | string | Blob,
  ): Promise<R2Object> {
    let bytes: Uint8Array
    if (typeof value === 'string') {
      bytes = new TextEncoder().encode(value)
    } else if (value instanceof ArrayBuffer) {
      bytes = new Uint8Array(value)
    } else if (ArrayBuffer.isView(value)) {
      bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength)
    } else if (value instanceof Blob) {
      bytes = new Uint8Array(await value.arrayBuffer())
    } else {
      bytes = new Uint8Array(await new Response(value).arrayBuffer())
    }
    this.objects.set(key, bytes.slice())
    return {} as R2Object
  }
}

const activeWindow = () => {
  const planned = buildTrainingWindowRecord({
    makeId: () => 'window',
    nowIso: '2026-06-29T10:00:00.000Z',
    request: {
      trainingRunRef: 'training.run.marathon.0001',
      windowRef: 'training.window.marathon.0001',
    },
  })
  return transitionTrainingWindowRecord({
    actorRef: 'operator.training',
    eventId: 'activate',
    nextState: 'active',
    nowIso: '2026-06-29T10:05:00.000Z',
    receiptRef: 'receipt.training.window.activate',
    transitionKind: 'window_activate',
    window: planned,
  }).window
}

const baseSealMetadata: Omit<
  TrainingWindowSealMetadata,
  'checkpointDigestRef' | 'durableCheckpointSeal'
> = {
  churn: {
    joinCount: 0,
    lossCount: 0,
    standbyPromotionCount: 0,
  },
  staleness: {
    contributionCount: 0,
    stepsBehindMax: 0,
    stepsBehindMin: 0,
    stepsBehindP50: 0,
    stepsBehindP90: 0,
  },
  verificationOverhead: {
    fraction: 0.12,
    ladderRungRef: 'ladder.rung.r1',
  },
}

describe('durable checkpoint R2 store', () => {
  test('writes checkpoint bytes to a content-addressed key and verifies read-back by rehash', async () => {
    const bucket = new MemoryR2Bucket()
    const checkpoint = 'checkpoint bytes for marathon window'
    const result = await storeDurableCheckpointAndBuildSeal({
      bucket: bucket as unknown as R2Bucket,
      checkpoint,
      window: activeWindow(),
    })
    const expectedDigest = await sha256DigestRefForCheckpointBytes(
      new TextEncoder().encode(checkpoint),
    )

    expect(result.checkpointDigestRef).toBe(expectedDigest)
    expect(result.readbackDigestRef).toBe(expectedDigest)
    expect(result.r2Key).toBe(durableTrainingCheckpointR2Key(expectedDigest))
    expect(bucket.objects.has(result.r2Key)).toBe(true)
    expect(result.durableCheckpointSeal).toMatchObject({
      checkpointDigestRef: expectedDigest,
      retrievalVerified: true,
      storageClass: 'content_addressed_object_store',
    })
    expect(result.durableCheckpointSeal.retrievalProofRef).toMatch(
      /^receipt\.public\.training\.marathon_operations\.checkpoint_readback\./,
    )
    expect(verifyDurableCheckpointSealReceipt(result.receipt).verified).toBe(
      true,
    )
  })

  test('seals a window only after the remote checkpoint read-back receipt exists', async () => {
    const result = await sealTrainingWindowRecordWithDurableCheckpointStore({
      actorRef: 'operator.training',
      bucket: new MemoryR2Bucket() as unknown as R2Bucket,
      checkpoint: new TextEncoder().encode('sealed checkpoint'),
      eventId: 'seal',
      nowIso: '2026-06-29T10:10:00.000Z',
      sealMetadata: baseSealMetadata,
      window: activeWindow(),
    })

    expect(result.window.state).toBe('sealed')
    expect(result.window.receiptRefs).toContain(result.receipt.receiptRef)
    expect(result.window.sealMetadata?.checkpointDigestRef).toBe(
      result.checkpointDigestRef,
    )
    expect(result.window.sealMetadata?.durableCheckpointSeal).toEqual(
      result.durableCheckpointSeal,
    )
    expect(selectLastDurableSealWindow([result.window])?.windowRef).toBe(
      result.window.windowRef,
    )
  })
})
