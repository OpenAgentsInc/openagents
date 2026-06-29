import { describe, expect, it } from 'vitest'

import { Cs336A4HomeworkStages } from './cs336-a4-data-refinery'
import {
  buildCs336A4SyntheticCorpus,
  Cs336A4RefineryWorkloadError,
  Cs336A4RefineryWorkloadRef,
  runCs336A4RefineryStage,
} from './cs336-a4-refinery-workload'
import { computeCs336A1TokenizerShard } from './cs336-a1-homework-workload'

describe('CS336 A4 refinery workload', () => {
  it('reproduces the output digest on a deterministic_recompute re-run for every stage', async () => {
    for (const stage of Cs336A4HomeworkStages) {
      const first = await runCs336A4RefineryStage({ stage })
      const second = await runCs336A4RefineryStage({ stage })

      expect(first.outputDigestHex).toMatch(/^[0-9a-f]{64}$/)
      expect(second.outputDigestHex).toBe(first.outputDigestHex)
      expect(first.workloadRef).toBe(Cs336A4RefineryWorkloadRef)
      expect(first.inputDocumentCount).toBeGreaterThan(0)
    }
  })

  it('rejects a tampered corpus: a different corpus shape yields a different digest', async () => {
    const baseline = await runCs336A4RefineryStage({ stage: 'exact_line_dedup' })
    const tampered = await runCs336A4RefineryStage({
      documentCount: 32,
      stage: 'exact_line_dedup',
    })

    expect(tampered.outputDigestHex).not.toBe(baseline.outputDigestHex)
  })

  it('masks synthetic PII and reports per-class counts', async () => {
    const result = await runCs336A4RefineryStage({ stage: 'pii_masking' })

    expect(result.stats.maskedTotal).toBeGreaterThan(0)
    expect(result.stats.maskedTotal).toBe(
      result.stats.maskedEmails! +
        result.stats.maskedPhones! +
        result.stats.maskedIpv4!,
    )
  })

  it('removes exact duplicate lines including the shared boilerplate footer', async () => {
    const result = await runCs336A4RefineryStage({ stage: 'exact_line_dedup' })

    expect(result.stats.removedLines).toBeGreaterThan(0)
    expect(result.stats.uniqueLines).toBeLessThan(result.stats.inputLines!)
    expect(result.stats.inputLines).toBe(
      result.stats.uniqueLines! + result.stats.removedLines!,
    )
  })

  it('rejects low-quality documents under Gopher rules without dropping every doc', async () => {
    const result = await runCs336A4RefineryStage({ stage: 'gopher_rules' })

    expect(result.stats.keptDocuments).toBeGreaterThan(0)
    expect(result.stats.keptDocuments).toBeLessThanOrEqual(
      result.stats.inputDocuments!,
    )
    expect(result.stats.inputDocuments).toBe(result.inputDocumentCount)
  })

  it('removes near-duplicate documents under deterministic MinHash dedup', async () => {
    const result = await runCs336A4RefineryStage({ stage: 'minhash_dedup' })

    expect(result.stats.removedDocuments).toBeGreaterThan(0)
    expect(result.stats.keptDocuments).toBe(
      result.stats.inputDocuments! - result.stats.removedDocuments!,
    )
    expect(result.stats.confirmedNearDuplicatePairs).toBeGreaterThan(0)
  })

  it('builds a deterministic synthetic corpus seeded from the A1 tokenizer shard', async () => {
    const shard = await computeCs336A1TokenizerShard()
    const first = buildCs336A4SyntheticCorpus({ shardDigestHex: shard.digestHex })
    const second = buildCs336A4SyntheticCorpus({
      shardDigestHex: shard.digestHex,
    })

    expect(second).toEqual(first)
    expect(() =>
      buildCs336A4SyntheticCorpus({ documentCount: 1, shardDigestHex: 'x' }),
    ).toThrow(Cs336A4RefineryWorkloadError)
  })
})
