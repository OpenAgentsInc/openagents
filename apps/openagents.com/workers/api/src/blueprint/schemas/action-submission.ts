import { Schema as S } from 'effect'

export const BlueprintActionSubmissionKind = S.Literals([
  'create_pull_request',
  'deploy',
  'legal_sensitive_action',
  'payment',
  'public_claim_upgrade',
  'send_email',
  'source_writeback',
])
export type BlueprintActionSubmissionKind =
  typeof BlueprintActionSubmissionKind.Type

export const BlueprintActionSubmissionStatus = S.Literals([
  'proposed',
  'dry_run_required',
  'dry_run_completed',
  'pending_approval',
  'approved',
  'executing',
  'receipt_recorded',
  'failed',
  'rejected',
  'cancelled',
])
export type BlueprintActionSubmissionStatus =
  typeof BlueprintActionSubmissionStatus.Type

export const BlueprintActionApprovalState = S.Literals([
  'not_requested',
  'pending',
  'approved',
  'rejected',
])
export type BlueprintActionApprovalState =
  typeof BlueprintActionApprovalState.Type

export const BlueprintActionSubmission = S.Struct({
  actionKind: BlueprintActionSubmissionKind,
  approvalPolicyRef: S.String,
  approvalReceiptRef: S.NullOr(S.String),
  approvalState: BlueprintActionApprovalState,
  approvedByRef: S.NullOr(S.String),
  contentRedacted: S.Boolean,
  contextPackRefs: S.Array(S.String),
  createdAt: S.String,
  directExecution: S.Boolean,
  directProgramRunExecutionAllowed: S.Boolean,
  dryRunReceiptRef: S.NullOr(S.String),
  dryRunRequired: S.Boolean,
  evidenceRefs: S.Array(S.String),
  executionReceiptRef: S.NullOr(S.String),
  failureRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  modelConfidenceBypassDisabled: S.Boolean,
  programRunAuthorityBoundary: S.Literal('evidence_only'),
  proposalOnly: S.Boolean,
  proposedByProgramRunId: S.String,
  proposedEffectRef: S.String,
  receiptRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: BlueprintActionSubmissionStatus,
  summaryRef: S.String,
  toolRefs: S.Array(S.String),
  updatedAt: S.String,
})
export type BlueprintActionSubmission =
  typeof BlueprintActionSubmission.Type

export const blueprintActionSubmissionIsApprovalGated = (
  submission: BlueprintActionSubmission,
): boolean =>
  submission.approvalState !== 'not_requested' ||
  submission.status === 'pending_approval' ||
  submission.status === 'approved' ||
  submission.status === 'executing' ||
  submission.status === 'receipt_recorded'

export const blueprintActionSubmissionHasDryRun = (
  submission: BlueprintActionSubmission,
): boolean =>
  !submission.dryRunRequired || submission.dryRunReceiptRef !== null

export const blueprintActionSubmissionCanExecute = (
  submission: BlueprintActionSubmission,
): boolean =>
  submission.status === 'approved' &&
  submission.approvalState === 'approved' &&
  submission.approvalReceiptRef !== null &&
  submission.approvedByRef !== null &&
  submission.executionReceiptRef === null &&
  submission.failureRef === null &&
  !submission.directExecution &&
  !submission.directProgramRunExecutionAllowed &&
  submission.modelConfidenceBypassDisabled &&
  submission.programRunAuthorityBoundary === 'evidence_only' &&
  blueprintActionSubmissionHasDryRun(submission)

export const blueprintActionSubmissionIsTerminal = (
  submission: BlueprintActionSubmission,
): boolean =>
  submission.status === 'receipt_recorded' ||
  submission.status === 'failed' ||
  submission.status === 'rejected' ||
  submission.status === 'cancelled'
