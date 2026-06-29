import { Schema as S } from 'effect'

import { GITHUB_WRITE_REQUIRED_SCOPES } from './github-write-connections'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const GitHubWritebackOperation = S.Literals([
  'create_branch',
  'push_commit',
  'open_pull_request',
  'open_fork_pull_request',
])
export type GitHubWritebackOperation =
  typeof GitHubWritebackOperation.Type

export const GitHubWritebackAuthorityMode = S.Literals([
  'customer_grant',
  'openagents_fork',
  'openagents_app',
])
export type GitHubWritebackAuthorityMode =
  typeof GitHubWritebackAuthorityMode.Type

export const GitHubWritebackApprovalSource = S.Literals([
  'customer_action',
  'operator_action',
  'system_policy',
])
export type GitHubWritebackApprovalSource =
  typeof GitHubWritebackApprovalSource.Type

export const GitHubWritebackBlockedReason = S.Literals([
  'explicit_approval_required',
  'source_access_required',
  'github_write_connection_required',
  'github_write_grant_required',
  'github_write_grant_expired',
  'github_write_grant_not_issued',
  'github_write_connection_unusable',
  'github_write_permission_missing',
  'openagents_app_not_configured',
  'unsupported_repository',
])
export type GitHubWritebackBlockedReason =
  typeof GitHubWritebackBlockedReason.Type

export type GitHubWritebackRepositoryRef = Readonly<{
  fullName: string
  isPrivate: boolean
}>

export type GitHubWritebackApproval = Readonly<{
  approvedAt: string
  source: GitHubWritebackApprovalSource
}>

export type GitHubWritebackConnectionAuthority = Readonly<{
  connectionRef: string
  hasSecretRef: boolean
  health: 'healthy' | 'requires_reauth' | 'unhealthy'
  scopes: ReadonlyArray<string>
  status: 'connected' | 'disconnected' | 'unhealthy'
}>

export type GitHubWritebackGrantAuthority = Readonly<{
  connectionRef: string
  expiresAt: string
  grantRef: string
  runnerSessionId: string | null
  status: 'issued' | 'used' | 'expired' | 'revoked' | 'failed'
}>

export type GitHubWritebackAuthorityRequest = Readonly<{
  approval: GitHubWritebackApproval | null
  assignmentId: string | null
  connection: GitHubWritebackConnectionAuthority | null
  grant: GitHubWritebackGrantAuthority | null
  operation: GitHubWritebackOperation
  repository: GitHubWritebackRepositoryRef | null
  softwareOrderId: string
  userId: string
}>

export type GitHubWritebackAuthorityAllowed = Readonly<{
  authorityMode: GitHubWritebackAuthorityMode
  connectionRef: string | null
  customerMessage: string
  decision: 'allowed'
  grantRef: string | null
  metadata: GitHubWritebackAuthorityMetadata
}>

export type GitHubWritebackAuthorityBlocked = Readonly<{
  blockedReason: GitHubWritebackBlockedReason
  customerMessage: string
  decision: 'blocked'
  metadata: GitHubWritebackAuthorityMetadata
}>

export type GitHubWritebackAuthorityDecision =
  | GitHubWritebackAuthorityAllowed
  | GitHubWritebackAuthorityBlocked

export type GitHubWritebackAuthorityMetadata = Readonly<{
  approvalSource: GitHubWritebackApprovalSource | null
  approvedAt: string | null
  authorityMode: GitHubWritebackAuthorityMode | null
  blockedReason: GitHubWritebackBlockedReason | null
  connectionRef: string | null
  decision: 'allowed' | 'blocked'
  grantRef: string | null
  operation: GitHubWritebackOperation
  repositoryFullName: string | null
  repositoryPrivate: boolean | null
}>

export type GitHubWritebackAuthorityReceipt = Readonly<{
  id: string
  softwareOrderId: string
  assignmentId: string | null
  userId: string
  repositoryFullName: string
  repositoryPrivate: boolean
  requestedOperation: GitHubWritebackOperation
  decision: 'allowed' | 'blocked'
  authorityMode: GitHubWritebackAuthorityMode | null
  blockedReason: GitHubWritebackBlockedReason | null
  connectionRef: string | null
  grantRef: string | null
  approvalSource: GitHubWritebackApprovalSource | null
  approvedAt: string | null
  customerMessage: string
  metadataJson: string
  createdAt: string
  updatedAt: string
}>

