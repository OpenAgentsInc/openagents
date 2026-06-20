import { describe, expect, it } from 'vitest'

import {
  assertCs336A4EvalDeltaMeasurementBinding,
  Cs336A4EvalDeltaMeasurementBindingError,
  Cs336A4EvalDeltaMeasurementBindingValidationError,
  verifyCs336A4EvalDeltaMeasurementBinding,
} from './cs336-a4-eval-delta-measurement-binding'
import type { Cs336A4EvalDeltaMeasurement } from './cs336-a4-eval-delta-payment'
import {
  buildCs336A4ProvenanceReceipt,
  type Cs336A4ProvenanceReceipt,
  type Cs336A4SourceProvenance,
} from './cs336-a4-provenance'

const provenance: Cs336A4SourceProvenance = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildReceipt = (
  source: Cs336A4SourceProvenance = provenance,
): Promise<Cs336A4ProvenanceReceipt> =>
  buildCs336A4ProvenanceReceipt({
    assignmentRef: 'assignment.cs336_a4.crawl_shard.cc.0_2.deadbeefdeadbeef',
    finalOutputDigestRef: 'digest.stage.minhash_dedup',
    inputShardRef: 'shard.cc_main.2026_05.0_2',
    provenance: source,
    sourceInputDigestRef: 'digest.source.input',
    transformChain: [
      {
        codeVersionRef: 'psionic.refinery.v1',
        inputDigestRef: 'digest.source.input',
        outputDigestRef: 'digest.stage.pii_mask',
        recomputedDigestRef: 'digest.stage.pii_mask',
        stage: 'pii_masking',
      },
      {
        codeVersionRef: 'psionic.refinery.v1',
        inputDigestRef: 'digest.stage.pii_mask',
        outputDigestRef: 'digest.stage.minhash_dedup',
        recomputedDigestRef: 'digest.stage.minhash_dedup',
        stage: 'minhash_dedup',
      },
    ],
  })

const measurementOn = (sourceRef: string): Cs336A4EvalDeltaMeasurement => ({
  baselineScore: 0.40,
  filteredScore: 0.52,
  fixedReferenceModelRef: 'trainer.fixed.cs336_a4.ref_v1',
  heldOutEvalSetRef: 'eval.cs336_a4.held_out_v1',
  sourceRef,
})

describe('verifyCs336A4EvalDeltaMeasurementBinding', () => {
  it('binds a measurement taken on the receipt source', async () => {
    const provenanceReceipt = await buildReceipt()
    const result = verifyCs336A4EvalDeltaMeasurementBinding({
      measurement: measurementOn(provenance.sourceRef),
      provenanceReceipt,
    })

    expect(result.bound).toBe(true)
    if (result.bound) {
      expect(result.sourceRef).toBe(provenance.sourceRef)
      expect(result.provenanceReceiptRef).toBe(provenanceReceipt.receiptRef)
    }
  })

  it('binds when the measurement source differs only by surrounding whitespace', async () => {
    const provenanceReceipt = await buildReceipt()
    const result = verifyCs336A4EvalDeltaMeasurementBinding({
      measurement: measurementOn(`  ${provenance.sourceRef}  `),
      provenanceReceipt,
    })

    expect(result.bound).toBe(true)
  })

  it('rejects a measurement taken on a different source', async () => {
    const provenanceReceipt = await buildReceipt()
    const result = verifyCs336A4EvalDeltaMeasurementBinding({
      measurement: measurementOn('source.some_other_crawl'),
      provenanceReceipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('source_ref_mismatch')
    }
  })

  it('rejects a measurement even when only the source casing differs', async () => {
    const provenanceReceipt = await buildReceipt()
    const result = verifyCs336A4EvalDeltaMeasurementBinding({
      measurement: measurementOn(provenance.sourceRef.toUpperCase()),
      provenanceReceipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('source_ref_mismatch')
    }
  })

  it('fails closed when the measurement source is empty', async () => {
    const provenanceReceipt = await buildReceipt()
    expect(() =>
      verifyCs336A4EvalDeltaMeasurementBinding({
        measurement: measurementOn('   '),
        provenanceReceipt,
      }),
    ).toThrow(Cs336A4EvalDeltaMeasurementBindingValidationError)
  })
})

describe('assertCs336A4EvalDeltaMeasurementBinding', () => {
  it('returns the provenance receipt ref when the measurement binds', async () => {
    const provenanceReceipt = await buildReceipt()
    expect(
      assertCs336A4EvalDeltaMeasurementBinding({
        measurement: measurementOn(provenance.sourceRef),
        provenanceReceipt,
      }),
    ).toBe(provenanceReceipt.receiptRef)
  })

  it('throws a tagged error carrying the mismatch reason when unbound', async () => {
    const provenanceReceipt = await buildReceipt()
    try {
      assertCs336A4EvalDeltaMeasurementBinding({
        measurement: measurementOn('source.some_other_crawl'),
        provenanceReceipt,
      })
      expect.unreachable('expected an unbound measurement to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4EvalDeltaMeasurementBindingError)
      if (error instanceof Cs336A4EvalDeltaMeasurementBindingError) {
        expect(error.reason).toBe('source_ref_mismatch')
      }
    }
  })
})
