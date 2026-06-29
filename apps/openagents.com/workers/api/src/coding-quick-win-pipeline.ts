import { Schema as S } from 'effect'
import { buildBusinessQuickWinReceipt, BusinessQuickWinReceipt, BusinessQuickWinReceiptInvariantError } from './business-quick-win-receipt'
import { QuickWinScope } from './business-quick-win-scope'
import { CodingQuickWinProvisioning, codingQuickWinProvisionedWorktreeRef } from './coding-quick-win-provisioning'
import { CodingQuickWinRuntimeInvocation, codingQuickWinInvocationCandidatePatchRef } from './coding-quick-win-runtime-invocation'
import { CodingQuickWinDeliveryEvidence, codingQuickWinDeliveredEvidenceRef } from './coding-quick-win-delivery'
import { CodingQuickWinAcceptanceEvidence, codingQuickWinAcceptedEvidenceRef } from './coding-quick-win-acceptance'
import { BusinessQuickWinPaymentEvidence, businessQuickWinPaidEvidenceRef } from './business-quick-win-payment'

/**
 * Coding-quick-win SELF-SERVE PIPELINE ORCHESTRATOR.
 *
 * Promise business.coding_quick_win.v1 is yellow with one remaining blocker
 * after the self-serve pipeline surface:
 *   - blocker.product_promises.business_coding_quick_win_paid_receipt_missing
 *
 * This module orchestrates the deterministic step events (scope, provisioning,
 * invocation, delivery, acceptance, payment) into a single verifiable pipeline.
 * It enforces that the events link to each other correctly (e.g., the invocation
 * matches the provisioned worktree, the delivery diff matches the accepted diff)
 * and produces the final BusinessQuickWinReceipt.
 */
export type CodingQuickWinPipelineInput = Readonly<{
  scope: QuickWinScope
  provisioning?: CodingQuickWinProvisioning
  invocation?: CodingQuickWinRuntimeInvocation
  delivery?: CodingQuickWinDeliveryEvidence
  acceptance?: CodingQuickWinAcceptanceEvidence
  payment?: BusinessQuickWinPaymentEvidence
}>

export class CodingQuickWinPipelineInvariantError extends S.TaggedErrorClass<CodingQuickWinPipelineInvariantError>()(
  'CodingQuickWinPipelineInvariantError',
  { reason: S.String },
) {}

export const buildCodingQuickWinPipelineReceipt = (
  input: CodingQuickWinPipelineInput,
): BusinessQuickWinReceipt => {
  const { scope, provisioning, invocation, delivery, acceptance, payment } = input

  if (scope.offeringPromiseId !== 'business.coding_quick_win.v1') {
    throw new CodingQuickWinPipelineInvariantError({
      reason: `Pipeline expects scope for business.coding_quick_win.v1, got ${scope.offeringPromiseId}`,
    })
  }

  let deliveredEvidenceRef: string | null = null
  let outcomeAcceptedRef: string | null = null
  let buyerPaidRef: string | null = null

  if (provisioning) {
    if (provisioning.scopeRef !== scope.quickWinScopedRef) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Provisioning scopeRef does not match scope.quickWinScopedRef',
      })
    }
  }

  if (invocation) {
    if (!provisioning) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Invocation provided but provisioning is missing',
      })
    }
    const expectedWorktreeRef = codingQuickWinProvisionedWorktreeRef(provisioning)
    if (invocation.provisionedWorktreeRef !== expectedWorktreeRef) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Invocation provisionedWorktreeRef does not match provisioning worktreeRef',
      })
    }
  }

  if (delivery) {
    if (!invocation) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Delivery provided but invocation is missing',
      })
    }
    const expectedPatchRef = codingQuickWinInvocationCandidatePatchRef(invocation)
    if (delivery.diffRef !== expectedPatchRef) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Delivery diffRef does not match invocation candidatePatchRef',
      })
    }
    deliveredEvidenceRef = codingQuickWinDeliveredEvidenceRef(delivery)
  }

  if (acceptance) {
    if (!delivery) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Acceptance provided but delivery is missing',
      })
    }
    if (acceptance.diffRef !== delivery.diffRef) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Acceptance diffRef does not match delivery diffRef',
      })
    }
    outcomeAcceptedRef = codingQuickWinAcceptedEvidenceRef(acceptance)
  }

  if (payment) {
    if (!acceptance) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: 'Payment provided but acceptance is missing',
      })
    }
    buyerPaidRef = businessQuickWinPaidEvidenceRef(payment)
  }

  // Build the underlying generic receipt
  try {
    return buildBusinessQuickWinReceipt({
      signupId: scope.signupId,
      offeringPromiseId: scope.offeringPromiseId,
      quickWinSummary: scope.requestedHelp || scope.category,
      quickWinScopedRef: scope.quickWinScopedRef,
      deliveredEvidenceRef,
      outcomeAcceptedRef,
      buyerPaidRef,
      providerSettledRef: null, // No provider settlement yet
    })
  } catch (err) {
    if (err instanceof BusinessQuickWinReceiptInvariantError) {
      throw new CodingQuickWinPipelineInvariantError({
        reason: `Receipt invariant failed: ${err.reason}`,
      })
    }
    throw err
  }
}