export type GitHubWritebackAuthorityRuntime = Readonly<{
  makeBlockedArtifactId: () => string
  makeReceiptId: () => string
  nowIso: () => string
}>

export const systemGitHubWritebackAuthorityRuntime: GitHubWritebackAuthorityRuntime =
  {
    makeBlockedArtifactId: () => compactRandomId('fulfillment_artifact'),
    makeReceiptId: () => compactRandomId('github_writeback_authority_receipt'),
    nowIso: currentIsoTimestamp,
  }

const missingApprovalMessage =
  'OpenAgents needs explicit approval before it creates a branch or pull request for this order.'

const sourceAccessMessage =
  'OpenAgents needs source access before it can create a reviewable pull request for this private repository.'

const missingConnectionMessage =
  'Connect a GitHub write account with repository permissions before OpenAgents can write to this repository.'

const missingGrantMessage =
  'Approve a fresh GitHub write grant for this run before OpenAgents can create the pull request.'

const publicForkMessage =
  'OpenAgents may prepare a pull request from an OpenAgents fork for this public repository.'

const customerGrantMessage =
  'OpenAgents may use the approved customer GitHub write grant for this order.'

const metadataFor = (
  input: GitHubWritebackAuthorityRequest,
  fields: Readonly<{
    authorityMode?: GitHubWritebackAuthorityMode | null
    blockedReason?: GitHubWritebackBlockedReason | null
    connectionRef?: string | null
    decision: 'allowed' | 'blocked'
    grantRef?: string | null
  }>,
): GitHubWritebackAuthorityMetadata => ({
  approvalSource: input.approval?.source ?? null,
  approvedAt: input.approval?.approvedAt ?? null,
  authorityMode: fields.authorityMode ?? null,
  blockedReason: fields.blockedReason ?? null,
  connectionRef: fields.connectionRef ?? null,
  decision: fields.decision,
  grantRef: fields.grantRef ?? null,
  operation: input.operation,
  repositoryFullName: input.repository?.fullName ?? null,
  repositoryPrivate: input.repository?.isPrivate ?? null,
})

const blocked = (
  input: GitHubWritebackAuthorityRequest,
  blockedReason: GitHubWritebackBlockedReason,
  customerMessage: string,
): GitHubWritebackAuthorityBlocked => ({
  blockedReason,
  customerMessage,
  decision: 'blocked',
  metadata: metadataFor(input, {
    blockedReason,
    decision: 'blocked',
  }),
})

const allowed = (
  input: GitHubWritebackAuthorityRequest,
  fields: Readonly<{
    authorityMode: GitHubWritebackAuthorityMode
    connectionRef: string | null
    customerMessage: string
    grantRef: string | null
  }>,
): GitHubWritebackAuthorityAllowed => ({
  authorityMode: fields.authorityMode,
  connectionRef: fields.connectionRef,
  customerMessage: fields.customerMessage,
  decision: 'allowed',
  grantRef: fields.grantRef,
  metadata: metadataFor(input, {
    authorityMode: fields.authorityMode,
    connectionRef: fields.connectionRef,
    decision: 'allowed',
    grantRef: fields.grantRef,
  }),
})

const hasRequiredScopes = (scopes: ReadonlyArray<string>): boolean => {
  const scopeSet = new Set(scopes)

  return GITHUB_WRITE_REQUIRED_SCOPES.every(scope => scopeSet.has(scope))
}

const resolveCustomerGrantAuthority = (
  input: GitHubWritebackAuthorityRequest,
  nowIso: string,
): GitHubWritebackAuthorityDecision => {
  if (input.connection === null) {
    return blocked(
      input,
      'github_write_connection_required',
      missingConnectionMessage,
    )
  }

  if (
    input.connection.status !== 'connected' ||
    input.connection.health !== 'healthy' ||
    !input.connection.hasSecretRef
  ) {
    return blocked(
      input,
      'github_write_connection_unusable',
      missingConnectionMessage,
    )
  }

  if (!hasRequiredScopes(input.connection.scopes)) {
    return blocked(
      input,
      'github_write_permission_missing',
      missingConnectionMessage,
    )
  }

  if (input.grant === null) {
    return blocked(input, 'github_write_grant_required', missingGrantMessage)
  }

  if (input.grant.connectionRef !== input.connection.connectionRef) {
    return blocked(
      input,
      'github_write_connection_unusable',
      missingConnectionMessage,
    )
  }

  if (input.grant.status !== 'issued') {
    return blocked(input, 'github_write_grant_not_issued', missingGrantMessage)
  }

  if (Date.parse(input.grant.expiresAt) <= Date.parse(nowIso)) {
    return blocked(input, 'github_write_grant_expired', missingGrantMessage)
  }

  return allowed(input, {
    authorityMode: 'customer_grant',
    connectionRef: input.connection.connectionRef,
    customerMessage: customerGrantMessage,
    grantRef: input.grant.grantRef,
  })
}

