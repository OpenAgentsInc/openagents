export const dispositionClasses = [
  "raw_audio",
  "normalized_audio",
  "transcript_hypothesis",
  "transcript_final",
  "transcript_correction",
  "embedding_eval_training_copy",
  "command_receipt",
  "aggregate_metric",
] as const

export type DispositionClass = (typeof dispositionClasses)[number]
export type DeletionState = "active" | "deleted" | "expired" | "legal_hold"

export interface RetainedSessionReceipt {
  readonly receiptId: string
  readonly ownerRef: string
  readonly deviceRef: string
  readonly threadRef: string
  readonly sessionRef: string
  readonly generation: number
  readonly policyVersion: string
  readonly consentVersion: string
  readonly keyEpoch: string
  readonly acceptedAt: string
  readonly expiresAt: string
}

export interface SegmentInput {
  readonly ownerRef: string
  readonly deviceRef: string
  readonly threadRef: string
  readonly sessionRef: string
  readonly generation: number
  readonly firstSequence: number
  readonly lastSequence: number
  readonly captureStartedAt: string
  readonly captureEndedAt: string
  readonly serverReceivedAt: string
  readonly codec: string
  readonly dispositionClass: DispositionClass
  readonly bytes: Uint8Array
  readonly digest: string
}

export interface SegmentManifest extends Omit<SegmentInput, "bytes"> {
  readonly segmentId: string
  readonly byteLength: number
  readonly objectRef: string
  readonly receiptId: string
  readonly policyVersion: string
  readonly consentVersion: string
  readonly keyEpoch: string
  readonly expiresAt: string
  readonly deletionState: DeletionState
  readonly exportedAt?: string
}

export interface GapManifest {
  readonly sessionRef: string
  readonly generation: number
  readonly firstSequence: number
  readonly lastSequence: number
  readonly reason: "transport_gap" | "storage_outage" | "quota_refused" | "policy_refused"
  readonly recordedAt: string
}

export interface AccessReceipt {
  readonly receiptId: string
  readonly operation: "read" | "export" | "delete" | "expire"
  readonly ownerRef: string
  readonly sessionRef: string
  readonly occurredAt: string
  readonly dispositionClasses: readonly DispositionClass[]
  readonly segmentIds: readonly string[]
  readonly remainingLawfulRecords: readonly string[]
}

export class RetentionError extends Error {
  constructor(readonly reason: "retention_not_active" | "identity_mismatch" | "digest_mismatch" | "sequence_conflict" | "quota_exceeded" | "storage_unavailable" | "legal_hold" | "not_found", message: string) {
    super(message)
    this.name = "RetentionError"
  }
}
