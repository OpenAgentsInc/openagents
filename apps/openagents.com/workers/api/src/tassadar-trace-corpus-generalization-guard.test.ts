import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'

import {
  tassadarGeneralizationGuardDigest,
  validateTassadarGeneralizationGuard,
  type TassadarGeneralizationGuardManifest,
} from './tassadar-trace-factory/generalization-guard'
import { buildMirrorCodeRun } from './inference/gym/mirrorcode-contract'

const corpusDir = join(import.meta.dirname, '..', 'corpus')

const readManifest = (fileName: string): TassadarGeneralizationGuardManifest =>
  JSON.parse(readFileSync(join(corpusDir, fileName), 'utf8')) as TassadarGeneralizationGuardManifest

describe('Tassadar trace corpus generalization guard', () => {
  test.each([
    [
      'tassadar-trace-corpus.v0_1.manifest.json',
      '0b8d825543cb6ba2edb67b13721b7c5170e35c533a7ca408f272cf9adfee61fb',
      46,
      456800,
    ],
    [
      'tassadar-trace-corpus.v0_2.w3_100m.manifest.json',
      '9d5c5b833ef6b8cac1c308eec77986824786a3118a72d1c93aa1f293479cfe8b',
      328,
      4798560,
    ],
  ])('%s declares a checksum-only GG held-out partition', async (
    fileName,
    expectedDigest,
    expectedHeldOutRecords,
    expectedHeldOutTokens,
  ) => {
    const manifest = readManifest(fileName)
    const guard = manifest.generalizationGuard
    expect(guard).toBeDefined()
    expect(await validateTassadarGeneralizationGuard(manifest)).toEqual([])
    expect(await tassadarGeneralizationGuardDigest(manifest, guard!)).toBe(expectedDigest)

    const heldOut = guard!.partitions.find(
      partition => partition.kind === 'gg_held_out',
    )
    const train = guard!.partitions.find(
      partition => partition.kind === 'train_allowed',
    )
    expect(heldOut).toMatchObject({
      recordCount: expectedHeldOutRecords,
      split: 'eval_heldout_family',
      tokenCount: expectedHeldOutTokens,
    })
    expect(heldOut?.shardRefs.every(ref => ref.includes('#'))).toBe(true)
    expect(heldOut?.familyIds).toEqual([
      'family.application_state_machine.v1',
      'family.stack_loop_sum.compiled.v1',
    ])
    expect(
      heldOut?.familyIds.some(familyId => train?.familyIds.includes(familyId)),
    ).toBe(false)
    expect(guard!.forbiddenConsumers).toEqual(
      expect.arrayContaining([
        'training',
        'memory_context',
        'trace_homework',
        'rag',
        'optimization',
        'homework_loop',
      ]),
    )
  })

  test('detects guard weakening if a held-out family is admitted to training', async () => {
    const manifest = readManifest('tassadar-trace-corpus.v0_2.w3_100m.manifest.json')
    const guard = manifest.generalizationGuard!
    const weakened: TassadarGeneralizationGuardManifest = {
      ...manifest,
      generalizationGuard: {
        ...guard,
        partitions: guard.partitions.map(partition =>
          partition.kind === 'train_allowed'
            ? {
                ...partition,
                familyIds: [
                  ...partition.familyIds,
                  'family.application_state_machine.v1',
                ],
              }
            : partition,
        ),
      },
    }

    await expect(validateTassadarGeneralizationGuard(weakened)).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ kind: 'partition_overlap' }),
        expect.objectContaining({ kind: 'guard_digest_mismatch' }),
      ]),
    )
  })

  test('detects checksum tampering on a held-out shard ref', async () => {
    const manifest = readManifest('tassadar-trace-corpus.v0_2.w3_100m.manifest.json')
    const guard = manifest.generalizationGuard!
    const tampered: TassadarGeneralizationGuardManifest = {
      ...manifest,
      generalizationGuard: {
        ...guard,
        partitions: guard.partitions.map(partition =>
          partition.kind === 'gg_held_out'
            ? {
                ...partition,
                shardRefs: partition.shardRefs.map((ref, index) =>
                  index === 0 ? ref.replace(/.$/, '0') : ref,
                ),
              }
            : partition,
        ),
      },
    }

    await expect(validateTassadarGeneralizationGuard(tampered)).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'held_out_shard_ref_checksum_mismatch',
        }),
        expect.objectContaining({ kind: 'guard_digest_mismatch' }),
      ]),
    )
  })

  test('detects held-out partition count drift against manifest rows', async () => {
    const manifest = readManifest('tassadar-trace-corpus.v0_1.manifest.json')
    const guard = manifest.generalizationGuard!
    const drifted: TassadarGeneralizationGuardManifest = {
      ...manifest,
      generalizationGuard: {
        ...guard,
        partitions: guard.partitions.map(partition =>
          partition.kind === 'gg_held_out'
            ? { ...partition, recordCount: partition.recordCount + 1 }
            : partition,
        ),
      },
    }

    await expect(validateTassadarGeneralizationGuard(drifted)).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'held_out_partition_count_mismatch',
        }),
        expect.objectContaining({ kind: 'guard_digest_mismatch' }),
      ]),
    )
  })

  test('MirrorCode public tasks are labeled as Khala GG evidence', () => {
    const run = buildMirrorCodeRun({
      runId: 'mc-decision-cal-py-gg-0001',
      model: 'openagents/khala',
      taskId: 'cal',
      bucket: 'S',
      language: 'python',
      status: 'failed',
      passRate: 0.5,
      tokens: { total: 1 },
      startedAt: '2026-06-27T00:00:00.000Z',
      finishedAt: '2026-06-27T00:01:00.000Z',
      summary: 'Public MirrorCode GG set run for cal.',
      grade: 'decision_grade',
    })

    expect(run.generalizationSet).toBe('mirrorcode_public_tasks_no_rag')
    expect(run.memoryPolicy).toBe('no_rag_public_tasks_only')
    expect(run.decisionGrade).toBe(true)
  })
})