export const resolveGitHubWritebackAuthority = (
  input: GitHubWritebackAuthorityRequest,
  nowIso: string,
): GitHubWritebackAuthorityDecision => {
  if (input.repository === null) {
    return blocked(input, 'unsupported_repository', sourceAccessMessage)
  }

  if (input.approval === null) {
    return blocked(input, 'explicit_approval_required', missingApprovalMessage)
  }

  if (input.repository.isPrivate) {
    return resolveCustomerGrantAuthority(input, nowIso)
  }

  const customerGrantDecision = resolveCustomerGrantAuthority(input, nowIso)

  if (customerGrantDecision.decision === 'allowed') {
    return customerGrantDecision
  }

  if (input.operation === 'open_fork_pull_request') {
    return allowed(input, {
      authorityMode: 'openagents_fork',
      connectionRef: null,
      customerMessage: publicForkMessage,
      grantRef: null,
    })
  }

  return blocked(input, 'github_write_grant_required', missingGrantMessage)
}

export const gitHubWritebackArtifactMetadataJson = (
  decision: GitHubWritebackAuthorityDecision,
): string =>
  JSON.stringify({
    githubWritebackAuthority: decision.metadata,
  })

export const makeGitHubWritebackAuthorityReceipt = (
  input: GitHubWritebackAuthorityRequest,
  decision: GitHubWritebackAuthorityDecision,
  runtime: GitHubWritebackAuthorityRuntime =
    systemGitHubWritebackAuthorityRuntime,
): GitHubWritebackAuthorityReceipt => {
  const now = runtime.nowIso()
  const repositoryFullName = input.repository?.fullName ?? 'unknown/unknown'
  const repositoryPrivate = input.repository?.isPrivate ?? true
  const authorityMode =
    decision.decision === 'allowed' ? decision.authorityMode : null
  const blockedReason =
    decision.decision === 'blocked' ? decision.blockedReason : null
  const connectionRef =
    decision.decision === 'allowed' ? decision.connectionRef : null
  const grantRef = decision.decision === 'allowed' ? decision.grantRef : null

  return {
    id: runtime.makeReceiptId(),
    softwareOrderId: input.softwareOrderId,
    assignmentId: input.assignmentId,
    userId: input.userId,
    repositoryFullName,
    repositoryPrivate,
    requestedOperation: input.operation,
    decision: decision.decision,
    authorityMode,
    blockedReason,
    connectionRef,
    grantRef,
    approvalSource: input.approval?.source ?? null,
    approvedAt: input.approval?.approvedAt ?? null,
    customerMessage: decision.customerMessage,
    metadataJson: gitHubWritebackArtifactMetadataJson(decision),
    createdAt: now,
    updatedAt: now,
  }
}

export const insertGitHubWritebackAuthorityReceipt = async (
  db: D1Database,
  receipt: GitHubWritebackAuthorityReceipt,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO order_github_write_authority_receipts
        (id, software_order_id, assignment_id, user_id, repository_full_name,
         repository_private, requested_operation, decision, authority_mode,
         blocked_reason, connection_ref, grant_ref, approval_source,
         approved_at, customer_message, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      receipt.id,
      receipt.softwareOrderId,
      receipt.assignmentId,
      receipt.userId,
      receipt.repositoryFullName,
      receipt.repositoryPrivate ? 1 : 0,
      receipt.requestedOperation,
      receipt.decision,
      receipt.authorityMode,
      receipt.blockedReason,
      receipt.connectionRef,
      receipt.grantRef,
      receipt.approvalSource,
      receipt.approvedAt,
      receipt.customerMessage,
      receipt.metadataJson,
      receipt.createdAt,
      receipt.updatedAt,
    )
    .run()
}

export type GitHubWritebackExecutorGateInput = Readonly<{
  authorityRequest: GitHubWritebackAuthorityRequest
  blockedArtifactTitle?: string | undefined
  recordBlockedArtifact: boolean
}>

