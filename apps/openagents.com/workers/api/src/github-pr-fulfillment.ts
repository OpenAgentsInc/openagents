import {
  gitHubWritebackArtifactMetadataJson,
  recordGitHubWritebackExecutorGate,
  type GitHubWritebackApproval,
  type GitHubWritebackAuthorityRequest,
  type GitHubWritebackConnectionAuthority,
  type GitHubWritebackExecutorGateResult,
  type GitHubWritebackGrantAuthority,
  type GitHubWritebackRepositoryRef,
} from './github-writeback-authority'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export type PublicForkPullRequestReceipt = Readonly<{
  commitSha: string | null
  forkFullName: string
  prNumber: number | null
  prUrl: string
  sourceBranch: string
  targetBranch: string
  testsSummary: string | null
}>

export type PublicForkPullRequestFulfillmentInput = Readonly<{
  approval: GitHubWritebackApproval | null
  assignmentId: string | null
  pullRequest: PublicForkPullRequestReceipt
  repository: GitHubWritebackRepositoryRef
  softwareOrderId: string
  summary: string
  title: string
  userId: string
}>

export type CustomerGrantPullRequestFulfillmentInput = Readonly<{
  approval: GitHubWritebackApproval | null
  assignmentId: string | null
  connection: GitHubWritebackConnectionAuthority | null
  grant: GitHubWritebackGrantAuthority | null
  pullRequest: PublicForkPullRequestReceipt
  repository: GitHubWritebackRepositoryRef
  softwareOrderId: string
  summary: string
  title: string
  userId: string
}>

export type PublicForkPullRequestFulfillmentRuntime = Readonly<{
  makeArtifactId: () => string
  nowIso: () => string
}>

export const systemPublicForkPullRequestFulfillmentRuntime: PublicForkPullRequestFulfillmentRuntime =
  {
    makeArtifactId: () => compactRandomId('fulfillment_artifact'),
    nowIso: currentIsoTimestamp,
  }

export type PublicForkPullRequestFulfillmentResult = Readonly<{
  artifactId: string | null
  gate: GitHubWritebackExecutorGateResult
}>

export type CustomerGrantPullRequestFulfillmentResult =
  PublicForkPullRequestFulfillmentResult

type PullRequestFulfillmentInput =
  | PublicForkPullRequestFulfillmentInput
  | CustomerGrantPullRequestFulfillmentInput

const clampText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const publicForkAuthorityRequest = (
  input: PublicForkPullRequestFulfillmentInput,
): GitHubWritebackAuthorityRequest => ({
  approval: input.approval,
  assignmentId: input.assignmentId,
  connection: null,
  grant: null,
  operation: 'open_fork_pull_request',
  repository: input.repository,
  softwareOrderId: input.softwareOrderId,
  userId: input.userId,
})

const customerGrantAuthorityRequest = (
  input: CustomerGrantPullRequestFulfillmentInput,
): GitHubWritebackAuthorityRequest => ({
  approval: input.approval,
  assignmentId: input.assignmentId,
  connection: input.connection,
  grant: input.grant,
  operation: 'open_pull_request',
  repository: input.repository,
  softwareOrderId: input.softwareOrderId,
  userId: input.userId,
})

const publicForkPullRequestMetadataJson = (
  gate: GitHubWritebackExecutorGateResult,
  pullRequest: PublicForkPullRequestReceipt,
): string =>
  JSON.stringify({
    githubPullRequest: {
      forkFullName: pullRequest.forkFullName,
      prNumber: pullRequest.prNumber,
      prUrl: pullRequest.prUrl,
      testsSummary: pullRequest.testsSummary,
    },
    githubWritebackAuthority: gate.decision.metadata,
    githubWritebackAuthorityReceiptId: gate.receipt.id,
  })

