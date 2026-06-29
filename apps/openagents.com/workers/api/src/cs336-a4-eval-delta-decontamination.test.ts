import { describe, expect, it } from 'vitest'

import {
  assertCs336A4EvalDeltaDecontamination,
  buildCs336A4EvalDeltaDecontaminationReceipt,
  Cs336A4EvalDeltaDecontaminationError,
  Cs336A4EvalDeltaDecontaminationUnsafeMaterialError,
  Cs336A4EvalDeltaDecontaminationValidationError,
  verifyCs336A4EvalDeltaDecontamination,
  type Cs336A4EvalDeltaDecontaminationReceipt,
} from './cs336-a4-eval-delta-decontamination'
import type { Cs336A4EvalDeltaMeasurement } from './cs336-a4-eval-delta-payment'

const SOURCE = 'source.cc_main'
const EVAL_SET = 'eval.cs336_a4.held_out_v1'

const cleanReceiptInput = {
  contaminatedSpansDetected: 3,
  contaminatedSpansRemoved: 3,
  heldOutEvalSetRef: EVAL_SET,
  methodRef: 'method.cs336_a4.ngram_overlap',
  ngramSize: 13,
  postDecontaminationDigestRef: 'digest.corpus.post',
  preDecontaminationDigestRef: 'digest.corpus.pre',
  recomputedPostDigestRef: 'digest.corpus.post',
  sourceRef: SOURCE,
}

const measurement = (
  overrides: Partial<Cs336A4EvalDeltaMeasurement> = {},
): Cs336A4EvalDeltaMeasurement => ({
  baselineScore: 0.4,
  filteredScore: 0.52,
  fixedReferenceModelRef: 'trainer.fixed.cs336_a4.ref_v1',
  heldOutEvalSetRef: EVAL_SET,
  sourceRef: SOURCE,
  ...overrides,
})

describe('buildCs336A4EvalDeltaDecontaminationReceipt', () => {
  it('builds a clean, content-addressed receipt when all detected spans removed and recompute verified', async () => {
    const receipt = await buildCs336A4EvalDeltaDecontaminationReceipt(
      cleanReceiptInput,
    )

    expect(receipt.clean).toBe(true)
    expect(receipt.receiptRef).toMatch(
      /^receipt\.cs336_a4\.eval_delta_decontamination\.[0-9a-f]{16}$/,
    )
    expect(receipt.contentDigestRef).toMatch(/^[0-9a-f]{64}$/)
    expect(receipt.schemaVersion).toBe(
      'openagents.training.data_refinery.eval_delta_decontamination.v1',
    )
  })

  it('is deterministic: same inputs yield the same receiptRef', async () => {
    const a = await buildCs336A4EvalDeltaDecontaminationReceipt(cleanReceiptInput)
    const b = await buildCs336A4EvalDeltaDecontaminationReceipt(cleanReceiptInput)

    expect(a.receiptRef).toBe(b.receiptRef)
    expect(a.contentDigestRef).toBe(b.contentDigestRef)
  })

  it('builds a clean receipt for an uncontaminated corpus (zero spans, unchanged digest)', async () => {
    const receipt = await buildCs336A4EvalDeltaDecontaminationReceipt({
      ...cleanReceiptInput,
      contaminatedSpansDetected: 0,
      contaminatedSpansRemoved: 0,
      postDecontaminationDigestRef: 'digest.corpus.pre',
      recomputedPostDigestRef: 'digest.corpus.pre',
    })

    expect(receipt.clean).toBe(true)
  })

  it('marks not clean when not every detected span was removed', async () => {
    const receipt = await buildCs336A4EvalDeltaDecontaminationReceipt({
      ...cleanReceiptInput,
      contaminatedSpansDetected: 3,
      contaminatedSpansRemoved: 2,
    })

    expect(receipt.clean).toBe(false)
  })

  it('marks not clean when the post-removal digest does not recompute-verify', async () => {
    const receipt = await buildCs336A4EvalDeltaDecontaminationReceipt({
      ...cleanReceiptInput,
      recomputedPostDigestRef: 'digest.corpus.post_tampered',
    })

    expect(receipt.clean).toBe(false)
  })

  it('fails closed when more spans were removed than detected', async () => {
    await expect(
      buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        contaminatedSpansDetected: 1,
        contaminatedSpansRemoved: 2,
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaDecontaminationValidationError)
  })

  it('fails closed when the digest changed but no spans were detected', async () => {
    await expect(
      buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        contaminatedSpansDetected: 0,
        contaminatedSpansRemoved: 0,
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaDecontaminationValidationError)
  })

  it('fails closed when spans were detected but the digest is unchanged', async () => {
    await expect(
      buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        postDecontaminationDigestRef: 'digest.corpus.pre',
        recomputedPostDigestRef: 'digest.corpus.pre',
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaDecontaminationValidationError)
  })

  it('fails closed on a non-positive n-gram size', async () => {
    await expect(
      buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        ngramSize: 0,
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaDecontaminationValidationError)
  })

  it('fails closed on an empty source ref', async () => {
    await expect(
      buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        sourceRef: '   ',
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaDecontaminationValidationError)
  })

  it('fails closed on unsafe material in a ref', async () => {
    await expect(
      buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        methodRef: 'method.with.raw_eval.contents',
      }),
    ).rejects.toBeInstanceOf(Cs336A4EvalDeltaDecontaminationUnsafeMaterialError)
  })
})