export type GitHubWritebackExecutorGateResult = Readonly<{
  blockedArtifactId: string | null
  decision: GitHubWritebackAuthorityDecision
  orderStatus: 'needs_customer_input' | 'unavailable' | null
  receipt: GitHubWritebackAuthorityReceipt
}>

const orderStatusForBlockedReason = (
  blockedReason: GitHubWritebackBlockedReason,
): 'needs_customer_input' | 'unavailable' =>
  blockedReason === 'unsupported_repository' ||
  blockedReason === 'openagents_app_not_configured'
    ? 'unavailable'
    : 'needs_customer_input'

const blockedArtifactTitleFor = (
  blockedReason: GitHubWritebackBlockedReason,
): string => {
  switch (blockedReason) {
    case 'explicit_approval_required':
      return 'GitHub writeback needs approval'
    case 'source_access_required':
    case 'github_write_connection_required':
    case 'github_write_connection_unusable':
      return 'GitHub source access is needed'
    case 'github_write_grant_required':
    case 'github_write_grant_expired':
    case 'github_write_grant_not_issued':
      return 'GitHub write approval is needed'
    case 'github_write_permission_missing':
      return 'GitHub write permissions are incomplete'
    case 'openagents_app_not_configured':
      return 'OpenAgents GitHub app writeback is not configured'
    case 'unsupported_repository':
      return 'Repository writeback is unavailable'
  }
}

const insertBlockedFulfillmentArtifact = async (
  db: D1Database,
  input: GitHubWritebackExecutorGateInput,
  decision: GitHubWritebackAuthorityBlocked,
  artifactId: string,
  now: string,
): Promise<void> => {
  const request = input.authorityRequest

  await db
    .prepare(
      `INSERT INTO order_fulfillment_artifacts
        (id, software_order_id, assignment_id, run_id, kind, title, summary,
         url, repository_full_name, source_branch, target_branch, commit_sha,
         status, visibility, metadata_json, created_by_user_id, created_at,
         updated_at, archived_at)
       VALUES (?, ?, ?, NULL, 'notes', ?, ?, NULL, ?, NULL, NULL, NULL,
         'customer_review_ready', 'public', ?, NULL, ?, ?, NULL)`,
    )
    .bind(
      artifactId,
      request.softwareOrderId,
      request.assignmentId,
      input.blockedArtifactTitle ??
        blockedArtifactTitleFor(decision.blockedReason),
      decision.customerMessage,
      request.repository?.fullName ?? null,
      gitHubWritebackArtifactMetadataJson(decision),
      now,
      now,
    )
    .run()
}

const updateSoftwareOrderForBlockedDecision = async (
  db: D1Database,
  softwareOrderId: string,
  status: 'needs_customer_input' | 'unavailable',
  now: string,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE software_orders
       SET status = ?,
           updated_at = ?
       WHERE id = ?
         AND archived_at IS NULL`,
    )
    .bind(status, now, softwareOrderId)
    .run()
}

export const recordGitHubWritebackExecutorGate = async (
  db: D1Database,
  input: GitHubWritebackExecutorGateInput,
  runtime: GitHubWritebackAuthorityRuntime =
    systemGitHubWritebackAuthorityRuntime,
): Promise<GitHubWritebackExecutorGateResult> => {
  const now = runtime.nowIso()
  const decision = resolveGitHubWritebackAuthority(
    input.authorityRequest,
    now,
  )
  const receipt = makeGitHubWritebackAuthorityReceipt(
    input.authorityRequest,
    decision,
    {
      ...runtime,
      nowIso: () => now,
    },
  )
  await insertGitHubWritebackAuthorityReceipt(db, receipt)

  if (decision.decision === 'allowed') {
    return {
      blockedArtifactId: null,
      decision,
      orderStatus: null,
      receipt,
    }
  }

  const orderStatus = orderStatusForBlockedReason(decision.blockedReason)
  await updateSoftwareOrderForBlockedDecision(
    db,
    input.authorityRequest.softwareOrderId,
    orderStatus,
    now,
  )

  if (!input.recordBlockedArtifact) {
    return {
      blockedArtifactId: null,
      decision,
      orderStatus,
      receipt,
    }
  }

  const blockedArtifactId = runtime.makeBlockedArtifactId()
  await insertBlockedFulfillmentArtifact(
    db,
    input,
    decision,
    blockedArtifactId,
    now,
  )

  return {
    blockedArtifactId,
    decision,
    orderStatus,
    receipt,
  }
}
