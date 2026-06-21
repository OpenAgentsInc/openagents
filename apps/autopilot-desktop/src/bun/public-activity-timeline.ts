import {
  assertPublicActivityTimelineEnvelopeSafe,
  type PublicActivityTimelineEnvelope,
} from "@openagentsinc/public-activity-timeline"

import type { PublicActivityTimelineResponse } from "../shared/rpc.js"

export type FetchPublicActivityTimelineInput = Readonly<{
  baseUrl: string
  fetchFn?: typeof fetch
  limit?: number
  nowIso?: () => string
}>

const normalizeBaseUrl = (baseUrl: string): string => baseUrl.replace(/\/+$/, "")

const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const timelineUrl = (baseUrl: string, limit: number): string => {
  const url = new URL("/api/public/activity-timeline", normalizeBaseUrl(baseUrl))
  url.searchParams.set("limit", String(limit))
  return url.toString()
}

export async function fetchPublicActivityTimeline(
  input: FetchPublicActivityTimelineInput,
): Promise<PublicActivityTimelineResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const limit = Math.max(1, Math.min(100, Math.trunc(input.limit ?? 20)))
  const fetchedAt = input.nowIso?.() ?? new Date().toISOString()
  const sourceUrl = timelineUrl(input.baseUrl, limit)

  try {
    const response = await fetchFn(sourceUrl, {
      headers: { accept: "application/json" },
    })
    if (!response.ok) {
      return {
        ok: false,
        fetchedAt,
        sourceUrl,
        envelope: null,
        error: `public activity timeline ${response.status}`,
      }
    }

    const envelope = assertPublicActivityTimelineEnvelopeSafe(
      await response.json(),
    ) as PublicActivityTimelineEnvelope
    return {
      ok: true,
      fetchedAt,
      sourceUrl,
      envelope,
    }
  } catch (error) {
    return {
      ok: false,
      fetchedAt,
      sourceUrl,
      envelope: null,
      error: errorText(error),
    }
  }
}
