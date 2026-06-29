import { describe, expect, it } from 'vitest'

import {
  Cs336A4ProvenanceSchemaVersion,
  Cs336A4ProvenanceUnsafeMaterialError,
  Cs336A4ProvenanceValidationError,
  buildCs336A4ProvenanceReceipt,
  type Cs336A4TransformStep,
} from './cs336-a4-provenance'

const provenance = {
  acquisitionMode: 'bounded_synthetic_corpus' as const,
  licenseRef: 'license.public.cc0.synthetic_corpus_v1',
  snapshotRef: 'snapshot.cs336_a4.seed.digest.0001',
  sourceRef: 'source.psion.bounded_synthetic_mixture.v1',
}

const sourceInputDigestRef = 'digest.cs336_a4.source.input.0001'

/**
 * A linked, recompute-verified four-stage chain over the landed Psionic
 * stages: each stage consumes the prior stage's output.
 */
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
  {
    codeVersionRef: 'psionic.refinery.v1.exact_line_dedup',
    inputDigestRef: 'digest.cs336_a4.stage.gopher',
    outputDigestRef: 'digest.cs336_a4.stage.exact',
    recomputedDigestRef: 'digest.cs336_a4.stage.exact',
    stage: 'exact_line_dedup',
  },
  {
    codeVersionRef: 'psionic.refinery.v1.minhash_dedup',
    inputDigestRef: 'digest.cs336_a4.stage.exact',
    outputDigestRef: 'digest.cs336_a4.stage.minhash',
    recomputedDigestRef: 'digest.cs336_a4.stage.minhash',
    stage: 'minhash_dedup',
  },
]

const baseInput = {
  assignmentRef: 'assignment.cs336_a4.shard.1',
  finalOutputDigestRef: 'digest.cs336_a4.stage.minhash',
  inputShardRef: 'shard.public.cs336_a4.1',
  provenance,
  sourceInputDigestRef,
  transformChain: linkedChain,
}

describe('CS336 A4 corpus provenance receipt', () => {
  it('binds source provenance to a linked, recompute-verified transform chain', async () => {
    const receipt = await buildCs336A4ProvenanceReceipt(baseInput)

    expect(receipt.schemaVersion).toBe(Cs336A4ProvenanceSchemaVersion)
    expect(receipt.recomputeVerified).toBe(true)
    expect(receipt.transformChain.map(step => step.stage)).toEqual([
      'pii_masking',
      'gopher_rules',
      'exact_line_dedup',
      'minhash_dedup',
    ])
    expect(receipt.provenance.licenseRef).toBe(provenance.licenseRef)
    expect(receipt.finalOutputDigestRef).toBe('digest.cs336_a4.stage.minhash')
    expect(receipt.receiptRef).toContain(
      'receipt.cs336_a4.provenance.assignment.cs336_a4.shard.1.',
    )
    expect(receipt.contentDigestRef).toMatch(/^[0-9a-f]{64}$/)
  })

  it('is content-addressed: the same provenance and chain yield the same receipt ref', async () => {
    const first = await buildCs336A4ProvenanceReceipt(baseInput)
    const second = await buildCs336A4ProvenanceReceipt(baseInput)

    expect(second.receiptRef).toBe(first.receiptRef)
    expect(second.contentDigestRef).toBe(first.contentDigestRef)
  })

  it('changes the receipt ref when the source license changes', async () => {
    const first = await buildCs336A4ProvenanceReceipt(baseInput)
    const second = await buildCs336A4ProvenanceReceipt({
      ...baseInput,
      provenance: {
        ...provenance,
        licenseRef: 'license.public.cc0.synthetic_corpus_v2',
      },
    })

    expect(second.contentDigestRef).not.toBe(first.contentDigestRef)
  })

  it('rejects a transform chain that is not linked', async () => {
    await expect(
      buildCs336A4ProvenanceReceipt({
        ...baseInput,
        transformChain: [
          linkedChain[0]!,
          {
            ...linkedChain[1]!,
            inputDigestRef: 'digest.cs336_a4.stage.WRONG',
          },
          linkedChain[2]!,
          linkedChain[3]!,
        ],
      }),
    ).rejects.toBeInstanceOf(Cs336A4ProvenanceValidationError)
  })

  it('rejects a chain whose first input does not match the source input digest', async () => {
    await expect(
      buildCs336A4ProvenanceReceipt({
        ...baseInput,
        sourceInputDigestRef: 'digest.cs336_a4.source.input.MISMATCH',
      }),
    ).rejects.toBeInstanceOf(Cs336A4ProvenanceValidationError)
  })

  it('rejects a final output digest that does not match the last stage output', async () => {
    await expect(
      buildCs336A4ProvenanceReceipt({
        ...baseInput,
        finalOutputDigestRef: 'digest.cs336_a4.stage.NOT_LAST',
      }),
    ).rejects.toBeInstanceOf(Cs336A4ProvenanceValidationError)
  })

  it('rejects a step whose recomputed digest does not match its output', async () => {
    await expect(
      buildCs336A4ProvenanceReceipt({
        ...baseInput,
        transformChain: [
          {
            ...linkedChain[0]!,
            recomputedDigestRef: 'digest.cs336_a4.stage.pii.TAMPERED',
          },
          linkedChain[1]!,
          linkedChain[2]!,
          linkedChain[3]!,
        ],
      }),
    ).rejects.toBeInstanceOf(Cs336A4ProvenanceValidationError)
  })

  it('rejects an empty transform chain', async () => {
    await expect(
      buildCs336A4ProvenanceReceipt({ ...baseInput, transformChain: [] }),
    ).rejects.toBeInstanceOf(Cs336A4ProvenanceValidationError)
  })

  it('fails closed on private or wallet material in provenance refs', async () => {
    await expect(
      buildCs336A4ProvenanceReceipt({
        ...baseInput,
        provenance: {
          ...provenance,
          sourceRef: '/Users/operator/private/raw-crawl.warc',
        },
      }),
    ).rejects.toBeInstanceOf(Cs336A4ProvenanceUnsafeMaterialError)
  })
})