describe('verifyCs336A4EvalDeltaDecontamination', () => {
  const cleanReceipt = (): Promise<Cs336A4EvalDeltaDecontaminationReceipt> =>
    buildCs336A4EvalDeltaDecontaminationReceipt(cleanReceiptInput)

  it('covers a measurement on the same source and eval set with a clean receipt', async () => {
    const decontaminationReceipt = await cleanReceipt()
    const result = verifyCs336A4EvalDeltaDecontamination({
      decontaminationReceipt,
      measurement: measurement(),
    })

    expect(result.covered).toBe(true)
    if (result.covered) {
      expect(result.receiptRef).toBe(decontaminationReceipt.receiptRef)
    }
  })

  it('rejects a receipt covering a different source', async () => {
    const decontaminationReceipt = await cleanReceipt()
    const result = verifyCs336A4EvalDeltaDecontamination({
      decontaminationReceipt,
      measurement: measurement({ sourceRef: 'source.other' }),
    })

    expect(result.covered).toBe(false)
    if (!result.covered) {
      expect(result.reason).toBe('source_ref_mismatch')
    }
  })

  it('rejects a receipt covering a different held-out eval set', async () => {
    const decontaminationReceipt = await cleanReceipt()
    const result = verifyCs336A4EvalDeltaDecontamination({
      decontaminationReceipt,
      measurement: measurement({ heldOutEvalSetRef: 'eval.other' }),
    })

    expect(result.covered).toBe(false)
    if (!result.covered) {
      expect(result.reason).toBe('held_out_eval_set_ref_mismatch')
    }
  })

  it('rejects a not-clean receipt', async () => {
    const decontaminationReceipt =
      await buildCs336A4EvalDeltaDecontaminationReceipt({
        ...cleanReceiptInput,
        contaminatedSpansRemoved: 2,
      })
    const result = verifyCs336A4EvalDeltaDecontamination({
      decontaminationReceipt,
      measurement: measurement(),
    })

    expect(result.covered).toBe(false)
    if (!result.covered) {
      expect(result.reason).toBe('receipt_not_clean')
    }
  })

  it('fails closed when the measurement source is empty', async () => {
    const decontaminationReceipt = await cleanReceipt()
    expect(() =>
      verifyCs336A4EvalDeltaDecontamination({
        decontaminationReceipt,
        measurement: measurement({ sourceRef: '   ' }),
      }),
    ).toThrow(Cs336A4EvalDeltaDecontaminationValidationError)
  })
})

describe('assertCs336A4EvalDeltaDecontamination', () => {
  it('returns the receipt ref when a clean receipt covers the measurement', async () => {
    const decontaminationReceipt = await buildCs336A4EvalDeltaDecontaminationReceipt(
      cleanReceiptInput,
    )
    expect(
      assertCs336A4EvalDeltaDecontamination({
        decontaminationReceipt,
        measurement: measurement(),
      }),
    ).toBe(decontaminationReceipt.receiptRef)
  })

  it('throws a tagged error carrying the mismatch reason when not covered', async () => {
    const decontaminationReceipt = await buildCs336A4EvalDeltaDecontaminationReceipt(
      cleanReceiptInput,
    )
    try {
      assertCs336A4EvalDeltaDecontamination({
        decontaminationReceipt,
        measurement: measurement({ heldOutEvalSetRef: 'eval.other' }),
      })
      expect.unreachable('expected an uncovered measurement to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4EvalDeltaDecontaminationError)
      if (error instanceof Cs336A4EvalDeltaDecontaminationError) {
        expect(error.reason).toBe('held_out_eval_set_ref_mismatch')
      }
    }
  })
})
