import { Schema as S } from 'effect'
import { CodingQuickWinProvisioning, codingQuickWinProvisionedWorktreeRef } from './coding-quick-win-provisioning'

/**
 * Coding-quick-win RUNTIME INVOCATION contract.
 *
 * Promise business.coding_quick_win.v1 is yellow with two open blockers:
 *   - blocker.product_promises.business_coding_quick_win_self_serve_missing
 *   - blocker.product_promises.business_coding_quick_win_paid_receipt_missing
 *
 * This module tackles the "runtime invocation" step of the self-serve loop.
 * After a repository is provisioned into an isolated worktree, the OpenAgents 
 * coding runtime (Probe/Pylon CLI) must be invoked against that worktree with
 * the negotiated objective.
 *
 * This module defines the verifiable structure for a runtime invocation event.
 *
 * Honesty rules, enforced by construction:
 * - An invocation must be bound to a successfully provisioned worktree.
 * - An invocation must record the agent runtime used (e.g. 'pylon_claude_bridge').
 * - A completed invocation must expose an output log reference.
 * - Only a 'completed' run with a captured patch can proceed to the verification
 *   and delivery step.
 */

export const RuntimeInvocationStatus = S.Literals([
  'running',
  'completed',
  'failed',
])
export type RuntimeInvocationStatus = typeof RuntimeInvocationStatus.Type

export const CodingQuickWinRuntimeInvocation = S.Struct({
  eventKind: S.Literal('coding_quick_win_runtime_invocation'),
  
  // The business_quick_win_scope this invocation attempts to fulfill.
  scopeRef: S.String,
  
  // The isolated sandbox identifier or worktree path where the runtime runs.
  // Extracted from a verified provisioning event.
  provisionedWorktreeRef: S.String,
  
  // The identifier of the agent runtime used (e.g., 'pylon_claude_bridge').
  runtimeAgentId: S.String,
  
  status: RuntimeInvocationStatus,
  
  // A dereferenceable reference to the complete runtime execution log.
  // Required if the status is 'completed' or 'failed'.
  executionLogRef: S.NullOr(S.String),
  
  // The output patch/diff reference if the agent produced code.
  // Can be null even if completed, if the agent decided no change was needed,
  // but a patch is required to proceed to delivery.
  candidatePatchRef: S.NullOr(S.String),
  
  // Human-readable error if the runtime crashed or failed to start.
  failureReason: S.NullOr(S.String),
})
export type CodingQuickWinRuntimeInvocation = typeof CodingQuickWinRuntimeInvocation.Type

export class CodingQuickWinRuntimeInvocationInvariantError extends S.TaggedErrorClass<CodingQuickWinRuntimeInvocationInvariantError>()(
  'CodingQuickWinRuntimeInvocationInvariantError',
  { reason: S.String }
) {
  override get message() {
    return this.reason
  }
}

export type RuntimeInvocationInput = Readonly<{
  scopeRef: string
  // The prior provisioning event, ensuring we only invoke against a ready repo.
  provisioning: CodingQuickWinProvisioning
  runtimeAgentId: string
  status: RuntimeInvocationStatus
  executionLogRef?: string | null
  candidatePatchRef?: string | null
  failureReason?: string | null
}>

/**
 * Builds a validated runtime invocation event.
 * Enforces that a completed/failed run has a log, and extracts the worktree
 * securely from the provisioning event.
 */
export const buildCodingQuickWinRuntimeInvocation = (
  input: RuntimeInvocationInput,
): CodingQuickWinRuntimeInvocation => {
  if (input.scopeRef.trim() === '') {
    throw new CodingQuickWinRuntimeInvocationInvariantError({
      reason: 'scopeRef cannot be empty.',
    })
  }

  if (input.runtimeAgentId.trim() === '') {
    throw new CodingQuickWinRuntimeInvocationInvariantError({
      reason: 'runtimeAgentId cannot be empty.',
    })
  }

  // Enforce that the repository was actually provisioned before we run.
  // This throws its own invariant error if not provisioned.
  const provisionedWorktreeRef = codingQuickWinProvisionedWorktreeRef(input.provisioning)

  if (input.status === 'completed' || input.status === 'failed') {
    if (!input.executionLogRef || input.executionLogRef.trim() === '') {
      throw new CodingQuickWinRuntimeInvocationInvariantError({
        reason: `a ${input.status} invocation must expose an executionLogRef.`,
      })
    }
  }

  if (input.status === 'failed') {
    if (!input.failureReason || input.failureReason.trim() === '') {
      throw new CodingQuickWinRuntimeInvocationInvariantError({
        reason: 'a failed invocation must include a failureReason.',
      })
    }
  }

  const executionLogRef = input.executionLogRef?.trim() || null
  const candidatePatchRef = input.candidatePatchRef?.trim() || null
  const failureReason = input.failureReason?.trim() || null

  return {
    eventKind: 'coding_quick_win_runtime_invocation',
    scopeRef: input.scopeRef,
    provisionedWorktreeRef,
    runtimeAgentId: input.runtimeAgentId,
    status: input.status,
    executionLogRef,
    candidatePatchRef,
    failureReason,
  }
}

/**
 * Gate for proceeding to the verification and delivery step: throws unless
 * the runtime completed and successfully generated a candidate patch.
 */
export const assertCodingQuickWinInvocationHasPatch = (
  invocation: CodingQuickWinRuntimeInvocation,
): void => {
  if (invocation.status !== 'completed') {
    throw new CodingQuickWinRuntimeInvocationInvariantError({
      reason: `runtime is not completed: status is ${invocation.status}.`,
    })
  }
  if (!invocation.candidatePatchRef) {
    throw new CodingQuickWinRuntimeInvocationInvariantError({
      reason: 'runtime completed but did not produce a candidatePatchRef.',
    })
  }
}

/**
 * Produce the stable `candidatePatchRef` string to feed the delivery verification.
 * Returns the patch reference ONLY for a successfully completed run with a patch;
 * throws otherwise.
 */
export const codingQuickWinInvocationCandidatePatchRef = (
  invocation: CodingQuickWinRuntimeInvocation,
): string => {
  assertCodingQuickWinInvocationHasPatch(invocation)
  return invocation.candidatePatchRef as string
}

/**
 * Public projection: surfaces the runtime identity and status without exposing
 * internal worktree paths.
 */
export const publicCodingQuickWinRuntimeInvocationProjection = (
  invocation: CodingQuickWinRuntimeInvocation,
) => ({
  eventKind: invocation.eventKind,
  runtimeAgentId: invocation.runtimeAgentId,
  status: invocation.status,
  hasCandidatePatch: invocation.candidatePatchRef !== null,
})
