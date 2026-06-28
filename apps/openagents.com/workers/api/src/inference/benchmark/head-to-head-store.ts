// D1-backed store for published Khala external head-to-head snapshots (#6308).
//
// One row per head-to-head, keyed by the public-safe `headToHeadRef`. Each row
// stores the already-public-safe `openagents.khala.head_to_head.v1` object as a
// JSON blob. The object is built by `buildKhalaHeadToHead`, which derives every
// matchup from the shipped `buildGymLeaderboardProjection` (decision-grade,
// public-safety-checked rows only) — so the store never touches raw prompts,
// responses, logs, trajectories, keys, or private endpoints.
//
// On READ the stored JSON is re-decoded through the `KhalaHeadToHead` schema, so a
// tampered or stale-shape row is rejected rather than served. This mirrors
// `ladder-store.ts` exactly so the publishing pattern stays uniform across the
// benchmark surfaces.
import { Effect, Schema as S } from 'effect'

import { parseJsonWithSchema } from '../../json-boundary'
import {
  KhalaHeadToHeadSchemaVersion,
  type KhalaHeadToHead,
} from './head-to-head'

// A decode schema for the stored snapshot. Kept structurally aligned with the
// `KhalaHeadToHead` type in `head-to-head.ts`; the snapshot store only needs a
// shape-faithful decode to reject tampered rows, not the full builder logic.
const KhalaHeadToHeadSideSchema = S.Struct({
  lane: S.NullOr(S.String),
  reportRef: S.String,
  candidateRef: S.String,
  acceptedOutcomes: S.Number,
  attemptedVerifications: S.Number,
  inputTokens: S.Number,
  outputTokens: S.Number,
  totalTokens: S.Number,
  meanWallClockMs: S.NullOr(S.Number),
  solveRateBps: S.NullOr(S.Number),
  costPerAcceptedOutcomeMsat: S.Number,
})

const KhalaHeadToHeadMatchupSchema = S.Struct({
  lane: S.String,
  label: S.String,
  category: S.Literals([
    'default_coding_agent_model',
    'free_or_open',
    'paid_frontier',
  ]),
  state: S.Literals(['published', 'awaiting_owner']),
  barToClear: S.String,
  khala: S.NullOr(KhalaHeadToHeadSideSchema),
  comparator: S.NullOr(KhalaHeadToHeadSideSchema),
  verdict: S.NullOr(
    S.Literals([
      'khala_wins_both',
      'khala_wins_cost',
      'khala_wins_quality',
      'comparator_ahead',
      'even',
    ]),
  ),
  solveRateDeltaBps: S.NullOr(S.Number),
  costPerAcceptedOutcomeDeltaMsat: S.NullOr(S.Number),
  blockerRefs: S.Array(S.String),
})

export const KhalaHeadToHeadStored = S.Struct({
  schemaVersion: S.Literal(KhalaHeadToHeadSchemaVersion),
  headToHeadRef: S.String,
  cadence: S.Literals(['weekly', 'per_khala_release', 'on_demand']),
  publishPath: S.String,
  demandKind: S.Literal('internal'),
  demandSource: S.Literal('head_to_head'),
  projectionRef: S.String,
  decisionGradeRowCount: S.Number,
  khala: S.NullOr(KhalaHeadToHeadSideSchema),
  matchups: S.Array(KhalaHeadToHeadMatchupSchema),
  excludedReports: S.Array(
    S.Struct({
      reportRef: S.String,
      reason: S.Literals([
        'not_decision_grade',
        'no_accepted_outcomes',
        'public_safety_violation',
      ]),
    }),
  ),
  caveatRefs: S.Array(S.String),
})

export type KhalaHeadToHeadSnapshot = Readonly<{
  headToHead: KhalaHeadToHead
  publishedAt: string | null
}>

// Read side: the public projection route only needs to fetch the latest
// head-to-head by ref. Kept narrow so the public route takes a read-only source.
export type KhalaHeadToHeadSourceStore = Readonly<{
  getHeadToHead: (
    headToHeadRef: string,
  ) => Effect.Effect<KhalaHeadToHeadSnapshot | undefined>
}>

// Write side: the operator publish route upserts a built head-to-head snapshot.
export type KhalaHeadToHeadStore = KhalaHeadToHeadSourceStore &
  Readonly<{
    upsertHeadToHead: (
      headToHead: KhalaHeadToHead,
      publishedAtIso: string,
    ) => Effect.Effect<void>
  }>

type KhalaHeadToHeadD1Row = Readonly<{
  head_to_head_json: string
  published_at: string | null
}>

const parseStored = (json: string): KhalaHeadToHead | undefined => {
  try {
    return parseJsonWithSchema(
      KhalaHeadToHeadStored,
      json,
    ) as KhalaHeadToHead
  } catch {
    return undefined
  }
}

export const makeD1KhalaHeadToHeadStore = (
  db: D1Database,
): KhalaHeadToHeadStore => ({
  getHeadToHead: headToHeadRef =>
    Effect.promise(async () => {
      const row = await db
        .prepare(
          `SELECT head_to_head_json, published_at
            FROM khala_head_to_head_snapshots
            WHERE head_to_head_ref = ?`,
        )
        .bind(headToHeadRef)
        .first<KhalaHeadToHeadD1Row>()
      if (row === null) {
        return undefined
      }
      const headToHead = parseStored(row.head_to_head_json)
      return headToHead === undefined
        ? undefined
        : { headToHead, publishedAt: row.published_at ?? null }
    }),
  upsertHeadToHead: (headToHead, publishedAtIso) =>
    Effect.promise(async () => {
      await db
        .prepare(
          `INSERT INTO khala_head_to_head_snapshots (
            head_to_head_ref,
            head_to_head_json,
            published_at,
            created_at
          ) VALUES (?, ?, ?, ?)
          ON CONFLICT(head_to_head_ref) DO UPDATE SET
            head_to_head_json = excluded.head_to_head_json,
            published_at = excluded.published_at`,
        )
        .bind(
          headToHead.headToHeadRef,
          JSON.stringify(headToHead),
          publishedAtIso,
          publishedAtIso,
        )
        .run()
    }),
})
