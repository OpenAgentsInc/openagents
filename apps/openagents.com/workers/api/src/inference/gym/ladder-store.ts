// D1-backed store for published Gym benchmark LADDER snapshots (#6309).
//
// One row per ladder, keyed by the public-safe `ladderRef`. Each row stores the
// already-public-safe `openagents.gym.ladder_leaderboard.v1` object as a JSON
// blob. The object is built by `buildGymLadderLeaderboard`, which derives every
// rung from the shipped `buildGymLeaderboardProjection` (decision-grade,
// public-safety-checked rows only) — so the store never touches raw prompts,
// responses, logs, trajectories, keys, or private endpoints.
//
// On READ the stored JSON is re-decoded through the `GymLadderLeaderboard`
// schema, so a tampered or stale-shape row is rejected rather than served. This
// mirrors `run-progress-store.ts` exactly so the publishing pattern stays
// uniform across the Gym surfaces.
import { Effect, Schema as S } from 'effect'

import { parseJsonWithSchema } from '../../json-boundary'
import {
  GymLadderLeaderboardSchemaVersion,
  type GymLadderLeaderboard,
} from './ladder'

// A decode schema for the stored ladder snapshot. Kept structurally aligned with
// the `GymLadderLeaderboard` type in `ladder.ts`; the snapshot store only needs a
// shape-faithful decode to reject tampered rows, not the full builder logic.
const GymLadderOpponentEntrySchema = S.Struct({
  rank: S.Number,
  lane: S.NullOr(S.String),
  reportRef: S.String,
  candidateRef: S.String,
  acceptedOutcomes: S.Number,
  verificationRateBps: S.NullOr(S.Number),
  costPerAcceptedOutcomeMsat: S.Number,
})

const GymLadderMirrorCodeEntrySchema = S.Struct({
  rank: S.Number,
  runId: S.String,
  model: S.String,
  taskId: S.String,
  bucket: S.Literals(['S', 'M', 'L']),
  language: S.NullOr(S.String),
  status: S.Literals(['queued', 'running', 'passed', 'failed', 'error']),
  passRateBps: S.Number,
  tokensTotal: S.Number,
  startedAt: S.String,
  finishedAt: S.NullOr(S.String),
  decisionGrade: S.Literal(true),
  demandKind: S.Literal('internal'),
  demandSource: S.Literal('gym_mirrorcode'),
  generalizationSet: S.String,
  memoryPolicy: S.String,
})

const GymLadderRungSchema = S.Struct({
  rung: S.Literals(['rung1', 'rung2', 'rung3', 'rung4']),
  title: S.String,
  state: S.Literals(['published', 'awaiting_owner']),
  barToClear: S.String,
  opponentLanes: S.Array(S.String),
  entries: S.Array(GymLadderOpponentEntrySchema),
  mirrorCodeEntries: S.Array(GymLadderMirrorCodeEntrySchema),
  blockerRefs: S.Array(S.String),
})

export const GymLadderLeaderboardStored = S.Struct({
  schemaVersion: S.Literal(GymLadderLeaderboardSchemaVersion),
  ladderRef: S.String,
  cadence: S.Literals(['weekly', 'per_model_release', 'on_demand']),
  publishPath: S.String,
  demandKind: S.Literal('internal'),
  demandSource: S.Literal('gym_ladder'),
  projectionRef: S.String,
  decisionGradeRowCount: S.Number,
  rungs: S.Array(GymLadderRungSchema),
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

export type GymLadderSnapshot = Readonly<{
  ladder: GymLadderLeaderboard
  publishedAt: string | null
}>

// Read side: the public projection route only needs to fetch the latest ladder
// by ref. Kept narrow so the public route takes a read-only source.
export type GymLadderSourceStore = Readonly<{
  getLadder: (
    ladderRef: string,
  ) => Effect.Effect<GymLadderSnapshot | undefined>
}>

// Write side: the operator publish route upserts a built ladder snapshot.
export type GymLadderStore = GymLadderSourceStore &
  Readonly<{
    upsertLadder: (
      ladder: GymLadderLeaderboard,
      publishedAtIso: string,
    ) => Effect.Effect<void>
  }>

type GymLadderD1Row = Readonly<{
  ladder_json: string
  published_at: string | null
}>

const parseStoredLadder = (json: string): GymLadderLeaderboard | undefined => {
  try {
    const raw = JSON.parse(json) as unknown
    const normalized =
      typeof raw === 'object' && raw !== null && Array.isArray((raw as { rungs?: unknown }).rungs)
        ? {
            ...raw,
            rungs: (raw as { rungs: ReadonlyArray<unknown> }).rungs.map(rung =>
              typeof rung === 'object' &&
              rung !== null &&
              !('mirrorCodeEntries' in rung)
                ? { ...rung, mirrorCodeEntries: [] }
                : rung,
            ),
          }
        : raw
    return parseJsonWithSchema(
      GymLadderLeaderboardStored,
      JSON.stringify(normalized),
    ) as GymLadderLeaderboard
  } catch {
    return undefined
  }
}

export const makeD1GymLadderStore = (db: D1Database): GymLadderStore => ({
  getLadder: ladderRef =>
    Effect.promise(async () => {
      const row = await db
        .prepare(
          `SELECT ladder_json, published_at
            FROM gym_ladder_leaderboard_snapshots
            WHERE ladder_ref = ?`,
        )
        .bind(ladderRef)
        .first<GymLadderD1Row>()
      if (row === null) {
        return undefined
      }
      const ladder = parseStoredLadder(row.ladder_json)
      return ladder === undefined
        ? undefined
        : { ladder, publishedAt: row.published_at ?? null }
    }),
  upsertLadder: (ladder, publishedAtIso) =>
    Effect.promise(async () => {
      await db
        .prepare(
          `INSERT INTO gym_ladder_leaderboard_snapshots (
            ladder_ref,
            ladder_json,
            published_at,
            created_at
          ) VALUES (?, ?, ?, ?)
          ON CONFLICT(ladder_ref) DO UPDATE SET
            ladder_json = excluded.ladder_json,
            published_at = excluded.published_at`,
        )
        .bind(
          ladder.ladderRef,
          JSON.stringify(ladder),
          publishedAtIso,
          publishedAtIso,
        )
        .run()
    }),
})
