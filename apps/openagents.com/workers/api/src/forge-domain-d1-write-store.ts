// KS-8.16 follow-up (#8358): the generic D1-side twin of the Forge domain
// row seam (`upsertForgeDomainRows` in `@openagentsinc/khala-sync-server`
// is the Postgres side). Extracted into its own module (rather than living
// inline in `forge-domain-store.ts`, its original home) so it can be
// imported by BOTH the forward D1→Postgres mirror
// (`forge-domain-store.ts`) and the reverse Postgres→D1 mirror-back
// (`forge-git-canonical-postgres-store.ts`) without a circular import
// between those two files (`forge-domain-store.ts` already imports
// `makePostgresForgeGitCanonicalStore` from the latter).
//
// Table-driven by the SAME shared `FORGE_DOMAIN_TABLE_SPECS` registry used
// by the Postgres side, so a mirror running in either direction can never
// invent a column the other side does not also converge-upsert.

import {
  FORGE_DOMAIN_TABLE_SPECS,
  normalizeForgeDomainValue,
  type ForgeDomainRow,
  type ForgeDomainTable,
} from '@openagentsinc/khala-sync-server'

export type ForgeDomainWriteStore = Readonly<{
  upsertRows: (
    table: ForgeDomainTable,
    rows: ReadonlyArray<ForgeDomainRow>,
  ) => Promise<number>
}>

/**
 * The D1 twin of the row-level seam. Same converge semantics over the
 * same composite-PK arbiters, driven by the SAME shared registry.
 */
export const makeD1ForgeDomainWriteStore = (
  db: D1Database,
): ForgeDomainWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const spec = FORGE_DOMAIN_TABLE_SPECS[table]
    const setClauses = spec.columns
      .filter(column => !spec.keyColumns.includes(column))
      .map(column => `${column} = excluded.${column}`)
      .join(', ')
    const updateClause =
      setClauses.length === 0 ? 'DO NOTHING' : `DO UPDATE SET ${setClauses}`
    let touched = 0
    for (const row of rows) {
      const values = spec.columns.map(column =>
        normalizeForgeDomainValue(row[column]),
      )
      const placeholders = spec.columns.map(() => '?').join(', ')
      await db
        .prepare(
          `INSERT INTO ${table} (${spec.columns.join(', ')}) VALUES (${placeholders})
           ON CONFLICT(${spec.keyColumns.join(', ')}) ${updateClause}`,
        )
        .bind(...values)
        .run()
      touched += 1
    }
    return touched
  },
})
