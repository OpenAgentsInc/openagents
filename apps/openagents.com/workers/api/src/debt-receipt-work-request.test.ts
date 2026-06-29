import { describe, expect, test } from 'vitest'

import { buildForumWorkRequestLbrDraft } from './forum-work-requests'
import {
  DebtReceiptWorkRequestUnsafe,
  buildDebtReceiptWorkRequestFiling,
  debtReceiptWorkRequestIdempotencyKey,
} from './debt-receipt-work-request'

const fundedDebtReceiptInput = {
  baselineMetricRefs: [
    'metric.public.debt_receipt.5334.baseline.dual_format_lines_460k',
  ],
  budgetCapSats: 100_000,
  deadlineRef: 'deadline.public.debt_receipt.20260625',
  debtReceiptRef: 'receipt.public.debt.5334.tassadar_fixture_dedup',
  fundingApprovalRefs: ['approval.public.debt_receipt.5334.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewerActorRef: 'actor.public.reviewer.trigger',
  scopeRefs: ['scope.public.debt_receipt.5334.tassadar_fixture_pairs'],
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5334'],
  stopConditionRefs: [
    'stop.public.debt_receipt.5334.retire_once_after_dedup',
  ],
  targetMetricRefs: [
    'metric.public.debt_receipt.5334.target.committed_generated_churn_0',
  ],
  title: 'Debt receipt #5334: dedupe generated Tassadar fixtures',
  verificationCommandRefs: [
    'command.public.debt_receipt.5334.regenerate_and_diff',
  ],
  workerActorRef: 'actor.public.worker.codex_loop',
}

describe('Debt receipt work-request adapter', () => {
  test('maps a funded debt receipt to the existing ref-only Forum work-request contract', () => {
    const filing = buildDebtReceiptWorkRequestFiling(fundedDebtReceiptInput)

    expect(filing).toMatchObject({
      debtReceiptRef: 'receipt.public.debt.5334.tassadar_fixture_dedup',
      idempotencyKey:
        'debt-receipt:receipt_public_debt_5334_tassadar_fixture_dedup',
      input: {
        budgetSats: 100_000,
        deadlineRef: 'deadline.public.debt_receipt.20260625',
        objectiveRef: 'receipt.public.debt.5334.tassadar_fixture_dedup',
        repositoryRefs: ['repo.public.openagents'],
        requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
        verificationCommandRef:
          'command.public.debt_receipt.5334.regenerate_and_diff',
      },
    })
    expect(filing.debtReceiptProjection).toMatchObject({
      settlementClaimAllowed: false,
      state: 'funded',
      workerPayoutEligible: false,
    })

    const lbr = buildForumWorkRequestLbrDraft(filing.normalizedInput, {
      relayUrl: 'wss://relay.test.openagents.dev',
      topicId: 'topic-debt-receipt-5334',
    })

    expect(lbr.draft.kind).toBe(5934)
    expect(lbr.draft.tags).toContainEqual([
      'param',
      'lbr_objective_ref',
      'receipt.public.debt.5334.tassadar_fixture_dedup',
    ])
    expect(lbr.draft.tags).toContainEqual(['bid', '100000000'])
  })

  test('refuses to list unfunded discovery as market work', () => {
    expect(() =>
      buildDebtReceiptWorkRequestFiling({
        ...fundedDebtReceiptInput,
        fundingApprovalRefs: [],
      }),
    ).toThrow(/must be funded before market listing/)
  })

  test('requires one verifier because the current Forum work-request contract has one verifier slot', () => {
    expect(() =>
      buildDebtReceiptWorkRequestFiling({
        ...fundedDebtReceiptInput,
        verificationCommandRefs: [
          'command.public.debt_receipt.5334.regenerate_and_diff',
          'command.public.debt_receipt.5334.bun_test',
        ],
      }),
    ).toThrow(DebtReceiptWorkRequestUnsafe)
  })

  test('keeps role-overlap receipts out of market listing', () => {
    expect(() =>
      buildDebtReceiptWorkRequestFiling({
        ...fundedDebtReceiptInput,
        reviewerActorRef: 'actor.public.worker.codex_loop',
      }),
    ).toThrow(/must be funded before market listing/)
  })

  test('uses stable idempotency keys for receipt relisting protection', () => {
    expect(
      debtReceiptWorkRequestIdempotencyKey(
        'receipt.public.debt.5334.tassadar_fixture_dedup',
      ),
    ).toBe('debt-receipt:receipt_public_debt_5334_tassadar_fixture_dedup')
  })
})
