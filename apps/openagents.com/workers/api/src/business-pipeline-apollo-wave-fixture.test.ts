import { describe, expect, test } from 'vitest'

import { runOb2ApolloWaveDryRun } from './business-pipeline-apollo-wave-dry-run'
import {
  apolloWaveIngestBodyFromFixture,
  buildOb2ApolloWaveFixture,
  expectedFirstPassApolloWaveCounts,
  expectedReplayApolloWaveCounts,
  expectedSubjectDedupeApolloWaveCounts,
  OB2_APOLLO_WAVE_FIXTURE_MIN_COUNT,
  OB2_LIVE_WAVE_SEGMENT_PAIR,
} from './business-pipeline-apollo-wave-fixture'
import {
  makeD1BusinessPipelineStore,
  type BusinessPipelineRuntime,
} from './business-pipeline-queue'
import { makeSqliteD1 } from './test/sqlite-d1'
import { readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const migrationsDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'migrations')
const migration = (name: string): string =>
  readFileSync(join(migrationsDir, name), 'utf8')

const makeDb = (): { db: D1Database; close: () => void } => {
  const sqlite = makeSqliteD1()
  sqlite.exec(migration('0191_business_signup_requests.sql'))
  sqlite.exec(migration('0270_business_funnel_events.sql'))
  sqlite.exec(migration('0278_business_commitment_ledger.sql'))
  sqlite.exec(migration('0294_business_pipeline_queue.sql'))
  sqlite.exec(migration('0296_business_outreach_sequences.sql'))
  sqlite.exec(migration('0299_business_pipeline_partner_routing.sql'))
  sqlite.exec(migration('0314_business_pipeline_subject_ref.sql'))
  sqlite.exec(migration('0297_business_source_attribution.sql'))
  return { close: sqlite.close, db: sqlite.db }
}

const runtime: BusinessPipelineRuntime = {
  makeId: (prefix: string) => `${prefix}_fixture`,
  nowIso: () => '2026-07-09T12:00:00.000Z',
}

const storeProspectsFromFixture = (
  fixture: ReturnType<typeof buildOb2ApolloWaveFixture>,
) =>
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

