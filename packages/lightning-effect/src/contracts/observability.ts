import { Schema } from "effect"

import { Msats } from "./payment.js"

const TimestampMs = Schema.Int.pipe(Schema.nonNegative())
const NullableMsats = Schema.NullOr(Msats)

export const L402ObservabilityExecutor = Schema.Literal("desktop", "gateway", "system")
export type L402ObservabilityExecutor = typeof L402ObservabilityExecutor.Type

export const L402ObservabilityPlane = Schema.Literal("control", "gateway", "settlement", "ui")
export type L402ObservabilityPlane = typeof L402ObservabilityPlane.Type

export const L402ExecutionPath = Schema.Literal("local-node", "hosted-node")
export type L402ExecutionPath = typeof L402ExecutionPath.Type

export const L402DesktopRuntimeStatus = Schema.Literal(
  "unavailable",
  "stopped",
  "starting",
  "running",
  "stopping",
  "backoff",
  "failed",
)
export type L402DesktopRuntimeStatus = typeof L402DesktopRuntimeStatus.Type

export const L402WalletState = Schema.Literal("locked", "unlocked", "initializing", "recovering")
export type L402WalletState = typeof L402WalletState.Type

export const L402NodeSyncStatus = Schema.Literal("syncing", "synced", "degraded")
export type L402NodeSyncStatus = typeof L402NodeSyncStatus.Type

export const L402ObservabilityFieldKeys = [
  "requestId",
  "userId",
  "paywallId",
  "taskId",
  "endpoint",
  "quotedCostMsats",
  "capAppliedMsats",
  "paidAmountMsats",
  "paymentProofRef",
  "cacheHit",
  "denyReason",
  "executor",
  "plane",
  "executionPath",
  "desktopSessionId",
  "desktopRuntimeStatus",
  "walletState",
  "nodeSyncStatus",
  "observedAtMs",
] as const
export type L402ObservabilityFieldKey = (typeof L402ObservabilityFieldKeys)[number]

export const L402ObservabilityRecord = Schema.Struct({
  requestId: Schema.NullOr(Schema.NonEmptyString),
  userId: Schema.NullOr(Schema.NonEmptyString),
  paywallId: Schema.NullOr(Schema.NonEmptyString),
  taskId: Schema.NullOr(Schema.NonEmptyString),
  endpoint: Schema.NullOr(Schema.NonEmptyString),
  quotedCostMsats: NullableMsats,
  capAppliedMsats: NullableMsats,
  paidAmountMsats: NullableMsats,
  paymentProofRef: Schema.NullOr(Schema.NonEmptyString),
  cacheHit: Schema.NullOr(Schema.Boolean),
  denyReason: Schema.NullOr(Schema.String),
  executor: L402ObservabilityExecutor,
  plane: L402ObservabilityPlane,
  executionPath: L402ExecutionPath,
  desktopSessionId: Schema.NullOr(Schema.NonEmptyString),
  desktopRuntimeStatus: Schema.NullOr(L402DesktopRuntimeStatus),
  walletState: Schema.NullOr(L402WalletState),
  nodeSyncStatus: Schema.NullOr(L402NodeSyncStatus),
  observedAtMs: TimestampMs,
})
export type L402ObservabilityRecord = typeof L402ObservabilityRecord.Type

export const L402ObservabilitySnapshot = Schema.Struct({
  generatedAtMs: TimestampMs,
  records: Schema.Array(L402ObservabilityRecord),
})
export type L402ObservabilitySnapshot = typeof L402ObservabilitySnapshot.Type

export const decodeL402ObservabilityRecord = Schema.decodeUnknown(L402ObservabilityRecord)
export const decodeL402ObservabilityRecordSync = Schema.decodeUnknownSync(L402ObservabilityRecord)
export const encodeL402ObservabilityRecord = Schema.encode(L402ObservabilityRecord)
export const encodeL402ObservabilityRecordSync = Schema.encodeSync(L402ObservabilityRecord)

export const decodeL402ObservabilitySnapshot = Schema.decodeUnknown(L402ObservabilitySnapshot)
export const decodeL402ObservabilitySnapshotSync = Schema.decodeUnknownSync(L402ObservabilitySnapshot)
export const encodeL402ObservabilitySnapshot = Schema.encode(L402ObservabilitySnapshot)
export const encodeL402ObservabilitySnapshotSync = Schema.encodeSync(L402ObservabilitySnapshot)
