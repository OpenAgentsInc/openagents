// Artanis owner-scoped unsupported-request ledger READER (iteration-8 capability).
//
// This is the production seam behind the `get_unsupported_requests` read tool
// (`artanis-operator-tools.ts`). It resolves the most recent entries from the
// live unsupported-request ledger (#6357) — the running list of user-facing
// capability gaps that block Khala adoption, fed by trace reviews, Khala CLI
// feedback, and forum reports — into bounded, public-safe record projections, so
// Artanis can see exactly which gaps suppress usage, match each to an open issue,
// and target Codex dispatch / forum mobilization at the highest-leverage gaps.
// This directly speeds the 10x-daily-Khala-token goal by closing the gaps that
// block usage.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It only calls
//     `KhalaUnsupportedRequestStore.listRecent` (the same admin-gated store the
//     `GET /api/operator/khala/unsupported-requests` route uses); it never
//     writes, mutates, dispatches, or spends.
//   - PUBLIC-SAFE PROJECTION. It projects each stored ledger record to the
//     bounded `ArtanisUnsupportedRequestRecord` shape. The ledger already
//     enforces public-safe summaries/refs on write; the tool applies a defensive
//     second pass. The internal `evidenceRefs`/`forumTopicRef`/
//     `suggestedIssueTitle`/`issueRequired` fields are intentionally NOT
//     surfaced — they are not needed for the gap-triage read and keep the
//     projection tight.
//   - OWNER-SCOPED. It is only wired into the owner-authenticated operator chat
//     (`makeArtanisOperatorTools`), the same admin path as the other read tools.

import type {
  ArtanisUnsupportedRequestRecord,
  ArtanisUnsupportedRequestsReader,
} from './artanis-operator-tools'
import type { KhalaUnsupportedRequestStore } from './khala-unsupported-request-routes'

export type ArtanisUnsupportedRequestsReaderDeps = Readonly<{
  // The unsupported-request ledger store used to read recent entries (read-only).
  store: KhalaUnsupportedRequestStore
}>

// Build the owner-scoped unsupported-request ledger reader for the
// `get_unsupported_requests` tool. Returns the most recent ledger record
// projections, newest first (the store's `listRecent` already orders by
// `updated_at DESC`), or `[]` when the ledger is empty. The optional status
// filter is passed straight through to the store's bounded enum filter.
export const makeArtanisUnsupportedRequestsReader = (
  deps: ArtanisUnsupportedRequestsReaderDeps,
): ArtanisUnsupportedRequestsReader => {
  return async (input): Promise<ReadonlyArray<ArtanisUnsupportedRequestRecord>> => {
    const records = await deps.store.listRecent({
      limit: input.limit,
      status: input.status,
    })
    return records.map(
      (record): ArtanisUnsupportedRequestRecord => ({
        githubIssueRef: record.githubIssueRef,
        nextAction: record.nextAction,
        requestRef: record.requestRef,
        sourceKind: record.sourceKind,
        status: record.status,
        summary: record.summary,
        title: record.title,
        triageKind: record.triageKind,
        updatedAt: record.updatedAt,
      }),
    )
  }
}
