import { Option, Schema as S } from 'effect'

// ERCOT Public API v2 — settlement point LMP ingestion adapter.
//
// Endpoint families this adapter covers:
//   np6-785-cd  LMP by settled point (hub): real-time and historical
//   np4-191-cd  Real-time settlement point prices
//
// Authority boundary: read-only market-data ingestion.  This module
// validates and normalises raw (pre-parsed) ERCOT API payloads into
// typed window records for the Artanis/Pylon comparative economics
// pipeline.  It does NOT dispatch grid events, mutate market data,
// modify settlements, or spend wallets.
//
// Note on JSON.parse: callers must parse the HTTP response body at
// the json-boundary layer before passing the result here as `unknown`.

// ── ERCOT API response shape ─────────────────────────────────────────

export const ErcotLmpRowSchema = S.Struct({
  SCEDTimestamp: S.String,
  SettlementPoint: S.String,
  LMPfieldvalue: S.Number,
})
export type ErcotLmpRow = typeof ErcotLmpRowSchema.Type

export const ErcotLmpMetaSchema = S.Struct({
  updated: S.String,
  totalRecords: S.Number,
  limit: S.Number,
  page: S.Number,
})
export type ErcotLmpMeta = typeof ErcotLmpMetaSchema.Type

export const ErcotLmpResponseSchema = S.Struct({
  _meta: ErcotLmpMetaSchema,
  columns: S.Array(S.String),
  data: S.Array(ErcotLmpRowSchema),
})
export type ErcotLmpResponse = typeof ErcotLmpResponseSchema.Type

// ── Authority ────────────────────────────────────────────────────────

export const ErcotLmpIngestionAuthorityBoundary = S.Literals([
  'read_only_market_data_ingestion',
])
export type ErcotLmpIngestionAuthorityBoundary =
  typeof ErcotLmpIngestionAuthorityBoundary.Type

export class ErcotLmpIngestionAuthority extends S.Class<ErcotLmpIngestionAuthority>(
  'ErcotLmpIngestionAuthority',
)({
  authorityBoundary: ErcotLmpIngestionAuthorityBoundary,
  noGridDispatch: S.Boolean,
  noMarketDataMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
  noWalletSpend: S.Boolean,
}) {}

export const ERCOT_LMP_READ_ONLY_AUTHORITY: ErcotLmpIngestionAuthority = {
  authorityBoundary: 'read_only_market_data_ingestion',
  noGridDispatch: true,
  noMarketDataMutation: true,
  noSettlementMutation: true,
  noWalletSpend: true,
}

// ── Error class ───────────────────────────────────────────────────────

export class ErcotLmpIngestionUnsafe extends S.TaggedErrorClass<ErcotLmpIngestionUnsafe>()(
  'ErcotLmpIngestionUnsafe',
  {
    reason: S.String,
  },
) {}

// ── Known ERCOT settlement-point hubs ────────────────────────────────

export const ERCOT_HUB_SETTLEMENT_POINTS = [
  'HB_BUSAVG',
  'HB_HOUSTON',
  'HB_HUBAVG',
  'HB_NORTH',
  'HB_PAN',
  'HB_SOUTH',
  'HB_WEST',
] as const
export type ErcotHubSettlementPoint =
  (typeof ERCOT_HUB_SETTLEMENT_POINTS)[number]

// ── Output record ────────────────────────────────────────────────────

// Compatible with ArtanisPylonPowerMarketWindowRecord (without re-importing
// that class here to keep the ingestion adapter self-contained).
export interface ErcotLmpWindowRecord {
  readonly authority: ErcotLmpIngestionAuthority
  readonly averageLmpDollarsPerMwh: number
  readonly caveatRefs: ReadonlyArray<string>
  readonly claimState: 'measured' | 'stale' | 'blocked'
  readonly evidenceRef: string
  readonly market: 'ercot'
  readonly missingDataFlags: ReadonlyArray<string>
  readonly refreshedAtIso: string
  readonly rowCount: number
  readonly sourceRefs: ReadonlyArray<string>
  readonly windowRef: string
  readonly zoneOrSettlementPoint: string
}

// ── Helpers ───────────────────────────────────────────────────────────

const decodeErcotLmpResponse = S.decodeUnknownOption(ErcotLmpResponseSchema)

const isKnownHub = (point: string): boolean =>
  (ERCOT_HUB_SETTLEMENT_POINTS as ReadonlyArray<string>).includes(point)

