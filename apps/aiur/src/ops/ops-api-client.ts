/**
 * Client-side (browser) API for the Aiur ops views (AIUR-3, #8501). Hits
 * Aiur's own same-origin admin-credits-proxy paths (shared with AIUR-2),
 * which forward to the main Worker with the signed-in owner's bearer.
 */
import {
  AIUR_ADMIN_OPS_DAILY_SALES_LEDGER_PATH,
  AIUR_ADMIN_OPS_HEALTH_PATH,
  AIUR_ADMIN_OPS_RUNS_PATH,
} from '../admin-credits-proxy'

export type OpsRun = Readonly<{
  observedAt: string
  userId: string | null
  threadId: string | null
  turnId: string | null
  provider: string | null
  model: string | null
  totalTokens: number
  costAmount: number | null
  currency: string | null
  usageTruth: string
}>

export type OpsRunsResponse = Readonly<{
  ok: true
  runs: ReadonlyArray<OpsRun>
  liveViaKhalaSync: boolean
}>

export type OpsHealthCheck =
  | Readonly<{ status: 'ok'; value: string; checkedAt: string }>
  | Readonly<{ status: 'not_measured'; reasonRef: string }>
  | Readonly<{ status: 'error'; messageSafe: string; checkedAt: string }>

export type OpsHealthResponse = Readonly<{
  ok: true
  checks: Readonly<{
    lastOrgCloudTurnCompletedAt: OpsHealthCheck
    pushDeviceTokensRegistered: OpsHealthCheck
    khalaPublicStatsReachable: OpsHealthCheck
  }>
}>

export type OpsApiError = Readonly<{
  ok: false
  status: number
  messageSafe: string
}>

export type OpsApiResult<T> = Readonly<{ ok: true; value: T }> | OpsApiError

const parseJsonSafe = async (response: Response): Promise<Record<string, unknown>> => {
  try {
    const parsed: unknown = await response.json()
    return typeof parsed === 'object' && parsed !== null
      ? (parsed as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

async function requestJson<T>(path: string): Promise<OpsApiResult<T>> {
  const response = await fetch(path)
  const body = await parseJsonSafe(response)
  if (!response.ok) {
    return {
      messageSafe:
        typeof body.messageSafe === 'string'
          ? body.messageSafe
          : `Request failed (${response.status}).`,
      ok: false,
      status: response.status,
    }
  }
  return { ok: true, value: body as unknown as T }
}

export const fetchOpsRuns = (limit = 50): Promise<OpsApiResult<OpsRunsResponse>> =>
  requestJson(`${AIUR_ADMIN_OPS_RUNS_PATH}?limit=${limit}`)

export const fetchOpsHealth = (): Promise<OpsApiResult<OpsHealthResponse>> =>
  requestJson(AIUR_ADMIN_OPS_HEALTH_PATH)

// OB-6 (P1 Track C, #8563): the daily sales ledger — per-day, per-segment
// outbound funnel plus deliverability health. See
// business-outreach-daily-ledger.ts for the exact/not_measured field split.
export type DailySalesLedgerNotMeasured = Readonly<{
  status: 'not_measured'
  reasonRef: string
}>

export type DailySalesLedgerSegmentDay = Readonly<{
  date: string
  segmentRef: string
  sourced: number
  drafted: number
  approved: number
  sent: number
  quoted: number
  closedWon: number
  closedLost: number
  replies: DailySalesLedgerNotMeasured
  reportClicks: DailySalesLedgerNotMeasured
  conversations: DailySalesLedgerNotMeasured
}>

export type DailySalesLedgerRateMetric =
  | Readonly<{ status: 'measured'; valuePct: number }>
  | DailySalesLedgerNotMeasured

export type DailySalesLedgerDeliverabilityDay = Readonly<{
  date: string
  delivered: number
  bounced: number
  complained: number
  failed: number
  optOuts: number
  bounceRatePct: DailySalesLedgerRateMetric
  complaintRatePct: DailySalesLedgerRateMetric
  health: 'healthy' | 'at_risk' | 'breach' | 'not_measured'
}>

export type DailySalesLedgerTotals = Readonly<{
  sourced: number
  drafted: number
  approved: number
  sent: number
  delivered: number
  bounced: number
  complained: number
  optOuts: number
  quoted: number
  closedWon: number
  closedLost: number
}>

export type DailySalesLedgerCountMetric =
  | Readonly<{ status: 'measured'; count: number }>
  | DailySalesLedgerNotMeasured

export type DailySalesLedgerEngagementDay = Readonly<{
  date: string
  replies: DailySalesLedgerCountMetric
  reportClicks: DailySalesLedgerCountMetric
  conversations: DailySalesLedgerCountMetric
}>

export type DailySalesLedger = Readonly<{
  since: string
  until: string
  generatedAt: string
  segmentRefs: ReadonlyArray<string>
  segmentDays: ReadonlyArray<DailySalesLedgerSegmentDay>
  deliverabilityDays: ReadonlyArray<DailySalesLedgerDeliverabilityDay>
  engagementDays: ReadonlyArray<DailySalesLedgerEngagementDay>
  totals: DailySalesLedgerTotals
  operatorMinutes: DailySalesLedgerNotMeasured
  digestLine: string
  notMeasured: ReadonlyArray<Readonly<{ field: string; reasonRef: string }>>
}>

export type DailySalesLedgerResponse = Readonly<{
  ok: true
  ledger: DailySalesLedger
}>

export const fetchDailySalesLedger = (
  params: Readonly<{ since?: string; until?: string }> = {},
): Promise<OpsApiResult<DailySalesLedgerResponse>> => {
  const query = new URLSearchParams()
  if (params.since !== undefined) query.set('since', params.since)
  if (params.until !== undefined) query.set('until', params.until)
  const search = query.toString()
  return requestJson(
    `${AIUR_ADMIN_OPS_DAILY_SALES_LEDGER_PATH}${search === '' ? '' : `?${search}`}`,
  )
}
