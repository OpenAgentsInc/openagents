import { describe, expect, it } from 'vitest'
import {
  buildCodingQuickWinPipelineReceipt,
  CodingQuickWinPipelineInvariantError,
} from './coding-quick-win-pipeline'
import { scopeQuickWinFromIntake } from './business-quick-win-scope'
import { buildCodingQuickWinProvisioning } from './coding-quick-win-provisioning'
import { buildCodingQuickWinRuntimeInvocation } from './coding-quick-win-runtime-invocation'
import { buildCodingQuickWinDeliveryEvidence } from './coding-quick-win-delivery'
import { buildCodingQuickWinAcceptanceEvidence } from './coding-quick-win-acceptance'
import { buildBusinessQuickWinPaymentEvidence } from './business-quick-win-payment'

describe('coding-quick-win-pipeline', () => {
  const scope = scopeQuickWinFromIntake({
    signupId: 'signup_123',
    helpWith: 'fix the bug in the code', // matches coding_agent_work -> business.coding_quick_win.v1
  })

  const provisioning = buildCodingQuickWinProvisioning({
    scopeRef: scope.quickWinScopedRef,
    repositoryUrl: 'https://github.com/org/repo',
    requestedBranch: 'main',
    status: 'provisioned',
    baseCommitSha: 'sha123',
    worktreeRef: 'worktree_456',
    failureReason: null,
  })

  const invocation = buildCodingQuickWinRuntimeInvocation({
    scopeRef: scope.quickWinScopedRef,
    provisioning,
    runtimeAgentId: 'pylon_claude_bridge',
    status: 'completed',
    executionLogRef: 'log_789',
    candidatePatchRef: 'patch_abc',
    failureReason: null,
  })

  const delivery = buildCodingQuickWinDeliveryEvidence({
    repo: 'org/repo',
    baseRef: 'sha123',
    verificationCommand: 'npm run test',
    verificationExitCode: 0,
    verificationOutputRef: 'log_def',
    diffRef: 'patch_abc',
  })

  const acceptance = buildCodingQuickWinAcceptanceEvidence({
    diffRef: 'patch_abc',
    acceptedByUserId: 'user_123',
    acceptanceAction: 'diff_approved',
    attestationRef: 'pr_review_123',
  })

  const payment = buildBusinessQuickWinPaymentEvidence({
    signupId: 'signup_123',
    amount: 10000,
    currency: 'USD',
    paymentStatus: 'settled',
    paymentRef: 'stripe_ch_123',
  })

  it('builds a receipt for a fully progressed pipeline', () => {
    const receipt = buildCodingQuickWinPipelineReceipt({
      scope,
      provisioning,
      invocation,
      delivery,
      acceptance,
      payment,
    })

    expect(receipt.receiptKind).toBe('business_quick_win')
    expect(receipt.offeringPromiseId).toBe('business.coding_quick_win.v1')
    expect(receipt.paidQuickWin).toBe(true)
    expect(receipt.evidencedStateCount).toBe(5) // everything except provider_settled
  })

  it('builds a partial receipt when pipeline has only progressed to invocation', () => {
    // invocation is provided but delivery is missing
    const receipt = buildCodingQuickWinPipelineReceipt({
      scope,
      provisioning,
      invocation,
    })

    expect(receipt.paidQuickWin).toBe(false)
    expect(receipt.evidencedStateCount).toBe(2) // intake_recorded, quick_win_scoped
    const deliveredLine = receipt.lines.find(l => l.stateId === 'delivered_with_evidence')
    expect(deliveredLine?.evidenceState).toBe('not_yet_evidenced')
  })

  it('throws if scope is for a different offering', () => {
    const otherScope = scopeQuickWinFromIntake({
      signupId: 'signup_123',
      helpWith: 'I need a landing page', // matches sites_commerce -> autopilot_sites.site_build_and_host.v1
    })

    expect(() =>
      buildCodingQuickWinPipelineReceipt({ scope: otherScope })
    ).toThrowError(CodingQuickWinPipelineInvariantError)
  })

  it('throws if provisioning scopeRef does not match', () => {
    const badProvisioning = buildCodingQuickWinProvisioning({
      ...provisioning,
      scopeRef: 'some_other_ref',
    })

    expect(() =>
      buildCodingQuickWinPipelineReceipt({
        scope,
        provisioning: badProvisioning,
      })
    ).toThrowError(CodingQuickWinPipelineInvariantError)
  })

  it('throws if invocation worktree does not match provisioning', () => {
    const otherProvisioning = buildCodingQuickWinProvisioning({
      ...provisioning,
      worktreeRef: 'worktree_999',
    })

    expect(() =>
      buildCodingQuickWinPipelineReceipt({
        scope,
        provisioning: otherProvisioning, // has worktree_999
        invocation, // has worktree_456
      })
    ).toThrowError(CodingQuickWinPipelineInvariantError)
  })

  it('throws if delivery diffRef does not match invocation candidatePatchRef', () => {
    const badDelivery = buildCodingQuickWinDeliveryEvidence({
      ...delivery,
      diffRef: 'patch_xyz',
    })

    expect(() =>
      buildCodingQuickWinPipelineReceipt({
        scope,
        provisioning,
        invocation,
        delivery: badDelivery,
      })
    ).toThrowError(CodingQuickWinPipelineInvariantError)
  })

  it('throws if acceptance diffRef does not match delivery diffRef', () => {
    const badAcceptance = buildCodingQuickWinAcceptanceEvidence({
      ...acceptance,
      diffRef: 'patch_xyz',
    })

    expect(() =>
      buildCodingQuickWinPipelineReceipt({
        scope,
        provisioning,
        invocation,
        delivery,
        acceptance: badAcceptance,
      })
    ).toThrowError(CodingQuickWinPipelineInvariantError)
  })
})
