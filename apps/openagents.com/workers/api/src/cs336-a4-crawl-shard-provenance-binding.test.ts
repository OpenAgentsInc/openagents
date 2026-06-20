import { describe, expect, it } from 'vitest'

import { deriveCs336A4CrawlShardAssignment } from './cs336-a4-crawl-shard-assignment'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'
import {
  assertCs336A4CrawlShardProvenanceBinding,
  Cs336A4CrawlShardProvenanceBindingError,
  verifyCs336A4CrawlShardProvenanceBinding,
} from './cs336-a4-crawl-shard-provenance-binding'
import {
  buildCs336A4ProvenanceReceipt,
  type Cs336A4ProvenanceReceipt,
  type Cs336A4SourceProvenance,
} from './cs336-a4-provenance'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildAssignment = async (index = 1) => {
  const plan = await buildCs336A4CrawlShardPlan({
    descriptor,
    targetShardCount: 4,
  })

  return deriveCs336A4CrawlShardAssignment({ index, plan })
}

const buildReceiptFor = (
  input: Readonly<{
    assignmentRef: string
    inputShardRef: string
    provenance: Cs336A4SourceProvenance
  }>,
): Promise<Cs336A4ProvenanceReceipt> =>
  buildCs336A4ProvenanceReceipt({
    assignmentRef: input.assignmentRef,
    finalOutputDigestRef: 'digest.stage.minhash_dedup',
    inputShardRef: input.inputShardRef,
    provenance: input.provenance,
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

describe('verifyCs336A4CrawlShardProvenanceBinding', () => {
  it('binds a receipt built from the assignment to it', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: assignment.assignmentRef,
      inputShardRef: assignment.inputShardRef,
      provenance: assignment.provenanceSource,
    })

    const result = verifyCs336A4CrawlShardProvenanceBinding({
      assignment,
      receipt,
    })

    expect(result.bound).toBe(true)
    if (result.bound) {
      expect(result.assignmentRef).toBe(assignment.assignmentRef)
      expect(result.inputShardRef).toBe(assignment.inputShardRef)
      expect(result.provenanceReceiptRef).toBe(receipt.receiptRef)
    }
  })

  it('rejects a receipt that closes out a different assignment', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: 'assignment.cs336_a4.crawl_shard.other.0_2.deadbeefdeadbeef',
      inputShardRef: assignment.inputShardRef,
      provenance: assignment.provenanceSource,
    })

    const result = verifyCs336A4CrawlShardProvenanceBinding({
      assignment,
      receipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('assignment_ref_mismatch')
    }
  })

  it('rejects a receipt whose input shard ref is not the assigned shard', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: assignment.assignmentRef,
      inputShardRef: 'shard.cc_main.2026_05.8_10.bogus',
      provenance: assignment.provenanceSource,
    })

    const result = verifyCs336A4CrawlShardProvenanceBinding({
      assignment,
      receipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('input_shard_ref_mismatch')
    }
  })

  it('rejects a receipt that re-attributes the corpus to a different source', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: assignment.assignmentRef,
      inputShardRef: assignment.inputShardRef,
      provenance: {
        ...assignment.provenanceSource,
        sourceRef: 'source.some_other_crawl',
      },
    })

    const result = verifyCs336A4CrawlShardProvenanceBinding({
      assignment,
      receipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('source_ref_mismatch')
    }
  })

  it('rejects a receipt that re-attributes the corpus to a different snapshot', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: assignment.assignmentRef,
      inputShardRef: assignment.inputShardRef,
      provenance: {
        ...assignment.provenanceSource,
        snapshotRef: 'snapshot.cc_main.2026_06',
      },
    })

    const result = verifyCs336A4CrawlShardProvenanceBinding({
      assignment,
      receipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('snapshot_ref_mismatch')
    }
  })

  it('rejects a receipt that swaps the license it was admitted under', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: assignment.assignmentRef,
      inputShardRef: assignment.inputShardRef,
      provenance: {
        ...assignment.provenanceSource,
        licenseRef: 'license.public.cc_main.2099_01',
      },
    })

    const result = verifyCs336A4CrawlShardProvenanceBinding({
      assignment,
      receipt,
    })

    expect(result.bound).toBe(false)
    if (!result.bound) {
      expect(result.reason).toBe('license_ref_mismatch')
    }
  })
})

describe('assertCs336A4CrawlShardProvenanceBinding', () => {
  it('returns the provenance receipt ref when the binding holds', async () => {
    const assignment = await buildAssignment(2)
    const receipt = await buildReceiptFor({
      assignmentRef: assignment.assignmentRef,
      inputShardRef: assignment.inputShardRef,
      provenance: assignment.provenanceSource,
    })

    expect(
      assertCs336A4CrawlShardProvenanceBinding({ assignment, receipt }),
    ).toBe(receipt.receiptRef)
  })

  it('throws a tagged error carrying the mismatch reason when unbound', async () => {
    const assignment = await buildAssignment()
    const receipt = await buildReceiptFor({
      assignmentRef: 'assignment.cs336_a4.crawl_shard.other.0_2.deadbeefdeadbeef',
      inputShardRef: assignment.inputShardRef,
      provenance: assignment.provenanceSource,
    })

    try {
      assertCs336A4CrawlShardProvenanceBinding({ assignment, receipt })
      expect.unreachable('expected an unbound binding to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardProvenanceBindingError)
      if (error instanceof Cs336A4CrawlShardProvenanceBindingError) {
        expect(error.reason).toBe('assignment_ref_mismatch')
      }
    }
  })
})
