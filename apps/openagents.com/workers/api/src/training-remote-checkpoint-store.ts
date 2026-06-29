import {
  type DurableCheckpointSeal,
  MinDurableReplicationFactor,
} from './training-durable-checkpoint-seal'
import {
  type DurableCheckpointSealReceipt,
  buildDurableCheckpointSealReceipt,
  durableCheckpointSealReceiptRef,
} from './training-durable-checkpoint-seal-receipt'

export const TrainingRemoteCheckpointContentType =
  'application/octet-stream'

export const TrainingRemoteCheckpointStorePrefix =
  'private/training/marathon-operations/checkpoints/sha256'

export const TrainingRemoteCheckpointStoreSchemaVersion =
  'openagents.training.marathon_operations.remote_checkpoint_store.v1'

type CheckpointBytesInput = ArrayBuffer | Uint8Array | string

export type TrainingRemoteCheckpointSealInput = Readonly<{
  bucket: Pick<R2Bucket, 'get' | 'put'>
  checkpoint: CheckpointBytesInput
  nowIso: string
  runRef: string
  windowRef: string
}>

export type TrainingRemoteCheckpointSealResult = Readonly<{
  checkpointDigestRef: string
  objectKey: string
  readbackDigestRef: string
  receipt: DurableCheckpointSealReceipt
  seal: DurableCheckpointSeal
}>

export class TrainingRemoteCheckpointStoreError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'TrainingRemoteCheckpointStoreError'
  }
}

const toBytes = (value: CheckpointBytesInput): Uint8Array => {
  if (typeof value === 'string') {
    return new TextEncoder().encode(value)
  }
  if (value instanceof Uint8Array) {
    return value
  }
  return new Uint8Array(value)
}

const hexFromBytes = (bytes: Uint8Array): string =>
  [...bytes].map(byte => byte.toString(16).padStart(2, '0')).join('')

const sha256Hex = async (bytes: Uint8Array): Promise<string> =>
  hexFromBytes(
    new Uint8Array(await crypto.subtle.digest('SHA-256', bytes as BufferSource)),
  )

const safeSegment = (value: string): string => {
  const safe = value.replace(/[^A-Za-z0-9_.:-]+/g, '_').replace(/^_+|_+$/g, '')
  return safe === '' ? 'unknown' : safe.slice(0, 160)
}

export const trainingRemoteCheckpointObjectKey = (
  checkpointDigestRef: string,
): string => {
  const digest = checkpointDigestRef.replace(/^sha256:/, '')
  return `${TrainingRemoteCheckpointStorePrefix}/${safeSegment(digest)}.checkpoint`
}

const readR2ObjectBytes = async (object: R2ObjectBody): Promise<Uint8Array> => {
  if (typeof object.arrayBuffer === 'function') {
    return new Uint8Array(await object.arrayBuffer())
  }

  if (object.body === null) {
    throw new TrainingRemoteCheckpointStoreError(
      'remote checkpoint object had no readable body',
    )
  }

  return new Uint8Array(await new Response(object.body).arrayBuffer())
}

/**
 * Writes checkpoint bytes to the bound remote content-addressed store, reads the
 * object back, re-hashes it, and returns the seal descriptor plus public-safe
 * seal receipt. This function does not mutate window records by itself; callers
 * must pass the returned seal into the existing window-seal transition gate.
 */
export const storeAndVerifyRemoteCheckpointSeal = async (
  input: TrainingRemoteCheckpointSealInput,
): Promise<TrainingRemoteCheckpointSealResult> => {
  const checkpointBytes = toBytes(input.checkpoint)
  if (checkpointBytes.byteLength === 0) {
    throw new TrainingRemoteCheckpointStoreError(
      'checkpoint bytes must be non-empty',
    )
  }

  const digestHex = await sha256Hex(checkpointBytes)
  const checkpointDigestRef = `sha256:${digestHex}`
  const objectKey = trainingRemoteCheckpointObjectKey(checkpointDigestRef)

  await input.bucket.put(objectKey, checkpointBytes, {
    customMetadata: {
      checkpointDigestRef,
      runRef: input.runRef,
      schemaVersion: TrainingRemoteCheckpointStoreSchemaVersion,
      storedAt: input.nowIso,
      visibility: 'operator_only',
      windowRef: input.windowRef,
    },
    httpMetadata: {
      contentType: TrainingRemoteCheckpointContentType,
    },
    sha256: digestHex,
  })

  const readbackObject = await input.bucket.get(objectKey)
  if (readbackObject === null) {
    throw new TrainingRemoteCheckpointStoreError(
      'remote checkpoint read-back returned no object',
    )
  }

  const readbackBytes = await readR2ObjectBytes(readbackObject)
  const readbackDigestHex = await sha256Hex(readbackBytes)
  const readbackDigestRef = `sha256:${readbackDigestHex}`

  if (readbackDigestRef !== checkpointDigestRef) {
    throw new TrainingRemoteCheckpointStoreError(
      `remote checkpoint read-back digest mismatch: expected ${checkpointDigestRef}, got ${readbackDigestRef}`,
    )
  }

  const retrievalProofRef = durableCheckpointSealReceiptRef(
    input.windowRef,
    checkpointDigestRef,
  )
  const seal: DurableCheckpointSeal = {
    checkpointDigestRef,
    replicationFactor: MinDurableReplicationFactor,
    retrievalProofRef,
    retrievalVerified: true,
    sizeBytes: checkpointBytes.byteLength,
    storageClass: 'content_addressed_object_store',
    windowRef: input.windowRef,
  }

  return {
    checkpointDigestRef,
    objectKey,
    readbackDigestRef,
    receipt: buildDurableCheckpointSealReceipt(seal),
    seal,
  }
}
