import { describe, expect, it } from 'vitest'

import {
  buildCs336A4EvalDeltaDecontaminationReceipt,
  Cs336A4EvalDeltaDecontaminationError,
} from './cs336-a4-eval-delta-decontamination'
import { closeCs336A4EvalDeltaSettlement } from './cs336-a4-eval-delta-settlement-closeout'
import { Cs336A4EvalDeltaMeasurementBindingError } from './cs336-a4-eval-delta-measurement-binding'
import { Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError } from './cs336-a4-eval-delta-settlement-receipt'
import {
  buildCs336A4ProvenanceReceipt,
  type Cs336A4ProvenanceReceipt,
  type Cs336A4TransformStep,
} from './cs336-a4-provenance'

const assignmentRef = 'assignment.cs336_a4.shard.1'
const sourceRef = 'source.psion.bounded_synthetic_mixture.v1'
const heldOutEvalSetRef = 'eval.cs336_a4.held_out.v1'

const measurement = {
  baselineScore: 0.4,
  filteredScore: 0.5,
  fixedReferenceModelRef: 'config.cs336_a4.fixed_reference_trainer.v1',
  heldOutEvalSetRef,
  sourceRef,
}

const buildDecontaminationReceipt = (
  overrides: Partial<{ heldOutEvalSetRef: string; sourceRef: string }> = {},
) =>
  buildCs336A4EvalDeltaDecontaminationReceipt({
    contaminatedSpansDetected: 2,
    contaminatedSpansRemoved: 2,
    heldOutEvalSetRef: overrides.heldOutEvalSetRef ?? heldOutEvalSetRef,
    methodRef: 'method.cs336_a4.ngram_overlap.v1',
    ngramSize: 13,
    postDecontaminationDigestRef: 'digest.cs336_a4.corpus.post.0001',
    preDecontaminationDigestRef: 'digest.cs336_a4.corpus.pre.0001',
    recomputedPostDigestRef: 'digest.cs336_a4.corpus.post.0001',
    sourceRef: overrides.sourceRef ?? sourceRef,
  })

const sourceInputDigestRef = 'digest.cs336_a4.source.input.0001'

const linkedChain: ReadonlyArray<Cs336A4TransformStep> = [
  {
    codeVersionRef: 'psionic.refinery.v1.pii_masking',
    inputDigestRef: sourceInputDigestRef,
    outputDigestRef: 'digest.cs336_a4.stage.pii',
    recomputedDigestRef: 'digest.cs336_a4.stage.pii',
    stage: 'pii_masking',
  },
  {
    codeVersionRef: 'psionic.refinery.v1.gopher_rules',
    inputDigestRef: 'digest.cs336_a4.stage.pii',
    outputDigestRef: 'digest.cs336_a4.stage.gopher',
    recomputedDigestRef: 'digest.cs336_a4.stage.gopher',
    stage: 'gopher_rules',
  },
]

const finalOutputDigestRef = 'digest.cs336_a4.stage.gopher'

const buildProvenanceReceipt = (overrides: Partial<{ sourceRef: string }> = {}) =>
  buildCs336A4ProvenanceReceipt({
    assignmentRef,
    finalOutputDigestRef,
    inputShardRef: 'shard.cs336_a4.input.1',
    provenance: {
      acquisitionMode: 'bounded_synthetic_corpus',
      licenseRef: 'license.public.cc0.synthetic_corpus_v1',
      snapshotRef: 'snapshot.cs336_a4.seed.digest.0001',
      sourceRef: overrides.sourceRef ?? sourceRef,
    },
    sourceInputDigestRef,
    transformChain: linkedChain,
  })

const funding = { bonusRateSatsPerUnit: 1000, deltaCap: 1 }

