// D1-backed store for live Gym / Harbor run-progress snapshots (#6271).
//
// One row per Harbor run, keyed by the public-safe `runRef`. The store holds the
// already-public-safe `openagents.gym.run_progress.v1` object as a JSON blob: the
// ingest route builds it via `buildGymRunProgress` and asserts it via
// `checkGymRunProgressPublicSafety` BEFORE handing it here, so the store never
// touches raw prompts/responses/logs/trajectories/keys/private endpoints.
//
// On READ the stored JSON is re-decoded through the `GymRunProgress` schema, so a
// tampered or stale-shape row is rejected rather than served. The route still
// only ever returns objects that passed the safety boundary at ingest time.
import { Effect } from 'effect'

import { parseJsonWithSchema } from '../../json-boundary'
import { GymRunProgress, type GymRunProgress as GymRunProgressType } from './run-progress'

// Read side: the projection routes only need to LIST stored runs. Kept narrow so
// the public route can take a read-only source without the upsert capability.
export type GymRunProgressSourceStore = Readonly<{
  listRunProgress: () => Effect.Effect<ReadonlyArray<GymRunProgressType>>
}>

// Write side: the operator ingest route upserts a built, safety-checked snapshot.
export type GymRunProgressStore = GymRunProgressSourceStore &
  Readonly<{
    upsertRunProgress: (progress: GymRunProgressType) => Effect.Effect<void>
  }>

type GymRunProgressD1Row = Readonly<{
  progress_json: string
}>

const parseStoredProgress = (json: string): GymRunProgressType | undefined => {
  try {
    return parseJsonWithSchema(GymRunProgress, json)
  } catch {
    return undefined
  }
}

export const makeD1GymRunProgressStore = (db: D1Database): GymRunProgressStore => ({
  listRunProgress: () =>
    Effect.promise(async () => {
      const rows = await db
        .prepare(
          `SELECT progress_json
            FROM gym_run_progress_snapshots
            ORDER BY last_updated_at DESC, run_ref ASC`,
        )
        .all<GymRunProgressD1Row>()

      return (rows.results ?? []).flatMap(row => {
        const parsed = parseStoredProgress(row.progress_json)
        return parsed === undefined ? [] : [parsed]
      })
    }),
  upsertRunProgress: progress =>
    Effect.promise(async () => {
      await db
        .prepare(
          `INSERT INTO gym_run_progress_snapshots (
            run_ref,
            progress_json,
            last_updated_at,
            ingested_at,
            created_at
          ) VALUES (?, ?, ?, ?, ?)
          ON CONFLICT(run_ref) DO UPDATE SET
            progress_json = excluded.progress_json,
            last_updated_at = excluded.last_updated_at,
            ingested_at = excluded.ingested_at`,
        )
        .bind(
          progress.runRef,
          JSON.stringify(progress),
          progress.lastUpdatedAt,
          progress.lastUpdatedAt,
          progress.lastUpdatedAt,
        )
        .run()
    }),
})
