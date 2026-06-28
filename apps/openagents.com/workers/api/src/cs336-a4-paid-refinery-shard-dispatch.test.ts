import { describe, expect, it } from 'vitest'

import {
  deriveCs336A4CrawlShardAssignments,
  type Cs336A4CrawlShardAssignment,
} from './cs336-a4-crawl-shard-assignment'
import { buildCs336A4CrawlShardBatchCloseoutReceipt } from './cs336-a4-crawl-shard-batch-closeout'
import {
  buildCs336A4CrawlShardDispatchManifest,
  type Cs336A4CrawlShardDispatchManifest,
} from './cs336-a4-crawl-shard-dispatch-manifest'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'
import {
  buildCs336A4EvalDeltaDecontaminationReceipt,
  type Cs336A4EvalDeltaDecontaminationReceipt,
} from './cs336-a4-eval-delta-decontamination'
import {
  type Cs336A4EvalDeltaMeasurement,
  type Cs336A4EvalDeltaPaymentInput,
} from './cs336-a4-eval-delta-payment'
import {
  closeCs336A4EvalDeltaSettlement,
  type Cs336A4EvalDeltaSettlementCloseout,
} from './cs336-a4-eval-delta-settlement-closeout'
import {
  buildCs336A4PaidRefineryShardDispatchReceipt,
  Cs336A4PaidRefineryShardDispatchError,
  Cs336A4PaidRefineryShardDispatchSchemaVersion,
  Cs336A4PaidRefineryShardDispatchUnsafeMaterialError,
} from './cs336-a4-paid-refinery-shard-dispatch'
import {
  buildCs336A4ProvenanceReceipt,
  type Cs336A4ProvenanceReceipt,
} from './cs336-a4-provenance'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 4,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildProvenanceReceipt = (
  assignment: Cs336A4CrawlShardAssignment,
): Promise<Cs336A4ProvenanceReceipt> => {
  const sourceInputDigestRef = `digest.cs336_a4.source.${assignment.index}`
  const finalOutputDigestRef = `digest.cs336_a4.final.${assignment.index}`

  return buildCs336A4ProvenanceReceipt({
    assignmentRef: assignment.assignmentRef,
    finalOutputDigestRef,
    inputShardRef: assignment.inputShardRef,
    provenance: assignment.provenanceSource,
    sourceInputDigestRef,
    transformChain: [
      {
        codeVersionRef: 'psionic.refinery.v1.pii_masking',
        inputDigestRef: sourceInputDigestRef,
        outputDigestRef: finalOutputDigestRef,
        recomputedDigestRef: finalOutputDigestRef,
        stage: 'pii_masking',
      },
    ],
  })
}

const measurementFor = (
  assignment: Cs336A4CrawlShardAssignment,
): Cs336A4EvalDeltaMeasurement => ({
  baselineScore: 0.4,
  filteredScore: 0.5,
  fixedReferenceModelRef: 'config.cs336_a4.fixed_reference_trainer.v1',
  heldOutEvalSetRef: 'eval.cs336_a4.held_out.v1',
  sourceRef: assignment.provenanceSource.sourceRef,
})

const buildDecontaminationReceipt = (
  measurement: Cs336A4EvalDeltaPaymentInput['measurement'],
): Promise<Cs336A4EvalDeltaDecontaminationReceipt> =>
  buildCs336A4EvalDeltaDecontaminationReceipt({
    contaminatedSpansDetected: 1,
    contaminatedSpansRemoved: 1,
    heldOutEvalSetRef: measurement.heldOutEvalSetRef,
    methodRef: 'method.cs336_a4.ngram_overlap.v1',
    ngramSize: 13,
    postDecontaminationDigestRef: 'digest.cs336_a4.corpus.post.0001',
    preDecontaminationDigestRef: 'digest.cs336_a4.corpus.pre.0001',
    recomputedPostDigestRef: 'digest.cs336_a4.corpus.post.0001',
    sourceRef: measurement.sourceRef,
  })

const setup = async (): Promise<{
  assignments: ReadonlyArray<Cs336A4CrawlShardAssignment>
  closeout: Awaited<ReturnType<typeof buildCs336A4CrawlShardBatchCloseoutReceipt>>
  evalDeltaCloseout: Cs336A4EvalDeltaSettlementCloseout
  manifest: Cs336A4CrawlShardDispatchManifest
}> => {
  const plan = await buildCs336A4CrawlShardPlan({
    descriptor,
    targetShardCount: 2,
  })
  const assignments = await deriveCs336A4CrawlShardAssignments(plan)
  const manifest = await buildCs336A4CrawlShardDispatchManifest({
    assignments,
    plan,
  })
  const provenanceReceipts = await Promise.all(
    assignments.map(buildProvenanceReceipt),
  )
  const closeout = await buildCs336A4CrawlShardBatchCloseoutReceipt({
    assignments,
    manifest,
    receipts: provenanceReceipts,
  })
  const measurement = measurementFor(assignments[0]!)
  const evalDeltaCloseout = await closeCs336A4EvalDeltaSettlement({
    decontaminationReceipt: await buildDecontaminationReceipt(measurement),
    fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    measurement,
    provenanceReceipt: provenanceReceipts[0]!,
  })

  return { assignments, closeout, evalDeltaCloseout, manifest }
}

