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
  ArtanisUnsupportedRequestWriter,
} from './artanis-operator-tools'
import type {
  KhalaUnsupportedRequestRecord,
  KhalaUnsupportedRequestStore,
} from './khala-unsupported-request-routes'
import { currentIsoTimestamp } from './runtime-primitives'

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
    return records.map(projectUnsupportedRequestRecord)
  }
}

// Project a stored ledger record into the bounded public-safe tool record shape.
// The internal `evidenceRefs`/`forumTopicRef`/`suggestedIssueTitle`/`sourceRef`/
// `issueRequired` fields are intentionally NOT surfaced — they are not needed for
// the gap-triage read/write and keep the projection tight.
const projectUnsupportedRequestRecord = (
  record: KhalaUnsupportedRequestRecord,
): ArtanisUnsupportedRequestRecord => ({
  evidenceRefs: record.evidenceRefs,
  githubIssueRef: record.githubIssueRef,
  nextAction: record.nextAction,
  requestRef: record.requestRef,
  sourceKind: record.sourceKind,
  status: record.status,
  summary: record.summary,
  suggestedIssueTitle: record.suggestedIssueTitle,
  title: record.title,
  triageKind: record.triageKind,
  updatedAt: record.updatedAt,
})

export type ArtanisUnsupportedRequestWriterDeps = Readonly<{
  // The unsupported-request ledger store used to read the existing row and write
  // the merged update (admin-gated; the same store the operator route uses).
  store: KhalaUnsupportedRequestStore
  // Injected clock for the new `updated_at`; defaults to the runtime clock.
  nowIso?: (() => string) | undefined
  // How many recent rows to scan to resolve the entry by ref (bounded; defaults
  // to the store's max page size of 100). The owner-triage flow operates on
  // recent gaps, so a bounded scan is sufficient and avoids a parallel query
  // surface on the store.
  maxScan?: number | undefined
}>

// Build the owner-scoped unsupported-request ledger WRITER for the iteration-9
// `update_unsupported_request` write tool. It resolves the existing row by its
// public-safe `requestRef` (via a bounded `listRecent` scan), merges ONLY the
// provided change fields (status / triageKind / githubIssueRef) onto it, and
// upserts the result on the SAME `(source_kind, source_ref)` key so the existing
// row is updated in place and its ref/created-at are preserved. It returns the
// UPDATED public-safe record projection, or `null` when no row matches the ref
// (honest absence — it never creates a new row for an unknown ref). It mutates
// only this owner-scoped internal ledger: no spend, payout, deploy, delete, or
// outward action.
export const makeArtanisUnsupportedRequestWriter = (
  deps: ArtanisUnsupportedRequestWriterDeps,
): ArtanisUnsupportedRequestWriter => {
  const nowIso = deps.nowIso ?? currentIsoTimestamp
  const maxScan = deps.maxScan ?? 100

  return async (update): Promise<ArtanisUnsupportedRequestRecord | null> => {
    const records = await deps.store.listRecent({ limit: maxScan })
    const existing = records.find(
      record => record.requestRef === update.ref,
    )
    if (existing === undefined) {
      return null
    }

    const updated = await deps.store.upsert({
      createdAt: existing.createdAt,
      evidenceRefs: existing.evidenceRefs,
      forumTopicRef: existing.forumTopicRef,
      githubIssueRef: update.githubIssueRef ?? existing.githubIssueRef,
      requestRef: existing.requestRef,
      sourceKind: existing.sourceKind,
      sourceRef: existing.sourceRef,
      status: update.status ?? existing.status,
      suggestedIssueTitle: existing.suggestedIssueTitle,
      summary: existing.summary,
      title: existing.title,
      triageKind: update.triageKind ?? existing.triageKind,
      updatedAt: nowIso(),
    })
    return projectUnsupportedRequestRecord(updated)
  }
}
