// KS-8.16 (#8327): Forge (git intake + coordination) domain — D1 → Cloud
// SQL migration machinery, following the freshest KS-8 templates
// (`agent-runtime-store.ts` #8316 for the store-factory wrap,
// `billing-store.ts` #8318 and `forum/forum-content-store.ts` #8321 for
// the row seam + flags).
//
// Domain tables (khala-sync migration `0021_forge_domain.sql`, ALL
// SIXTEEN `forge_*` tables): coordination issues/PRs/status, dispatch
// leases, merge-queue ledger, packfile archives, tenants, git access
// tokens (+scopes), verification receipts, promotion decisions,
// receive-pack intakes, canonical refs, object tips, ref locks, GitHub
// mirror receipts.
//
// THE SEAM: this domain's writes are already CLOSED behind five typed
// store objects — `makeD1ForgeCoordinationStore`,
// `makeD1ForgeGitCanonicalStore`, `makeD1R2ForgeGitPackfileArchiveStore`,
// `makeD1ForgeTenantGitAuthStore`, `makeD1ForgeGitHubMirrorStore` — the
// ONLY writers of the sixteen tables (grep-verified: no other module
// issues `INSERT/UPDATE/DELETE … forge_*`). So the production wiring is
// the KS-8.5 pattern, not a statement-classifying database proxy: five
// `makeForge*StoreForEnv` drop-in factories wrap each store's WRITE
// methods; the authoritative D1 SQL runs unchanged, and after a
// successful D1 write the wrapper READS BACK the affected rows by their
// composite keys and converge-upserts the byte-exact rows into Postgres.
// Read-back mirroring keeps the D1-resolved rows (lock release states,
// lease expiry transitions, attempt-count bumps, dedupe outcomes)
// hash-identical across stores. A mirror failure NEVER fails the request
// — it logs the typed drift diagnostic `khala_sync_forge_dual_write_failed`.
//
// REF LOCKING: D1 stays the SOLE lock authority in this lane — the held/
// applied/rejected lock dance and the partial-unique enforcement remain
// on D1, and the Postgres twin only ever receives resolved lock rows via
// read-back. Porting the lock protocol onto real `SELECT ... FOR UPDATE`
// row locks is the deliberate READ/WRITE-cutover step (MIGRATION_PLAN
// §3.13) — this lane does not emulate the D1 dance in Postgres.
//
// SECRETS (SPEC invariant 9): `forge_git_access_tokens` carries token
// HASHES/prefixes only — raw tokens are never stored on either engine
// and the Postgres twin is column-for-column with D1 (no widening).
// Custody values never appear in diagnostics: log lines carry row KEYS
// (tenant_ref/token_ref) only, and the one mirror path that must key on
// `token_hash` (the authenticate-path `last_used_at`/expiry transitions,
// where the store never surfaces tenant/token refs on the miss path)
// redacts its refs to a count.
//
// Flags (per KS-8 convention):
//   KHALA_SYNC_FORGE_DUAL_WRITE (default ON; '0'|'off'|'false'|'disabled'|'no')
//   KHALA_SYNC_FORGE_READS      (default 'd1'; 'd1'|'compare'|'postgres')
// With no KHALA_SYNC_DB binding everything degrades to plain D1. Read
// routing in this lane covers the canonical `listRefs` scan (the ref
// advertisement — the §3.13 "ref-set equality against git ls-remote"
// surface): `compare` shadow-runs it against Postgres, SERVES D1, and
// logs `khala_sync_forge_read_compare_mismatch`. `postgres` (KS-8.16
// follow-up #8358 read cutover) SERVES the `listRefs` ref advertisement
// from the Postgres twin via the already-tested
// `makePostgresForgeGitCanonicalStore.listRefs`, and is FAIL-SOFT: any
// Postgres error (acquire, query, decode) falls back to the D1 authority
// for that one call and logs `khala_sync_forge_postgres_read_serve_failed`
// — the ref advertisement can never break, it can only fail over to D1.
// The read serve was gated on a silent compare-mode soak + a live
// ground-truth `git`-advertisement cross-check + exact backfill --verify
// ref-set digests (RUNBOOK "Forge domain cutover"). WRITE authority stays
// on D1 in this lane — the `makePostgresForgeGitCanonicalStore` ref-lock
// port is NOT yet wired as write authority (that is the separate,
// domain-wide write cutover; see the file header of
// `forge-git-canonical-postgres-store.ts`).
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Forge domain cutover"):
// dual-write on → backfill (khala-sync-server scripts/backfill-forge.ts)
// → second sweep → --verify (exact counts, per-repo ref-set digests,
// merge-queue replay digests, newest-N row hashes) → live `git ls-remote`
// cross-check per tenant repo → compare reads → read/write cutover (real
// FOR UPDATE ref locks) + D1 drop in the follow-up.

