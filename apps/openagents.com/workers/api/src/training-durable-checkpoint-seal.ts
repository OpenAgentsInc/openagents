import { Schema as S } from 'effect'

/**
 * Durable content-addressed checkpoint-seal contract for
 * training.marathon_operations.v1.
 *
 * The marathon-operations green gate requires a window "sealed only on durable
 * content-addressed checkpoint storage". The window-seal metadata contract
 * (#4849) already carries an optional `checkpointDigestRef`, but a bare ref does
 * not prove the checkpoint is (a) content-addressed, (b) on durable storage, and
 * (c) actually retrievable from that storage. This module supplies the missing
 * durability predicate: a typed descriptor plus a pure evaluator that decides
 * whether a window may seal on its checkpoint or must HOLD.
 *
 * The evaluator FAILS TOWARD HOLD (never seals on an unproven/ephemeral
 * checkpoint), mirroring the seal-in-flight join barrier that fails toward
 * queueing (#4850/#4851). It is contract-level only: it grants no dispatch,
 * settlement, state-flip, or green-claim authority, and proving it against a
 * live remote checkpoint store remains future work.
 */

export const DurableCheckpointSealBlocker =
  'blocker.product_promises.durable_checkpoint_seal_missing'

export const DurableCheckpointSealSchemaVersion =
  'openagents.training.marathon_operations.durable_checkpoint_seal.v1'

/** Content-addressed checkpoint digests are sha256 of the serialized bytes. */
export const DurableCheckpointDigestPattern = /^sha256:[a-f0-9]{64}$/

/**
 * Minimum replication factor a storage backend must report before a checkpoint
 * counts as durable. A single replica is a single point of loss; durability
 * requires at least two independent copies.
 */
export const MinDurableReplicationFactor = 2

const PublicSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/
export const PublicSafeRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(PublicSafeRefPattern),
)

/**
 * All storage classes a checkpoint may live on. The durable, content-addressed
 * classes are enumerated explicitly so a new backend is durable only after it is
 * added here deliberately — an unknown class fails decoding and therefore HOLDs.
 */
export const CheckpointStorageClass = S.Literals([
  'content_addressed_object_store',
  'content_addressed_replicated_blob',
  'coordinator_memory',
  'local_scratch',
  'node_ram',
  'single_host_disk',
])
export type CheckpointStorageClass = typeof CheckpointStorageClass.Type

/** Storage classes that are both durable and content-addressed. */
export const DurableContentAddressedStorageClasses: ReadonlySet<CheckpointStorageClass> =
  new Set<CheckpointStorageClass>([
    'content_addressed_object_store',
    'content_addressed_replicated_blob',
  ])

export const DurableCheckpointSeal = S.Struct({
  /** Content-addressed digest of the serialized checkpoint bytes. */
  checkpointDigestRef: PublicSafeRef,
  /** How many independent durable replicas the backend reports. */
  replicationFactor: S.Number.check(
    S.isInt(),
    S.isBetween({ minimum: 0, maximum: 1_000 }),
  ),
  /**
   * True only when the seal pipeline fetched the checkpoint back from durable
   * storage and re-hashed it to `checkpointDigestRef`. Asserting durability
   * without a read-back receipt is not durability.
   */
  retrievalVerified: S.Boolean,
  /** Public-safe ref to the read-back verification receipt, when present. */
  retrievalProofRef: S.optional(PublicSafeRef),
  sizeBytes: S.Number.check(
    S.isInt(),
    S.isBetween({ minimum: 1, maximum: 1_099_511_627_776 }),
  ),
  storageClass: CheckpointStorageClass,
  windowRef: PublicSafeRef,
})
export type DurableCheckpointSeal = typeof DurableCheckpointSeal.Type

export type DurableCheckpointSealDecision =
  | 'seal_on_durable_checkpoint'
  | 'hold_for_durable_checkpoint'

export type DurableCheckpointSealReason =
  | 'checkpoint_digest_not_content_addressed'
  | 'storage_class_not_durable_content_addressed'
  | 'replication_factor_below_durable_minimum'
  | 'checkpoint_retrieval_not_verified'
  | 'checkpoint_retrieval_proof_ref_missing'
  | 'seal_descriptor_malformed'

export type DurableCheckpointSealGate = Readonly<{
  authorityBoundary: string
  blockerRef: typeof DurableCheckpointSealBlocker
  decision: DurableCheckpointSealDecision
  durable: boolean
  reasons: ReadonlyArray<DurableCheckpointSealReason>
  schemaVersion: typeof DurableCheckpointSealSchemaVersion
}>

const sealGateAuthorityBoundary =
  'Durable checkpoint-seal evaluation is a window-seal admissibility predicate only. A durable verdict permits a window to seal on its content-addressed checkpoint; it grants no dispatch, settlement, promise-state, or green-claim authority, and a hold verdict is the safe default, never a failure to record.'

const isContentAddressed = (digestRef: string): boolean =>
  DurableCheckpointDigestPattern.test(digestRef)

const isDurableStorageClass = (
  storageClass: CheckpointStorageClass,
): boolean => DurableContentAddressedStorageClasses.has(storageClass)

/**
 * Pure durability predicate for an already-decoded seal descriptor. A window may
 * seal on its checkpoint only when every durability condition holds; any failing
 * condition routes to HOLD with the failing reasons enumerated.
 */
export const evaluateDurableCheckpointSeal = (
  seal: DurableCheckpointSeal,
): DurableCheckpointSealGate => {
  const reasons: Array<DurableCheckpointSealReason> = []

  if (!isContentAddressed(seal.checkpointDigestRef)) {
    reasons.push('checkpoint_digest_not_content_addressed')
  }
  if (!isDurableStorageClass(seal.storageClass)) {
    reasons.push('storage_class_not_durable_content_addressed')
  }
  if (seal.replicationFactor < MinDurableReplicationFactor) {
    reasons.push('replication_factor_below_durable_minimum')
  }
  if (!seal.retrievalVerified) {
    reasons.push('checkpoint_retrieval_not_verified')
  }
  if (seal.retrievalVerified && seal.retrievalProofRef === undefined) {
    reasons.push('checkpoint_retrieval_proof_ref_missing')
  }

  const durable = reasons.length === 0

  return {
    authorityBoundary: sealGateAuthorityBoundary,
    blockerRef: DurableCheckpointSealBlocker,
    decision: durable
      ? 'seal_on_durable_checkpoint'
      : 'hold_for_durable_checkpoint',
    durable,
    reasons,
    schemaVersion: DurableCheckpointSealSchemaVersion,
  }
}

/**
 * Decode an untrusted seal descriptor and evaluate it. A descriptor that fails
 * to decode (missing fields, unknown storage class, malformed digest length)
 * yields a HOLD verdict — failing toward queueing rather than sealing on an
 * unverifiable checkpoint.
 */
export const evaluateUntrustedDurableCheckpointSeal = (
  input: unknown,
): DurableCheckpointSealGate => {
  let decoded: DurableCheckpointSeal
  try {
    decoded = S.decodeUnknownSync(DurableCheckpointSeal)(input)
  } catch {
    return {
      authorityBoundary: sealGateAuthorityBoundary,
      blockerRef: DurableCheckpointSealBlocker,
      decision: 'hold_for_durable_checkpoint',
      durable: false,
      reasons: ['seal_descriptor_malformed'],
      schemaVersion: DurableCheckpointSealSchemaVersion,
    }
  }
  return evaluateDurableCheckpointSeal(decoded)
}
