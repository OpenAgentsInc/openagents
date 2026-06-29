import { describe, expect, test } from 'vitest'

import {
  deriveDebtReceiptKey,
  derivePatchNoveltyKey,
} from './debt-receipt-key'
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
  stopConditionRefs: ['stop.public.debt_receipt.5334.retire_once_after_dedup'],
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

const studiedKnowledgeSource = {
  correctnessGatePassed: true,
  graphRef: 'openagents_repo_studied_knowledge_graph.5335',
  packetRef: 'openagents_repo_study_packet.5335',
  rejectedCount: 0,
  schemaRef: 'openagents.repo_studied_knowledge_verification.v0' as const,
  sourceBoundary: 'public_refs_only' as const,
  validatorReviewRefs: [],
  validatorReviewRequired: false,
  verificationRef: 'openagents_repo_studied_knowledge_verification.5335',
}

const debtReceiptKeyInput = {
  debtReceiptRef: 'receipt.public.debt.5334',
  objectiveDigest: 'objective.public.debt_receipt.5334.dual_format_to_zero',
  repoBaselineRef: 'baseline.public.commit.c43992567',
  scopeDigest: 'scope.public.debt_receipt.5334.tassadar_fixture_pairs',
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
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
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
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
    })

    expect(projection).toMatchObject({
      payableSats: 50_000,
      publicCopyRefs: ['copy.public.debt_receipt.payable_pending_settlement'],
      settledSats: 0,
      settlementClaimAllowed: false,
      state: 'payable',
      workClass: 'code_hygiene',
      workerPayoutEligible: true,
    })
    expect(projection.blockerRefs).toEqual([
      'blocker.public.debt_receipt.settlement_receipt_missing',
    ])
  })

  test('classifies documentation and journal work as credit instead of payable', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
      settlementReceiptRefs: [
        'settlement.public.debt_receipt.5334.worker_codex_loop',
      ],
      settledSats: 0,
      workClass: 'documentation_or_journal',
    })

    expect(projection).toMatchObject({
      payableSats: 0,
      publicCopyRefs: ['copy.public.debt_receipt.credit_class_not_payable'],
      settlementClaimAllowed: false,
      state: 'credit_class',
      workClass: 'documentation_or_journal',
      workerPayoutEligible: false,
    })
    expect(projection.blockerRefs).toEqual([
      'blocker.public.debt_receipt.documentation_or_journal_credit_not_payable',
    ])
    expect(projection.caveatRefs).toContain(
      'caveat.public.debt_receipt.documentation_or_journal_not_size_scaled',
    )
  })

  test('uses studied knowledge as evidence for hygiene receipts without granting authority', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5335.settlement'],
      studiedKnowledgeRequired: true,
      studiedKnowledgeSource,
    })

    expect(projection).toMatchObject({
      spendAuthorityDelegatedToWorker: false,
      state: 'payable',
      studiedKnowledgeGatePassed: true,
      studiedKnowledgeRequired: true,
      workerPayoutEligible: true,
    })
    expect(projection.studiedKnowledgeSourceRefs).toEqual([
      'openagents_repo_studied_knowledge_graph.5335',
      'openagents_repo_studied_knowledge_verification.5335',
      'openagents_repo_study_packet.5335',
    ])
  })

  test('blocks payable state when required studied knowledge is missing or rejected', () => {
    const missing = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5335.settlement'],
      studiedKnowledgeRequired: true,
    })

    expect(missing).toMatchObject({
      state: 'funded',
      studiedKnowledgeGatePassed: false,
      workerPayoutEligible: false,
    })
    expect(missing.blockerRefs).toContain(
      'blocker.public.debt_receipt.studied_knowledge_source_missing',
    )

    const rejected = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5335.settlement'],
      studiedKnowledgeRequired: true,
      studiedKnowledgeSource: {
        ...studiedKnowledgeSource,
        correctnessGatePassed: false,
        rejectedCount: 1,
        verificationRef:
          'openagents_repo_studied_knowledge_verification.5335_rejected',
      },
    })

    expect(rejected).toMatchObject({
      state: 'funded',
      studiedKnowledgeGatePassed: false,
      workerPayoutEligible: false,
    })
    expect(rejected.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.debt_receipt.studied_knowledge_correctness_failed',
        'blocker.public.debt_receipt.studied_knowledge_rejected_claims',
      ]),
    )
  })

  // Optional-gate fail-closed (reviewer-flagged #5344 inconsistency): bad
  // OPTIONAL studied-knowledge evidence must not leave the contribution payable
  // while attaching blockers.
  test('fails closed when optional studied knowledge evidence is invalid', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5335.settlement'],
      // not required, but a bad source is attached anyway
      studiedKnowledgeRequired: false,
      studiedKnowledgeSource: {
        ...studiedKnowledgeSource,
        correctnessGatePassed: false,
        rejectedCount: 2,
      },
    })

    expect(projection).toMatchObject({
      state: 'funded',
      studiedKnowledgeGatePassed: false,
      studiedKnowledgeRequired: false,
      workerPayoutEligible: false,
    })
    expect(projection.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.public.debt_receipt.studied_knowledge_correctness_failed',
        'blocker.public.debt_receipt.studied_knowledge_rejected_claims',
      ]),
    )
  })

  test('fails closed when optional studied knowledge needs validator review', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5335.settlement'],
      studiedKnowledgeRequired: false,
      studiedKnowledgeSource: {
        ...studiedKnowledgeSource,
        validatorReviewRefs: [
          'validator_review.public.study_verification.remainder',
        ],
        validatorReviewRequired: true,
      },
    })

    expect(projection.studiedKnowledgeGatePassed).toBe(false)
    expect(projection.workerPayoutEligible).toBe(false)
    expect(projection.state).toBe('funded')
    expect(projection.blockerRefs).toContain(
      'blocker.public.debt_receipt.studied_knowledge_validator_review_required',
    )
  })

  test('retires a receipt once settlement evidence exactly matches the payable amount', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
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

  // Typed DebtReceiptKey / PatchNoveltyKey model (EPIC #5335 fingerprint comment).
  test('derives stable, deterministic, distinct typed fingerprint keys', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      debtReceiptKeyInput,
      patchNoveltyKeyInput: {
        behaviorReceiptDigest: 'sha256:behavior',
        normalizedPatchDigest: 'patch-id:abc123',
      },
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
    })

    const expectedKey = deriveDebtReceiptKey(debtReceiptKeyInput)
    expect(projection.debtReceiptKey).toBe(expectedKey)
    expect(projection.patchNoveltyKey).toBe(
      derivePatchNoveltyKey({
        behaviorReceiptDigest: 'sha256:behavior',
        debtReceiptKey: expectedKey,
        normalizedPatchDigest: 'patch-id:abc123',
      }),
    )
    // a different scope yields a different DebtReceiptKey
    expect(
      deriveDebtReceiptKey({
        ...debtReceiptKeyInput,
        scopeDigest: 'scope.public.debt_receipt.other',
      }),
    ).not.toBe(expectedKey)
  })

  test('allows exactly one settlement per DebtReceiptKey, then it is retired', () => {
    // First accepted settlement against a fresh key: payable.
    const firstPass = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      debtReceiptKeyInput,
      patchNoveltyKeyInput: {
        behaviorReceiptDigest: 'sha256:behavior-1',
        normalizedPatchDigest: 'patch-id:first',
      },
      payableSats: 50_000,
      retiredDebtReceiptKeys: [],
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
    })

    expect(firstPass.duplicateReplay).toBe(false)
    expect(firstPass.state).toBe('payable')
    expect(firstPass.workerPayoutEligible).toBe(true)
  })

  test('rejects a near-duplicate patch against an already retired DebtReceiptKey as duplicate replay', () => {
    const retiredKey = deriveDebtReceiptKey(debtReceiptKeyInput)

    const replay = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      debtReceiptKeyInput,
      // a near-duplicate patch (different patch digest) against the retired key
      patchNoveltyKeyInput: {
        behaviorReceiptDigest: 'sha256:behavior-2',
        normalizedPatchDigest: 'patch-id:near-duplicate',
      },
      payableSats: 50_000,
      retiredDebtReceiptKeys: [retiredKey],
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
    })

    expect(replay).toMatchObject({
      duplicateReplay: true,
      settlementClaimAllowed: false,
      state: 'duplicate_replay',
      workerPayoutEligible: false,
    })
    expect(replay.blockerRefs).toContain(
      'blocker.public.debt_receipt.duplicate_replay',
    )
  })

  test('still blocks loose duplicate replay against an already retired receipt fingerprint', () => {
    const projection = projectDebtReceiptSettlement({
      ...verifiedReceiptInput,
      duplicateFingerprintRefs: [
        'fingerprint.public.debt_receipt.5334.scope_patch_digest_v1',
      ],
      payableSats: 50_000,
      retiredReceiptRefs: ['receipt.public.debt_receipt.5334.retired'],
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
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
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
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
      debtReceiptKeyInput,
      patchNoveltyKeyInput: {
        behaviorReceiptDigest: 'sha256:behavior',
        normalizedPatchDigest: 'patch-id:abc123',
      },
      payableSats: 50_000,
      settlementApprovalRefs: ['approval.public.debt_receipt.5334.settlement'],
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
