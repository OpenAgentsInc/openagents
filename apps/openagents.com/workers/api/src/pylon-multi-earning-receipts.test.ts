import { describe, expect, test } from 'vitest'

import {
  projectPylonMultiEarningNode,
} from './pylon-multi-earning-node'
import {
  RECEIPTABLE_AMOUNT_CLASSES,
  foldWorkReceiptsIntoEarningStore,
  makeInMemoryPylonModeWorkReceiptStore,
  recordModeWorkReceipt,
  verifyWorkReceiptSettlementCoverage,
  verifyWorkReceiptWorkUnitCoverage,
} from './pylon-multi-earning-receipts'

const okReceipt = (input: Parameters<typeof recordModeWorkReceipt>[0]) => {
  const result = recordModeWorkReceipt(input)
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.receipt
}

describe('pylon multi-earning work receipt (#5527)', () => {
  test('receiptable classes are observed/pending/paid/settled (no modeled)', () => {
    expect(RECEIPTABLE_AMOUNT_CLASSES).toEqual([
      'observed',
      'pending',
      'paid',
      'settled',
    ])
    expect(
      (RECEIPTABLE_AMOUNT_CLASSES as ReadonlyArray<string>).includes('modeled'),
    ).toBe(false)
  })

  test('builds a valid non-settled receipt with no settlement ref', () => {
    const receipt = okReceipt({
      mode: 'compute',
      amountClass: 'observed',
      assignmentRef: 'assignment.public.pylon.compute.a',
      receiptRef: 'receipt.public.pylon.compute.work_a',
    })
    expect(receipt.amountClass).toBe('observed')
    expect(receipt.settlementReceiptRef).toBeUndefined()
  })

  test('a settled receipt requires a settlement receipt ref', () => {
    const result = recordModeWorkReceipt({
      mode: 'training',
      amountClass: 'settled',
      assignmentRef: 'assignment.public.pylon.training.a',
      receiptRef: 'receipt.public.pylon.training.work_a',
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toMatch(/settlementReceiptRef/)
    }
  })

  test('a settlement ref is only valid on a settled receipt', () => {
    const result = recordModeWorkReceipt({
      mode: 'training',
      amountClass: 'paid',
      assignmentRef: 'assignment.public.pylon.training.a',
      receiptRef: 'receipt.public.pylon.training.work_a',
      settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
    })
    expect(result.ok).toBe(false)
  })

  test('rejects an unknown amount class', () => {
    const result = recordModeWorkReceipt({
      mode: 'training',
      // intentionally invalid amount class
      amountClass: 'modeled' as never,
      assignmentRef: 'assignment.public.pylon.training.a',
      receiptRef: 'receipt.public.pylon.training.work_a',
    })
    expect(result.ok).toBe(false)
  })

  test('rejects unsafe refs in any ref field', () => {
    expect(
      recordModeWorkReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'lnbc-payout',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }).ok,
    ).toBe(false)
    expect(
      recordModeWorkReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'wallet_secret',
      }).ok,
    ).toBe(false)
  })

  test('store is idempotent per receiptRef', () => {
    const store = makeInMemoryPylonModeWorkReceiptStore([
      okReceipt({
        mode: 'compute',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'paid',
        assignmentRef: 'assignment.public.pylon.compute.b',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(store.list()).toHaveLength(1)
    expect(store.list()[0]?.amountClass).toBe('observed')
  })
})

describe('pylon multi-earning receipt fold (#5527)', () => {
  test('empty receipts fold to an empty, still-red projection', () => {
    const folded = foldWorkReceiptsIntoEarningStore([])
    expect(folded.ok).toBe(true)
    if (!folded.ok) return
    const projection = projectPylonMultiEarningNode(folded.store)
    expect(projection.promiseState).toBe('red')
    expect(projection.modes).toHaveLength(0)
    expect(projection.settledModeCount).toBe(0)
  })

  test('folds per-mode receipts into projection counts backed by receipts', () => {
    const folded = foldWorkReceiptsIntoEarningStore([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.b',
        receiptRef: 'receipt.public.pylon.training.work_b',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_b',
      }),
      okReceipt({
        mode: 'forum_tips',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.forum_tips.a',
        receiptRef: 'receipt.public.pylon.forum_tips.work_a',
        settlementReceiptRef: 'receipt.public.pylon.forum_tips.settlement_a',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'paid',
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(folded.ok).toBe(true)
    if (!folded.ok) return

    const projection = projectPylonMultiEarningNode(folded.store)
    // Two settled modes are now backed by settlement receipts...
    expect(projection.settledModeCount).toBe(2)
    expect(projection.settledModesBarMet).toBe(true)
    // ...yet the surface NEVER flips the promise.
    expect(projection.promiseState).toBe('red')
    expect(projection.inert).toBe(true)

    const training = projection.modes.find(m => m.mode === 'training')
    expect(training?.observedCount).toBe(1)
    expect(training?.settledCount).toBe(1)
    expect(training?.settlementReceiptRef).toBe(
      'receipt.public.pylon.training.settlement_b',
    )
    // modeled never comes from a receipt
    expect(training?.modeledCount).toBe(0)

    const compute = projection.modes.find(m => m.mode === 'compute')
    expect(compute?.paidCount).toBe(1)
    expect(compute?.settledCount).toBe(0)
    expect(compute?.settlementReceiptRef).toBeUndefined()
  })

  test('rejects an in-mode settlement over-claim (settled count > distinct settlements)', () => {
    // Two DISTINCT settled work units that share ONE settlement receipt: the
    // settled count would be 2 but only one real settlement backs them.
    const folded = foldWorkReceiptsIntoEarningStore([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_shared',
      }),
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.b',
        receiptRef: 'receipt.public.pylon.training.work_b',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_shared',
      }),
    ])
    expect(folded.ok).toBe(false)
    if (folded.ok) return
    expect(folded.error.reason).toMatch(/over-claim/)
  })

  test('rejects a cross-mode settlement reuse', () => {
    const folded = foldWorkReceiptsIntoEarningStore([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.shared.settlement',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'receipt.public.pylon.compute.work_a',
        settlementReceiptRef: 'receipt.public.pylon.shared.settlement',
      }),
    ])
    expect(folded.ok).toBe(false)
    if (folded.ok) return
    expect(folded.error.reason).toMatch(/across earning modes/)
  })

  test('a settled mode always carries a settlement receipt ref after fold', () => {
    const folded = foldWorkReceiptsIntoEarningStore([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
      }),
    ])
    expect(folded.ok).toBe(true)
    if (!folded.ok) return
    const projection = projectPylonMultiEarningNode(folded.store)
    expect(
      projection.modes.every(
        m => m.settledCount === 0 || m.settlementReceiptRef !== undefined,
      ),
    ).toBe(true)
  })
})

