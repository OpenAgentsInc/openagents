import { describe, expect, it } from 'vitest'

import {
  deriveCs336A4CrawlShardAssignment,
  type Cs336A4CrawlShardAssignment,
} from './cs336-a4-crawl-shard-assignment'
import {
  assertCs336A4CrawlShardAssignmentAuthenticity,
  Cs336A4CrawlShardAssignmentAuthenticityError,
  verifyCs336A4CrawlShardAssignmentAuthenticity,
} from './cs336-a4-crawl-shard-assignment-authenticity'
import {
  buildCs336A4CrawlShardPlan,
  type Cs336A4CrawlSnapshotDescriptor,
} from './cs336-a4-crawl-shard-plan'

const descriptor: Cs336A4CrawlSnapshotDescriptor = {
  acquisitionMode: 'public_crawl_snapshot',
  licenseRef: 'license.public.cc_main.2026_05',
  segmentCount: 10,
  snapshotRef: 'snapshot.cc_main.2026_05',
  sourceRef: 'source.cc_main',
}

const buildPlan = () =>
  buildCs336A4CrawlShardPlan({ descriptor, targetShardCount: 4 })

const buildGenuineAssignment = async (index = 1) => {
  const plan = await buildPlan()
  const assignment = await deriveCs336A4CrawlShardAssignment({ index, plan })
  return { assignment, plan }
}

// Re-mints the content-addressed refs the way the builder does, so a forged
// field can be paired with refs that are self-consistent for the FORGED body
// (the hard case: a tampered assignment whose own digest matches its tampered
// body, which only re-derivation against the trusted plan can catch).
const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )
  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

describe('verifyCs336A4CrawlShardAssignmentAuthenticity', () => {
  it('accepts the assignment the plan deterministically derives', async () => {
    const { assignment, plan } = await buildGenuineAssignment(2)

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment,
      plan,
    })

    expect(result.authentic).toBe(true)
    if (result.authentic) {
      expect(result.assignmentRef).toBe(assignment.assignmentRef)
      expect(result.contentDigestRef).toBe(assignment.contentDigestRef)
      expect(result.index).toBe(2)
      expect(result.planRef).toBe(plan.planRef)
    }
  })

  it('accepts a freshly re-derived assignment (round-trip is stable)', async () => {
    const { plan } = await buildGenuineAssignment(0)
    const reDerived = await deriveCs336A4CrawlShardAssignment({ index: 3, plan })

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: reDerived,
      plan,
    })

    expect(result.authentic).toBe(true)
  })

  it('rejects an out-of-range index', async () => {
    const { assignment, plan } = await buildGenuineAssignment(0)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      index: plan.shards.length,
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('index_out_of_range')
    }
  })

  it('rejects a non-integer index', async () => {
    const { assignment, plan } = await buildGenuineAssignment(0)
    const forged: Cs336A4CrawlShardAssignment = { ...assignment, index: 1.5 }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('index_out_of_range')
    }
  })

  it('rejects a forged planRef', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      planRef: 'plan.cs336_a4.crawl_shard.forged.0000000000000000',
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('plan_ref_mismatch')
    }
  })

  it('rejects a forged inputShardRef', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      inputShardRef: 'shard.cs336_a4.crawl.forged.0_1.0000000000000000',
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('input_shard_ref_mismatch')
    }
  })

  it('rejects a tampered segment range', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      endSegment: assignment.endSegment + 1,
      segmentCount: assignment.segmentCount + 1,
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('segment_range_mismatch')
    }
  })

  it('rejects a re-attributed provenance source', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      provenanceSource: {
        ...assignment.provenanceSource,
        sourceRef: 'source.easier_corpus',
      },
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('provenance_source_mismatch')
    }
  })

  it('rejects a stale assignmentRef whose body otherwise matches', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      assignmentRef: `${assignment.assignmentRef}.stale`,
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('assignment_ref_mismatch')
    }
  })

  it('catches a self-consistent forgery: digest recomputed over a tampered body', async () => {
    // The forger keeps planRef + segment range + provenance honest but swaps
    // the source to an easier corpus AND recomputes contentDigestRef over the
    // tampered body so the assignment is internally self-consistent. Only the
    // provenance comparison against the trusted plan catches it.
    const { assignment, plan } = await buildGenuineAssignment(1)
    const tamperedDigest = await sha256Hex('forged-canonical-body')
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      contentDigestRef: tamperedDigest,
      provenanceSource: {
        ...assignment.provenanceSource,
        licenseRef: 'license.unverified',
      },
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('provenance_source_mismatch')
    }
  })

  it('catches a tampered contentDigestRef when the body fields are honest', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      contentDigestRef: `${assignment.contentDigestRef.slice(0, 60)}deadbeef`,
    }

    const result = await verifyCs336A4CrawlShardAssignmentAuthenticity({
      assignment: forged,
      plan,
    })

    expect(result.authentic).toBe(false)
    if (!result.authentic) {
      expect(result.reason).toBe('content_digest_ref_mismatch')
    }
  })
})

describe('assertCs336A4CrawlShardAssignmentAuthenticity', () => {
  it('returns the recomputed assignmentRef for a genuine assignment', async () => {
    const { assignment, plan } = await buildGenuineAssignment(2)

    await expect(
      assertCs336A4CrawlShardAssignmentAuthenticity({ assignment, plan }),
    ).resolves.toBe(assignment.assignmentRef)
  })

  it('throws a tagged error carrying the mismatch reason', async () => {
    const { assignment, plan } = await buildGenuineAssignment(1)
    const forged: Cs336A4CrawlShardAssignment = {
      ...assignment,
      planRef: 'plan.cs336_a4.crawl_shard.forged.0000000000000000',
    }

    await expect(
      assertCs336A4CrawlShardAssignmentAuthenticity({
        assignment: forged,
        plan,
      }),
    ).rejects.toBeInstanceOf(Cs336A4CrawlShardAssignmentAuthenticityError)

    try {
      await assertCs336A4CrawlShardAssignmentAuthenticity({
        assignment: forged,
        plan,
      })
      expect.unreachable('expected authenticity assertion to throw')
    } catch (error) {
      expect(error).toBeInstanceOf(Cs336A4CrawlShardAssignmentAuthenticityError)
      if (error instanceof Cs336A4CrawlShardAssignmentAuthenticityError) {
        expect(error.reason).toBe('plan_ref_mismatch')
      }
    }
  })
})
