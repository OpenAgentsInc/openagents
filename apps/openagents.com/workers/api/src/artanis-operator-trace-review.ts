// Artanis owner-scoped Khala TRACE-REVIEW report LOADER (iteration-11 capability).
//
// This is the in-worker production seam behind the `get_trace_review` read tool
// (`artanis-operator-tools.ts`). It resolves the SAME public-safe trace-review
// report the admin-gated `GET /api/operator/khala/trace-review` route (#6356)
// serves, so Artanis can read it IN-LOOP — spotting recurring failure modes and
// unmet user intents, triaging them into the unsupported-request ledger (#6357),
// and planning targeted Codex burndown at the gaps that block adoption.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It reuses the EXACT report builder the route
//     uses (`buildKhalaTraceReviewReport` over a `KhalaTraceReviewStore`); it
//     never returns raw trajectories, raw SDK payloads, prompts, or private refs.
//   - IN-WORKER. The Worker cannot reliably HTTP-fetch its OWN admin-gated zone,
//     so the operator tool reads this report directly instead of an HTTP hop
//     (mirroring the get_network_stats / get_glm_fleet_status loaders).
//   - BOUNDED. The default window (24h) and row limit (10) match the route's
//     parse defaults, so the in-loop read stays cheap.

import {
  buildKhalaTraceReviewReport,
  type KhalaTraceReviewReport,
  type KhalaTraceReviewStore,
} from './khala-trace-review-routes'
import { currentIsoTimestamp, isoTimestampAfterIso } from './runtime-primitives'

// The default review window (hours) and per-section row limit. These mirror the
// route's parse defaults (`KHALA_TRACE_REVIEW_DEFAULT_HOURS` = 24, limit = 10) so
// the in-loop read matches what the operator route returns by default.
export const ARTANIS_TRACE_REVIEW_DEFAULT_HOURS = 24
export const ARTANIS_TRACE_REVIEW_DEFAULT_LIMIT = 10

export type ArtanisTraceReviewLoaderDeps = Readonly<{
  // The trace-review store (the same `makeD1KhalaTraceReviewStore` the route
  // uses). Required: the loader reads facts through it and builds the report.
  store: KhalaTraceReviewStore
  // Injected for testability; defaults to the live ISO clock.
  nowIso?: (() => string) | undefined
  // Window size in hours (default 24) and per-section row limit (default 10).
  hours?: number | undefined
  limit?: number | undefined
}>

// Build the in-worker trace-review report loader for the `get_trace_review`
// tool. Returns the SAME public-safe report the operator route serves, over a
// bounded recent window. Any storage rejection propagates as a rejected promise;
// the tool maps it to an honest "(could not fetch trace review)" soft failure.
export const makeArtanisTraceReviewLoader = (
  deps: ArtanisTraceReviewLoaderDeps,
): (() => Promise<KhalaTraceReviewReport>) => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  const hours = deps.hours ?? ARTANIS_TRACE_REVIEW_DEFAULT_HOURS
  const limit = deps.limit ?? ARTANIS_TRACE_REVIEW_DEFAULT_LIMIT

  return async (): Promise<KhalaTraceReviewReport> => {
    const until = nowIso()
    const window = {
      hours,
      since: isoTimestampAfterIso(until, -hours * 60 * 60 * 1000),
      until,
    }
    const facts = await deps.store.readFacts({ limit, window })
    return buildKhalaTraceReviewReport({
      facts,
      generatedAt: nowIso(),
      window,
    })
  }
}