import {
  FORGE_DOMAIN_TABLE_SPECS,
  makeCompareSoakMetrics,
  normalizeForgeDomainValue,
  requireForgeDomainUnsafe,
  upsertForgeDomainRows,
  type CompareSoakMetrics,
  type ForgeDomainRow,
  type ForgeDomainTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  makeD1ForgeCoordinationStore,
  type ForgeCoordinationStore,
} from './forge-coordination-store'
import {
  makeD1ForgeDomainWriteStore,
  type ForgeDomainWriteStore,
} from './forge-domain-d1-write-store'
import {
  makeD1ForgeGitCanonicalStore,
  type ForgeGitCanonicalRefRow,
  type ForgeGitCanonicalRefState,
  type ForgeGitCanonicalStore,
} from './forge-git-canonical-store'
import { makePostgresForgeGitCanonicalStore } from './forge-git-canonical-postgres-store'
import {
  makeD1R2ForgeGitPackfileArchiveStore,
  type ForgeGitPackfileArchiveStore,
} from './forge-git-packfile-archive-store'
import {
  makeD1ForgeGitHubMirrorStore,
  type ForgeGitHubMirrorStore,
} from './forge-github-mirror-store'
import {
  forgeGitAccessTokenHash,
  makeD1ForgeTenantGitAuthStore,
  type ForgeTenantGitAuthStore,
} from './forge-tenant-git-auth-store'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import { openAgentsDatabase } from './runtime'

