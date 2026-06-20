import { describe, expect, it } from 'vitest'

import {
  buildCs336A4EvalDeltaFundingBudgetLedger,
  Cs336A4EvalDeltaFundingBudgetError,
  Cs336A4EvalDeltaFundingBudgetUnsafeMaterialError,
} from './cs336-a4-eval-delta-funding-budget'
import {
  settleCs336A4EvalDeltaPayment,
  type Cs336A4EvalDeltaPayableSettlement,
  type Cs336A4EvalDeltaSettlement,
} from './cs336-a4-eval-delta-payment'

const heldOutEvalSetRef = 'eval.cs336_a4.held_out.v1'
const sourceRef = 'source.psion.bounded_synthetic_mixture.v1'
const fixedReferenceModelRef = 'config.cs336_a4.fixed_reference_trainer.v1'

// funding => perShardMax = round(1 * 1000) = 1000 sats
const fundingParameters = { bonusRateSatsPerUnit: 1000, deltaCap: 1 }

const payableSettlement = (
  assignmentRef: string,
  delta: number,
): Cs336A4EvalDeltaSettlement =>
  settleCs336A4EvalDeltaPayment({
    assignmentRef,
    fundingParameters,
    measurement: {
      baselineScore: 0.4,
      filteredScore: 0.4 + delta,
      fixedReferenceModelRef,
      heldOutEvalSetRef,
      sourceRef,
    },
    stageRecomputeVerified: true,
  })

const blockedSettlement = (assignmentRef: string): Cs336A4EvalDeltaSettlement =>
  // No funding parameters => blocked settlement.
  settleCs336A4EvalDeltaPayment({
    assignmentRef,
    measurement: {
      baselineScore: 0.4,
      filteredScore: 0.5,
      fixedReferenceModelRef,
      heldOutEvalSetRef,
      sourceRef,
    },
    stageRecomputeVerified: true,
  })

const authorization = {
  authorityRef: 'operator.cs336_a4.funding_authority.v1',
  budgetCapSats: 1000,
  fundingParameters,
}

