import {
  type TrainingWindowRecord,
  type TrainingWindowSealMetadata,
  transitionTrainingWindowRecord,
} from './training-run-window-authority'
import {
  type DurableCheckpointSeal,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  type DurableCheckpointSealReceipt,
  buildDurableCheckpointSealReceipt,
} from './training-durable-checkpoint-seal-receipt'

type CheckpointBytes = ArrayBuffer | Uint8Array | string | Blob

export type DurableCheckpointStoreSealInput = Readonly<{
  actorRef: string
  bucket: R2Bucket
  checkpoint: CheckpointBytes
  eventId: string
  nowIso: string
  receiptRef?: string | undefined
  sealMetadata: Omit<
    TrainingWindowSealMetadata,
    'checkpointDigestRef' | 'durableCheckpointSeal'
  >
  transitionKind?: string | undefined
  window: TrainingWindowRecord
}>

export type DurableCheckpointStoreSealResult = Readonly<{
  checkpointDigestRef: string
  createdObject: boolean
  durableCheckpointSeal: DurableCheckpointSeal
  objectRef: string
  readbackDigestRef: string
  receipt: DurableCheckpointSealReceipt
  retrievalProofRef: string
  r2Key: string
  window: TrainingWindowRecord
}>

export class DurableCheckpointStoreError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'DurableCheckpointStoreError'
  }
}

const textEncoder = new TextEncoder()

const bytesFromCheckpoint = async (
  checkpoint: CheckpointBytes,
): Promise<Uint8Array> => {
  if (typeof checkpoint === 'string') {
    return textEncoder.encode(checkpoint)
  }
  if (checkpoint instanceof Uint8Array) {
    return checkpoint
  }
  if (checkpoint instanceof ArrayBuffer) {
    return new Uint8Array(checkpoint)
  }
  return new Uint8Array(await checkpoint.arrayBuffer())
}

const hexFromBuffer = (buffer: ArrayBuffer): string =>
  [...new Uint8Array(buffer)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer

export const sha256DigestRefForCheckpointBytes = async (
  bytes: Uint8Array,
): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    arrayBufferFromBytes(bytes),
  )
  return `sha256:${hexFromBuffer(digest)}`
}

const checkpointHex = (digestRef: string): string =>
  digestRef.slice('sha256:'.length)

export const durableTrainingCheckpointR2Key = (
  checkpointDigestRef: string,
): string =>
  [
    'private',
    'training',
    'marathon-operations',
    'checkpoints',
    'sha256',
    `${checkpointHex(checkpointDigestRef)}.checkpoint`,
  ].join('/')

export const durableTrainingCheckpointObjectRef = (
  checkpointDigestRef: string,
): string =>
  `r2.private.training.marathon_operations.checkpoint.${checkpointHex(
    checkpointDigestRef,
  )}`

export const durableTrainingCheckpointReadbackProofRef = (
  input: Pick<TrainingWindowRecord, 'windowRef'> &
    Readonly<{ checkpointDigestRef: string }>,
): string =>
  `receipt.public.training.marathon_operations.checkpoint_readback.${input.windowRef}.${checkpointHex(
    input.checkpointDigestRef,
  )}`

const readObjectBytes = async (
  bucket: R2Bucket,
  r2Key: string,
): Promise<Uint8Array> => {
  const object = await bucket.get(r2Key)
  if (object === null) {
    throw new DurableCheckpointStoreError(
      'durable checkpoint read-back failed: object missing after put',
    )
  }
  return new Uint8Array(await object.arrayBuffer())
}

/**
 * Store checkpoint bytes in a content-addressed R2 key, read them back, rehash
 * the fetched bytes, and return the durable seal descriptor plus public-safe
 * seal receipt. The checkpoint bytes themselves stay in private R2.
 */
export const storeDurableCheckpointAndBuildSeal = async (
  input: Readonly<{
    bucket: R2Bucket
    checkpoint: CheckpointBytes
    window: TrainingWindowRecord
  }>,
): Promise<
  Omit<DurableCheckpointStoreSealResult, 'receipt' | 'window'> &
    Readonly<{ receipt: DurableCheckpointSealReceipt }>
> => {
  const bytes = await bytesFromCheckpoint(input.checkpoint)
  if (bytes.byteLength <= 0) {
    throw new DurableCheckpointStoreError(
      'durable checkpoint bytes must be non-empty',
    )
  }

  const checkpointDigestRef = await sha256DigestRefForCheckpointBytes(bytes)
  const r2Key = durableTrainingCheckpointR2Key(checkpointDigestRef)
  const existingObject = await input.bucket.head(r2Key)
  let createdObject = false

  if (existingObject === null) {
    await input.bucket.put(r2Key, bytes, {
      customMetadata: {
        checkpointDigestRef,
        objectRef: durableTrainingCheckpointObjectRef(checkpointDigestRef),
        promiseRef: 'training.marathon_operations.v1',
        visibility: 'private_training_checkpoint',
        windowRef: input.window.windowRef,
      },
      httpMetadata: {
        contentType: 'application/octet-stream',
      },
    })
    createdObject = true
  }

  const readbackBytes = await readObjectBytes(input.bucket, r2Key)
  const readbackDigestRef =
    await sha256DigestRefForCheckpointBytes(readbackBytes)
  if (readbackDigestRef !== checkpointDigestRef) {
    throw new DurableCheckpointStoreError(
      'durable checkpoint read-back digest did not match the written digest',
    )
  }

  const retrievalProofRef = durableTrainingCheckpointReadbackProofRef({
    checkpointDigestRef,
    windowRef: input.window.windowRef,
  })
  const durableCheckpointSeal: DurableCheckpointSeal = {
    checkpointDigestRef,
    replicationFactor: MinDurableReplicationFactor,
    retrievalProofRef,
    retrievalVerified: true,
    sizeBytes: readbackBytes.byteLength,
    storageClass: 'content_addressed_object_store',
    windowRef: input.window.windowRef,
  }
  const receipt = buildDurableCheckpointSealReceipt(durableCheckpointSeal)

  return {
    checkpointDigestRef,
    createdObject,
    durableCheckpointSeal,
    objectRef: durableTrainingCheckpointObjectRef(checkpointDigestRef),
    readbackDigestRef,
    receipt,
    retrievalProofRef,
    r2Key,
  }
}

/**
 * End-to-end seal helper for the Worker path: content-address the checkpoint in
 * R2, verify it by read-back-and-rehash, then call the existing transition gate
 * with the verified durableCheckpointSeal descriptor.
 */
export const sealTrainingWindowRecordWithDurableCheckpointStore = async (
  input: DurableCheckpointStoreSealInput,
): Promise<DurableCheckpointStoreSealResult> => {
  const stored = await storeDurableCheckpointAndBuildSeal({
    bucket: input.bucket,
    checkpoint: input.checkpoint,
    window: input.window,
  })
  const receiptRef = input.receiptRef ?? stored.receipt.receiptRef
  const transitioned = transitionTrainingWindowRecord({
    actorRef: input.actorRef,
    eventId: input.eventId,
    nextState: 'sealed',
    nowIso: input.nowIso,
    receiptRef,
    sealMetadata: {
      ...input.sealMetadata,
      checkpointDigestRef: stored.checkpointDigestRef,
      durableCheckpointSeal: stored.durableCheckpointSeal,
    },
    transitionKind:
      input.transitionKind ?? 'window_seal_durable_checkpoint_readback',
    window: input.window,
  })

  return {
    ...stored,
    window: transitioned.window,
  }
}