export type { ForgeDomainRow, ForgeDomainTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type ForgeDomainReadsMode = 'd1' | 'postgres' | 'compare'

export type ForgeDomainFlags = Readonly<{
  dualWrite: boolean
  reads: ForgeDomainReadsMode
}>

export type ForgeDomainFlagEnv = Readonly<{
  KHALA_SYNC_FORGE_DUAL_WRITE?: string | undefined
  KHALA_SYNC_FORGE_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.16 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding
 * exists); reads default to D1 authority — read flips are EPIC-GATED ops
 * decisions (#8282), never a code default. Unknown read values fall back
 * to 'd1' — never fail open into an unproven read path on a typo.
 */
export const forgeDomainFlagsFromEnv = (
  env: ForgeDomainFlagEnv,
): ForgeDomainFlags => {
  const dualWriteRaw = env.KHALA_SYNC_FORGE_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_FORGE_READS?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    reads:
      readsRaw === 'postgres' || readsRaw === 'compare' ? readsRaw : 'd1',
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type ForgeDomainDiagnosticEvent =
  | 'khala_sync_forge_dual_write_failed'
  | 'khala_sync_forge_read_compare_mismatch'
  | 'khala_sync_forge_read_compare_failed'
  | 'khala_sync_forge_postgres_read_serve_failed'

export type ForgeDomainDiagnostic = Readonly<{
  /** The mirrored table or read operation, e.g. 'mirror:forge_git_refs'. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only
   * (tenant/refs). NEVER token hashes, token prefixes, or any custody
   * column value (paths keyed on token_hash pass a redacted count).
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type ForgeDomainLog = (
  event: ForgeDomainDiagnosticEvent,
  fields: ForgeDomainDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts (composite-PK arbiter,
 * D1 snapshot wins) for all sixteen tables. Returns how many rows were
 * touched. One behavioral contract suite runs against BOTH concrete
 * implementations (`forge-domain-repository.contract.test.ts`).
 *
 * Re-exported from `forge-domain-d1-write-store.ts` (KS-8.16 follow-up
 * #8358): that module also backs the Postgres→D1 mirror-back wired into
 * `forge-git-canonical-postgres-store.ts`, so it lives outside this file
 * to avoid a circular import between the two (this file already imports
 * `makePostgresForgeGitCanonicalStore` from there).
 */
export type { ForgeDomainWriteStore }

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresForgeDomainStore = ForgeDomainWriteStore &
  Readonly<{
    /**
     * Run one read-only statement on the Postgres twin (compare-mode
     * shadow reads and verification). `text` uses `$n` placeholders.
     */
    queryRows: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }>

export type MakePostgresForgeDomainStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the other KS-8 stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresForgeDomainStore = (
  deps: MakePostgresForgeDomainStoreDependencies,
): PostgresForgeDomainStore => {
  const withSql = async <A>(fn: (sql: SyncSql) => Promise<A>): Promise<A> => {
    const client = await deps.acquireSql()
    try {
      return await fn(client.sql)
    } finally {
      try {
        await client.end()
      } catch {
        // best-effort teardown, same discipline as the push route.
      }
    }
  }

  return {
    queryRows: (text, params) =>
      withSql(async sql => requireForgeDomainUnsafe(sql)(text, [...params])),
    upsertRows: (table, rows) =>
      withSql(sql => upsertForgeDomainRows(sql, table, rows)),
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

// `makeD1ForgeDomainWriteStore` now lives in `forge-domain-d1-write-store.ts`
// (see the `ForgeDomainWriteStore` re-export above for why) and is
// re-exported here unchanged so existing imports from this module keep
// working.
export { makeD1ForgeDomainWriteStore }

// ---------------------------------------------------------------------------
// Dual-write wrapper over the row seam
// ---------------------------------------------------------------------------

export type MakeDualWriteForgeDomainWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: ForgeDomainWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: ForgeDomainWriteStore | undefined
  flags: ForgeDomainFlags
  log?: ForgeDomainLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_forge_dual_write_failed` (the drift metric).
 */
export const makeDualWriteForgeDomainWriteStore = (
  deps: MakeDualWriteForgeDomainWriteStoreDependencies,
): ForgeDomainWriteStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? (() => {})

  if (postgres === undefined || !flags.dualWrite) {
    return d1
  }

  return {
    upsertRows: async (table, rows) => {
      const outcome = await d1.upsertRows(table, rows)
      try {
        await postgres.upsertRows(table, rows)
      } catch (error) {
        log('khala_sync_forge_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: diagnosticRefsForRows(table, rows),
        })
      }
      return outcome
    },
  }
}

/** Row keys for diagnostics — composite PK values, custody-safe. */
const diagnosticRefsForRows = (
  table: ForgeDomainTable,
  rows: ReadonlyArray<ForgeDomainRow>,
): ReadonlyArray<string> =>
  rows
    .slice(0, 10)
    .map(row =>
      FORGE_DOMAIN_TABLE_SPECS[table].keyColumns
        .map(column => String(row[column] ?? ''))
        .join('/'),
    )

// ---------------------------------------------------------------------------
// The read-back mirror
// ---------------------------------------------------------------------------

/** One composite key: values in the table's `keyColumns` order. */
export type ForgeDomainKey = ReadonlyArray<string>

export type ForgeDomainMirror = Readonly<{
  /** Read the rows for the composite keys back from D1 → Postgres. */
  mirrorRowsByKey: (
    table: ForgeDomainTable,
    keys: ReadonlyArray<ForgeDomainKey>,
  ) => Promise<void>
  /**
   * Read every row matching a bounded equality scan back from D1 →
   * Postgres (lease acquisition by work_ref, lock sets by
   * receive_pack_ref, token scope sets, the authenticate path by
   * token_hash). `refs` overrides the diagnostic refs — REQUIRED when the
   * scan values are custody-bearing.
   */
  mirrorRowsWhere: (
    table: ForgeDomainTable,
    whereColumns: ReadonlyArray<string>,
    values: ReadonlyArray<string>,
    refs?: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeForgeDomainMirrorDependencies = Readonly<{
  db: D1Database
  postgres: ForgeDomainWriteStore
  log: ForgeDomainLog
}>

/** Bounded read-back scan size (per-key row families are small by
 * construction: locks per push, scopes per token, leases per work). */
const MIRROR_SCAN_LIMIT = 500

/**
 * Fail-soft read-back mirror: every method reads the authoritative rows
 * from D1 and converge-upserts them into Postgres; every failure is
 * logged (keys only) and swallowed. NEVER throws. A key that never
 * matched a D1 row (e.g. a dedupe that kept the existing row) mirrors
 * zero rows — exactly right, the surviving row was mirrored when it was
 * first written.
 */
export const makeForgeDomainMirror = (
  deps: MakeForgeDomainMirrorDependencies,
): ForgeDomainMirror => {
  const { db, log, postgres } = deps

  const guarded = async (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> => {
    try {
      await run()
    } catch (error) {
      log('khala_sync_forge_dual_write_failed', {
        messageSafe: safeMessage(error),
        op,
        refs: refs.slice(0, 10),
      })
    }
  }

  return {
    mirrorRowsByKey: (table, keys) =>
      keys.length === 0
        ? Promise.resolve()
        : guarded(
            `mirror:${table}`,
            keys.slice(0, 10).map(key => key.join('/')),
            async () => {
              const keyColumns = FORGE_DOMAIN_TABLE_SPECS[table].keyColumns
              const tuple = `(${keyColumns.map(column => `${column} = ?`).join(' AND ')})`
              const where = keys.map(() => tuple).join(' OR ')
              const rows = await db
                .prepare(`SELECT * FROM ${table} WHERE ${where}`)
                .bind(...keys.flat())
                .all<ForgeDomainRow>()
              await postgres.upsertRows(table, rows.results ?? [])
            },
          ),

    mirrorRowsWhere: (table, whereColumns, values, refs) =>
      guarded(
        `mirror:${table}:scan`,
        refs ?? values.map(String),
        async () => {
          const where = whereColumns
            .map(column => `${column} = ?`)
            .join(' AND ')
          const rows = await db
            .prepare(
              `SELECT * FROM ${table} WHERE ${where} LIMIT ${MIRROR_SCAN_LIMIT}`,
            )
            .bind(...values)
            .all<ForgeDomainRow>()
          await postgres.upsertRows(table, rows.results ?? [])
        },
      ),
  }
}

// ---------------------------------------------------------------------------
// Env plumbing
// ---------------------------------------------------------------------------

export type ForgeDomainStoreEnv = ForgeDomainFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
    /**
     * Compare-mode soak observability (#8282 shared follow-up). Optional:
     * absent until the `analytics_engine_datasets` wrangler binding is
     * deployed, in which case the `listRefs` shadow compare simply skips
     * the durable metric (existing diagnostics unaffected).
     */
    ANALYTICS?: AnalyticsEngineDataset | undefined
  }>

export type MakeForgeDomainStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: ForgeDomainLog | undefined
  /**
   * D1 handle override. Some call sites already hold a (possibly
   * proxied) database — e.g. the forum webhook path's mirroring database
   * — and must keep using it as the authority.
   */
  db?: D1Database | undefined
  /** Compare-mode soak metrics override (tests inject a collector). */
  metrics?: CompareSoakMetrics | undefined
}>

const defaultLog: ForgeDomainLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

/**
 * The raw SQL-client acquirer for the Postgres twin, or undefined when no
 * KHALA_SYNC_DB binding. One client per operation; the caller always ends
 * it. Both the row-level compare/mirror store and the canonical
 * Postgres-read-serving path (`makePostgresForgeGitCanonicalStore`) source
 * their clients here, so they can never diverge on connection wiring.
 */
type AcquireForgeSql = () => Promise<KhalaSyncPushSqlClient>

const acquireSqlForEnv = (
  env: ForgeDomainStoreEnv,
  options: MakeForgeDomainStoreOptions,
): AcquireForgeSql | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return () => makeSqlClient(connectionString)
}

const postgresStoreForEnv = (
  acquireSql: AcquireForgeSql | undefined,
): PostgresForgeDomainStore | undefined =>
  acquireSql === undefined
    ? undefined
    : makePostgresForgeDomainStore({ acquireSql })

type ForgeDomainRuntime = Readonly<{
  db: D1Database
  flags: ForgeDomainFlags
  log: ForgeDomainLog
  mirror: ForgeDomainMirror | undefined
  compareStore: PostgresForgeDomainStore | undefined
  /**
   * Acquire a raw Postgres SQL client for canonical read serving, or
   * undefined when there is no Postgres twin. Present regardless of read
   * mode; the canonical factory only USES it when `flags.reads` selects
   * Postgres read serving.
   */
  acquireSql: AcquireForgeSql | undefined
  /** Compare-mode soak observability recorder (no-op when unbound). */
  metrics: CompareSoakMetrics
}>

const runtimeForEnv = (
  env: ForgeDomainStoreEnv,
  options: MakeForgeDomainStoreOptions,
): ForgeDomainRuntime => {
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const flags = forgeDomainFlagsFromEnv(env)
  const log = options.log ?? defaultLog
  const acquireSql = acquireSqlForEnv(env, options)
  const postgres = postgresStoreForEnv(acquireSql)
  const metrics = options.metrics ?? makeCompareSoakMetrics(env.ANALYTICS)
  return {
    acquireSql,
    compareStore:
      postgres !== undefined && flags.reads !== 'd1' ? postgres : undefined,
    db,
    flags,
    log,
    metrics,
    mirror:
      postgres !== undefined && flags.dualWrite
        ? makeForgeDomainMirror({ db, log, postgres })
        : undefined,
  }
}

const isZeroObjectId = (objectId: string): boolean => /^0+$/.test(objectId)

// ---------------------------------------------------------------------------
// Domain store factories (the call-site drop-ins)
// ---------------------------------------------------------------------------

/** Drop-in for `makeD1ForgeCoordinationStore(openAgentsDatabase(env))`. */
export const makeForgeCoordinationStoreForEnv = (
  env: ForgeDomainStoreEnv,
  options: MakeForgeDomainStoreOptions = {},
): ForgeCoordinationStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1ForgeCoordinationStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    acquireDispatchLease: async input => {
      const result = await base.acquireDispatchLease(input)
      // The acquire flow may expire a stale lease AND insert the new one;
      // both rows share (tenant_ref, work_ref), so one bounded scan
      // mirrors every touched row (older rows were mirrored when written).
      await mirror.mirrorRowsWhere(
        'forge_dispatch_leases',
        ['tenant_ref', 'work_ref'],
        [input.tenantRef, input.workRef],
      )
      return result
    },
    recordMergeQueueLedger: async input => {
      const result = await base.recordMergeQueueLedger(input)
      await mirror.mirrorRowsByKey('forge_merge_queue_ledger', [
        [input.tenantRef, input.queueRef],
      ])
      return result
    },
    recordPromotionDecisionReceipt: async (receipt, createdAt) => {
      const result = await base.recordPromotionDecisionReceipt(
        receipt,
        createdAt,
      )
      await mirror.mirrorRowsByKey('forge_promotion_decisions', [
        [receipt.tenant_ref, receipt.promotion_ref],
      ])
      return result
    },
    recordStatus: async input => {
      const result = await base.recordStatus(input)
      await mirror.mirrorRowsByKey('forge_coordination_status', [
        [input.tenantRef, input.statusRef],
      ])
      return result
    },
    recordVerificationReceipt: async (receipt, createdAt) => {
      const result = await base.recordVerificationReceipt(receipt, createdAt)
      await mirror.mirrorRowsByKey('forge_verification_receipts', [
        [receipt.tenant_ref, receipt.verification_ref],
      ])
      return result
    },
    upsertChange: async input => {
      const result = await base.upsertChange(input)
      await mirror.mirrorRowsByKey('forge_coordination_prs', [
        [input.tenantRef, input.prRef],
      ])
      return result
    },
    upsertIssue: async input => {
      const result = await base.upsertIssue(input)
      await mirror.mirrorRowsByKey('forge_coordination_issues', [
        [input.tenantRef, input.issueRef],
      ])
      return result
    },
  }
}

/** Rows-equal check for the listRefs compare shadow (registry-normalized). */
const refRowsEqual = (
  d1Rows: ReadonlyArray<Record<string, unknown>>,
  postgresRows: ReadonlyArray<Record<string, unknown>>,
): boolean => {
  if (d1Rows.length !== postgresRows.length) {
    return false
  }
  const spec = FORGE_DOMAIN_TABLE_SPECS.forge_git_refs
  for (let i = 0; i < d1Rows.length; i++) {
    for (const column of spec.columns) {
      const left = normalizeForgeDomainValue(d1Rows[i]?.[column])
      const right = normalizeForgeDomainValue(postgresRows[i]?.[column])
      if (String(left) !== String(right)) {
        return false
      }
    }
  }
  return true
}

/** Drop-in for `makeD1ForgeGitCanonicalStore(openAgentsDatabase(env))`. */
export const makeForgeGitCanonicalStoreForEnv = (
  env: ForgeDomainStoreEnv,
  options: MakeForgeDomainStoreOptions = {},
): ForgeGitCanonicalStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1ForgeGitCanonicalStore(runtime.db)
  const { acquireSql, compareStore, flags, log, metrics, mirror } = runtime

  // KS-8.16 follow-up (#8358) read cutover: in `postgres` mode SERVE the
  // `listRefs` ref advertisement from the Postgres twin, reusing the
  // already-tested `makePostgresForgeGitCanonicalStore.listRefs` (same
  // ORDER BY / bounded-limit / state-filter semantics as the D1 lane).
  // FAIL-SOFT: any Postgres error returns undefined so the caller falls
  // back to D1 authority — the ref advertisement can never break. WRITE
  // authority stays on D1; this only moves the read of the ref set.
  const servePostgresListRefs =
    flags.reads === 'postgres' && acquireSql !== undefined
      ? async (
          tenantRef: string,
          repositoryRef: string,
          input:
            | Readonly<{ state?: ForgeGitCanonicalRefState; limit?: number }>
            | undefined,
        ): Promise<ReadonlyArray<ForgeGitCanonicalRefRow> | undefined> => {
          let client: KhalaSyncPushSqlClient | undefined
          try {
            client = await acquireSql()
            return await makePostgresForgeGitCanonicalStore(
              client.sql,
            ).listRefs(tenantRef, repositoryRef, input)
          } catch (error) {
            log('khala_sync_forge_postgres_read_serve_failed', {
              messageSafe: safeMessage(error),
              op: 'listRefs',
              refs: [tenantRef, repositoryRef],
            })
            return undefined
          } finally {
            if (client !== undefined) {
              try {
                await client.end()
              } catch {
                // best-effort teardown, same discipline as the push route.
              }
            }
          }
        }
      : undefined

  const compareListRefs =
    compareStore === undefined
      ? undefined
      : async (
          tenantRef: string,
          repositoryRef: string,
          state: string | undefined,
          limit: number,
          d1Rows: ReadonlyArray<Record<string, unknown>>,
        ): Promise<void> => {
          try {
            const postgresRows =
              state === undefined
                ? await compareStore.queryRows(
                    `SELECT * FROM forge_git_refs WHERE tenant_ref = $1 AND repository_ref = $2 ORDER BY ref_name ASC LIMIT $3`,
                    [tenantRef, repositoryRef, limit],
                  )
                : await compareStore.queryRows(
                    `SELECT * FROM forge_git_refs WHERE tenant_ref = $1 AND repository_ref = $2 AND state = $3 ORDER BY ref_name ASC LIMIT $4`,
                    [tenantRef, repositoryRef, state, limit],
                  )
            if (!refRowsEqual(d1Rows, postgresRows)) {
              log('khala_sync_forge_read_compare_mismatch', {
                messageSafe: `ref advertisement differs: d1=${d1Rows.length} postgres=${postgresRows.length} rows`,
                op: 'listRefs',
                refs: [tenantRef, repositoryRef],
              })
              metrics.record({ domain: 'forge', outcome: 'mismatch', readKind: 'listRefs' })
            } else {
              metrics.record({ domain: 'forge', outcome: 'match', readKind: 'listRefs' })
            }
          } catch (error) {
            log('khala_sync_forge_read_compare_failed', {
              messageSafe: safeMessage(error),
              op: 'listRefs',
              refs: [tenantRef, repositoryRef],
            })
            metrics.record({ domain: 'forge', outcome: 'error', readKind: 'listRefs' })
          }
        }

  const mirrorReceivePack = async (input: {
    tenantRef: string
    repositoryRef: string
    receivePackRef: string
    refUpdates: ReadonlyArray<{ refName: string; newObjectId: string }>
  }): Promise<void> => {
    if (mirror === undefined) {
      return
    }
    await mirror.mirrorRowsByKey('forge_git_receive_pack_intakes', [
      [input.tenantRef, input.receivePackRef],
    ])
    await mirror.mirrorRowsByKey(
      'forge_git_refs',
      input.refUpdates.map(update => [
        input.tenantRef,
        input.repositoryRef,
        update.refName,
      ]),
    )
    await mirror.mirrorRowsByKey(
      'forge_git_objects',
      input.refUpdates
        .filter(update => !isZeroObjectId(update.newObjectId))
        .map(update => [
          input.tenantRef,
          input.repositoryRef,
          update.newObjectId,
        ]),
    )
    await mirror.mirrorRowsWhere(
      'forge_git_ref_locks',
      ['tenant_ref', 'receive_pack_ref'],
      [input.tenantRef, input.receivePackRef],
    )
  }

  return {
    ...base,
    applyReceivePack:
      mirror === undefined
        ? base.applyReceivePack
        : async input => {
            try {
              const result = await base.applyReceivePack(input)
              await mirrorReceivePack(input)
              return result
            } catch (error) {
              // The failure path released held locks as 'rejected' —
              // mirror the resolved lock rows, then surface the error
              // unchanged (fail-soft mirror, fail-loud protocol).
              await mirror.mirrorRowsWhere(
                'forge_git_ref_locks',
                ['tenant_ref', 'receive_pack_ref'],
                [input.tenantRef, input.receivePackRef],
              )
              throw error
            }
          },
    importExternalRef:
      mirror === undefined
        ? base.importExternalRef
        : async input => {
            const result = await base.importExternalRef(input)
            await mirror.mirrorRowsByKey('forge_git_refs', [
              [input.tenantRef, input.repositoryRef, input.refName],
            ])
            await mirror.mirrorRowsByKey('forge_git_objects', [
              [input.tenantRef, input.repositoryRef, input.objectId],
            ])
            return result
          },
    listRefs:
      servePostgresListRefs !== undefined
        ? async (tenantRef, repositoryRef, input) => {
            const served = await servePostgresListRefs(
              tenantRef,
              repositoryRef,
              input,
            )
            // Fail-soft: a Postgres serve failure falls back to the D1
            // authority so the ref advertisement is always answered.
            return (
              served ?? (await base.listRefs(tenantRef, repositoryRef, input))
            )
          }
        : compareListRefs === undefined
          ? base.listRefs
          : async (tenantRef, repositoryRef, input) => {
              const rows = await base.listRefs(tenantRef, repositoryRef, input)
              await compareListRefs(
                tenantRef,
                repositoryRef,
                input?.state,
                Math.min(Math.max(Math.floor(input?.limit ?? 100), 1), 500),
                rows as unknown as ReadonlyArray<Record<string, unknown>>,
              )
              return rows
            },
  }
}

/** Drop-in for `makeD1ForgeTenantGitAuthStore(openAgentsDatabase(env))`. */
export const makeForgeTenantGitAuthStoreForEnv = (
  env: ForgeDomainStoreEnv,
  options: MakeForgeDomainStoreOptions = {},
): ForgeTenantGitAuthStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1ForgeTenantGitAuthStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    authenticateGitAccessToken: async input => {
      const session = await base.authenticateGitAccessToken(input)
      if (session !== undefined) {
        // Success path touched last_used_at.
        await mirror.mirrorRowsByKey('forge_git_access_tokens', [
          [session.tenantRef, session.tokenRef],
        ])
      } else if (input.token.startsWith('oa_forge_git_')) {
        // The miss path may have flipped state to 'expired', and the
        // store never surfaces tenant/token refs there — the only usable
        // key is the token hash. CUSTODY: the hash is used strictly as a
        // read-back bind value; diagnostics get a redacted ref.
        const tokenHash = await forgeGitAccessTokenHash(input.token)
        await mirror.mirrorRowsWhere(
          'forge_git_access_tokens',
          ['token_hash'],
          [tokenHash],
          ['<redacted:token_hash>'],
        )
      }
      return session
    },
    mintGitAccessToken: async (input, mintOptions) => {
      const result = await base.mintGitAccessToken(input, mintOptions)
      await mirror.mirrorRowsByKey('forge_git_access_tokens', [
        [input.tenantRef, input.tokenRef],
      ])
      await mirror.mirrorRowsWhere(
        'forge_git_access_token_scopes',
        ['tenant_ref', 'token_ref'],
        [input.tenantRef, input.tokenRef],
      )
      return result
    },
    revokeGitAccessToken: async (tenantRef, tokenRef, revokedAt) => {
      const result = await base.revokeGitAccessToken(
        tenantRef,
        tokenRef,
        revokedAt,
      )
      await mirror.mirrorRowsByKey('forge_git_access_tokens', [
        [tenantRef, tokenRef],
      ])
      return result
    },
    upsertTenant: async input => {
      const result = await base.upsertTenant(input)
      await mirror.mirrorRowsByKey('forge_tenants', [[input.tenantRef]])
      return result
    },
  }
}

/**
 * Drop-in for
 * `makeD1R2ForgeGitPackfileArchiveStore(openAgentsDatabase(env), bucket)`.
 * Raw packfile bytes go to R2 exactly as before — only the metadata row
 * mirrors to Postgres.
 */
export const makeForgeGitPackfileArchiveStoreForEnv = (
  env: ForgeDomainStoreEnv,
  bucket: R2Bucket,
  options: MakeForgeDomainStoreOptions = {},
): ForgeGitPackfileArchiveStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1R2ForgeGitPackfileArchiveStore(runtime.db, bucket)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    putPackfile: async input => {
      const result = await base.putPackfile(input)
      // Dedupe may return an existing row under a DIFFERENT packfile_ref
      // (digest match) — mirror the resolved record's key, not the input.
      await mirror.mirrorRowsByKey('forge_git_packfile_archives', [
        [
          String(result.record.tenant_ref),
          String(result.record.packfile_ref),
        ],
      ])
      return result
    },
  }
}

/** Drop-in for `makeD1ForgeGitHubMirrorStore(openAgentsDatabase(env))`. */
export const makeForgeGitHubMirrorStoreForEnv = (
  env: ForgeDomainStoreEnv,
  options: MakeForgeDomainStoreOptions = {},
): ForgeGitHubMirrorStore => {
  const runtime = runtimeForEnv(env, options)
  const base = makeD1ForgeGitHubMirrorStore(runtime.db)
  const mirror = runtime.mirror
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    recordReceipt: async input => {
      const receipt = await base.recordReceipt(input)
      await mirror.mirrorRowsByKey('forge_github_mirror_receipts', [
        [receipt.tenant_ref, receipt.mirror_ref],
      ])
      return receipt
    },
  }
}