describe('buildCs336A4PaidRefineryShardDispatchReceipt', () => {
  it('binds paid shard dispatch, provenance closeout, recompute refs, and an eval-delta payment', async () => {
    const { closeout, evalDeltaCloseout, manifest } = await setup()

    const receipt = await buildCs336A4PaidRefineryShardDispatchReceipt({
      baseRateSatsPerVerifiedShard: 25,
      closeout,
      evalDeltaCloseouts: [evalDeltaCloseout],
      manifest,
      verificationRefs: ['challenge.cs336_a4.deterministic_recompute.1'],
    })

    expect(receipt.schemaVersion).toBe(
      Cs336A4PaidRefineryShardDispatchSchemaVersion,
    )
    expect(receipt.assignmentCount).toBe(manifest.assignmentCount)
    expect(receipt.basePayoutSats).toBe(50)
    expect(receipt.evalDeltaSettledBonusSats).toBe(100)
    expect(receipt.totalComputedPayoutSats).toBe(150)
    expect(receipt.dispatchManifestRef).toBe(manifest.manifestRef)
    expect(receipt.batchCloseoutRef).toBe(closeout.closeoutRef)
    expect(receipt.evalDeltaSettlementReceiptRefs).toEqual([
      evalDeltaCloseout.settlementReceipt.receiptRef,
    ])
    expect(receipt.paidAssignmentRefs).toEqual([...manifest.assignmentRefs].sort())
    expect(receipt.provenanceReceiptRefs).toEqual(
      closeout.closures
        .map(closure => closure.provenanceReceiptRef)
        .sort(),
    )
    expect(receipt.receiptRef).toContain(receipt.contentDigestRef.slice(0, 16))
  })

  it('is deterministic for the same inputs', async () => {
    const { closeout, evalDeltaCloseout, manifest } = await setup()
    const input = {
      baseRateSatsPerVerifiedShard: 25,
      closeout,
      evalDeltaCloseouts: [evalDeltaCloseout],
      manifest,
      verificationRefs: ['challenge.cs336_a4.deterministic_recompute.1'],
    }

    const first = await buildCs336A4PaidRefineryShardDispatchReceipt(input)
    const second = await buildCs336A4PaidRefineryShardDispatchReceipt(input)

    expect(second.receiptRef).toBe(first.receiptRef)
    expect(second.contentDigestRef).toBe(first.contentDigestRef)
  })

  it('rejects paid dispatch when no payable eval-delta closeout exists', async () => {
    const { assignments, closeout, manifest } = await setup()
    const provenanceReceipt = await buildProvenanceReceipt(assignments[0]!)
    const measurement = measurementFor(assignments[0]!)
    const blockedEvalDeltaCloseout = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(measurement),
      measurement,
      provenanceReceipt,
    })

    await expect(
      buildCs336A4PaidRefineryShardDispatchReceipt({
        baseRateSatsPerVerifiedShard: 25,
        closeout,
        evalDeltaCloseouts: [blockedEvalDeltaCloseout],
        manifest,
        verificationRefs: ['challenge.cs336_a4.deterministic_recompute.1'],
      }),
    ).rejects.toThrow(Cs336A4PaidRefineryShardDispatchError)
  })

  it('rejects an eval-delta closeout for an assignment outside the dispatched batch', async () => {
    const { closeout, evalDeltaCloseout, manifest } = await setup()
    const strayEvalDeltaCloseout: Cs336A4EvalDeltaSettlementCloseout = {
      ...evalDeltaCloseout,
      settlementReceipt: {
        ...evalDeltaCloseout.settlementReceipt,
        assignmentRef: 'assignment.cs336_a4.crawl_shard.stray.0_1.deadbeef',
      },
    }

    await expect(
      buildCs336A4PaidRefineryShardDispatchReceipt({
        baseRateSatsPerVerifiedShard: 25,
        closeout,
        evalDeltaCloseouts: [strayEvalDeltaCloseout],
        manifest,
        verificationRefs: ['challenge.cs336_a4.deterministic_recompute.1'],
      }),
    ).rejects.toThrow(Cs336A4PaidRefineryShardDispatchError)
  })

  it('rejects a closeout that does not match the dispatch manifest', async () => {
    const { closeout, evalDeltaCloseout, manifest } = await setup()

    await expect(
      buildCs336A4PaidRefineryShardDispatchReceipt({
        baseRateSatsPerVerifiedShard: 25,
        closeout: { ...closeout, manifestRef: 'manifest.cs336_a4.other' },
        evalDeltaCloseouts: [evalDeltaCloseout],
        manifest,
        verificationRefs: ['challenge.cs336_a4.deterministic_recompute.1'],
      }),
    ).rejects.toThrow(Cs336A4PaidRefineryShardDispatchError)
  })

  it('rejects unsafe public refs', async () => {
    const { closeout, evalDeltaCloseout, manifest } = await setup()

    await expect(
      buildCs336A4PaidRefineryShardDispatchReceipt({
        baseRateSatsPerVerifiedShard: 25,
        closeout,
        evalDeltaCloseouts: [evalDeltaCloseout],
        manifest,
        verificationRefs: ['challenge.cs336_a4.wallet.lnbc1leak'],
      }),
    ).rejects.toThrow(Cs336A4PaidRefineryShardDispatchUnsafeMaterialError)
  })
})
