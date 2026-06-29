import {
  type DurableCheckpointSeal,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  type DurableCheckpointSealReceipt,
  buildDurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'

export const TrainingDurableCheckpointSealR2StoreSourceRef =
  'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-r2-store.ts'

export class DurableCheckpointSealR2StoreError extends Error {
  readonly _tag = 'DurableCheckpointSealR2StoreError'
}

export type DurableCheckpointSealR2StoreResult = Readonly<{
  checkpointDigestRef: string
  objectKey: string
  readBackDigestRef: string
  receipt: DurableCheckpointSealReceipt
  seal: DurableCheckpointSeal
}>

const keySafeSegment = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 160)

const bytesFromBody = (body: string | ArrayBuffer | Uint8Array): Uint8Array => {
  if (typeof body === 'string') {
    return new TextEncoder().encode(body)
  }
  if (body instanceof Uint8Array) {
    return body
  }
  return new Uint8Array(body)
}

const hexSha256 = async (bytes: Uint8Array): Promise<string> => {
  const digestInput = bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer
  const digest = await crypto.subtle.digest('SHA-256', digestInput)
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

export const durableCheckpointR2ObjectKey = (
  input: Readonly<{ checkpointDigestRef: string; windowRef: string }>,
): string => {
  const digestHex = input.checkpointDigestRef.replace(/^sha256:/, '')
  return [
    'training',
    'marathon-operations',
    'checkpoints',
    keySafeSegment(input.windowRef),
    `sha256-${digestHex}`,
  ].join('/')
}

export const durableCheckpointReadbackProofRef = (
  input: Readonly<{ checkpointDigestRef: string; windowRef: string }>,
): string =>
  `receipt.training.checkpoint_readback.${keySafeSegment(
    input.windowRef,
  )}.${keySafeSegment(input.checkpointDigestRef)}`

export const writeReadBackR2DurableCheckpointSeal = async (
  input: Readonly<{
    bucket: R2Bucket
    checkpoint: string | ArrayBuffer | Uint8Array
    contentType?: string | undefined
    replicationFactor?: number | undefined
    windowRef: string
  }>,
): Promise<DurableCheckpointSealR2StoreResult> => {
  const checkpointBytes = bytesFromBody(input.checkpoint)
  if (checkpointBytes.byteLength === 0) {
    throw new DurableCheckpointSealR2StoreError(
      'A durable checkpoint seal cannot store an empty checkpoint.',
    )
  }

  const checkpointDigestRef = `sha256:${await hexSha256(checkpointBytes)}`
  const objectKey = durableCheckpointR2ObjectKey({
    checkpointDigestRef,
    windowRef: input.windowRef,
  })

  await input.bucket.put(objectKey, checkpointBytes, {
    customMetadata: {
      checkpointDigestRef,
      sealSchema: 'openagents.training.marathon_operations.r2_checkpoint_store.v1',
      windowRef: input.windowRef,
    },
    httpMetadata: {
      contentType: input.contentType ?? 'application/octet-stream',
    },
    sha256: checkpointDigestRef.slice('sha256:'.length),
  })

  const object = await input.bucket.get(objectKey)
  if (object === null) {
    throw new DurableCheckpointSealR2StoreError(
      'R2 checkpoint read-back failed: object was not found after put.',
    )
  }

  const readBackBytes = new Uint8Array(await object.arrayBuffer())
  const readBackDigestRef = `sha256:${await hexSha256(readBackBytes)}`
  if (readBackDigestRef !== checkpointDigestRef) {
    throw new DurableCheckpointSealR2StoreError(
      'R2 checkpoint read-back failed: rehashed bytes did not match the content-addressed digest.',
    )
  }

  const seal: DurableCheckpointSeal = {
    checkpointDigestRef,
    replicationFactor:
      input.replicationFactor ?? MinDurableReplicationFactor,
    retrievalProofRef: durableCheckpointReadbackProofRef({
      checkpointDigestRef,
      windowRef: input.windowRef,
    }),
    retrievalVerified: true,
    sizeBytes: readBackBytes.byteLength,
    storageClass: 'content_addressed_object_store',
    windowRef: input.windowRef,
  }

  const receipt = buildDurableCheckpointSealReceipt(seal)
  if (
    receipt.receiptRef !==
    durableCheckpointSealReceiptRef(input.windowRef, checkpointDigestRef)
  ) {
    throw new DurableCheckpointSealR2StoreError(
      'Durable checkpoint seal receipt ref did not match the stored checkpoint digest.',
    )
  }

  return {
    checkpointDigestRef,
    objectKey,
    readBackDigestRef,
    receipt,
    seal,
  }
}
