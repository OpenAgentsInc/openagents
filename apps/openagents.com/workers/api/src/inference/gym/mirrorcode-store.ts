// D1-backed store for MirrorCode-as-a-service demo run rows (#6378, epic #6376).
//
// One row per run, keyed by the public-safe `runId`. Each row stores the
// already-public-safe `MirrorCodeRun` object (built via buildMirrorCodeRun,
// which re-asserts the public-safety / no-task-contents / no-canary boundary
// BEFORE storage) as a JSON blob, plus the freshness columns used for ordering.
// There is NO raw benchmark content here: prompts, responses, logs,
// trajectories, keys, task source, and test data are rejected at the ingest
// boundary and never reach this table.
//
// On READ the stored JSON is re-decoded through the `MirrorCodeRun` schema, so a
// tampered or stale-shape row is rejected rather than served. This mirrors
// run-progress-store.ts and ladder-store.ts so the gym storage pattern stays
// uniform.
import { Effect } from 'effect'

import { parseJsonWithSchema } from '../../json-boundary'
import { MirrorCodeRun, type MirrorCodeRun as MirrorCodeRunType } from './mirrorcode-contract'

// Read side: the public projection routes list every run and fetch one by id.
export type MirrorCodeRunSourceStore = Readonly<{
  listRuns: () => Effect.Effect<ReadonlyArray<MirrorCodeRunType>>
  getRun: (runId: string) => Effect.Effect<MirrorCodeRunType | undefined>
}>

// Write side: the owner-gated launch/ingest route upserts a built run.
export type MirrorCodeRunStore = MirrorCodeRunSourceStore &
  Readonly<{
    upsertRun: (
      run: MirrorCodeRunType,
      updatedAtIso: string,
    ) => Effect.Effect<void>
  }>

type MirrorCodeRunD1Row = Readonly<{ run_json: string }>

const parseStoredRun = (json: string): MirrorCodeRunType | undefined => {
  try {
    return parseJsonWithSchema(MirrorCodeRun, json)
  } catch {
    return undefined
  }
}

export const makeD1MirrorCodeRunStore = (db: D1Database): MirrorCodeRunStore => ({
  listRuns: () =>
    Effect.promise(async () => {
      const rows = await db
        .prepare(
          `SELECT run_json
            FROM mirrorcode_runs
            ORDER BY started_at DESC, run_id ASC`,
        )
        .all<MirrorCodeRunD1Row>()
      return (rows.results ?? []).flatMap(row => {
        const parsed = parseStoredRun(row.run_json)
        return parsed === undefined ? [] : [parsed]
      })
    }),
  getRun: runId =>
    Effect.promise(async () => {
      const row = await db
        .prepare(`SELECT run_json FROM mirrorcode_runs WHERE run_id = ?`)
        .bind(runId)
        .first<MirrorCodeRunD1Row>()
      if (row === null) {
        return undefined
      }
      return parseStoredRun(row.run_json)
    }),
  upsertRun: (run, updatedAtIso) =>
    Effect.promise(async () => {
      await db
        .prepare(
          `INSERT INTO mirrorcode_runs (
            run_id,
            run_json,
            bucket,
            grade,
            status,
            started_at,
            updated_at,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT(run_id) DO UPDATE SET
            run_json = excluded.run_json,
            bucket = excluded.bucket,
            grade = excluded.grade,
            status = excluded.status,
            started_at = excluded.started_at,
            updated_at = excluded.updated_at`,
        )
        .bind(
          run.runId,
          JSON.stringify(run),
          run.bucket,
          run.grade,
          run.status,
          run.startedAt,
          updatedAtIso,
          updatedAtIso,
        )
        .run()
    }),
})
