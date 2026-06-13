export type RetentionClass = "ephemeral" | "standard" | "audit"

export type RetentionRecord = {
  readonly id: string
  readonly createdAtMs: number
  readonly retentionClass?: RetentionClass
  readonly kind?: string
  readonly content?: unknown
}

export type Tombstone = {
  readonly id: string
  readonly deletedAt: number
}

export type ProjectionInvalidation = {
  readonly invalidate: true
  readonly recordId: string
  readonly reason: string
}

export type RetentionPolicy = {
  readonly retentionClass: RetentionClass
  readonly maxAgeMs: number
}

export type DeletionDecision =
  | {
      readonly action: "keep"
      readonly reason: string
      readonly retentionClass: RetentionClass
      readonly expiresAtMs: number
    }
  | {
      readonly action: "delete" | "tombstone"
      readonly reason: string
      readonly retentionClass: RetentionClass
      readonly tombstone: Tombstone
      readonly projectionInvalidation: ProjectionInvalidation
    }

const HOUR_MS = 60 * 60 * 1_000
const DAY_MS = 24 * HOUR_MS

export const RETENTION_POLICIES: Record<RetentionClass, RetentionPolicy> = {
  ephemeral: {
    retentionClass: "ephemeral",
    maxAgeMs: HOUR_MS,
  },
  standard: {
    retentionClass: "standard",
    maxAgeMs: 30 * DAY_MS,
  },
  audit: {
    retentionClass: "audit",
    maxAgeMs: 7 * 365 * DAY_MS,
  },
}

const KIND_RETENTION_CLASS: Record<string, RetentionClass> = {
  artifact_payload: "standard",
  capture: "ephemeral",
  credential_metadata: "audit",
  event_log_payload: "standard",
  memory: "standard",
  product_receipt: "audit",
  public_event_ref: "audit",
  session_summary: "standard",
  telemetry_aggregate: "audit",
  workspace_cache: "ephemeral",
}

export function classifyRecord(record: RetentionRecord): RetentionClass {
  if (record.retentionClass) {
    return record.retentionClass
  }

  if (record.kind && KIND_RETENTION_CLASS[record.kind]) {
    return KIND_RETENTION_CLASS[record.kind]
  }

  return "standard"
}

export function getRetentionPolicy(retentionClass: RetentionClass): RetentionPolicy {
  return RETENTION_POLICIES[retentionClass]
}

export function decideDeletion(record: RetentionRecord, nowMs: number): DeletionDecision {
  const retentionClass = classifyRecord(record)
  const policy = getRetentionPolicy(retentionClass)
  const expiresAtMs = record.createdAtMs + policy.maxAgeMs

  if (nowMs < expiresAtMs) {
    return {
      action: "keep",
      reason: "within_retention_window",
      retentionClass,
      expiresAtMs,
    }
  }

  return {
    action: "tombstone",
    reason: "retention_window_expired",
    retentionClass,
    tombstone: {
      id: record.id,
      deletedAt: nowMs,
    },
    projectionInvalidation: {
      invalidate: true,
      recordId: record.id,
      reason: "record_tombstoned",
    },
  }
}
