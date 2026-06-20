import { describe, expect, it } from 'vitest'

import {
  Cs336A4EvalDeltaSettlementReceiptError,
  Cs336A4EvalDeltaSettlementReceiptSchemaVersion,
  Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError,
  buildCs336A4EvalDeltaSettlementReceipt,
} from './cs336-a4-eval-delta-settlement-receipt'
import {
  settleCs336A4EvalDeltaPayment,
  type Cs336A4EvalDeltaPaymentInput,
} from './cs336-a4-eval-delta-payment'
import {
  buildCs336A4ProvenanceReceipt,
  type Cs336A4TransformStep,
} from './cs336-a4-provenance'

const assignmentRef = 'assignment.cs336_a4.shard.1'

const measurement = {
  baselineScore: 0.4,
  filteredScore: 0.5,
  fixedReferenceModelRef: 'config.cs336_a4.fixed_reference_trainer.v1',
  heldOutEvalSetRef: 'eval.cs336_a4.held_out.v1',
  sourceRef: 'source.psion.bounded_synthetic_mixture.v1',
}

const verifiedInput: Cs336A4EvalDeltaPaymentInput = {
  assignmentRef,
  measurement,
  stageRecomputeVerified: true,
}

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

const buildProvenanceReceipt = (
  overrides: Partial<{
    assignmentRef: string
    recomputedDigestRef: string
  }> = {},
) =>
  buildCs336A4ProvenanceReceipt({
    assignmentRef: overrides.assignmentRef ?? assignmentRef,
    finalOutputDigestRef,
    inputShardRef: 'shard.cs336_a4.input.1',
    provenance: {
      acquisitionMode: 'bounded_synthetic_corpus',
      licenseRef: 'license.public.cc0.synthetic_corpus_v1',
      snapshotRef: 'snapshot.cs336_a4.seed.digest.0001',
      sourceRef: 'source.psion.bounded_synthetic_mixture.v1',
    },
    sourceInputDigestRef,
    transformChain: linkedChain,
  })

describe('CS336 A4 eval-delta settlement receipt', () => {
  it('binds a payable settlement to its provenance receipt', async () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })
    expect(settlement.payable).toBe(true)

    const provenanceReceipt = await buildProvenanceReceipt()
    const receipt = await buildCs336A4EvalDeltaSettlementReceipt({
      provenanceReceipt,
      settlement,
    })

    expect(receipt.schemaVersion).toBe(
      Cs336A4EvalDeltaSettlementReceiptSchemaVersion,
    )
    expect(receipt.payable).toBe(true)
    expect(receipt.settledBonusSats).toBe(100)
    expect(receipt.assignmentRef).toBe(assignmentRef)
    expect(receipt.finalOutputDigestRef).toBe(finalOutputDigestRef)
    expect(receipt.provenanceReceiptRef).toBe(provenanceReceipt.receiptRef)
    expect(receipt.receiptRef).toContain(receipt.contentDigestRef.slice(0, 16))
  })

  it('records a blocked settlement with zero bonus and no recompute requirement', async () => {
    // Unfunded settlement is blocked; recompute verification is irrelevant.
    const settlement = settleCs336A4EvalDeltaPayment(verifiedInput)
    expect(settlement.payable).toBe(false)

    const provenanceReceipt = await buildProvenanceReceipt()
    const receipt = await buildCs336A4EvalDeltaSettlementReceipt({
      provenanceReceipt,
      settlement,
    })

    expect(receipt.payable).toBe(false)
    expect(receipt.settledBonusSats).toBe(0)
  })

  it('is deterministic: same inputs yield the same receipt ref', async () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })
    const a = await buildCs336A4EvalDeltaSettlementReceipt({
      provenanceReceipt: await buildProvenanceReceipt(),
      settlement,
    })
    const b = await buildCs336A4EvalDeltaSettlementReceipt({
      provenanceReceipt: await buildProvenanceReceipt(),
      settlement,
    })

    expect(a.receiptRef).toBe(b.receiptRef)
    expect(a.contentDigestRef).toBe(b.contentDigestRef)
  })

  it('rejects a settlement and provenance receipt with mismatched assignment refs', async () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })
    const provenanceReceipt = await buildProvenanceReceipt({
      assignmentRef: 'assignment.cs336_a4.shard.OTHER',
    })

    await expect(
      buildCs336A4EvalDeltaSettlementReceipt({ provenanceReceipt, settlement }),
    ).rejects.toThrow(Cs336A4EvalDeltaSettlementReceiptError)
  })

  it('rejects unsafe material in a caller-derived ref', async () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      assignmentRef: 'assignment.cs336_a4.wallet.lnbc1leak',
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })
    const provenanceReceipt = {
      ...(await buildProvenanceReceipt()),
      assignmentRef: 'assignment.cs336_a4.wallet.lnbc1leak',
    }

    await expect(
      buildCs336A4EvalDeltaSettlementReceipt({ provenanceReceipt, settlement }),
    ).rejects.toThrow(Cs336A4EvalDeltaSettlementReceiptUnsafeMaterialError)
  })

  it('refuses to record a payable bonus against an unverified provenance receipt', async () => {
    const settlement = settleCs336A4EvalDeltaPayment({
      ...verifiedInput,
      fundingParameters: { bonusRateSatsPerUnit: 1000, deltaCap: 1 },
    })
    expect(settlement.payable).toBe(true)

    // A provenance receipt builds only when recompute verifies, so simulate an
    // unverified one by clearing the flag on a real receipt.
    const verifiedReceipt = await buildProvenanceReceipt()
    const unverifiedReceipt = { ...verifiedReceipt, recomputeVerified: false }

    await expect(
      buildCs336A4EvalDeltaSettlementReceipt({
        provenanceReceipt: unverifiedReceipt,
        settlement,
      }),
    ).rejects.toThrow(Cs336A4EvalDeltaSettlementReceiptError)
  })
})