const insertPullRequestFulfillmentArtifact = async (
  db: D1Database,
  input: PullRequestFulfillmentInput,
  gate: GitHubWritebackExecutorGateResult,
  artifactId: string,
  now: string,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO order_fulfillment_artifacts
        (id, software_order_id, assignment_id, run_id, kind, title, summary,
         url, repository_full_name, source_branch, target_branch, commit_sha,
         status, visibility, metadata_json, created_by_user_id, created_at,
         updated_at, archived_at)
       VALUES (?, ?, ?, NULL, 'pull_request', ?, ?, ?, ?, ?, ?, ?,
         'customer_review_ready', 'public', ?, NULL, ?, ?, NULL)`,
    )
    .bind(
      artifactId,
      input.softwareOrderId,
      input.assignmentId,
      clampText(input.title, 240),
      clampText(input.summary, 1200),
      input.pullRequest.prUrl,
      input.repository.fullName,
      input.pullRequest.sourceBranch,
      input.pullRequest.targetBranch,
      input.pullRequest.commitSha,
      publicForkPullRequestMetadataJson(gate, input.pullRequest),
      now,
      now,
    )
    .run()
}

const markOrderDelivered = async (
  db: D1Database,
  softwareOrderId: string,
  now: string,
): Promise<void> => {
  await db
    .prepare(
      `UPDATE software_orders
       SET status = 'delivered',
           updated_at = ?
       WHERE id = ?
         AND archived_at IS NULL`,
    )
    .bind(now, softwareOrderId)
    .run()
}

export const recordPublicForkPullRequestFulfillment = async (
  db: D1Database,
  input: PublicForkPullRequestFulfillmentInput,
  runtime: PublicForkPullRequestFulfillmentRuntime =
    systemPublicForkPullRequestFulfillmentRuntime,
): Promise<PublicForkPullRequestFulfillmentResult> => {
  const now = runtime.nowIso()
  const gate = await recordGitHubWritebackExecutorGate(
    db,
    {
      authorityRequest: publicForkAuthorityRequest(input),
      recordBlockedArtifact: true,
    },
    {
      makeBlockedArtifactId: () => runtime.makeArtifactId(),
      makeReceiptId: () =>
        compactRandomId('github_writeback_authority_receipt'),
      nowIso: () => now,
    },
  )

  if (gate.decision.decision === 'blocked') {
    return {
      artifactId: null,
      gate,
    }
  }

  const artifactId = runtime.makeArtifactId()
  await insertPullRequestFulfillmentArtifact(db, input, gate, artifactId, now)
  await markOrderDelivered(db, input.softwareOrderId, now)

  return {
    artifactId,
    gate,
  }
}

export const recordCustomerGrantPullRequestFulfillment = async (
  db: D1Database,
  input: CustomerGrantPullRequestFulfillmentInput,
  runtime: PublicForkPullRequestFulfillmentRuntime =
    systemPublicForkPullRequestFulfillmentRuntime,
): Promise<CustomerGrantPullRequestFulfillmentResult> => {
  const now = runtime.nowIso()
  const gate = await recordGitHubWritebackExecutorGate(
    db,
    {
      authorityRequest: customerGrantAuthorityRequest(input),
      recordBlockedArtifact: true,
    },
    {
      makeBlockedArtifactId: () => runtime.makeArtifactId(),
      makeReceiptId: () =>
        compactRandomId('github_writeback_authority_receipt'),
      nowIso: () => now,
    },
  )

  if (gate.decision.decision === 'blocked') {
    return {
      artifactId: null,
      gate,
    }
  }

  const artifactId = runtime.makeArtifactId()
  await insertPullRequestFulfillmentArtifact(db, input, gate, artifactId, now)
  await markOrderDelivered(db, input.softwareOrderId, now)

  return {
    artifactId,
    gate,
  }
}

export const publicForkPullRequestArtifactMetadataJson = (
  gate: GitHubWritebackExecutorGateResult,
  pullRequest: PublicForkPullRequestReceipt,
): string => publicForkPullRequestMetadataJson(gate, pullRequest)

export { gitHubWritebackArtifactMetadataJson }
