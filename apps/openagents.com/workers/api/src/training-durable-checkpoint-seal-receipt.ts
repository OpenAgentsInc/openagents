import { Schema as S } from 'effect'

import {
  DurableCheckpointSeal,
  DurableCheckpointSealBlocker,
  DurableCheckpointSealSchemaVersion,
  MinDurableReplicationFactor,
  PublicSafeRef,
  evaluateDurableCheckpointSeal,
} from './training-durable-checkpoint-seal'

type DurableCheckpointSealValue = typeof DurableCheckpointSeal.Type

/**
 * Durable-checkpoint-seal receipt emitter for training.marathon_operations.v1.
 *
 * The durable-checkpoint-seal PREDICATE (training-durable-checkpoint-seal.ts)
 * decides whether a window may SEAL on its checkpoint: the digest is
 * content-addressed, the storage class is a durable content-addressed backend, the
 * replication factor meets the durable minimum, and the checkpoint was actually
 * read back from durable storage and re-hashed. It does not, however, emit the
 * public-safe RECEIPT the live runtime must publish once a window has actually been
 * sealed on a durable checkpoint — the artifact a reviewer dereferences to confirm
 * "this window was sealed on this content-addressed checkpoint, replicated N ways,
 * read back and verified". That receipt shape and its derivation are what this
 * module adds, mirroring the standby-dispatch and curtailment-drill receipt
 * emitters.
 *
 * Like those emitters, this REFUSES to fabricate a receipt: it re-runs the seal
 * predicate and throws unless the seal is durable, so a receipt can never be
 * minted for a non-content-addressed, ephemeral, under-replicated,
 * never-read-back, or read-back-without-proof checkpoint. The receipt ref is
 * derived deterministically from the window ref and the checkpoint digest, so the
 * same durable seal always maps to the same id.
 *
 * It is contract-level only. Emitting a receipt here records that a recorded seal
 * satisfied the durability conditions and carried a read-back proof ref; it
 * grants no dispatch, settlement, storage-backend, promise-state, or green-claim
 * authority. No window has been
 * sealed on a real remote content-addressed checkpoint store, so the public
 * projection's `durableCheckpointSealReceiptAvailable` /
 * `remoteCheckpointStoreReadbackReceiptAvailable` flags stay false — this is the
 * format the runtime will emit once a real durable seal happens.
 */

export const DurableCheckpointSealReceiptSchemaVersion =
  'openagents.training.marathon_operations.durable_checkpoint_seal_receipt.v1'
export type DurableCheckpointSealReceiptSchemaVersion =
  typeof DurableCheckpointSealReceiptSchemaVersion

export const DurableCheckpointSealReceipt = S.Struct({
  authorityBoundary: S.String,
  blockerRef: S.Literal(DurableCheckpointSealBlocker),
  checkpointDigestRef: S.String,
  minimumReplicationFactor: S.Literal(MinDurableReplicationFactor),
  outcome: S.Literal('seal_on_durable_checkpoint'),
  predicateSchemaVersion: S.Literal(DurableCheckpointSealSchemaVersion),
  publicSafe: S.Literal(true),
  receiptRef: S.String,
  replicationFactor: S.Int,
  retrievalProofRef: PublicSafeRef,
  schemaVersion: S.Literal(DurableCheckpointSealReceiptSchemaVersion),
  sizeBytes: S.Int,
  sourceRefs: S.Array(S.String),
  storageClass: S.String,
  windowRef: S.String,
})
export type DurableCheckpointSealReceipt =
  typeof DurableCheckpointSealReceipt.Type

export class DurableCheckpointSealReceiptUnsafe extends S.TaggedErrorClass<DurableCheckpointSealReceiptUnsafe>()(
  'DurableCheckpointSealReceiptUnsafe',
  {
    blockerRef: S.Literal(DurableCheckpointSealBlocker),
    reason: S.String,
  },
) {}

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:-]/g, '_').slice(0, 120)

/**
 * Derive the canonical, public-safe durable-checkpoint-seal receipt ref from the
 * window ref and the content-addressed checkpoint digest so the same durable seal
 * always maps to the same receipt id.
 */
