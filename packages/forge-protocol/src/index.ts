import { Schema as S } from "effect";

export const ForgeProtocolSchemaVersion = S.Literal(
  "openagents.forge.protocol.v0.1",
);
export type ForgeProtocolSchemaVersion = typeof ForgeProtocolSchemaVersion.Type;

export const FORGE_PROTOCOL_SCHEMA_VERSION: ForgeProtocolSchemaVersion =
  "openagents.forge.protocol.v0.1";

export const ForgeCoordinationStatusState = S.Literals([
  "open",
  "applied",
  "closed",
  "draft",
]);
export type ForgeCoordinationStatusState =
  typeof ForgeCoordinationStatusState.Type;

export const forgeCoordinationStatusStates: ReadonlyArray<ForgeCoordinationStatusState> =
  ["open", "applied", "closed", "draft"];

export const ForgeCoordinationIssueState = S.Literals([
  "open",
  "closed",
  "draft",
]);
export type ForgeCoordinationIssueState =
  typeof ForgeCoordinationIssueState.Type;

export const ForgeCoordinationChangeState = S.Literals([
  "draft",
  "open",
  "ready",
  "blocked",
  "applied",
  "closed",
]);
export type ForgeCoordinationChangeState =
  typeof ForgeCoordinationChangeState.Type;

export const ForgeDispatchLeaseState = S.Literals([
  "active",
  "released",
  "expired",
  "cancelled",
]);
export type ForgeDispatchLeaseState = typeof ForgeDispatchLeaseState.Type;

export const ForgeMergeQueueLedgerState = S.Literals([
  "projected",
  "blocked",
  "promoting",
  "promoted",
  "superseded",
]);
export type ForgeMergeQueueLedgerState = typeof ForgeMergeQueueLedgerState.Type;

export const ForgeGitPackfileObjectFormat = S.Literals([
  "sha1",
  "sha256",
  "unknown",
]);
export type ForgeGitPackfileObjectFormat =
  typeof ForgeGitPackfileObjectFormat.Type;

export const ForgeTenantState = S.Literals(["active", "suspended"]);
export type ForgeTenantState = typeof ForgeTenantState.Type;

export const ForgeTenantConfidentialWorkspaceMode = S.Literals([
  "disabled",
  "enabled",
  "attested",
]);
export type ForgeTenantConfidentialWorkspaceMode =
  typeof ForgeTenantConfidentialWorkspaceMode.Type;

export const ForgeGitAccessTokenState = S.Literals([
  "active",
  "revoked",
  "expired",
]);
export type ForgeGitAccessTokenState = typeof ForgeGitAccessTokenState.Type;

export const ForgeGitAccessScope = S.Literals([
  "git:upload-pack",
  "git:receive-pack",
  "git:admin",
]);
export type ForgeGitAccessScope = typeof ForgeGitAccessScope.Type;

export const ForgeControlPlaneScope = S.Literals([
  "forge:work:read",
  "forge:work:write",
  "forge:change:read",
  "forge:change:write",
  "forge:status:write",
  "forge:lease:write",
  "forge:queue:read",
  "forge:queue:write",
  "forge:receipt:write",
  "forge:promotion:decide",
  "forge:admin",
]);
export type ForgeControlPlaneScope = typeof ForgeControlPlaneScope.Type;

export const forgeControlPlaneScopes: ReadonlyArray<ForgeControlPlaneScope> = [
  "forge:work:read",
  "forge:work:write",
  "forge:change:read",
  "forge:change:write",
  "forge:status:write",
  "forge:lease:write",
  "forge:queue:read",
  "forge:queue:write",
  "forge:receipt:write",
  "forge:promotion:decide",
  "forge:admin",
];

export const ForgeDispatchWorkClass = S.Literals([
  "codex_agent_task",
  "claude_agent_task",
  "cloud_coding_session",
]);
export type ForgeDispatchWorkClass = typeof ForgeDispatchWorkClass.Type;

export const ForgeDispatchPaymentMode = S.Literals(["no-spend", "paid"]);
export type ForgeDispatchPaymentMode = typeof ForgeDispatchPaymentMode.Type;

export const ForgeDispatchDecisionState = S.Literals(["accepted", "rejected"]);
export type ForgeDispatchDecisionState = typeof ForgeDispatchDecisionState.Type;

