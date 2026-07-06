// Newest-window rebuild from authoritative Postgres (CFG-5, #8520).
//
// The hub is a cache: a Cloud Run instance restart loses every in-memory
// window. Rather than waiting for capture's next append to 409-gap-heal
// (the DO's only rehydrate path), the LiveHub proactively rebuilds a
// scope's window from `khala_sync_changelog` on first touch: the newest
// `rebuildVersions` whole version groups (never splitting a version), read
// in ONE REPEATABLE READ transaction so the rows and the scope counters
// come from the same snapshot.
//
// Failure posture: rebuild is BEST-EFFORT. When Postgres is unreachable or
// the scope has no rows, the hub simply starts empty — exactly the DO's
// fresh-hub semantics (log answers behind-window, the route layer serves
// Postgres, capture's next append starts the window mid-stream).

import type { ChangelogEntry } from "@openagentsinc/khala-sync"
import {
  changelogEntryFromRow,
  type ChangelogRow,
  type SyncSql,
} from "@openagentsinc/khala-sync-server"

export const DEFAULT_REBUILD_VERSIONS = 1_000

/**
 * Load the newest `versions` whole version groups for `scope`, ascending
 * (the order `ScopeHub.append` requires). Empty array when the scope has
 * no changelog rows.
 */
export const loadNewestWindow = async (
  sql: SyncSql,
  scope: string,
  versions: number,
): Promise<Array<ChangelogEntry>> => {
  if (!Number.isSafeInteger(versions) || versions < 1) return []
  const rows: Array<ChangelogRow> = await sql.begin(
    "isolation level repeatable read",
    async (tx) =>
      tx`
        SELECT scope, version, entity_type, entity_id, op,
               post_image_json, mutation_ref, committed_at
          FROM khala_sync_changelog
         WHERE scope = ${scope}
           AND version >= COALESCE(
             (SELECT min(nv.version) FROM (
                SELECT DISTINCT version
                  FROM khala_sync_changelog
                 WHERE scope = ${scope}
                 ORDER BY version DESC
                 LIMIT ${versions}
              ) AS nv),
             1)
         ORDER BY version, entity_type, entity_id
      `,
  )
  return rows.map(changelogEntryFromRow)
}
