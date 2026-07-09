/**
 * OB-2 (#8559) in-memory dry-run for Apollo segment waves.
 *
 * Uses the real D1 store over `node:sqlite` (same double as Worker API tests).
 * Proves two ≥100 synthetic waves, suppression, idempotent re-wave, and
 * subjectRef dedupe without Apollo MCP or production writes.
 */

import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  buildOb2ApolloWaveFixture,
  expectedFirstPassApolloWaveCounts,
  expectedReplayApolloWaveCounts,
  expectedSubjectDedupeApolloWaveCounts,
  OB2_LIVE_WAVE_SEGMENT_PAIR,
  type Ob2ApolloWaveFixture,
} from './business-pipeline-apollo-wave-fixture'
import {
  makeD1BusinessPipelineStore,
  type BusinessPipelineRuntime,
} from './business-pipeline-queue'
import { makeSqliteD1 } from './test/sqlite-d1'

const migrationsDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'migrations',
)

const migration = (name: string): string =>
  readFileSync(join(migrationsDir, name), 'utf8')

const bootstrapPipelineDb = (): ReturnType<typeof makeSqliteD1> => {
  const sqlite = makeSqliteD1()
  sqlite.exec(migration('0191_business_signup_requests.sql'))
  sqlite.exec(migration('0270_business_funnel_events.sql'))
  sqlite.exec(migration('0278_business_commitment_ledger.sql'))
  sqlite.exec(migration('0294_business_pipeline_queue.sql'))
  sqlite.exec(migration('0296_business_outreach_sequences.sql'))
  sqlite.exec(migration('0299_business_pipeline_partner_routing.sql'))
  sqlite.exec(migration('0314_business_pipeline_subject_ref.sql'))
  sqlite.exec(migration('0297_business_source_attribution.sql'))
  return sqlite
}

const runtime: BusinessPipelineRuntime = {
  makeId: (prefix: string) => `${prefix}_wave_dry_run`,
  nowIso: () => '2026-07-09T12:00:00.000Z',
}

const seedSuppressions = async (
  db: D1Database,
  fixture: Ob2ApolloWaveFixture,
): Promise<void> => {
  for (const [index, subjectRef] of fixture.suppressedSubjectRefs.entries()) {
    await db
      .prepare(
        `INSERT INTO business_outreach_suppressions (
          suppression_ref,
          subject_ref,
          reason,
          source_ref,
          created_at
        ) VALUES (?, ?, 'existing_customer', ?, ?)`,
      )
      .bind(
        `business.outreach.suppression.dry_run.${fixture.segmentKey}.${index + 1}`,
        subjectRef,
        'crm.suppression.dry_run',
        runtime.nowIso(),
      )
      .run()
  }
}

const storeProspectsFromFixture = (fixture: Ob2ApolloWaveFixture) =>
  fixture.prospects.map(prospect => ({
    pipelineRef: prospect.pipelineRef,
    quotedBand: {
      label: prospect.quotedBandLabel,
      maxUsdCents: prospect.quotedMaxUsdCents,
      minUsdCents: prospect.quotedMinUsdCents,
    },
    subjectRef: prospect.subjectRef,
    vertical: prospect.vertical,
  }))

const summarizeWave = (wave: {
  acceptedCount: number
  duplicateCount: number
  suppressedCount: number
  waveRef: string
  sourceRef: string
  segmentRef: string
}) => ({
  acceptedCount: wave.acceptedCount,
  duplicateCount: wave.duplicateCount,
  segmentRef: wave.segmentRef,
  sourceRef: wave.sourceRef,
  suppressedCount: wave.suppressedCount,
  waveRef: wave.waveRef,
})

export type Ob2ApolloWaveDryRunReceipt = Readonly<{
  actualTotalRows: number
  countPerSegment: number
  expectedTotalRows: number
  note: string
  ok: true
  privacyBoundary: Readonly<{
    excludes: ReadonlyArray<string>
    opaqueRefsOnly: true
  }>
  schemaVersion: 'openagents.ob2_apollo_wave_dry_run_receipt.v1'
  segments: ReadonlyArray<{
    firstPass: ReturnType<typeof summarizeWave>
    replay: ReturnType<typeof summarizeWave>
    segmentKey: string
    subjectDedupe: ReturnType<typeof summarizeWave>
    suppressedSubjectRefs: ReadonlyArray<string>
  }>
}>

/**
 * Run the fixture-tier two-wave proof against an in-memory D1.
 * Throws if any gate fails.
 */