export const ForgeDispatchCloseoutStatus = S.Literals([
  "accepted",
  "rejected",
  "cancelled",
  "timed-out",
  "stale",
]);
export type ForgeDispatchCloseoutStatus =
  typeof ForgeDispatchCloseoutStatus.Type;

export const ForgeDispatchSettlementState = S.Literals([
  "not_applicable",
  "pending",
  "recorded",
  "blocked",
]);
export type ForgeDispatchSettlementState =
  typeof ForgeDispatchSettlementState.Type;

export const ForgeVerificationVerdict = S.Literals([
  "passed",
  "failed",
  "timed_out",
  "cancelled",
  "errored",
]);
export type ForgeVerificationVerdict = typeof ForgeVerificationVerdict.Type;

export const ForgePromotionDecisionState = S.Literals([
  "approved",
  "blocked",
  "superseded",
]);
export type ForgePromotionDecisionState =
  typeof ForgePromotionDecisionState.Type;

export const ForgeDispatchGitAccessDelivery = S.Literals([
  "out_of_band",
  "same_response_ephemeral",
]);
export type ForgeDispatchGitAccessDelivery =
  typeof ForgeDispatchGitAccessDelivery.Type;

export const ForgeNip34StatusKind = S.Literals([1630, 1631, 1632, 1633]);
export type ForgeNip34StatusKind = typeof ForgeNip34StatusKind.Type;

export const forgeNip34StatusKindForState = (
  state: ForgeCoordinationStatusState,
): ForgeNip34StatusKind =>
  state === "open"
    ? 1630
    : state === "applied"
      ? 1631
      : state === "closed"
        ? 1632
        : 1633;

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
});
export type ForgeCoordinationIssueRow = typeof ForgeCoordinationIssueRow.Type;

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
});
export type ForgeCoordinationPrRow = typeof ForgeCoordinationPrRow.Type;

export const ForgeCoordinationStatusRow = S.Struct({
  tenant_ref: S.String,
  status_ref: S.String,
  subject_ref: S.String,
  nip34_kind: ForgeNip34StatusKind,
  state: ForgeCoordinationStatusState,
  actor_ref: S.String,
  source_refs_json: S.String,
  created_at: S.String,
});
export type ForgeCoordinationStatusRow = typeof ForgeCoordinationStatusRow.Type;

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
});
export type ForgeDispatchLeaseRow = typeof ForgeDispatchLeaseRow.Type;

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
});
export type ForgeMergeQueueLedgerRow = typeof ForgeMergeQueueLedgerRow.Type;

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
});
export type ForgeGitPackfileArchiveRow = typeof ForgeGitPackfileArchiveRow.Type;

export const ForgeTenantRow = S.Struct({
  tenant_ref: S.String,
  display_name: S.String,
  state: ForgeTenantState,
  confidential_workspace_mode: S.optionalKey(
    S.NullOr(ForgeTenantConfidentialWorkspaceMode),
  ),
  attestation_ref: S.optionalKey(S.NullOr(S.String)),
  encrypted_knowledge_pack_ref: S.optionalKey(S.NullOr(S.String)),
  refusal_reason: S.optionalKey(S.NullOr(S.String)),
  retention_policy_ref: S.optionalKey(S.NullOr(S.String)),
  created_at: S.String,
  updated_at: S.String,
});
export type ForgeTenantRow = typeof ForgeTenantRow.Type;

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
});
export type ForgeGitAccessTokenRow = typeof ForgeGitAccessTokenRow.Type;

export const ForgeGitAccessTokenScopeRow = S.Struct({
  tenant_ref: S.String,
  token_ref: S.String,
  scope: ForgeGitAccessScope,
  created_at: S.String,
});
export type ForgeGitAccessTokenScopeRow =
  typeof ForgeGitAccessTokenScopeRow.Type;

export const ForgeDispatchGitAccess = S.Struct({
  token_ref: S.String,
  token_prefix: S.String,
  scopes: S.Array(ForgeGitAccessScope),
  expires_at: S.String,
  delivery: ForgeDispatchGitAccessDelivery,
});
export type ForgeDispatchGitAccess = typeof ForgeDispatchGitAccess.Type;

export const ForgeDispatchVerificationCommand = S.Struct({
  command_ref: S.String,
  runner_ref: S.String,
  working_directory: S.String,
  args: S.Array(S.String),
  timeout_seconds: S.Number,
});
export type ForgeDispatchVerificationCommand =
  typeof ForgeDispatchVerificationCommand.Type;