describe('OB-2 Apollo wave fixture (≥100 synthetic prospects)', () => {
  test('builds two ≥100-prospect segment waves with public-safe refs only', () => {
    for (const segmentKey of OB2_LIVE_WAVE_SEGMENT_PAIR) {
      const fixture = buildOb2ApolloWaveFixture({
        segmentKey,
        waveId: '20260709a',
      })
      expect(fixture.count).toBe(OB2_APOLLO_WAVE_FIXTURE_MIN_COUNT)
      expect(fixture.prospects).toHaveLength(OB2_APOLLO_WAVE_FIXTURE_MIN_COUNT)
      expect(fixture.suppressedSubjectRefs).toHaveLength(1)
      expect(fixture.prospects[0]?.pipelineRef).toMatch(/^biz-pipe-/)
      expect(fixture.prospects[0]?.subjectRef).toMatch(/^prospect\./)
      expect(JSON.stringify(fixture)).not.toMatch(
        /@|https?:\/\/|www\.|\.com\b|email|phone|token|secret|mnemonic/i,
      )
      const body = apolloWaveIngestBodyFromFixture(fixture)
      expect(body.waveRef).toBe(fixture.waveRef)
      expect(body.prospects).toHaveLength(100)
      expect(expectedFirstPassApolloWaveCounts(fixture)).toEqual({
        acceptedCount: 99,
        duplicateCount: 0,
        suppressedCount: 1,
      })
      expect(expectedReplayApolloWaveCounts(fixture)).toEqual({
        acceptedCount: 0,
        duplicateCount: 99,
        suppressedCount: 1,
      })
      expect(expectedSubjectDedupeApolloWaveCounts(fixture)).toEqual({
        acceptedCount: 0,
        duplicateCount: 99,
        suppressedCount: 1,
      })
    }
  })

  test('proves store-level volume ingest, suppression, replay, and subjectRef dedupe at 100', async () => {
    const { db, close } = makeDb()
    try {
      const store = makeD1BusinessPipelineStore(db)
      const first = buildOb2ApolloWaveFixture({
        count: 100,
        segmentKey: 'agencies_seo',
        waveId: '20260709a',
      })

      for (const [index, subjectRef] of first.suppressedSubjectRefs.entries()) {
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
            `business.outreach.suppression.fixture.${first.segmentKey}.${index + 1}`,
            subjectRef,
            'crm.suppression.fixture',
            runtime.nowIso(),
          )
          .run()
      }

      const firstExpected = expectedFirstPassApolloWaveCounts(first)
      const firstPass = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(first),
          segmentRef: first.segmentRef,
          sourceRef: first.sourceRef,
          waveRef: first.waveRef,
        },
        runtime,
      )
      expect(firstPass).toMatchObject({
        acceptedCount: firstExpected.acceptedCount,
        duplicateCount: firstExpected.duplicateCount,
        schemaVersion: 'openagents.business_pipeline_apollo_wave_ingest.v1',
        suppressedCount: firstExpected.suppressedCount,
        waveRef: first.waveRef,
      })
      expect(firstPass.acceptedCount).toBe(99)
      expect(firstPass.suppressed).toEqual([
        expect.objectContaining({
          subjectRef: 'prospect.agency.050',
        }),
      ])
      expect(
        firstPass.inserted.some(row => row.subjectRef === 'prospect.agency.050'),
      ).toBe(false)
      expect(firstPass.inserted).toContainEqual(
        expect.objectContaining({
          pipelineRef: 'biz-pipe-agency-001',
          subjectRef: 'prospect.agency.001',
        }),
      )

      const replayExpected = expectedReplayApolloWaveCounts(first)
      const replay = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(first),
          segmentRef: first.segmentRef,
          sourceRef: first.sourceRef,
          waveRef: first.waveRef,
        },
        runtime,
      )
      expect(replay).toMatchObject({
        acceptedCount: replayExpected.acceptedCount,
        duplicateCount: replayExpected.duplicateCount,
        suppressedCount: replayExpected.suppressedCount,
      })
      expect(replay.duplicates).toContain('biz-pipe-agency-001')
      expect(replay.duplicates).not.toContain('biz-pipe-agency-050')

      const second = buildOb2ApolloWaveFixture({
        count: 100,
        distinctPipelineRefs: true,
        segmentKey: 'agencies_seo',
        waveId: '20260709b',
      })
      const subjectDedupeExpected = expectedSubjectDedupeApolloWaveCounts(second)
      const subjectDedupe = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(second),
          segmentRef: second.segmentRef,
          sourceRef: second.sourceRef,
          waveRef: second.waveRef,
        },
        runtime,
      )
      expect(subjectDedupe).toMatchObject({
        acceptedCount: subjectDedupeExpected.acceptedCount,
        duplicateCount: subjectDedupeExpected.duplicateCount,
        suppressedCount: subjectDedupeExpected.suppressedCount,
      })
      expect(subjectDedupe.duplicates).toContain(
        'biz-pipe-agency-001-20260709b',
      )
      expect(subjectDedupe.acceptedCount).toBe(0)

      const legal = buildOb2ApolloWaveFixture({
        count: 100,
        segmentKey: 'legal_small_firm',
        suppressIndexes: [],
        waveId: '20260709a',
      })
      const legalPass = await store.ingestApolloWave(
        {
          prospects: storeProspectsFromFixture(legal),
          segmentRef: legal.segmentRef,
          sourceRef: legal.sourceRef,
          waveRef: legal.waveRef,
        },
        runtime,
      )
      expect(legalPass.acceptedCount).toBe(100)
      expect(legalPass.duplicateCount).toBe(0)
      expect(legalPass.suppressedCount).toBe(0)

      const rows = await store.listPipelineRows()
      expect(rows).toHaveLength(199)
      expect(
        rows.filter(row => row.subjectRef === 'prospect.agency.001'),
      ).toHaveLength(1)
      expect(rows.some(row => row.subjectRef === 'prospect.agency.050')).toBe(
        false,
      )
      expect(
        rows.filter(row => row.sourceRef === 'apollo_agent_readiness_agency'),
      ).toHaveLength(99)
      expect(
        rows.filter(row => row.sourceRef === 'apollo_model_custody'),
      ).toHaveLength(100)
    } finally {
      close()
    }
  })

  test('dry-run receipt proves two ≥100 waves with re-wave + suppression + subjectRef dedupe', async () => {
    const receipt = await runOb2ApolloWaveDryRun(100)
    expect(receipt.ok).toBe(true)
    expect(receipt.schemaVersion).toBe(
      'openagents.ob2_apollo_wave_dry_run_receipt.v1',
    )
    expect(receipt.countPerSegment).toBe(100)
    expect(receipt.expectedTotalRows).toBe(198)
    expect(receipt.actualTotalRows).toBe(198)
    expect(receipt.segments).toHaveLength(2)
    for (const segment of receipt.segments) {
      expect(segment.firstPass).toMatchObject({
        acceptedCount: 99,
        duplicateCount: 0,
        suppressedCount: 1,
      })
      expect(segment.replay).toMatchObject({
        acceptedCount: 0,
        duplicateCount: 99,
        suppressedCount: 1,
      })
      expect(segment.subjectDedupe).toMatchObject({
        acceptedCount: 0,
        duplicateCount: 99,
        suppressedCount: 1,
      })
      expect(segment.suppressedSubjectRefs).toHaveLength(1)
    }
    expect(JSON.stringify(receipt)).not.toMatch(
      /@example\.|https?:\/\/|www\.|\.com\b|access_token|refresh_token|private_key|wallet_secret|mnemonic/i,
    )
  })
})
