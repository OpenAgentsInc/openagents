import { describe, expect, test } from 'vitest'

import {
  ERCOT_LMP_READ_ONLY_AUTHORITY,
  ERCOT_HUB_SETTLEMENT_POINTS,
  ErcotLmpIngestionUnsafe,
  exampleErcotLmpWindowRecord,
  normalizeErcotLmpResponse,
} from './ercot-lmp-ingestion'

const ingestionIso = '2026-06-20T10:05:00.000Z'

// Minimal fixture representing a single 5-minute SCED interval slice
// from an ERCOT public-reports v2 LMP-by-hub endpoint.
const fixtureResponse = (
  rows: Array<{
    SCEDTimestamp: string
    SettlementPoint: string
    LMPfieldvalue: number
  }> = [
    { SCEDTimestamp: '2026-06-19 14:05:00', SettlementPoint: 'HB_NORTH', LMPfieldvalue: 28.4 },
    { SCEDTimestamp: '2026-06-19 14:10:00', SettlementPoint: 'HB_NORTH', LMPfieldvalue: 31.6 },
    { SCEDTimestamp: '2026-06-19 14:15:00', SettlementPoint: 'HB_NORTH', LMPfieldvalue: 30.0 },
    { SCEDTimestamp: '2026-06-19 14:20:00', SettlementPoint: 'HB_NORTH', LMPfieldvalue: 34.8 },
    // Another settlement point that should be ignored when filtering HB_NORTH
    { SCEDTimestamp: '2026-06-19 14:05:00', SettlementPoint: 'HB_SOUTH', LMPfieldvalue: 999.0 },
  ],
) => ({
  _meta: {
    updated: '2026-06-19T14:25:00',
    totalRecords: rows.length,
    limit: 1000,
    page: 1,
  },
  columns: ['SCEDTimestamp', 'SettlementPoint', 'LMPfieldvalue'],
  data: rows,
})

describe('ERCOT LMP ingestion adapter', () => {
  test('normalises a multi-row ERCOT LMP window into a measured window record', () => {
    const record = normalizeErcotLmpResponse(
      fixtureResponse(),
      'HB_NORTH',
      ingestionIso,
    )

    // Average of 28.4 + 31.6 + 30.0 + 34.8 = 124.8 / 4 = 31.2
    expect(record.averageLmpDollarsPerMwh).toBeCloseTo(31.2, 10)
    expect(record.claimState).toBe('measured')
    expect(record.market).toBe('ercot')
    expect(record.zoneOrSettlementPoint).toBe('HB_NORTH')
    expect(record.rowCount).toBe(4)
    expect(record.windowRef).toBe('window.public.ercot.hb_north.20260619')
    expect(record.evidenceRef).toBe('evidence.public.ercot.lmp.hb_north')
    expect(record.sourceRefs).toContain('source.public.ercot_api_v2_public_reports')
    expect(record.caveatRefs).toContain('caveat.public.ercot_lmp.api_v2_cache_lag')
    expect(record.missingDataFlags).toHaveLength(0)
    expect(record.refreshedAtIso).toBe(ingestionIso)
    expect(record.authority).toEqual(ERCOT_LMP_READ_ONLY_AUTHORITY)
  })

  test('sets authority to read-only with all mutation flags true', () => {
    const record = normalizeErcotLmpResponse(
      fixtureResponse(),
      'HB_NORTH',
      ingestionIso,
    )

    expect(record.authority.noGridDispatch).toBe(true)
    expect(record.authority.noMarketDataMutation).toBe(true)
    expect(record.authority.noSettlementMutation).toBe(true)
    expect(record.authority.noWalletSpend).toBe(true)
    expect(record.authority.authorityBoundary).toBe('read_only_market_data_ingestion')
  })

  test('filters out rows for other settlement points before averaging', () => {
    // Fixture has HB_SOUTH row at 999.0 which must be excluded
    const record = normalizeErcotLmpResponse(
      fixtureResponse(),
      'HB_NORTH',
      ingestionIso,
    )

    expect(record.averageLmpDollarsPerMwh).toBeLessThan(40)
    expect(record.rowCount).toBe(4)
  })

  test('flags sparse windows (fewer than 4 rows) with missing-data ref', () => {
    const sparseRows = [
      { SCEDTimestamp: '2026-06-19 14:05:00', SettlementPoint: 'HB_NORTH', LMPfieldvalue: 28.0 },
      { SCEDTimestamp: '2026-06-19 14:10:00', SettlementPoint: 'HB_NORTH', LMPfieldvalue: 32.0 },
    ]

    const record = normalizeErcotLmpResponse(
      fixtureResponse(sparseRows),
      'HB_NORTH',
      ingestionIso,
    )

    expect(record.missingDataFlags).toContain(
      'missing_data.public.ercot.sparse_interval_window',
    )
    expect(record.rowCount).toBe(2)
  })

  test('flags unknown settlement points with missing-data ref', () => {
    const unknownRows = Array.from({ length: 6 }, (_, i) => ({
      SCEDTimestamp: `2026-06-19 14:0${i}:00`,
      SettlementPoint: 'UNKNOWN_POINT',
      LMPfieldvalue: 30.0,
    }))

    const record = normalizeErcotLmpResponse(
      fixtureResponse(unknownRows),
      'UNKNOWN_POINT',
      ingestionIso,
    )

    expect(record.missingDataFlags).toContain(
      'missing_data.public.ercot.unknown_hub_settlement_point',
    )
  })

  test('throws ErcotLmpIngestionUnsafe for a malformed ERCOT API response', () => {
    for (const malformed of [
      null,
      {},
      { _meta: {}, data: [] },
      { _meta: { updated: 'x', totalRecords: 0, limit: 0, page: 0 }, columns: [], data: 'notanarray' },
      { _meta: { updated: 'x', totalRecords: 0, limit: 0, page: 0 }, columns: [], data: [{ SCEDTimestamp: 1, SettlementPoint: 'HB_NORTH', LMPfieldvalue: 30 }] },
    ]) {
      expect(() =>
        normalizeErcotLmpResponse(malformed, 'HB_NORTH', ingestionIso),
      ).toThrow(ErcotLmpIngestionUnsafe)
    }
  })

  test('throws ErcotLmpIngestionUnsafe when no rows match the settlement point', () => {
    expect(() =>
      normalizeErcotLmpResponse(
        fixtureResponse(),
        'HB_BUSAVG',
        ingestionIso,
      ),
    ).toThrow(ErcotLmpIngestionUnsafe)
  })

  test('covers all seven known ERCOT hub settlement points', () => {
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toHaveLength(7)
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_NORTH')
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_SOUTH')
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_WEST')
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_HOUSTON')
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_HUBAVG')
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_PAN')
    expect(ERCOT_HUB_SETTLEMENT_POINTS).toContain('HB_BUSAVG')
  })

  test('example record has correct shape for use as a modeled ArtanisPylonPowerMarketWindowRecord seed', () => {
    const example = exampleErcotLmpWindowRecord()

    expect(example.market).toBe('ercot')
    expect(example.averageLmpDollarsPerMwh).toBe(31.2)
    expect(example.claimState).toBe('measured')
    expect(example.authority).toEqual(ERCOT_LMP_READ_ONLY_AUTHORITY)
    expect(example.missingDataFlags).toHaveLength(0)
    // windowRef must not contain ISO timestamps (only YYYYMMDD date parts)
    expect(example.windowRef).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/)
    expect(example.evidenceRef).not.toMatch(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/)
  })
})