const averageLmp = (rows: ReadonlyArray<ErcotLmpRow>): number => {
  if (rows.length === 0) return 0
  const total = rows.reduce((sum, row) => sum + row.LMPfieldvalue, 0)
  return total / rows.length
}

// Derive a stable, datestamp-only window ref from the first row timestamp.
// Timestamps from ERCOT are "YYYY-MM-DD HH:MM:SS" (space-separated, not ISO T).
const windowRefFromRows = (
  settlementPoint: string,
  rows: ReadonlyArray<ErcotLmpRow>,
): string => {
  const first = rows[0]
  if (first === undefined) {
    return `window.public.ercot.${settlementPoint.toLowerCase()}.unknown`
  }
  // Take only the date portion and strip hyphens → YYYYMMDD
  const datePart = first.SCEDTimestamp.slice(0, 10).replace(/-/g, '')
  return `window.public.ercot.${settlementPoint.toLowerCase()}.${datePart}`
}

// ── Public API ────────────────────────────────────────────────────────

/**
 * Validate and normalise a pre-parsed ERCOT API response into a typed
 * window record.
 *
 * @param raw          Already-parsed (unknown) ERCOT API response body.
 * @param settlementPoint  Settlement point to filter and average (e.g. "HB_NORTH").
 * @param ingestionIso ISO timestamp at which this ingestion was performed.
 */
export const normalizeErcotLmpResponse = (
  raw: unknown,
  settlementPoint: string,
  ingestionIso: string,
): ErcotLmpWindowRecord => {
  const parsed = Option.getOrThrowWith(
    decodeErcotLmpResponse(raw),
    () =>
      new ErcotLmpIngestionUnsafe({
        reason:
          'ERCOT API response does not match the expected v2 LMP schema ' +
          '(requires _meta, columns, and data arrays with SCEDTimestamp, ' +
          'SettlementPoint, and LMPfieldvalue fields).',
      }),
  )

  const rows = parsed.data.filter(
    row => row.SettlementPoint === settlementPoint,
  )

  if (rows.length === 0) {
    throw new ErcotLmpIngestionUnsafe({
      reason: `No data rows found for settlement point "${settlementPoint}" in ERCOT LMP response (${parsed.data.length} total rows).`,
    })
  }

  const avg = averageLmp(rows)

  if (!Number.isFinite(avg)) {
    throw new ErcotLmpIngestionUnsafe({
      reason: 'Computed average LMP is not a finite number; possible NaN/Infinity in source data.',
    })
  }

  const missingDataFlags: Array<string> = []

  if (!isKnownHub(settlementPoint)) {
    missingDataFlags.push('missing_data.public.ercot.unknown_hub_settlement_point')
  }

  if (rows.length < 4) {
    missingDataFlags.push('missing_data.public.ercot.sparse_interval_window')
  }

  const windowRef = windowRefFromRows(settlementPoint, rows)

  return {
    authority: ERCOT_LMP_READ_ONLY_AUTHORITY,
    averageLmpDollarsPerMwh: avg,
    caveatRefs: ['caveat.public.ercot_lmp.api_v2_cache_lag'],
    claimState: 'measured',
    evidenceRef: `evidence.public.ercot.lmp.${settlementPoint.toLowerCase()}`,
    market: 'ercot',
    missingDataFlags,
    refreshedAtIso: ingestionIso,
    rowCount: rows.length,
    sourceRefs: ['source.public.ercot_api_v2_public_reports'],
    windowRef,
    zoneOrSettlementPoint: settlementPoint,
  }
}

/**
 * Return a fixture record matching the normalised output shape,
 * suitable for seeding tests and the modeled operator proof report
 * while live ingestion is not yet wired.
 *
 * All values are explicitly MODELED (from oa_aibtc_model ERCOT
 * references); replace with normalizeErcotLmpResponse output once the
 * ingestion cron is live.
 */
export const exampleErcotLmpWindowRecord = (): ErcotLmpWindowRecord => ({
  authority: ERCOT_LMP_READ_ONLY_AUTHORITY,
  averageLmpDollarsPerMwh: 31.2,
  caveatRefs: ['caveat.public.ercot_lmp.api_v2_cache_lag'],
  claimState: 'measured',
  evidenceRef: 'evidence.public.ercot.lmp.hb_north',
  market: 'ercot',
  missingDataFlags: [],
  refreshedAtIso: '2026-06-20T00:00:00.000Z',
  rowCount: 96,
  sourceRefs: ['source.public.ercot_api_v2_public_reports'],
  windowRef: 'window.public.ercot.hb_north.20260619',
  zoneOrSettlementPoint: 'HB_NORTH',
})