export const ForgeDispatchGitTarget = S.Struct({
  repository_ref: S.String,
  remote_url: S.String,
  base_ref: S.String,
  base_head: S.String,
  branch_ref: S.String,
  receive_pack_ref: S.String,
  git_access: ForgeDispatchGitAccess,
});
export type ForgeDispatchGitTarget = typeof ForgeDispatchGitTarget.Type;

export const ForgeDispatchWorkItem = S.Struct({
  schema: S.Literal("openagents.forge.dispatch.work_item.v0.1"),
  tenant_ref: S.String,
  dispatch_ref: S.String,
  work_ref: S.String,
  issue_ref: S.NullOr(S.String),
  objective_ref: S.String,
  objective_summary: S.String,
  work_class: ForgeDispatchWorkClass,
  payment_mode: ForgeDispatchPaymentMode,
  capability_refs: S.Array(S.String),
  git: ForgeDispatchGitTarget,
  verification_command: S.NullOr(ForgeDispatchVerificationCommand),
  lease_ref: S.String,
  expires_at: S.String,
  created_at: S.String,
  source_refs: S.Array(S.String),
});
export type ForgeDispatchWorkItem = typeof ForgeDispatchWorkItem.Type;

export const ForgeDispatchDecision = S.Struct({
  schema: S.Literal("openagents.forge.dispatch.decision.v0.1"),
  tenant_ref: S.String,
  dispatch_ref: S.String,
  work_ref: S.String,
  lease_ref: S.String,
  pylon_ref: S.String,
  state: ForgeDispatchDecisionState,
  accepted_at: S.NullOr(S.String),
  rejected_at: S.NullOr(S.String),
  blocker_refs: S.Array(S.String),
  source_refs: S.Array(S.String),
});
export type ForgeDispatchDecision = typeof ForgeDispatchDecision.Type;

export const ForgeDispatchCloseout = S.Struct({
  schema: S.Literal("openagents.forge.dispatch.closeout.v0.1"),
  tenant_ref: S.String,
  dispatch_ref: S.String,
  work_ref: S.String,
  lease_ref: S.String,
  pylon_ref: S.String,
  status: ForgeDispatchCloseoutStatus,
  payment_mode: ForgeDispatchPaymentMode,
  settlement_state: ForgeDispatchSettlementState,
  payout_claim_allowed: S.Boolean,
  change_ref: S.NullOr(S.String),
  packfile_ref: S.NullOr(S.String),
  verification_ref: S.NullOr(S.String),
  artifact_refs: S.Array(S.String),
  blocker_refs: S.Array(S.String),
  build_refs: S.Array(S.String),
  closeout_refs: S.Array(S.String),
  preview_refs: S.Array(S.String),
  proof_refs: S.Array(S.String),
  receipt_refs: S.Array(S.String),
  result_refs: S.Array(S.String),
  summary_refs: S.Array(S.String),
  test_refs: S.Array(S.String),
  source_refs: S.Array(S.String),
  redacted: S.Literal(true),
  completed_at: S.String,
});
export type ForgeDispatchCloseout = typeof ForgeDispatchCloseout.Type;

export const ForgeVerificationReceipt = S.Struct({
  schema: S.Literal("openagents.forge.verification.receipt.v0.1"),
  tenant_ref: S.String,
  verification_ref: S.String,
  change_ref: S.String,
  repository_ref: S.String,
  base_ref: S.String,
  base_head: S.String,
  head_ref: S.String,
  head_head: S.String,
  packfile_ref: S.String,
  packfile_sha256: S.String,
  executor_identity_ref: S.String,
  command_ref: S.String,
  command_args: S.Array(S.String),
  exit_code: S.NullOr(S.Number),
  verdict: ForgeVerificationVerdict,
  started_at: S.String,
  completed_at: S.String,
  artifact_refs: S.Array(S.String),
  log_sha256: S.String,
  source_refs: S.Array(S.String),
  redacted: S.Literal(true),
});
export type ForgeVerificationReceipt = typeof ForgeVerificationReceipt.Type;

