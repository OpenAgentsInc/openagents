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

export const ForgeGitPackfileObjectFormat = S.Literals([
  "sha1",
  "sha256",
  "unknown",
])
export type ForgeGitPackfileObjectFormat = typeof ForgeGitPackfileObjectFormat.Type

export const ForgeTenantState = S.Literals(["active", "suspended"])
export type ForgeTenantState = typeof ForgeTenantState.Type

export const ForgeGitAccessTokenState = S.Literals([
  "active",
  "revoked",
  "expired",
])
export type ForgeGitAccessTokenState = typeof ForgeGitAccessTokenState.Type

export const ForgeGitAccessScope = S.Literals([
  "git:upload-pack",
  "git:receive-pack",
  "git:admin",
])
export type ForgeGitAccessScope = typeof ForgeGitAccessScope.Type

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

export const ForgeGitPackfileArchiveRow = S.Struct({
  tenant_ref: S.String,
  packfile_ref: S.String,
  repository_ref: S.String,
  change_ref: S.NullOr(S.String),
  receive_pack_ref: S.NullOr(S.String),
  artifact_r2_key: S.String,
  packfile_sha256: S.String,
  packfile_bytes: S.Number,
  object_format: ForgeGitPackfileObjectFormat,
  command_count: S.Number,
  capabilities_json: S.String,
  ref_updates_json: S.String,
  source_refs_json: S.String,
  content_type: S.String,
  visibility: S.Literal("operator_only"),
  created_at: S.String,
  updated_at: S.String,
})
export type ForgeGitPackfileArchiveRow = typeof ForgeGitPackfileArchiveRow.Type

export const ForgeTenantRow = S.Struct({
  tenant_ref: S.String,
  display_name: S.String,
  state: ForgeTenantState,
  created_at: S.String,
  updated_at: S.String,
})
export type ForgeTenantRow = typeof ForgeTenantRow.Type

export const ForgeGitAccessTokenRow = S.Struct({
  tenant_ref: S.String,
  token_ref: S.String,
  subject_ref: S.String,
  repository_ref: S.String,
  token_hash: S.String,
  token_prefix: S.String,
  state: ForgeGitAccessTokenState,
  created_at: S.String,
  expires_at: S.String,
  last_used_at: S.NullOr(S.String),
  revoked_at: S.NullOr(S.String),
  source_refs_json: S.String,
})
export type ForgeGitAccessTokenRow = typeof ForgeGitAccessTokenRow.Type

export const ForgeGitAccessTokenScopeRow = S.Struct({
  tenant_ref: S.String,
  token_ref: S.String,
  scope: ForgeGitAccessScope,
  created_at: S.String,
})
export type ForgeGitAccessTokenScopeRow =
  typeof ForgeGitAccessTokenScopeRow.Type

export const ForgeCoordinationRow = S.Union([
  ForgeCoordinationIssueRow,
  ForgeCoordinationPrRow,
  ForgeCoordinationStatusRow,
  ForgeDispatchLeaseRow,
  ForgeMergeQueueLedgerRow,
  ForgeGitPackfileArchiveRow,
  ForgeTenantRow,
  ForgeGitAccessTokenRow,
  ForgeGitAccessTokenScopeRow,
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
export const decodeForgeGitPackfileArchiveRow = S.decodeUnknownSync(
  ForgeGitPackfileArchiveRow,
)
export const decodeForgeTenantRow = S.decodeUnknownSync(ForgeTenantRow)
export const decodeForgeGitAccessTokenRow = S.decodeUnknownSync(
  ForgeGitAccessTokenRow,
)
export const decodeForgeGitAccessTokenScopeRow = S.decodeUnknownSync(
  ForgeGitAccessTokenScopeRow,
)

export const forgeCoordinationStatusStateForNip34Kind = (
  kind: ForgeNip34StatusKind,
): ForgeCoordinationStatusState =>
  kind === 1630 ? "open" : kind === 1631 ? "applied" : kind === 1632 ? "closed" : "draft"
