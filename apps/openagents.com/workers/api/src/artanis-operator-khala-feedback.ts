// Artanis owner-scoped Khala CLI feedback READER (iteration-6 capability).
//
// This is the production seam behind the `get_khala_feedback` read tool
// (`artanis-operator-tools.ts`). It resolves the most recent user feedback
// submitted through the Khala CLI `/feedback` command into bounded, public-safe
// record projections, so Artanis can hear directly from users, spot capability
// gaps / bugs / style preferences, and triage each one (route to the
// unsupported-requests track #6357 or plan a Codex fix). This closes the
// user-feedback -> fix loop that drives adoption and 10x daily Khala token usage.
//
// It stays conservative by construction:
//   - READ-ONLY + SIDE-EFFECT-FREE. It only calls `KhalaFeedbackStore.listRecent`
//     (the same admin-gated store the `GET /api/operator/khala/feedback` route
//     uses); it never writes, mutates, dispatches, or spends.
//   - PUBLIC-SAFE PROJECTION. It projects each stored row to the bounded
//     `ArtanisKhalaFeedbackRecord` shape. The free-text body is the value the
//     owner is entitled to read (kept verbatim; the tool truncates it). The
//     `userAgent`/`traceRef` fields are intentionally NOT surfaced — they are not
//     needed for triage and keep the projection tight.
//   - OWNER-SCOPED. It is only wired into the owner-authenticated operator chat
//     (`makeOperatorTools`), the same admin path as the other read tools.

import type {
  ArtanisKhalaFeedbackReader,
  ArtanisKhalaFeedbackRecord,
} from './artanis-operator-tools'
import type { KhalaFeedbackStore } from './khala-feedback-routes'

export type ArtanisKhalaFeedbackReaderDeps = Readonly<{
  // The Khala feedback store used to read recent feedback (read-only).
  store: KhalaFeedbackStore
}>

// Build the owner-scoped Khala feedback reader for the `get_khala_feedback` tool.
// Returns the most recent feedback record projections, newest first (the store's
// `listRecent` already orders by `created_at DESC`), or `[]` when there is none.
export const makeArtanisKhalaFeedbackReader = (
  deps: ArtanisKhalaFeedbackReaderDeps,
): ArtanisKhalaFeedbackReader => {
  return async (
    limit: number,
  ): Promise<ReadonlyArray<ArtanisKhalaFeedbackRecord>> => {
    const records = await deps.store.listRecent({ limit })
    return records.map(
      (record): ArtanisKhalaFeedbackRecord => ({
        clientVersion: record.clientVersion,
        createdAt: record.createdAt,
        feedback: record.feedback,
        feedbackRef: record.feedbackRef,
        source: record.source,
      }),
    )
  }
}