describe('pylon multi-earning settlement coverage (#5527)', () => {
  test('empty receipts are trivially covered', () => {
    const coverage = verifyWorkReceiptSettlementCoverage([])
    expect(coverage.allModesSettlementCovered).toBe(true)
    expect(coverage.perMode).toHaveLength(0)
    expect(coverage.totalSettledReceiptCount).toBe(0)
    expect(coverage.totalDistinctSettlementRefCount).toBe(0)
    expect(coverage.crossModeSettlementReuse).toBe(false)
  })

  test('distinct settlements across two modes are fully covered', () => {
    const coverage = verifyWorkReceiptSettlementCoverage([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
      }),
      okReceipt({
        mode: 'forum_tips',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.forum_tips.a',
        receiptRef: 'receipt.public.pylon.forum_tips.work_a',
        settlementReceiptRef: 'receipt.public.pylon.forum_tips.settlement_a',
      }),
    ])
    expect(coverage.allModesSettlementCovered).toBe(true)
    expect(coverage.totalSettledReceiptCount).toBe(2)
    expect(coverage.totalDistinctSettlementRefCount).toBe(2)
    expect(coverage.crossModeSettlementReuse).toBe(false)
    expect(coverage.perMode).toEqual([
      {
        mode: 'training',
        settledReceiptCount: 1,
        distinctSettlementRefCount: 1,
        settlementCoverageComplete: true,
      },
      {
        mode: 'forum_tips',
        settledReceiptCount: 1,
        distinctSettlementRefCount: 1,
        settlementCoverageComplete: true,
      },
    ])
  })

  test('flags an in-mode shared settlement as incomplete coverage', () => {
    const coverage = verifyWorkReceiptSettlementCoverage([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_shared',
      }),
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.b',
        receiptRef: 'receipt.public.pylon.training.work_b',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_shared',
      }),
    ])
    expect(coverage.allModesSettlementCovered).toBe(false)
    expect(coverage.perMode[0]).toEqual({
      mode: 'training',
      settledReceiptCount: 2,
      distinctSettlementRefCount: 1,
      settlementCoverageComplete: false,
    })
  })

  test('non-settled receipts contribute nothing to coverage', () => {
    const coverage = verifyWorkReceiptSettlementCoverage([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'paid',
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(coverage.allModesSettlementCovered).toBe(true)
    expect(coverage.perMode).toHaveLength(0)
    expect(coverage.totalSettledReceiptCount).toBe(0)
  })
})

