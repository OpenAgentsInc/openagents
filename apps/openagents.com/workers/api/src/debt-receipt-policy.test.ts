import { describe, expect, test } from 'vitest'

import {
  DebtReceiptPolicyUnsafe,
  debtReceiptSettlementHasPrivateMaterial,
  projectDebtReceiptSettlement,
} from './debt-receipt-policy'

const definedReceiptInput = {
  baselineMetricRefs: [
    'metric.public.debt_receipt.5334.baseline.dual_format_lines_460k',
  ],
  budgetCapSats: 100_000,
  scopeRefs: ['scope.public.debt_receipt.5334.tassadar_fixture_pairs'],
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5334'],
  stopConditionRefs: [
    'stop.public.debt_receipt.5334.retire_once_after_dedup',
  ],
  targetMetricRefs: [
    'metric.public.debt_receipt.5334.target.committed_generated_churn_0',
  ],
}

const fundedReceiptInput = {
  ...definedReceiptInput,
  fundingApprovalRefs: ['approval.public.debt_receipt.5334.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewerActorRef: 'actor.public.reviewer.trigger',
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  workerActorRef: 'actor.public.worker.codex_loop',
}

const verifiedReceiptInput = {
  ...fundedReceiptInput,
  acceptedWorkRefs: ['accepted_work.public.debt_receipt.5334.fixture_dedup'],
  hygieneDeltaRefs: ['delta.public.debt_receipt.5334.dual_format_removed'],
  noNewEqualOrWorseDebtRefs: [
    'check.public.debt_receipt.5334.no_equal_or_worse_debt',
  ],
  reviewDecisionRefs: ['review.public.debt_receipt.5334.accepted'],
  verificationCommandRefs: [
    'command.public.debt_receipt.5334.regenerate_and_diff',
  ],
}

describe('Debt receipt settlement policy', () => {
  test('treats discovered debt as fundable inventory, not spend or payout authority', () => {
    const projection = projectDebtReceiptSettlement(definedReceiptInput)

    expect(projection).toMatchObject({
      duplicateReplay: false,
      settlementClaimAllowed: false,
      spendAuthorityDelegatedToWorker: false,
      state: 'fundable',
      workerPayoutEligible: false,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.debt_receipt.funding_approval_missing',
    )
    expect(projection.caveatRefs).toContain(
      'caveat.public.debt_receipt.discovery_is_not_spend',
    )
  })

  test('requires the funding, worker, reviewer, and settlement roles to stay split', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      reviewerActorRef: 'actor.public.worker.codex_loop',
      settlementApprovalRefs: [
        'approval.public.debt_receipt.5334.settlement',
      ],
      payableSats: 50_000,
    })

    expect(projection).toMatchObject({
      settlementClaimAllowed: false,
      state: 'fundable',
      workerPayoutEligible: false,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.debt_receipt.role_overlap.reviewer_worker',
    )
  })

  test('makes verified delta payable without upgrading it into settled copy', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: [
        'approval.public.debt_receipt.5334.settlement',
      ],
    })

    expect(projection).toMatchObject({
      payableSats: 50_000,
      publicCopyRefs: [
        'copy.public.debt_receipt.payable_pending_settlement',
      ],
      settledSats: 0,
      settlementClaimAllowed: false,
      state: 'payable',
      workerPayoutEligible: true,
    })
    expect(projection.blockerRefs).toEqual([
      'blocker.public.debt_receipt.settlement_receipt_missing',
    ])
  })

  test('retires a receipt once settlement evidence exactly matches the payable amount', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: [
        'approval.public.debt_receipt.5334.settlement',
      ],
      settlementReceiptRefs: [
        'settlement.public.debt_receipt.5334.worker_codex_loop',
      ],
      settledSats: 50_000,
    })

    expect(projection).toMatchObject({
      publicCopyRefs: [
        'copy.public.debt_receipt.retired_with_settlement_receipt',
      ],
      settlementClaimAllowed: true,
      state: 'retired',
      workerPayoutEligible: true,
    })
  })

  test('blocks duplicate replay against an already retired receipt fingerprint', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      duplicateFingerprintRefs: [
        'fingerprint.public.debt_receipt.5334.scope_patch_digest_v1',
      ],
      payableSats: 50_000,
      retiredReceiptRefs: ['receipt.public.debt_receipt.5334.retired'],
      settlementApprovalRefs: [
        'approval.public.debt_receipt.5334.settlement',
      ],
    })

    expect(projection).toMatchObject({
      duplicateReplay: true,
      settlementClaimAllowed: false,
      state: 'duplicate_replay',
      workerPayoutEligible: false,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.debt_receipt.duplicate_replay',
    )
  })

  test('quarantines churn loops after repeated rejected or revision-required attempts', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      maxRevisionAttempts: 3,
      payableSats: 50_000,
      revisionAttemptCount: 3,
      settlementApprovalRefs: [
        'approval.public.debt_receipt.5334.settlement',
      ],
    })

    expect(projection).toMatchObject({
      manualReviewOnly: true,
      quarantineReasonRefs: [
        'quarantine.public.debt_receipt.revision_attempt_limit_reached',
      ],
      state: 'quarantined',
      workerPayoutEligible: false,
    })
    expect(projection.blockerRefs).toContain(
      'blocker.public.debt_receipt.manual_review_only',
    )
  })

  test('rejects impossible payment amounts and private material', () => {
    expect(() =>
      projectDebtReceiptSettlement({
        ...definedReceiptInput,
        payableSats: 100_001,
      }),
    ).toThrow(DebtReceiptPolicyUnsafe)

    expect(() =>
      projectDebtReceiptSettlement({
        ...definedReceiptInput,
        baselineMetricRefs: ['raw_diff.private_fixture_dump'],
      }),
    ).toThrow(DebtReceiptPolicyUnsafe)
  })

  test('keeps retired public projections free of private material', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: [
        'approval.public.debt_receipt.5334.settlement',
      ],
      settlementReceiptRefs: [
        'settlement.public.debt_receipt.5334.worker_codex_loop',
      ],
      settledSats: 50_000,
    })

    expect(debtReceiptSettlementHasPrivateMaterial(projection)).toBe(false)
    expect(JSON.stringify(projection)).not.toMatch(
      /raw_diff|private_fixture|provider_payload|customer_email|wallet|preimage|lnbc|@|github\.com\/[^:/]+\/private/i,
    )
  })
})
