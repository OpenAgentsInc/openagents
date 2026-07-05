// KS-8.18 (#8329): Identity and auth core domain — D1 → Cloud SQL migration
// machinery. The LAST and most sensitive KS-8 domain, following the
// freshest KS-8 templates (`forge-domain-store.ts` #8327 for the
// store-factory read-back mirror, `billing-store.ts` #8318 for the
// money-/secret-domain discipline).
//
// Domain tables (khala-sync migration `0028_identity_auth_domain.sql`, the
// SEVENTEEN canonical identity/auth tables): users, auth identities,
// OpenAuth storage + agent links, GitHub write connections / attempts /
// grants, and the provider (BYOK) account custody family (accounts,
// connection attempts, auth grants, events, sanity checks, parallel-probe
// receipts, leases, failover receipts, token custody + audit).
//
// MACHINERY ONLY (MIGRATION_PLAN §3.15). D1 is and remains the SOLE
// authority. The Postgres side is a best-effort dual-write read-back
// mirror + backfill target ONLY in this lane. There is NO read cutover:
// auth read serving from Postgres is the highest-risk, owner-gated,
// done-last step (`KHALA_SYNC_IDENTITY_READS=postgres` DEFERS — never
// serves an unproven auth read path). Session invalidation semantics stay
// exactly where they are: revoking on D1 denies immediately; the mirror
// only ever copies the resolved D1 row.
//
// THE SEAM: identity/auth writes are spread across ~7 owning modules (six
// typed store/repository factories plus scattered inline route helpers).
// The seam here is the KS-8.16 read-back mirror exposed two ways:
//   1. `identityAuthMirrorFromEnv(env)` — a fail-soft `IdentityAuthMirror`
//      handle a write call site invokes AFTER its authoritative D1 write
//      to converge the touched rows (by PK) into Postgres. This is the
//      uniform adoption path for every writer.
//   2. `makeProviderAccountTokenCustodyStoreForEnv(env)` — the flagship
//      drop-in: a `makeD1ProviderAccountTokenCustodyStore` wrap whose write
//      methods read-back mirror after the D1 write. Token custody is the
//      single most secret-bearing table (SPEC invariant 9) and its
//      construction is centralized, so it is wired end-to-end in THIS lane.
//      The remaining writers adopt `identityAuthMirrorFromEnv` in the
//      decommission/wiring follow-up (RUNBOOK "Identity/auth domain
//      cutover"); until then their rows converge on the backfill sweep.
//
// SECRETS (SPEC invariant 9 — the invariant this domain motivated): the
// Postgres twin holds EXACTLY what D1 holds (no widening), same at-rest
// encryption posture. Custody columns (token ciphertext/IVs/key ids,
// openauth value_json, device user_code, OAuth state nonce) are twinned
// byte-for-byte but NEVER appear in diagnostics — every log line carries
// row KEYS only (ids / refs / owner_user_id), which are never custody
// columns. The `custodyColumns` registry declares each one; a mirror path
// that must key on a custody column redacts its diagnostic refs.
//
// Flags (per KS-8 convention):
//   KHALA_SYNC_IDENTITY_DUAL_WRITE (default ON; '0'|'off'|'false'|'disabled'|'no')
//   KHALA_SYNC_IDENTITY_READS      (default 'd1'; 'd1'|'compare'|'postgres')
// With no KHALA_SYNC_DB binding everything degrades to plain D1. Reads
// NEVER serve from Postgres in this lane: `compare`/`postgres` are inert
// (there is no routed identity read here) and `postgres` logs
// `khala_sync_identity_postgres_reads_deferred` once — a premature flag
// flip can never serve an unproven auth read path.
//
// Cutover order (docs/khala-sync/RUNBOOK.md "Identity/auth domain
// cutover"): dual-write on → backfill (scripts/backfill-identity-auth.ts)
// → second sweep → --verify (identity set equality, custody-safe) → full
// write-site wiring → [OWNER-GATED, LAST] auth read cutover + KV cache +
// session-revocation verification + D1 drop.

