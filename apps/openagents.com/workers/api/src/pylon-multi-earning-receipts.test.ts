import { describe, expect, test } from 'vitest'

import {
  projectPylonMultiEarningNode,
} from './pylon-multi-earning-node'
import {
  RECEIPTABLE_AMOUNT_CLASSES,
  foldWorkReceiptsIntoEarningStore,
  makeInMemoryPylonModeWorkReceiptStore,
  projectPylonSettlementManifest,
  projectPylonWorkReceiptManifest,
  recordModeWorkReceipt,
  verifyWorkReceiptRefDisjointness,
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

describe('pylon multi-earning settlement manifest (#5527)', () => {
  test('empty receipts yield an empty, still-red, covered manifest', () => {
    const manifest = projectPylonSettlementManifest([])
    expect(manifest.promiseState).toBe('red')
    expect(manifest.inert).toBe(true)
    expect(manifest.perMode).toHaveLength(0)
    expect(manifest.totalSettledReceiptCount).toBe(0)
    expect(manifest.totalDistinctSettlementRefCount).toBe(0)
    expect(manifest.coverageComplete).toBe(true)
  })

  test('enumerates the DISTINCT settlement refs per mode (not just a count)', () => {
    const manifest = projectPylonSettlementManifest([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
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
    ])
    expect(manifest.coverageComplete).toBe(true)
    expect(manifest.totalSettledReceiptCount).toBe(3)
    expect(manifest.totalDistinctSettlementRefCount).toBe(3)
    expect(manifest.perMode).toEqual([
      {
        mode: 'training',
        settledReceiptCount: 2,
        settlementReceiptRefs: [
          'receipt.public.pylon.training.settlement_a',
          'receipt.public.pylon.training.settlement_b',
        ],
      },
      {
        mode: 'forum_tips',
        settledReceiptCount: 1,
        settlementReceiptRefs: ['receipt.public.pylon.forum_tips.settlement_a'],
      },
    ])
  })

  test('non-settled receipts contribute nothing to the manifest', () => {
    const manifest = projectPylonSettlementManifest([
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
    expect(manifest.perMode).toHaveLength(0)
    expect(manifest.totalSettledReceiptCount).toBe(0)
    expect(manifest.coverageComplete).toBe(true)
  })

  test('an in-mode shared settlement surfaces as count>refs and not covered', () => {
    const manifest = projectPylonSettlementManifest([
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
    expect(manifest.coverageComplete).toBe(false)
    expect(manifest.perMode[0]).toEqual({
      mode: 'training',
      settledReceiptCount: 2,
      settlementReceiptRefs: ['receipt.public.pylon.training.settlement_shared'],
    })
    // The over-claim is visible: 2 settled units behind 1 dereferenceable ref.
    expect(manifest.perMode[0]?.settledReceiptCount).toBeGreaterThan(
      manifest.perMode[0]?.settlementReceiptRefs.length ?? 0,
    )
  })

  test('manifest and coverage auditor never disagree on coverage', () => {
    const receipts = [
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
    ]
    const manifest = projectPylonSettlementManifest(receipts)
    const coverage = verifyWorkReceiptSettlementCoverage(receipts)
    expect(manifest.coverageComplete).toBe(coverage.allModesSettlementCovered)
    expect(manifest.coverageComplete).toBe(false)
    expect(manifest.totalDistinctSettlementRefCount).toBe(
      coverage.totalDistinctSettlementRefCount,
    )
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

describe('pylon multi-earning work-receipt manifest (#5527)', () => {
  test('empty receipts yield an empty, still-red, covered manifest', () => {
    const manifest = projectPylonWorkReceiptManifest([])
    expect(manifest.promiseState).toBe('red')
    expect(manifest.inert).toBe(true)
    expect(manifest.perMode).toHaveLength(0)
    expect(manifest.totalReceiptCount).toBe(0)
    expect(manifest.totalDistinctAssignmentRefCount).toBe(0)
    expect(manifest.coverageComplete).toBe(true)
  })

  test('enumerates the work-receipt refs per amount class per mode', () => {
    const manifest = projectPylonWorkReceiptManifest([
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
        mode: 'compute',
        amountClass: 'paid',
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'receipt.public.pylon.compute.work_a',
      }),
    ])
    expect(manifest.coverageComplete).toBe(true)
    expect(manifest.totalReceiptCount).toBe(3)
    expect(manifest.totalDistinctAssignmentRefCount).toBe(3)
    expect(manifest.perMode).toEqual([
      {
        mode: 'training',
        receiptCount: 2,
        distinctAssignmentRefCount: 2,
        workUnitCoverageComplete: true,
        observedReceiptRefs: ['receipt.public.pylon.training.work_a'],
        pendingReceiptRefs: [],
        paidReceiptRefs: [],
        settledReceiptRefs: ['receipt.public.pylon.training.work_b'],
      },
      {
        mode: 'compute',
        receiptCount: 1,
        distinctAssignmentRefCount: 1,
        workUnitCoverageComplete: true,
        observedReceiptRefs: [],
        pendingReceiptRefs: [],
        paidReceiptRefs: ['receipt.public.pylon.compute.work_a'],
        settledReceiptRefs: [],
      },
    ])
  })

  test('settledReceiptRefs lists WORK refs, not settlement refs', () => {
    const manifest = projectPylonWorkReceiptManifest([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
      }),
    ])
    expect(manifest.perMode[0]?.settledReceiptRefs).toEqual([
      'receipt.public.pylon.training.work_a',
    ])
    // The settlement ref lives in the settlement manifest, not here.
    const settlement = projectPylonSettlementManifest([
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
      }),
    ])
    expect(settlement.perMode[0]?.settlementReceiptRefs).toEqual([
      'receipt.public.pylon.training.settlement_a',
    ])
  })

  test('an in-mode work-unit over-claim surfaces as not covered', () => {
    const receipts = [
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
    ]
    const manifest = projectPylonWorkReceiptManifest(receipts)
    expect(manifest.coverageComplete).toBe(false)
    expect(manifest.perMode[0]?.receiptCount).toBe(2)
    expect(manifest.perMode[0]?.distinctAssignmentRefCount).toBe(1)
    expect(manifest.perMode[0]?.workUnitCoverageComplete).toBe(false)
    // The over-claim is visible: 2 receipts behind 1 distinct work unit.
    expect(manifest.perMode[0]?.observedReceiptRefs).toHaveLength(2)
  })

  test('manifest and work-unit auditor never disagree on coverage', () => {
    const receipts = [
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
    ]
    const manifest = projectPylonWorkReceiptManifest(receipts)
    const coverage = verifyWorkReceiptWorkUnitCoverage(receipts)
    expect(manifest.coverageComplete).toBe(coverage.allWorkUnitsDistinct)
    expect(manifest.coverageComplete).toBe(false)
    expect(manifest.totalDistinctAssignmentRefCount).toBe(
      coverage.totalDistinctAssignmentRefCount,
    )
  })

  test('a duplicate receiptRef collapses (idempotent) before enumeration', () => {
    const dup = okReceipt({
      mode: 'training',
      amountClass: 'observed',
      assignmentRef: 'assignment.public.pylon.training.a',
      receiptRef: 'receipt.public.pylon.training.work_a',
    })
    const manifest = projectPylonWorkReceiptManifest([dup, dup])
    expect(manifest.totalReceiptCount).toBe(1)
    expect(manifest.perMode[0]?.observedReceiptRefs).toEqual([
      'receipt.public.pylon.training.work_a',
    ])
  })
})

describe('pylon work-receipt ref-namespace disjointness (#5527)', () => {
  test('empty receipts are vacuously disjoint', () => {
    const report = verifyWorkReceiptRefDisjointness([])
    expect(report.assignmentReceiptOverlapCount).toBe(0)
    expect(report.assignmentSettlementOverlapCount).toBe(0)
    expect(report.receiptSettlementOverlapCount).toBe(0)
    expect(report.totalOverlapTokenCount).toBe(0)
    expect(report.allRefNamespacesDisjoint).toBe(true)
  })

  test('three genuinely distinct artifacts per receipt are disjoint', () => {
    const receipts = [
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'receipt.public.pylon.training.settlement_a',
      }),
      okReceipt({
        mode: 'forum_tips',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.forum_tips.a',
        receiptRef: 'receipt.public.pylon.forum_tips.work_a',
      }),
    ]
    const report = verifyWorkReceiptRefDisjointness(receipts)
    expect(report.totalOverlapTokenCount).toBe(0)
    expect(report.allRefNamespacesDisjoint).toBe(true)
  })

  test('a token used as both an assignmentRef and a receiptRef is not disjoint', () => {
    const receipts = [
      okReceipt({
        mode: 'training',
        amountClass: 'observed',
        assignmentRef: 'shared.public.pylon.training.shared',
        receiptRef: 'receipt.public.pylon.training.work_a',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'observed',
        // Reuses the first receipt's assignmentRef as THIS receipt's receiptRef:
        // one token posing as both a work unit and a work receipt.
        assignmentRef: 'assignment.public.pylon.compute.a',
        receiptRef: 'shared.public.pylon.training.shared',
      }),
    ]
    const report = verifyWorkReceiptRefDisjointness(receipts)
    expect(report.assignmentReceiptOverlapCount).toBe(1)
    expect(report.totalOverlapTokenCount).toBe(1)
    expect(report.allRefNamespacesDisjoint).toBe(false)
  })

  test('a settlement ref reused as another receipt-work ref is not disjoint', () => {
    const receipts = [
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'shared.public.pylon.shared',
      }),
      okReceipt({
        mode: 'compute',
        amountClass: 'observed',
        assignmentRef: 'assignment.public.pylon.compute.a',
        // Reuses the settlement proof token as a WORK proof token.
        receiptRef: 'shared.public.pylon.shared',
      }),
    ]
    const report = verifyWorkReceiptRefDisjointness(receipts)
    expect(report.receiptSettlementOverlapCount).toBe(1)
    expect(report.totalOverlapTokenCount).toBe(1)
    expect(report.allRefNamespacesDisjoint).toBe(false)
  })

  test('fold REJECTS when a settlement ref poses as a work-unit ref', () => {
    const receipts = [
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        // The work unit and the settlement proof are the SAME token.
        assignmentRef: 'shared.public.pylon.training.collapsed',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'shared.public.pylon.training.collapsed',
      }),
    ]
    const report = verifyWorkReceiptRefDisjointness(receipts)
    expect(report.assignmentSettlementOverlapCount).toBe(1)
    expect(report.allRefNamespacesDisjoint).toBe(false)

    const folded = foldWorkReceiptsIntoEarningStore(receipts)
    expect(folded.ok).toBe(false)
    if (!folded.ok) {
      expect(folded.error.reason).toMatch(/genuinely distinct artifacts/)
    }
  })

  test('fold ACCEPTS two settled modes when all three namespaces are disjoint, still red', () => {
    const receipts = [
      okReceipt({
        mode: 'training',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.training.a',
        receiptRef: 'receipt.public.pylon.training.work_a',
        settlementReceiptRef: 'settlement.public.pylon.training.a',
      }),
      okReceipt({
        mode: 'forum_tips',
        amountClass: 'settled',
        assignmentRef: 'assignment.public.pylon.forum_tips.a',
        receiptRef: 'receipt.public.pylon.forum_tips.work_a',
        settlementReceiptRef: 'settlement.public.pylon.forum_tips.a',
      }),
    ]
    expect(verifyWorkReceiptRefDisjointness(receipts).allRefNamespacesDisjoint).toBe(
      true,
    )
    const folded = foldWorkReceiptsIntoEarningStore(receipts)
    expect(folded.ok).toBe(true)
    if (folded.ok) {
      const projection = projectPylonMultiEarningNode(folded.store)
      expect(projection.settledModeFamilyCount).toBe(2)
      // Even with the bar met, the projection stays honest and red/inert.
      expect(projection.promiseState).toBe('red')
      expect(projection.inert).toBe(true)
    }
  })
})