export const durableCheckpointSealReceiptRef = (
  windowRef: string,
  checkpointDigestRef: string,
): string =>
  `receipt.public.training.marathon_operations.durable_checkpoint_seal.${safeSuffix(
    windowRef,
  )}.${safeSuffix(checkpointDigestRef)}`

const receiptAuthorityBoundary =
  'A durable-checkpoint-seal receipt records that one recorded window seal rested on a content-addressed checkpoint, on a durable content-addressed backend, replicated to at least the durable minimum, and read back from durable storage and re-hashed. It grants no dispatch, settlement, storage-backend, promise-state, or green-claim authority, and is emitted only for a seal the predicate scored as durable.'

const receiptSourceRefs: ReadonlyArray<string> = [
  'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal.ts',
  'apps/openagents.com/workers/api/src/training-durable-checkpoint-seal-receipt.ts',
  'docs/launch/vertex-fleet/training.marathon_operations.v1.md',
]

/**
 * Build the public-safe durable-checkpoint-seal receipt from a seal descriptor.
 *
 * Re-runs the seal predicate and throws DurableCheckpointSealReceiptUnsafe unless
 * the seal is DURABLE — a receipt is never emitted for a non-content-addressed,
 * ephemeral, under-replicated, or never-read-back checkpoint, so this cannot
 * manufacture a durability claim.
 */
export const buildDurableCheckpointSealReceipt = (
  seal: DurableCheckpointSealValue,
): DurableCheckpointSealReceipt => {
  const gate = evaluateDurableCheckpointSeal(seal)
  if (!gate.durable) {
    throw new DurableCheckpointSealReceiptUnsafe({
      blockerRef: DurableCheckpointSealBlocker,
      reason: `A durable-checkpoint-seal receipt may only be emitted for a durable seal; this seal is ${gate.decision} (${gate.reasons.join(', ')}).`,
    })
  }
  if (seal.retrievalProofRef === undefined) {
    throw new DurableCheckpointSealReceiptUnsafe({
      blockerRef: DurableCheckpointSealBlocker,
      reason:
        'A durable-checkpoint-seal receipt requires a read-back proof ref.',
    })
  }

  return DurableCheckpointSealReceipt.make({
    authorityBoundary: receiptAuthorityBoundary,
    blockerRef: DurableCheckpointSealBlocker,
    checkpointDigestRef: seal.checkpointDigestRef,
    minimumReplicationFactor: MinDurableReplicationFactor,
    outcome: 'seal_on_durable_checkpoint',
    predicateSchemaVersion: DurableCheckpointSealSchemaVersion,
    publicSafe: true,
    receiptRef: durableCheckpointSealReceiptRef(
      seal.windowRef,
      seal.checkpointDigestRef,
    ),
    replicationFactor: seal.replicationFactor,
    retrievalProofRef: seal.retrievalProofRef,
    schemaVersion: DurableCheckpointSealReceiptSchemaVersion,
    sizeBytes: seal.sizeBytes,
    sourceRefs: receiptSourceRefs,
    storageClass: seal.storageClass,
    windowRef: seal.windowRef,
  })
}

/**
 * Decode an untrusted seal descriptor and build its receipt. A descriptor that
 * fails to decode, or a seal that is not durable, throws
 * DurableCheckpointSealReceiptUnsafe — failing toward no-receipt rather than
 * minting one for an unverifiable seal.
 */
export const buildUntrustedDurableCheckpointSealReceipt = (
  input: unknown,
): DurableCheckpointSealReceipt => {
  let decoded: DurableCheckpointSealValue
  try {
    decoded = S.decodeUnknownSync(DurableCheckpointSeal)(input)
  } catch {
    throw new DurableCheckpointSealReceiptUnsafe({
      blockerRef: DurableCheckpointSealBlocker,
      reason:
        'A durable-checkpoint-seal receipt cannot be built from a malformed seal descriptor.',
    })
  }
  return buildDurableCheckpointSealReceipt(decoded)
}