import {
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
  normalizeIdentityAuthValue,
  requireIdentityAuthUnsafe,
  upsertIdentityAuthRows,
  type IdentityAuthDomainRow,
  type IdentityAuthDomainTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'

import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import {
  makeD1ProviderAccountTokenCustodyStore,
  type ProviderAccountTokenCustodyStore,
} from './provider-account-token-custody'
import { openAgentsDatabase } from './runtime'

export type { IdentityAuthDomainRow, IdentityAuthDomainTable }

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

export type IdentityAuthReadsMode = 'd1' | 'postgres' | 'compare'

export type IdentityAuthFlags = Readonly<{
  dualWrite: boolean
  reads: IdentityAuthReadsMode
}>

export type IdentityAuthFlagEnv = Readonly<{
  KHALA_SYNC_IDENTITY_DUAL_WRITE?: string | undefined
  KHALA_SYNC_IDENTITY_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

/**
 * Parse the KS-8.18 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority — read flips are OWNER-GATED ops decisions
 * (#8282), never a code default, and for AUTH they are the highest-risk
 * step of all. Unknown read values fall back to 'd1' — never fail open
 * into an unproven read path on a typo.
 */
export const identityAuthFlagsFromEnv = (
  env: IdentityAuthFlagEnv,
): IdentityAuthFlags => {
  const dualWriteRaw = env.KHALA_SYNC_IDENTITY_DUAL_WRITE?.trim().toLowerCase()
  const readsRaw = env.KHALA_SYNC_IDENTITY_READS?.trim().toLowerCase()

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

export type IdentityAuthDiagnosticEvent =
  | 'khala_sync_identity_dual_write_failed'
  | 'khala_sync_identity_postgres_reads_deferred'

export type IdentityAuthDiagnostic = Readonly<{
  /** The mirrored table or operation, e.g. 'mirror:provider_account_token_custody'. */
  op: string
  /**
   * Public-safe refs identifying the affected rows — row KEYS only (ids /
   * refs / owner_user_id). NEVER token ciphertext, session payloads,
   * device codes, OAuth state nonces, or any custody column value (a path
   * keyed on a custody column passes a redacted marker).
   */
  refs: ReadonlyArray<string>
  /** Public-safe failure summary (error class/message head, no values). */
  messageSafe: string
}>

export type IdentityAuthLog = (
  event: IdentityAuthDiagnosticEvent,
  fields: IdentityAuthDiagnostic,
) => void

const safeMessage = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  return raw.replaceAll(/\s+/g, ' ').slice(0, 200)
}

const defaultLog: IdentityAuthLog = (event, fields) => {
  logWorkerRouteWarning(event, {
    messageSafe: fields.messageSafe,
    op: fields.op,
    refs: fields.refs.slice(0, 10).join(','),
  })
}

// ---------------------------------------------------------------------------
// The row-level repository seam
// ---------------------------------------------------------------------------

/**
 * The typed row-level write seam: converge upserts (composite-PK arbiter,
 * D1 snapshot wins) for all seventeen tables. Returns how many rows were
 * touched. One behavioral contract suite runs against BOTH concrete
 * implementations (`identity-auth-domain-repository.contract.test.ts`).
 */
export type IdentityAuthWriteStore = Readonly<{
  upsertRows: (
    table: IdentityAuthDomainTable,
    rows: ReadonlyArray<IdentityAuthDomainRow>,
  ) => Promise<number>
}>

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

export type PostgresIdentityAuthStore = IdentityAuthWriteStore &
  Readonly<{
    /**
     * Run one read-only statement on the Postgres twin (verification and
     * any future compare shadow reads). `text` uses `$n` placeholders.
     */
    queryRows: (
      text: string,
      params: ReadonlyArray<unknown>,
    ) => Promise<ReadonlyArray<Record<string, unknown>>>
  }>

export type MakePostgresIdentityAuthStoreDependencies = Readonly<{
  /**
   * Acquire a transaction-mode-safe SQL client (Hyperdrive in production,
   * a direct local URL in tests). One client per store operation; always
   * ended, even on error — the same discipline as the other KS-8 stores.
   */
  acquireSql: () => Promise<KhalaSyncPushSqlClient>
}>

export const makePostgresIdentityAuthStore = (
  deps: MakePostgresIdentityAuthStoreDependencies,
): PostgresIdentityAuthStore => {
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
      withSql(async sql => requireIdentityAuthUnsafe(sql)(text, [...params])),
    upsertRows: (table, rows) =>
      withSql(sql => upsertIdentityAuthRows(sql, table, rows)),
  }
}

// ---------------------------------------------------------------------------
// D1 implementation of the same seam (contract-suite twin)
// ---------------------------------------------------------------------------

/**
 * The D1 twin of the row-level seam. Same converge semantics over the same
 * composite-PK arbiters, driven by the SAME shared registry.
 */
export const makeD1IdentityAuthWriteStore = (
  db: D1Database,
): IdentityAuthWriteStore => ({
  upsertRows: async (table, rows) => {
    if (rows.length === 0) {
      return 0
    }
    const spec = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table]
    const setClauses = spec.columns
      .filter(column => !spec.keyColumns.includes(column))
      .map(column => `${column} = excluded.${column}`)
      .join(', ')
    const updateClause =
      setClauses.length === 0 ? 'DO NOTHING' : `DO UPDATE SET ${setClauses}`
    let touched = 0
    for (const row of rows) {
      const values = spec.columns.map(column =>
        normalizeIdentityAuthValue(row[column]),
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

// ---------------------------------------------------------------------------
// Dual-write wrapper over the row seam
// ---------------------------------------------------------------------------

export type MakeDualWriteIdentityAuthWriteStoreDependencies = Readonly<{
  /** The authoritative D1 write store. */
  d1: IdentityAuthWriteStore
  /** The Postgres store, or undefined when no KHALA_SYNC_DB binding. */
  postgres: IdentityAuthWriteStore | undefined
  flags: IdentityAuthFlags
  log?: IdentityAuthLog | undefined
}>

/**
 * D1 writes first (authority); the same rows then mirror to Postgres
 * best-effort. A mirror failure never fails the write — it emits
 * `khala_sync_identity_dual_write_failed` (the drift metric, keys only).
 */
export const makeDualWriteIdentityAuthWriteStore = (
  deps: MakeDualWriteIdentityAuthWriteStoreDependencies,
): IdentityAuthWriteStore => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? defaultLog

  if (postgres === undefined || !flags.dualWrite) {
    return d1
  }

  return {
    upsertRows: async (table, rows) => {
      const outcome = await d1.upsertRows(table, rows)
      try {
        await postgres.upsertRows(table, rows)
      } catch (error) {
        log('khala_sync_identity_dual_write_failed', {
          messageSafe: safeMessage(error),
          op: `upsertRows:${table}`,
          refs: diagnosticRefsForRows(table, rows),
        })
      }
      return outcome
    },
  }
}

/** Row keys for diagnostics — composite PK values, custody-safe (PK
 * columns are never custody columns). */
const diagnosticRefsForRows = (
  table: IdentityAuthDomainTable,
  rows: ReadonlyArray<IdentityAuthDomainRow>,
): ReadonlyArray<string> =>
  rows
    .slice(0, 10)
    .map(row =>
      IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].keyColumns
        .map(column => String(row[column] ?? ''))
        .join('/'),
    )

// ---------------------------------------------------------------------------
// The read-back mirror
// ---------------------------------------------------------------------------

/** One composite key: values in the table's `keyColumns` order. */
export type IdentityAuthKey = ReadonlyArray<string>

export type IdentityAuthMirror = Readonly<{
  /** Read the rows for the composite keys back from D1 → Postgres. */
  mirrorRowsByKey: (
    table: IdentityAuthDomainTable,
    keys: ReadonlyArray<IdentityAuthKey>,
  ) => Promise<void>
  /**
   * Read every row matching a bounded equality scan back from D1 →
   * Postgres (e.g. resolving a pending github attempt by its `state`
   * nonce, or a provider account family by provider_account_id). `refs`
   * overrides the diagnostic refs — REQUIRED when the scan values are
   * custody-bearing (e.g. `state`, `user_code`).
   */
  mirrorRowsWhere: (
    table: IdentityAuthDomainTable,
    whereColumns: ReadonlyArray<string>,
    values: ReadonlyArray<string>,
    refs?: ReadonlyArray<string>,
  ) => Promise<void>
}>

export type MakeIdentityAuthMirrorDependencies = Readonly<{
  db: D1Database
  postgres: IdentityAuthWriteStore
  log: IdentityAuthLog
}>

/** Bounded read-back scan size (per-key row families are small by
 * construction: attempts per user, links per owner, custody per ref). */
const MIRROR_SCAN_LIMIT = 500

/**
 * Fail-soft read-back mirror: every method reads the authoritative rows
 * from D1 and converge-upserts them into Postgres; every failure is logged
 * (keys only) and swallowed. NEVER throws. A key that never matched a D1
 * row (e.g. an UPDATE that matched nothing) mirrors zero rows — exactly
 * right, D1 holds no such row either.
 */
export const makeIdentityAuthMirror = (
  deps: MakeIdentityAuthMirrorDependencies,
): IdentityAuthMirror => {
  const { db, log, postgres } = deps

  const guarded = async (
    op: string,
    refs: ReadonlyArray<string>,
    run: () => Promise<void>,
  ): Promise<void> => {
    try {
      await run()
    } catch (error) {
      log('khala_sync_identity_dual_write_failed', {
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
              const keyColumns = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].keyColumns
              const tuple = `(${keyColumns.map(column => `${column} = ?`).join(' AND ')})`
              const where = keys.map(() => tuple).join(' OR ')
              const rows = await db
                .prepare(`SELECT * FROM ${table} WHERE ${where}`)
                .bind(...keys.flat())
                .all<IdentityAuthDomainRow>()
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
            .all<IdentityAuthDomainRow>()
          await postgres.upsertRows(table, rows.results ?? [])
        },
      ),
  }
}

// ---------------------------------------------------------------------------
// Env plumbing
// ---------------------------------------------------------------------------

export type IdentityAuthStoreEnv = IdentityAuthFlagEnv &
  Readonly<{
    OPENAGENTS_DB?: D1Database
    KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding | undefined
  }>

export type MakeIdentityAuthStoreOptions = Readonly<{
  /** Injectable client factory (tests). Default: postgres.js/Hyperdrive. */
  makeSqlClient?: MakeKhalaSyncPushSqlClient | undefined
  log?: IdentityAuthLog | undefined
  /** D1 handle override (tests / already-held proxied databases). */
  db?: D1Database | undefined
}>

export const postgresIdentityAuthStoreForEnv = (
  env: IdentityAuthStoreEnv,
  options: MakeIdentityAuthStoreOptions = {},
): PostgresIdentityAuthStore | undefined => {
  const connectionString = env.KHALA_SYNC_DB?.connectionString
  if (connectionString === undefined || connectionString.length === 0) {
    return undefined
  }
  const makeSqlClient = options.makeSqlClient ?? defaultMakeKhalaSyncSqlClient
  return makePostgresIdentityAuthStore({
    acquireSql: () => makeSqlClient(connectionString),
  })
}

/**
 * The production fail-soft read-back mirror for this env, or undefined when
 * the binding is absent or KHALA_SYNC_IDENTITY_DUAL_WRITE is off. Every
 * identity/auth write call site adopts this handle: after its authoritative
 * D1 write it calls `mirror.mirrorRowsByKey(table, keys)` (or
 * `mirrorRowsWhere`). Reads NEVER route here in this lane.
 */
export const identityAuthMirrorFromEnv = (
  env: IdentityAuthStoreEnv,
  options: MakeIdentityAuthStoreOptions = {},
): IdentityAuthMirror | undefined => {
  const flags = identityAuthFlagsFromEnv(env)
  const log = options.log ?? defaultLog

  // A `postgres` read flag is inert here (no routed auth read) but must be
  // observable as a deliberate no-op so a premature flip is visible.
  if (flags.reads === 'postgres') {
    log('khala_sync_identity_postgres_reads_deferred', {
      messageSafe:
        'KHALA_SYNC_IDENTITY_READS=postgres is the highest-risk, owner-gated, done-last step; identity reads still serve D1 in this lane',
      op: 'reads',
      refs: [],
    })
  }

  if (!flags.dualWrite) {
    return undefined
  }
  const postgres = postgresIdentityAuthStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  return makeIdentityAuthMirror({ db, log, postgres })
}

// ---------------------------------------------------------------------------
// Token custody drop-in (the flagship wired writer)
// ---------------------------------------------------------------------------

/**
 * Drop-in for
 * `makeD1ProviderAccountTokenCustodyStore(openAgentsDatabase(env))`. Wraps
 * the token-custody store's WRITE methods: the authoritative D1 write runs
 * unchanged, then the affected rows read-back mirror by their PKs.
 * `provider_account_token_custody` is keyed by `provider_account_ref`;
 * `provider_account_token_custody_audit` by `id`. Both keys are safe to
 * log — the ENCRYPTED token material (ciphertext/IVs/key ids) never leaves
 * a diagnostic. A mirror failure never fails the custody write.
 */
export const makeProviderAccountTokenCustodyStoreForEnv = (
  env: IdentityAuthStoreEnv,
  options: MakeIdentityAuthStoreOptions = {},
): ProviderAccountTokenCustodyStore => {
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const base = makeD1ProviderAccountTokenCustodyStore(db)
  const mirror = identityAuthMirrorFromEnv(env, { ...options, db })
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    upsertConnectedAuth: async (record, auditEvent) => {
      await base.upsertConnectedAuth(record, auditEvent)
      await mirror.mirrorRowsByKey('provider_account_token_custody', [
        [record.providerAccountRef],
      ])
      await mirror.mirrorRowsByKey('provider_account_token_custody_audit', [
        [auditEvent.id],
      ])
    },
    saveRefreshedAuth: async (record, auditEvent) => {
      await base.saveRefreshedAuth(record, auditEvent)
      await mirror.mirrorRowsByKey('provider_account_token_custody', [
        [record.providerAccountRef],
      ])
      await mirror.mirrorRowsByKey('provider_account_token_custody_audit', [
        [auditEvent.id],
      ])
    },
    insertAuditEvent: async auditEvent => {
      await base.insertAuditEvent(auditEvent)
      await mirror.mirrorRowsByKey('provider_account_token_custody_audit', [
        [auditEvent.id],
      ])
    },
  }
}