describe('CS336 A4 eval-delta funding budget ledger', () => {
  it('builds a ledger for a batch that fits the authorized budget', async () => {
    // delta 0.1 => 100 sats each; two shards => 200 of 1000.
    const settlements = [
      payableSettlement('assignment.cs336_a4.shard.1', 0.1),
      payableSettlement('assignment.cs336_a4.shard.2', 0.1),
    ]

    const ledger = await buildCs336A4EvalDeltaFundingBudgetLedger({
      authorization,
      settlements,
    })

    expect(ledger.payableSettlementCount).toBe(2)
    expect(ledger.totalChargedBonusSats).toBe(200)
    expect(ledger.remainingBudgetSats).toBe(800)
    expect(ledger.perShardMaxBonusSats).toBe(1000)
    expect(ledger.entries).toHaveLength(2)
    expect(ledger.ledgerRef.startsWith('ledger.cs336_a4.eval_delta_funding_budget.')).toBe(
      true,
    )
  })

  it('records a blocked settlement as a zero charge', async () => {
    const settlements = [
      payableSettlement('assignment.cs336_a4.shard.1', 0.1),
      blockedSettlement('assignment.cs336_a4.shard.2'),
    ]

    const ledger = await buildCs336A4EvalDeltaFundingBudgetLedger({
      authorization,
      settlements,
    })

    expect(ledger.payableSettlementCount).toBe(1)
    expect(ledger.totalChargedBonusSats).toBe(100)
    expect(ledger.entries[1]).toEqual({
      assignmentRef: 'assignment.cs336_a4.shard.2',
      chargedBonusSats: 0,
      payable: false,
    })
  })

  it('is deterministic: same authorization + settlements yield the same ref', async () => {
    const make = () =>
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization,
        settlements: [payableSettlement('assignment.cs336_a4.shard.1', 0.1)],
      })

    const a = await make()
    const b = await make()

    expect(a.ledgerRef).toBe(b.ledgerRef)
    expect(a.contentDigestRef).toBe(b.contentDigestRef)
  })

  it('rejects an empty authority ref', async () => {
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization: { ...authorization, authorityRef: '  ' },
        settlements: [],
      }),
    ).rejects.toMatchObject({ reason: 'empty_authority_ref' })
  })

  it('rejects a non-positive-integer budget cap', async () => {
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization: { ...authorization, budgetCapSats: 0 },
        settlements: [],
      }),
    ).rejects.toMatchObject({ reason: 'budget_cap_not_positive_integer' })

    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization: { ...authorization, budgetCapSats: 10.5 },
        settlements: [],
      }),
    ).rejects.toMatchObject({ reason: 'budget_cap_not_positive_integer' })
  })

  it('rejects invalid funding parameters', async () => {
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization: {
          ...authorization,
          fundingParameters: { bonusRateSatsPerUnit: 0, deltaCap: 1 },
        },
        settlements: [],
      }),
    ).rejects.toMatchObject({ reason: 'funding_parameters_invalid' })
  })

  it('rejects an authorization whose per-shard max exceeds the whole budget', async () => {
    // perShardMax = round(1 * 1000) = 1000 > budget 500.
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization: { ...authorization, budgetCapSats: 500 },
        settlements: [],
      }),
    ).rejects.toMatchObject({ reason: 'per_shard_max_exceeds_budget' })
  })

  it('rejects a duplicate assignment in the batch', async () => {
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization,
        settlements: [
          payableSettlement('assignment.cs336_a4.shard.1', 0.1),
          payableSettlement('assignment.cs336_a4.shard.1', 0.2),
        ],
      }),
    ).rejects.toMatchObject({ reason: 'duplicate_assignment' })
  })

  it('rejects a settlement priced above the authorized per-shard max', async () => {
    // A settlement priced under unauthorized, larger funding (2000/unit) charges
    // 200 sats for delta 0.1 — above the authorized per-shard max of 100? No:
    // authorized perShardMax = round(deltaCap*rate) = 1000. Craft a charge above it.
    const overpriced: Cs336A4EvalDeltaPayableSettlement = {
      ...(payableSettlement(
        'assignment.cs336_a4.shard.1',
        0.1,
      ) as Cs336A4EvalDeltaPayableSettlement),
      settledBonusSats: 1500,
    }

    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization,
        settlements: [overpriced],
      }),
    ).rejects.toMatchObject({ reason: 'settlement_bonus_exceeds_per_shard_max' })
  })

  it('rejects a non-integer settled bonus', async () => {
    const fractional: Cs336A4EvalDeltaPayableSettlement = {
      ...(payableSettlement(
        'assignment.cs336_a4.shard.1',
        0.1,
      ) as Cs336A4EvalDeltaPayableSettlement),
      settledBonusSats: 50.5,
    }

    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization,
        settlements: [fractional],
      }),
    ).rejects.toMatchObject({
      reason: 'settlement_bonus_not_nonnegative_integer',
    })
  })

  it('rejects a batch whose cumulative bonus exceeds the budget', async () => {
    // Each shard charges 1000 (perShardMax); two exceed the 1000 budget.
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization,
        settlements: [
          payableSettlement('assignment.cs336_a4.shard.1', 1),
          payableSettlement('assignment.cs336_a4.shard.2', 1),
        ],
      }),
    ).rejects.toMatchObject({ reason: 'cumulative_bonus_exceeds_budget' })
  })

  it('fails closed on unsafe material in the authority ref', async () => {
    await expect(
      buildCs336A4EvalDeltaFundingBudgetLedger({
        authorization: {
          ...authorization,
          authorityRef: 'operator.wallet.lnbc1leak',
        },
        settlements: [],
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaFundingBudgetUnsafeMaterialError)
  })

  it('exposes a typed error class for dispatch audit branching', async () => {
    const error = await buildCs336A4EvalDeltaFundingBudgetLedger({
      authorization: { ...authorization, budgetCapSats: -1 },
      settlements: [],
    }).catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(Cs336A4EvalDeltaFundingBudgetError)
  })
})