export const runOb2ApolloWaveDryRun = async (
  count: number = 100,
): Promise<Ob2ApolloWaveDryRunReceipt> => {
  if (!Number.isInteger(count) || count < 1 || count > 500) {
    throw new Error('dry-run count must be an integer 1-500')
  }

  const sqlite = bootstrapPipelineDb()
  try {
    const store = makeD1BusinessPipelineStore(sqlite.db)
    const segments: Ob2ApolloWaveDryRunReceipt['segments'] = []

    for (const segmentKey of OB2_LIVE_WAVE_SEGMENT_PAIR) {
      const first = buildOb2ApolloWaveFixture({
        count,
        segmentKey,
        waveId: 'dryrun-a',
      })
      await seedSuppressions(sqlite.db, first)

      const firstPass = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(first),
          segmentRef: first.segmentRef,
          sourceRef: first.sourceRef,
          waveRef: first.waveRef,
        },
        runtime,
      )
      const firstExpected = expectedFirstPassApolloWaveCounts(first)
      if (
        firstPass.acceptedCount !== firstExpected.acceptedCount ||
        firstPass.duplicateCount !== firstExpected.duplicateCount ||
        firstPass.suppressedCount !== firstExpected.suppressedCount
      ) {
        throw new Error(
          `first-pass mismatch for ${segmentKey}: got ${JSON.stringify(summarizeWave(firstPass))}, expected ${JSON.stringify(firstExpected)}`,
        )
      }

      const replay = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(first),
          segmentRef: first.segmentRef,
          sourceRef: first.sourceRef,
          waveRef: first.waveRef,
        },
        runtime,
      )
      const replayExpected = expectedReplayApolloWaveCounts(first)
      if (
        replay.acceptedCount !== replayExpected.acceptedCount ||
        replay.duplicateCount !== replayExpected.duplicateCount ||
        replay.suppressedCount !== replayExpected.suppressedCount
      ) {
        throw new Error(
          `replay mismatch for ${segmentKey}: got ${JSON.stringify(summarizeWave(replay))}, expected ${JSON.stringify(replayExpected)}`,
        )
      }

      const second = buildOb2ApolloWaveFixture({
        count,
        distinctPipelineRefs: true,
        segmentKey,
        waveId: 'dryrun-b',
      })
      const subjectDedupe = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(second),
          segmentRef: second.segmentRef,
          sourceRef: second.sourceRef,
          waveRef: second.waveRef,
        },
        runtime,
      )
      const subjectExpected = expectedSubjectDedupeApolloWaveCounts(second)
      if (
        subjectDedupe.acceptedCount !== subjectExpected.acceptedCount ||
        subjectDedupe.duplicateCount !== subjectExpected.duplicateCount ||
        subjectDedupe.suppressedCount !== subjectExpected.suppressedCount
      ) {
        throw new Error(
          `subject-dedupe mismatch for ${segmentKey}: got ${JSON.stringify(summarizeWave(subjectDedupe))}, expected ${JSON.stringify(subjectExpected)}`,
        )
      }

      segments.push({
        firstPass: summarizeWave(firstPass),
        replay: summarizeWave(replay),
        segmentKey,
        subjectDedupe: summarizeWave(subjectDedupe),
        suppressedSubjectRefs: first.suppressedSubjectRefs,
      })
    }

    const rows = await store.listPipelineRows()
    const expectedTotalRows = OB2_LIVE_WAVE_SEGMENT_PAIR.reduce(
      (sum, segmentKey) => {
        const fixture = buildOb2ApolloWaveFixture({
          count,
          segmentKey,
          waveId: 'dryrun-a',
        })
        return sum + expectedFirstPassApolloWaveCounts(fixture).acceptedCount
      },
      0,
    )

    if (rows.length !== expectedTotalRows) {
      throw new Error(
        `row count mismatch: got ${rows.length}, expected ${expectedTotalRows}`,
      )
    }

    for (const segment of segments) {
      for (const subjectRef of segment.suppressedSubjectRefs) {
        if (rows.some(row => row.subjectRef === subjectRef)) {
          throw new Error(`suppressed subject entered queue: ${subjectRef}`)
        }
      }
    }

    return {
      actualTotalRows: rows.length,
      countPerSegment: count,
      expectedTotalRows,
      note: 'Fixture-tier proof only. Live ≥100 Apollo MCP waves still required to close #8559.',
      ok: true,
      privacyBoundary: {
        excludes: [
          'names',
          'domains',
          'emails',
          'phones',
          'raw_apollo_payloads',
        ],
        opaqueRefsOnly: true,
      },
      schemaVersion: 'openagents.ob2_apollo_wave_dry_run_receipt.v1',
      segments,
    }
  } finally {
    sqlite.close()
  }
}
