// Bun-side fetch of the live network snapshot for the home screen.
// GET https://openagents.com/api/public/pylon-stats -> PylonStatsSnapshot, which
// the UI projects into the scene via projectPylonNetworkScene (pure). Fail-soft:
// any error returns ok:false with the reason; the scene then renders dormant
// (never fake counts). Injectable fetchFn/nowIso for tests.
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene.js"

export type FetchPylonStatsInput = {
  readonly baseUrl: string
  readonly fetchFn?: typeof fetch
  readonly nowIso?: () => string
}

export type PylonStatsResult = {
  readonly ok: boolean
  readonly sourceUrl: string
  readonly fetchedAt: string
  readonly snapshot: PylonStatsSnapshot | null
  readonly error: string | null
}

const normalizeBaseUrl = (baseUrl: string): string => baseUrl.replace(/\/+$/, "")

export async function fetchPublicPylonStats(
  input: FetchPylonStatsInput,
): Promise<PylonStatsResult> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const sourceUrl = `${normalizeBaseUrl(input.baseUrl)}/api/public/pylon-stats`
  try {
    const response = await fetchFn(sourceUrl, { headers: { accept: "application/json" } })
    if (!response.ok) {
      return { ok: false, sourceUrl, fetchedAt, snapshot: null, error: `HTTP ${response.status}` }
    }
    const json = (await response.json()) as PylonStatsSnapshot
    return { ok: true, sourceUrl, fetchedAt, snapshot: json, error: null }
  } catch (error) {
    return {
      ok: false,
      sourceUrl,
      fetchedAt,
      snapshot: null,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}