export const ForgePromotionEligibilityBlocker = S.Literals([
  "blocker.forge.verification.missing_change",
  "blocker.forge.verification.missing_receipt_ref",
  "blocker.forge.verification.missing_receipt",
  "blocker.forge.verification.wrong_change",
  "blocker.forge.verification.stale_base",
  "blocker.forge.verification.stale_head",
  "blocker.forge.verification.not_passing",
]);
export type ForgePromotionEligibilityBlocker =
  typeof ForgePromotionEligibilityBlocker.Type;

export const ForgePromotionEligibility = S.Struct({
  tenant_ref: S.String,
  change_ref: S.String,
  eligible: S.Boolean,
  verification_ref: S.NullOr(S.String),
  base_head: S.NullOr(S.String),
  head_head: S.NullOr(S.String),
  blocker_refs: S.Array(ForgePromotionEligibilityBlocker),
});
export type ForgePromotionEligibility = typeof ForgePromotionEligibility.Type;

export const ForgePromotionDecisionReceipt = S.Struct({
  schema: S.Literal("openagents.forge.promotion.decision.v0.1"),
  tenant_ref: S.String,
  promotion_ref: S.String,
  queue_ref: S.String,
  change_ref: S.String,
  queue_position: S.Number,
  decision: ForgePromotionDecisionState,
  base_head: S.String,
  candidate_head: S.String,
  promoted_head: S.NullOr(S.String),
  verification_ref: S.NullOr(S.String),
  gate_refs: S.Array(S.String),
  blocker_refs: S.Array(S.String),
  decided_by_ref: S.String,
  decided_at: S.String,
  source_refs: S.Array(S.String),
  redacted: S.Literal(true),
});
export type ForgePromotionDecisionReceipt =
  typeof ForgePromotionDecisionReceipt.Type;

export const ForgeDispatchMessage = S.Union([
  ForgeDispatchWorkItem,
  ForgeDispatchDecision,
  ForgeDispatchCloseout,
]);
export type ForgeDispatchMessage = typeof ForgeDispatchMessage.Type;

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
]);
export type ForgeCoordinationRow = typeof ForgeCoordinationRow.Type;

export const decodeForgeCoordinationIssueRow = S.decodeUnknownSync(
  ForgeCoordinationIssueRow,
);
export const decodeForgeCoordinationPrRow = S.decodeUnknownSync(
  ForgeCoordinationPrRow,
);
export const decodeForgeCoordinationStatusRow = S.decodeUnknownSync(
  ForgeCoordinationStatusRow,
);
export const decodeForgeDispatchLeaseRow = S.decodeUnknownSync(
  ForgeDispatchLeaseRow,
);
export const decodeForgeMergeQueueLedgerRow = S.decodeUnknownSync(
  ForgeMergeQueueLedgerRow,
);
export const decodeForgeGitPackfileArchiveRow = S.decodeUnknownSync(
  ForgeGitPackfileArchiveRow,
);
export const decodeForgeTenantRow = S.decodeUnknownSync(ForgeTenantRow);
export const decodeForgeGitAccessTokenRow = S.decodeUnknownSync(
  ForgeGitAccessTokenRow,
);
export const decodeForgeGitAccessTokenScopeRow = S.decodeUnknownSync(
  ForgeGitAccessTokenScopeRow,
);
export const decodeForgeControlPlaneScope = S.decodeUnknownSync(
  ForgeControlPlaneScope,
);
export const decodeForgeDispatchWorkItem = S.decodeUnknownSync(
  ForgeDispatchWorkItem,
);
export const decodeForgeDispatchDecision = S.decodeUnknownSync(
  ForgeDispatchDecision,
);
export const decodeForgeDispatchCloseout = S.decodeUnknownSync(
  ForgeDispatchCloseout,
);
export const decodeForgeDispatchMessage =
  S.decodeUnknownSync(ForgeDispatchMessage);
export const decodeForgeVerificationReceipt = S.decodeUnknownSync(
  ForgeVerificationReceipt,
);
export const decodeForgePromotionEligibility = S.decodeUnknownSync(
  ForgePromotionEligibility,
);
export const decodeForgePromotionDecisionReceipt = S.decodeUnknownSync(
  ForgePromotionDecisionReceipt,
);

export const forgeCoordinationStatusStateForNip34Kind = (
  kind: ForgeNip34StatusKind,
): ForgeCoordinationStatusState =>
  kind === 1630
    ? "open"
    : kind === 1631
      ? "applied"
      : kind === 1632
        ? "closed"
        : "draft";
