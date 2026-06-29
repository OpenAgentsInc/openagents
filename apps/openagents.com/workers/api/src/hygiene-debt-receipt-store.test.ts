import { describe, expect, it } from 'vitest'

import { deriveDebtReceiptKey } from './debt-receipt-key'
import { type DebtReceiptSettlementInput } from './debt-receipt-policy'
import {
  HygieneDebtReceiptStoreError,
  buildHygieneDebtReceiptRecord,
  makeInMemoryHygieneDebtReceiptStore,
  reprojectHygieneDebtReceipt,
} from './hygiene-debt-receipt-store'

/**
 * Durable debt-receipt store tests (#5372, EPIC #5335, process step 1).
 *
 * The store persists a CREATED, PAYABLE funded debt receipt keyed by its typed
 * DebtReceiptKey (#5340). A payable row resolves to a payable projection; a
 * retired row reprojects to duplicate_replay; retired keys are not re-creatable.
 */

const payableInput: DebtReceiptSettlementInput = {
  acceptedWorkRefs: ['accepted_work.public.debt_receipt.5358.fixture_dedup'],
  baselineMetricRefs: ['metric.public.debt_receipt.5358.baseline'],
  budgetCapSats: 100,
  debtReceiptKeyInput: {
    debtReceiptRef: 'receipt.public.debt.5358',
    objectiveDigest: 'objective.public.debt_receipt.5358.dedup_to_zero',
    repoBaselineRef: 'baseline.public.commit.3f636c133',
    scopeDigest: 'scope.public.debt_receipt.5358.target',
  },
  fundingApprovalRefs: ['approval.public.debt_receipt.5358.funded'],
  fundingAuthorityActorRef: 'actor.public.owner.allocator',
  fundingAuthorityRefs: ['authority.public.debt_receipt.allocator_route'],
  hygieneDeltaRefs: ['delta.public.debt_receipt.5358.removed'],
  noNewEqualOrWorseDebtRefs: ['check.public.debt_receipt.5358.no_worse_debt'],
  payableSats: 40,
  proposerActorRef: 'actor.public.orrery.churn_probe',
  reviewDecisionRefs: ['review.public.debt_receipt.5358.accepted'],
  reviewerActorRef: 'actor.public.reviewer.trigger',
  scopeRefs: ['scope.public.debt_receipt.5358.target'],
  settlementApprovalRefs: ['approval.public.debt_receipt.5358.settlement'],
  settlementAuthorityActorRef: 'actor.public.treasury.policy',
  sourceRefs: ['issue.public.github.openagentsinc_openagents.5358'],
  stopConditionRefs: ['stop.public.debt_receipt.5358.retire_once'],
  targetMetricRefs: ['metric.public.debt_receipt.5358.target.zero'],
  verificationCommandRefs: ['command.public.debt_receipt.5358.regen_and_diff'],
  workerActorRef: 'actor.public.worker.codex_loop',
}

const KEY = deriveDebtReceiptKey(payableInput.debtReceiptKeyInput!)

const createInput = (
  overrides: Partial<DebtReceiptSettlementInput> = {},
) => ({
  mergedPrRef: 'pr.public.github.openagentsinc_openagents.5358',
  nowIso: '2026-06-18T12:00:00.000Z',
  reviewerAcceptanceRef: 'review.public.debt_receipt.5358.accepted',
  settlementInput: { ...payableInput, ...overrides },
})

describe('buildHygieneDebtReceiptRecord', () => {
  it('builds a payable record keyed by the DebtReceiptKey', () => {
    const record = buildHygieneDebtReceiptRecord(createInput())

    expect(record.state).toBe('payable')
    expect(record.debtReceiptKey).toBe(KEY)
    expect(record.payableSats).toBe(40)
    expect(record.budgetCapSats).toBe(100)
    expect(record.debtReceiptRef).toBe('receipt.public.debt.5358')
    expect(record.repoBaselineRef).toBe('baseline.public.commit.3f636c133')
    expect(record.mergedPrRef).toBe(
      'pr.public.github.openagentsinc_openagents.5358',
    )
    expect(record.settlementAuthorityActorRef).toBe('actor.public.treasury.policy')
  })

  it('refuses a non-payable input (missing settlement approval => verified, not payable)', () => {
    expect(() =>
      buildHygieneDebtReceiptRecord(
        createInput({ settlementApprovalRefs: [] }),
      ),
    ).toThrow(HygieneDebtReceiptStoreError)
  })

  it('refuses documentation or journal credit as a payable receipt', () => {
    expect(() =>
      buildHygieneDebtReceiptRecord(
        createInput({ workClass: 'documentation_or_journal' }),
      ),
    ).toThrow(HygieneDebtReceiptStoreError)
  })

  it('refuses an input with no DebtReceiptKey input', () => {
    expect(() =>
      buildHygieneDebtReceiptRecord(
        createInput({ debtReceiptKeyInput: undefined }),
      ),
    ).toThrow(HygieneDebtReceiptStoreError)
  })

  it('refuses an unsafe ref (policy rejects payment/wallet material)', () => {
    expect(() =>
      buildHygieneDebtReceiptRecord(
        createInput({ sourceRefs: ['lnbc1secretinvoice'] }),
      ),
    ).toThrow()
  })
})