describe('CS336 A4 eval-delta settlement closeout', () => {
  it('composes binding, decontamination, pricing, and receipt into a payable closeout', async () => {
    const provenanceReceipt = await buildProvenanceReceipt()
    const decontaminationReceipt = await buildDecontaminationReceipt()
    const closeout = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt,
      fundingParameters: funding,
      measurement,
      provenanceReceipt,
    })

    expect(closeout.boundSourceRef).toBe(sourceRef)
    expect(closeout.decontaminationReceiptRef).toBe(
      decontaminationReceipt.receiptRef,
    )
    expect(closeout.settlement.payable).toBe(true)
    expect(closeout.settlementReceipt.payable).toBe(true)
    expect(closeout.settlementReceipt.settledBonusSats).toBe(100)
    expect(closeout.settlementReceipt.assignmentRef).toBe(assignmentRef)
    expect(closeout.settlementReceipt.provenanceReceiptRef).toBe(
      provenanceReceipt.receiptRef,
    )
  })

  it('returns a blocked closeout when funding parameters are unset', async () => {
    const provenanceReceipt = await buildProvenanceReceipt()
    const closeout = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(),
      measurement,
      provenanceReceipt,
    })

    expect(closeout.settlement.payable).toBe(false)
    expect(closeout.settlementReceipt.payable).toBe(false)
    expect(closeout.settlementReceipt.settledBonusSats).toBe(0)
  })

  it('fails closed when the delta was measured on a different source', async () => {
    // The shard's admitted source differs from the source the delta was
    // measured on — the gap the settlement-receipt builder alone cannot catch.
    const provenanceReceipt = await buildProvenanceReceipt({
      sourceRef: 'source.psion.OTHER_easier_corpus.v1',
    })

    await expect(
      closeCs336A4EvalDeltaSettlement({
        decontaminationReceipt: await buildDecontaminationReceipt(),
        fundingParameters: funding,
        measurement,
        provenanceReceipt,
      }),
    ).rejects.toThrow(Cs336A4EvalDeltaMeasurementBindingError)
  })

  it('does not price a wrong-source measurement even if it would be payable', async () => {
    const provenanceReceipt = await buildProvenanceReceipt({
      sourceRef: 'source.psion.OTHER_easier_corpus.v1',
    })

    const error = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(),
      fundingParameters: funding,
      measurement,
      provenanceReceipt,
    }).catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(Cs336A4EvalDeltaMeasurementBindingError)
    expect((error as Cs336A4EvalDeltaMeasurementBindingError).reason).toBe(
      'source_ref_mismatch',
    )
  })

  it('refuses to record a payable bonus against an unverified provenance receipt', async () => {
    // A provenance receipt builds only when recompute verifies, so simulate an
    // unverified one. The closeout derives stageRecomputeVerified from it, so
    // pricing is blocked and the receipt builder also rejects a payable bonus.
    const verifiedReceipt = await buildProvenanceReceipt()
    const unverifiedReceipt: Cs336A4ProvenanceReceipt = {
      ...verifiedReceipt,
      recomputeVerified: false,
    }

    const closeout = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(),
      fundingParameters: funding,
      measurement,
      provenanceReceipt: unverifiedReceipt,
    })

    // Derived stageRecomputeVerified=false => blocked settlement, never payable.
    expect(closeout.settlement.payable).toBe(false)
    expect(closeout.settlementReceipt.payable).toBe(false)
  })

  it('is deterministic: same inputs yield the same settlement receipt ref', async () => {
    const a = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(),
      fundingParameters: funding,
      measurement,
      provenanceReceipt: await buildProvenanceReceipt(),
    })
    const b = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(),
      fundingParameters: funding,
      measurement,
      provenanceReceipt: await buildProvenanceReceipt(),
    })

    expect(a.settlementReceipt.receiptRef).toBe(b.settlementReceipt.receiptRef)
    expect(a.settlementReceipt.contentDigestRef).toBe(
      b.settlementReceipt.contentDigestRef,
    )
  })

  it('binds settlement and receipt to the provenance assignmentRef (no drift surface)', async () => {
    const provenanceReceipt = await buildProvenanceReceipt()
    const closeout = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: await buildDecontaminationReceipt(),
      fundingParameters: funding,
      measurement,
      provenanceReceipt,
    })

    expect(closeout.settlement.assignmentRef).toBe(provenanceReceipt.assignmentRef)
    expect(closeout.settlementReceipt.assignmentRef).toBe(
      provenanceReceipt.assignmentRef,
    )
  })

  it('surfaces a receipt-builder rejection (unsafe material) unchanged', async () => {
    const base = await buildProvenanceReceipt()
    // Tamper the assignmentRef on the trusted receipt with wallet material to
    // confirm the underlying public-safety guard still fires through the closeout.
    const tampered: Cs336A4ProvenanceReceipt = {
      ...base,
      assignmentRef: 'assignment.cs336_a4.wallet.lnbc1leak',
    }

    await expect(
      closeCs336A4EvalDeltaSettlement({
        decontaminationReceipt: await buildDecontaminationReceipt(),
        fundingParameters: funding,
        measurement,
        provenanceReceipt: tampered,
      }),
    ).rejects.toThrow(Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError)
  })

  it('fails closed when the decontamination receipt covers a different held-out eval set', async () => {
    // A clean receipt that decontaminated against an UNRELATED eval set does not
    // clear a bonus measured on the real held-out set — that is exactly the gap
    // the source-binding gate alone cannot catch.
    const provenanceReceipt = await buildProvenanceReceipt()
    const wrongEvalSetReceipt = await buildDecontaminationReceipt({
      heldOutEvalSetRef: 'eval.cs336_a4.OTHER_unrelated.v1',
    })

    const error = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: wrongEvalSetReceipt,
      fundingParameters: funding,
      measurement,
      provenanceReceipt,
    }).catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(Cs336A4EvalDeltaDecontaminationError)
    expect((error as Cs336A4EvalDeltaDecontaminationError).reason).toBe(
      'held_out_eval_set_ref_mismatch',
    )
  })

  it('does not price a contaminated (non-clean) corpus even if it would be payable', async () => {
    // Spans detected but not fully removed => not clean => the gamed positive
    // delta must never be priced or recorded.
    const provenanceReceipt = await buildProvenanceReceipt()
    const dirtyReceipt = await buildCs336A4EvalDeltaDecontaminationReceipt({
      contaminatedSpansDetected: 3,
      contaminatedSpansRemoved: 1,
      heldOutEvalSetRef,
      methodRef: 'method.cs336_a4.ngram_overlap.v1',
      ngramSize: 13,
      postDecontaminationDigestRef: 'digest.cs336_a4.corpus.post.0001',
      preDecontaminationDigestRef: 'digest.cs336_a4.corpus.pre.0001',
      recomputedPostDigestRef: 'digest.cs336_a4.corpus.post.0001',
      sourceRef,
    })

    expect(dirtyReceipt.clean).toBe(false)

    const error = await closeCs336A4EvalDeltaSettlement({
      decontaminationReceipt: dirtyReceipt,
      fundingParameters: funding,
      measurement,
      provenanceReceipt,
    }).catch((caught: unknown) => caught)

    expect(error).toBeInstanceOf(Cs336A4EvalDeltaDecontaminationError)
    expect((error as Cs336A4EvalDeltaDecontaminationError).reason).toBe(
      'receipt_not_clean',
    )
  })
})
