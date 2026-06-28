import { Schema as S } from "effect"

export const ForgeProtocolSchemaVersion = S.Literal("openagents.forge.protocol.v0.1")
export type ForgeProtocolSchemaVersion = typeof ForgeProtocolSchemaVersion.Type

export const FORGE_PROTOCOL_SCHEMA_VERSION: ForgeProtocolSchemaVersion =
  "openagents.forge.protocol.v0.1"

export const ForgeCoordinationStatusState = S.Literals([
  "open",
  "applied",
  "closed",
  "draft",
])
export type ForgeCoordinationStatusState = typeof ForgeCoordinationStatusState.Type

export const forgeCoordinationStatusStates: ReadonlyArray<ForgeCoordinationStatusState> = [
  "open",
  "applied",
  "closed",
  "draft",
]

export const ForgeCoordinationIssueState = S.Literals([
  "open",
  "closed",
  "draft",
])
export type ForgeCoordinationIssueState = typeof ForgeCoordinationIssueState.Type

export const ForgeCoordinationChangeState = S.Literals([
  "draft",
  "open",
  "ready",
  "blocked",
  "applied",
  "closed",
])
export type ForgeCoordinationChangeState = typeof ForgeCoordinationChangeState.Type

export const ForgeDispatchLeaseState = S.Literals([
  "active",
  "released",
  "expired",
  "cancelled",
])
export type ForgeDispatchLeaseState = typeof ForgeDispatchLeaseState.Type

export const ForgeMergeQueueLedgerState = S.Literals([
  "projected",
  "blocked",
  "promoting",
  "promoted",
  "superseded",
])
export type ForgeMergeQueueLedgerState = typeof ForgeMergeQueueLedgerState.Type

export const ForgeNip34StatusKind = S.Literals([1630, 1631, 1632, 1633])
export type ForgeNip34StatusKind = typeof ForgeNip34StatusKind.Type

export const forgeNip34StatusKindForState = (
  state: ForgeCoordinationStatusState,
): ForgeNip34StatusKind =>
  state === "open"
    ? 1630
    : state === "applied"
      ? 1631
      : state === "closed"
        ? 1632
        : 1633

export const ForgeCoordinationIssueRow = S.Struct({
  tenant_ref: S.String,
  issue_ref: S.String,
  github_issue_number: S.NullOr(S.Number),
  title: S.String,
  state: ForgeCoordinationIssueState,
  priority_ref: S.NullOr(S.String),
  source_refs_json: S.String,
  created_at: S.String,
  updated_at: S.String,
})
export type ForgeCoordinationIssueRow = typeof ForgeCoordinationIssueRow.Type

export const ForgeCoordinationPrRow = S.Struct({
  tenant_ref: S.String,
  pr_ref: S.String,
  issue_ref: S.String,
  change_ref: S.String,
  state: ForgeCoordinationChangeState,
  base_head: S.String,
  patch_head: S.String,
  verification_ref: S.NullOr(S.String),
  blocker_refs_json: S.String,
  source_refs_json: S.String,
  created_at: S.String,
  updated_at: S.String,
})
export type ForgeCoordinationPrRow = typeof ForgeCoordinationPrRow.Type

export const ForgeCoordinationStatusRow = S.Struct({
  tenant_ref: S.String,
  status_ref: S.String,
  subject_ref: S.String,
  nip34_kind: ForgeNip34StatusKind,
  state: ForgeCoordinationStatusState,
  actor_ref: S.String,
  source_refs_json: S.String,
  created_at: S.String,
})
export type ForgeCoordinationStatusRow = typeof ForgeCoordinationStatusRow.Type

export const ForgeDispatchLeaseRow = S.Struct({
  tenant_ref: S.String,
  lease_ref: S.String,
  work_ref: S.String,
  owner_agent_ref: S.String,
  state: ForgeDispatchLeaseState,
  idempotency_key_hash: S.NullOr(S.String),
  acquired_at: S.String,
  heartbeat_at: S.String,
  expires_at: S.String,
  released_at: S.NullOr(S.String),
  source_refs_json: S.String,
})
export type ForgeDispatchLeaseRow = typeof ForgeDispatchLeaseRow.Type

export const ForgeMergeQueueLedgerRow = S.Struct({
  tenant_ref: S.String,
  queue_ref: S.String,
  base_head: S.String,
  actual_head: S.String,
  virtual_head: S.String,
  state: ForgeMergeQueueLedgerState,
  next_promotion_ref: S.NullOr(S.String),
  ready_json: S.String,
  blocked_json: S.String,
  source_refs_json: S.String,
  created_at: S.String,
  updated_at: S.String,
})
export type ForgeMergeQueueLedgerRow = typeof ForgeMergeQueueLedgerRow.Type

export const ForgeCoordinationRow = S.Union([
  ForgeCoordinationIssueRow,
  ForgeCoordinationPrRow,
  ForgeCoordinationStatusRow,
  ForgeDispatchLeaseRow,
  ForgeMergeQueueLedgerRow,
])
export type ForgeCoordinationRow = typeof ForgeCoordinationRow.Type

export const decodeForgeCoordinationIssueRow = S.decodeUnknownSync(
  ForgeCoordinationIssueRow,
)
export const decodeForgeCoordinationPrRow = S.decodeUnknownSync(ForgeCoordinationPrRow)
export const decodeForgeCoordinationStatusRow = S.decodeUnknownSync(
  ForgeCoordinationStatusRow,
)
export const decodeForgeDispatchLeaseRow = S.decodeUnknownSync(ForgeDispatchLeaseRow)
export const decodeForgeMergeQueueLedgerRow = S.decodeUnknownSync(
  ForgeMergeQueueLedgerRow,
)

export const forgeCoordinationStatusStateForNip34Kind = (
  kind: ForgeNip34StatusKind,
): ForgeCoordinationStatusState =>
  kind === 1630 ? "open" : kind === 1631 ? "applied" : kind === 1632 ? "closed" : "draft"