describe('reprojectHygieneDebtReceipt', () => {
  it('a payable record reprojects to a payable projection', () => {
    const record = buildHygieneDebtReceiptRecord(createInput())
    const projection = reprojectHygieneDebtReceipt(record)

    expect(projection.state).toBe('payable')
    expect(projection.duplicateReplay).toBe(false)
    expect(projection.debtReceiptKey).toBe(KEY)
  })

  it('a retired record reprojects to duplicate_replay (one settlement per key)', () => {
    const record = buildHygieneDebtReceiptRecord(createInput())
    const retired = {
      ...record,
      retiredAt: '2026-06-18T12:05:00.000Z',
      settlementReceiptRef: 'receipt.nexus.hygiene_lane_settlement.test',
      state: 'retired' as const,
    }
    const projection = reprojectHygieneDebtReceipt(retired)

    expect(projection.state).toBe('duplicate_replay')
    expect(projection.duplicateReplay).toBe(true)
  })
})

describe('makeInMemoryHygieneDebtReceiptStore', () => {
  it('creates a payable receipt and resolves a payable projection', async () => {
    const store = makeInMemoryHygieneDebtReceiptStore()

    const result = await store.create(createInput())
    expect(result.kind).toBe('created')

    const projection = await store.resolveProjection(KEY)
    expect(projection?.state).toBe('payable')
  })

  it('is idempotent on the DebtReceiptKey (second create => already_payable)', async () => {
    const store = makeInMemoryHygieneDebtReceiptStore()

    const first = await store.create(createInput())
    const second = await store.create(createInput())

    expect(first.kind).toBe('created')
    expect(second.kind).toBe('already_payable')
    expect(store.rows.size).toBe(1)
  })

  it('resolves undefined for an absent key (fail-closed)', async () => {
    const store = makeInMemoryHygieneDebtReceiptStore()

    expect(await store.resolveProjection(KEY)).toBeUndefined()
    expect(await store.read(KEY)).toBeUndefined()
  })

  it('marks a payable key retired; then resolves duplicate_replay and refuses re-create', async () => {
    const store = makeInMemoryHygieneDebtReceiptStore()
    await store.create(createInput())

    const retired = await store.markRetired(
      KEY,
      'receipt.nexus.hygiene_lane_settlement.5358',
      '2026-06-18T12:05:00.000Z',
    )
    expect(retired?.state).toBe('retired')
    expect(retired?.settlementReceiptRef).toBe(
      'receipt.nexus.hygiene_lane_settlement.5358',
    )

    // The settle route would now resolve duplicate_replay, never re-pay.
    const projection = await store.resolveProjection(KEY)
    expect(projection?.state).toBe('duplicate_replay')
    expect(projection?.duplicateReplay).toBe(true)

    // A second create on a retired key is a duplicate replay.
    const recreate = await store.create(createInput())
    expect(recreate.kind).toBe('retired')
  })

  it('markRetired is idempotent and a no-op on an absent key', async () => {
    const store = makeInMemoryHygieneDebtReceiptStore()
    await store.create(createInput())

    const first = await store.markRetired(KEY, 'receipt.a', '2026-06-18T12:05:00.000Z')
    const second = await store.markRetired(KEY, 'receipt.b', '2026-06-18T12:06:00.000Z')

    // The first retirement sticks; the retry is a no-op (same receipt ref).
    expect(first?.settlementReceiptRef).toBe('receipt.a')
    expect(second?.settlementReceiptRef).toBe('receipt.a')

    expect(
      await store.markRetired('debt_receipt_key:nonexistent', 'r', 'now'),
    ).toBeUndefined()
  })
})