describe('pylon multi-earning work-unit coverage (#5527)', () => {
  test('empty receipts are trivially distinct', () => {
    const coverage = verifyWorkReceiptWorkUnitCoverage([])
    expect(coverage.allWorkUnitsDistinct).toBe(true)
    expect(coverage.perMode).toHaveLength(0)
    expect(coverage.totalReceiptCount).toBe(0)
    expect(coverage.totalDistinctAssignmentRefCount).toBe(0)
    expect(coverage.crossModeWorkUnitReuse).toBe(false)
  })

  test('distinct work units across two modes are fully covered', () => {
    const coverage = verifyWorkReceiptWorkUnitCoverage([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'training',
        amountClass: 'paid',
        assignmentRef: 'assignment.public.pylon.training.b',
        receiptRef: 'receipt.public.pylon.training.work_b',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(coverage.allWorkUnitsDistinct).toBe(true)
    expect(coverage.totalReceiptCount).toBe(3)
    expect(coverage.totalDistinctAssignmentRefCount).toBe(3)
    expect(coverage.crossModeWorkUnitReuse).toBe(false)
    expect(coverage.perMode).toEqual([
      {
        mode: 'training',
        receiptCount: 2,
        distinctAssignmentRefCount: 2,
        workUnitCoverageComplete: true,
      },
      {
        mode: 'compute',
        receiptCount: 1,
        distinctAssignmentRefCount: 1,
        workUnitCoverageComplete: true,
      },
    ])
  })

  test('flags an in-mode shared work unit as incomplete coverage', () => {
    // Two distinct receipts (distinct receiptRefs) re-counting ONE work unit:
    // observedCount would read 2 behind a single real assignment.
    const coverage = verifyWorkReceiptWorkUnitCoverage([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.shared',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'training',
        amountClass: 'paid',
        assignmentRef: 'assignment.public.pylon.training.shared',
        receiptRef: 'receipt.public.pylon.training.work_b',
      }),
    ])
    expect(coverage.allWorkUnitsDistinct).toBe(false)
    expect(coverage.perMode[0]).toEqual({
      mode: 'training',
      receiptCount: 2,
      distinctAssignmentRefCount: 1,
      workUnitCoverageComplete: false,
    })
  })

  test('flags a cross-mode shared work unit', () => {
    const coverage = verifyWorkReceiptWorkUnitCoverage([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.shared.unit',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.shared.unit',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(coverage.allWorkUnitsDistinct).toBe(false)
    expect(coverage.crossModeWorkUnitReuse).toBe(true)
  })
})

describe('pylon multi-earning fold work-unit integrity (#5527)', () => {
  test('rejects an in-mode work-unit over-claim (receipts > distinct work units)', () => {
    const folded = foldWorkReceiptsIntoEarningStore([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.shared',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.training.shared',
        receiptRef: 'receipt.public.pylon.training.work_b',
      }),
    ])
    expect(folded.ok).toBe(false)
    if (folded.ok) return
    expect(folded.error.reason).toMatch(/work-unit over-claim/)
    expect(folded.error.reason).toMatch(/distinct/)
  })

  test('rejects a cross-mode work-unit reuse', () => {
    const folded = foldWorkReceiptsIntoEarningStore([
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.shared.unit',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.shared.unit',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(folded.ok).toBe(false)
    if (folded.ok) return
    expect(folded.error.reason).toMatch(/across earning modes/)
  })
})
