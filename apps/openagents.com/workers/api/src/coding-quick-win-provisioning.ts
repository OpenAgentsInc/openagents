import { Schema as S } from 'effect'

/**
 * Coding-quick-win REPO PROVISIONING contract.
 *
 * Promise business.coding_quick_win.v1 is yellow with two open blockers:
 *   - blocker.product_promises.business_coding_quick_win_self_serve_missing
 *   - blocker.product_promises.business_coding_quick_win_paid_receipt_missing
 *
 * This module tackles the "repo provisioning" step of the self-serve loop
 * (automated intake -> repo provisioning -> runtime invocation -> ...). Before
 * the coding agent runtime can be invoked, it needs a verifiable, isolated
 * working directory cloned from the customer's repository.
 *
 * This module defines the structure and machine-checkable gates for a successful
 * repo provisioning event.
 *
 * Honesty rules, enforced by construction:
 * - A repo is only `provisioned` if a verifiable base commit SHA is recorded. 
 *   You cannot provision against a floating branch head; you must lock the SHA.
 * - The provisioning must be tied to a specific quick-win scope (the intent).
 * - Only a `provisioned` state can produce a valid `provisionedWorktreeRef` 
 *   for the runtime invocation step.
 */

export const RepoProvisioningStatus = S.Literals([
  'pending_clone',
  'provisioned',
  'failed_to_clone',
])
export type RepoProvisioningStatus = typeof RepoProvisioningStatus.Type

export const CodingQuickWinProvisioning = S.Struct({
  eventKind: S.Literal('coding_quick_win_provisioning'),
  
  // The business_quick_win_scope this provisioning fulfills.
  scopeRef: S.String,
  
  // Customer's repository URL (e.g. https://github.com/org/repo)
  repositoryUrl: S.String,
  
  // The branch the customer requested (e.g. main)
  requestedBranch: S.String,
  
  status: RepoProvisioningStatus,
  
  // The exact, locked commit SHA the clone was resolved to.
  // Must be populated if status is 'provisioned'.
  baseCommitSha: S.NullOr(S.String),
  
  // The isolated sandbox identifier or worktree path where the runtime
  // should mount and execute. Must be populated if status is 'provisioned'.
  worktreeRef: S.NullOr(S.String),
  
  // Human-readable error if failed.
  failureReason: S.NullOr(S.String),
})
export type CodingQuickWinProvisioning = typeof CodingQuickWinProvisioning.Type

export class CodingQuickWinProvisioningInvariantError extends S.TaggedErrorClass<CodingQuickWinProvisioningInvariantError>()(
  'CodingQuickWinProvisioningInvariantError',
  { reason: S.String }
) {
  override get message() {
    return this.reason
  }
}

/**
 * Builds a validated provisioning event.
 * Enforces that a successful clone has a locked SHA and a reachable worktree,
 * and a failed clone has a reason.
 */
export const buildCodingQuickWinProvisioning = (
  input: Omit<CodingQuickWinProvisioning, 'eventKind'>,
): CodingQuickWinProvisioning => {
  if (input.scopeRef.trim() === '') {
    throw new CodingQuickWinProvisioningInvariantError({
      reason: 'scopeRef cannot be empty.',
    })
  }
  if (input.repositoryUrl.trim() === '') {
    throw new CodingQuickWinProvisioningInvariantError({
      reason: 'repositoryUrl cannot be empty.',
    })
  }
  if (input.requestedBranch.trim() === '') {
    throw new CodingQuickWinProvisioningInvariantError({
      reason: 'requestedBranch cannot be empty.',
    })
  }

  if (input.status === 'provisioned') {
    if (!input.baseCommitSha || input.baseCommitSha.trim() === '') {
      throw new CodingQuickWinProvisioningInvariantError({
        reason: 'a provisioned repo must lock a baseCommitSha.',
      })
    }
    if (!input.worktreeRef || input.worktreeRef.trim() === '') {
      throw new CodingQuickWinProvisioningInvariantError({
        reason: 'a provisioned repo must expose a worktreeRef.',
      })
    }
  }

  if (input.status === 'failed_to_clone') {
    if (!input.failureReason || input.failureReason.trim() === '') {
      throw new CodingQuickWinProvisioningInvariantError({
        reason: 'a failed clone must include a failureReason.',
      })
    }
  }

  return {
    ...input,
    eventKind: 'coding_quick_win_provisioning',
  }
}

/**
 * Gate for runtime invocation: throws unless the repository is fully provisioned.
 * This is the check the self-serve loop must pass before spinning up the agent.
 */
export const assertCodingQuickWinProvisioned = (
  provisioning: CodingQuickWinProvisioning,
): void => {
  if (provisioning.status !== 'provisioned') {
    throw new CodingQuickWinProvisioningInvariantError({
      reason: `repository is not ready for runtime: status is ${provisioning.status}, not provisioned.`,
    })
  }
}

/**
 * Produce the stable `provisionedWorktreeRef` string to feed the runtime invocation.
 * Returns the worktree reference ONLY for a provisioned repository; throws otherwise.
 */
export const codingQuickWinProvisionedWorktreeRef = (
  provisioning: CodingQuickWinProvisioning,
): string => {
  assertCodingQuickWinProvisioned(provisioning)
  // We can safely assert non-null because buildCodingQuickWinProvisioning ensures it.
  return provisioning.worktreeRef as string
}

/**
 * Public projection: surfaces the repository and branch targets and the clone status,
 * without exposing internal sandbox references.
 */
export const publicCodingQuickWinProvisioningProjection = (
  provisioning: CodingQuickWinProvisioning,
) => ({
  eventKind: provisioning.eventKind,
  repositoryUrl: provisioning.repositoryUrl,
  requestedBranch: provisioning.requestedBranch,
  status: provisioning.status,
  baseCommitSha: provisioning.baseCommitSha,
})
