// #5050: live network activity for the homepage pylon scene.
//
// Fetches GET /api/public/pylon-stats and derives the activity intensity [0,1]
// that drives the pylon's blue glow (pylonDiamonds.setActivity). The mapping
// mirrors the Autopilot home screen + the shared visual-language runbook
// (openagents/docs/autopilot-coder/2026-06-15-autopilot-home-network-visual-language.md
// §2) so both surfaces agree: glow tracks sessions in flight, NIP-90 jobs
// settling, and training progress. Fail-soft: any error => intensity 0 (dormant).

import { parseJsonRecord } from '../json-boundary'

export type PylonStatsSnapshot = {
  readonly available?: boolean
  readonly status?: string
  readonly pylonsOnlineNow?: number
  readonly pylonsAssignmentReadyNow?: number
  readonly pylonSessionsOnlineNow?: number
  readonly publicRealSatsSettled24h?: number | null
  readonly trainingModelProgressContributors?: number
  readonly nip90MarketSettlementStats?: {
    readonly compute?: { readonly jobsSettled24h?: number; readonly satsSettled24h?: number }
    readonly data?: { readonly jobsSettled24h?: number; readonly satsSettled24h?: number }
    readonly labor?: { readonly jobsSettled24h?: number; readonly satsSettled24h?: number }
  } | null
}

export const PYLON_STATS_BOOT_SCRIPT_ID = 'openagents-pylon-stats-snapshot'

const pos = (value: number | null | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0

// Soft saturating curve: immediate response to the first unit of work, asymptotes
// to 1. k = the half-bright point. Matches the Autopilot mapping.
const saturate = (value: number, k: number): number => {
  const v = pos(value)
  return v <= 0 ? 0 : 1 - 1 / (1 + v / k)
}

const clamp01 = (value: number): number => Math.max(0, Math.min(1, value))

const sumJobs24h = (snapshot: PylonStatsSnapshot): number => {
  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  return pos(m.compute?.jobsSettled24h) + pos(m.data?.jobsSettled24h) + pos(m.labor?.jobsSettled24h)
}

// activityIntensity = weighted sum of three saturated signals, each capped at
// its 1/3 weight (no single signal fills the bar). Idle network => 0.
export const computeActivityIntensity = (snapshot: PylonStatsSnapshot | null): number => {
  if (snapshot === null || snapshot.available === false || snapshot.status === 'unavailable') {
    return 0
  }
  const sessions = saturate(snapshot.pylonSessionsOnlineNow ?? 0, 3)
  const nip90 = saturate(sumJobs24h(snapshot), 5)
  const training = saturate(snapshot.trainingModelProgressContributors ?? 0, 4)
  return clamp01((sessions + nip90 + training) / 3)
}

export const PYLON_STATS_URL = '/api/public/pylon-stats'

type PylonStatsBootDocument = Pick<Document, 'getElementById'>

export const readInitialPylonStatsSnapshot = (
  documentRef: PylonStatsBootDocument | undefined =
    typeof document === 'undefined' ? undefined : document,
): PylonStatsSnapshot | null => {
  const text =
    documentRef?.getElementById(PYLON_STATS_BOOT_SCRIPT_ID)?.textContent ?? null

  if (text === null || text.trim() === '') return null

  const parsed = parseJsonRecord(text)
  return parsed === undefined ? null : (parsed as PylonStatsSnapshot)
}

// Fetch the live snapshot. Fail-soft -> null (dormant).
export const fetchPylonStats = async (
  fetchFn: typeof fetch = fetch,
  url: string = PYLON_STATS_URL,
): Promise<PylonStatsSnapshot | null> => {
  try {
    const response = await fetchFn(url, { headers: { accept: 'application/json' } })
    if (!response.ok) return null
    return (await response.json()) as PylonStatsSnapshot
  } catch {
    return null
  }
}
