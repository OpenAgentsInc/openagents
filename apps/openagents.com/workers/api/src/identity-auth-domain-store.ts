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
// THE SEAM: identity/auth writes are spread across ~10 owning modules (six
// typed store/repository factories plus scattered inline route helpers).
// The seam here is the read-back mirror exposed two ways:
//   1. `identityAuthMirrorFromEnv(env)` — a fail-soft `IdentityAuthMirror`
//      handle a write call site invokes AFTER its authoritative D1 write
//      to converge the touched rows (by PK) into Postgres. This is the
//      uniform adoption path for every writer.
//   2. Drop-in `make*ForEnv(env)` factories — `makeProviderAccountTokenCustodyStoreForEnv`
//      (the flagship, secret-bearing token-custody vault, wired in #8329),
//      plus this follow-up's (#8362) `makeOpenAuthStorageForEnv`,
//      `makeGitHubWriteRepositoryForEnv`, and
//      `makeProviderAccountRepositoryForEnv` — each wraps a base D1
//      store/repository and read-back mirrors every write method.
//
// WIRING STATUS (#8362 — every write call site listed in the follow-up
// issue is now wired): all five typed factories adopt a mirror
// (`agent-registration.ts`'s and `agent-owner-claim-routes.ts`'s
// `makeAgentRegistrationStoreForEnv`/`makeAgentOwnerClaimStoreForEnv`
// compose a dedicated identity-auth wrapper ON TOP of their pre-existing,
// DIFFERENT-domain `AgentRuntimeRemainderMirror` wrap). The scattered
// inline writers (`index.ts`'s `upsertGitHubUser`/`upsertEmailUser`,
// `onboarding/repository.ts`'s five `users` UPDATEs,
// `auth/email-otp-hardening.ts`'s SECOND `openauth_storage` writer,
// `operator-provider-account-routes.ts`, `provider-account-pool-routes.ts`,
// `artanis-operator-dashboard-routes.ts`) all take an optional
// `IdentityAuthMirror` parameter, threaded from the nearest call site that
// holds `env`. Two categories are DELIBERATELY left unmirrored, documented
// inline at each site: (a) D1's own incidental bulk/lazy-expiry side
// effects on HOT READ paths (`provider_account_leases` stale-expiry sweeps,
// `openauth_storage.get()`'s lazy TTL cleanup) — mirroring those would add
// unbounded/per-request Postgres writes to a read path, exactly what this
// lane must avoid; they converge on the next `--restart` backfill sweep
// instead; (b) a small number of call sites that only ever invoke READ
// methods (e.g. `omni-handlers.ts`'s preflight checks, `artanis-forum-identity.ts`'s
// resolve helper) need no mirror at all.
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
//
// #8362 follow-up (bounded non-gate read allowlist, 2026-07-05): a SECOND,
// FULLY INDEPENDENT read surface, following the entitlements domain's
// `*_NON_GATE_READS` precedent (#8336) and the billing/business-domain
// bounded-allowlist precedent (#8337/#8360). `KHALA_SYNC_IDENTITY_READS`
// (above) governs ONLY the auth-decision gate reads and is DELIBERATELY
// left untouched (default 'd1', never flipped by this follow-up) — the KV
// cache layer and auth-matrix shadow-read replay tooling that step needs
// remain unbuilt. A brand-new flag, `KHALA_SYNC_IDENTITY_NON_GATE_READS`
// (d1|compare|postgres, default 'd1'), instead governs a bounded set of
// PUBLIC/OPERATOR PROJECTION reads that decide nothing. Every read call
// site outside this domain's own store was re-audited for this follow-up
// (`docs/khala-sync/RUNBOOK.md` "Identity/auth domain cutover" §"2026-07-05
// follow-up" records the inventory); only ONE cleared the conservative bar:
// `provider-account-usage-routes.ts`'s `listPoolState` (the admin-only,
// single-table, no-JOIN `provider_accounts` pool-state projection powering
// the token-usage-by-account report). Every other original candidate
// (admin user listing joined to a not-yet-Postgres-served `software_orders`;
// the operator-account-status reset route, which is a read-your-own-write
// immediately after a mirrored cooldown reset; the triage lease/failover/
// users reads, one of which sits on the domain's own documented
// `provider_account_leases` staleness gap and another of which is embedded
// in a cross-domain blob shared with an existence-gate consumer; the CRM
// target-resolution reads, which actually feed real money-grant and
// account-linking decisions; and the forum author-profile join, which the
// same function also uses to gate a follow-creation existence/self-follow
// check) turned out to be decision-adjacent, cross-domain-blocked, or a
// read-after-write hazard on closer inspection and stays D1-only.
// See `IdentityAuthNonGateReads` below for the exact bounded surface.

import {
  deleteIdentityAuthRows,
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
  normalizeIdentityAuthValue,
  requireIdentityAuthUnsafe,
  upsertIdentityAuthRows,
  type IdentityAuthDomainRow,
  type IdentityAuthDomainTable,
  type SyncSql,
} from '@openagentsinc/khala-sync-server'
import { joinKey } from '@openauthjs/openauth/storage/storage'
import type { StorageAdapter } from '@openauthjs/openauth/storage/storage'

import {
  type OpenAuthStorageRuntime,
  makeD1Storage,
  systemOpenAuthStorageRuntime,
} from './auth/openauth-storage'
import {
  makeD1GitHubWriteRepository,
  type GitHubWriteRepository,
} from './github-write-connections'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'
import { logWorkerRouteWarning } from './observability'
import type { ProviderAccountRepository } from './provider-account-domain'
import { makeD1ProviderAccountRepository } from './provider-account-repository'
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
  /**
   * #8362 follow-up: governs ONLY the bounded non-gate read surface
   * (`IdentityAuthNonGateReads`) — fully independent of `reads`, which
   * stays scoped to auth-decision gate reads (there are none routed yet).
   * Flipping this flag can never change what an auth decision ALLOWS or
   * DENIES; it only changes where a display/reporting projection reads
   * from.
   */
  nonGateReads: IdentityAuthReadsMode
}>

export type IdentityAuthFlagEnv = Readonly<{
  KHALA_SYNC_IDENTITY_DUAL_WRITE?: string | undefined
  KHALA_SYNC_IDENTITY_READS?: string | undefined
  KHALA_SYNC_IDENTITY_NON_GATE_READS?: string | undefined
}>

const FLAG_OFF_VALUES = new Set(['0', 'off', 'false', 'disabled', 'no'])

const parseIdentityAuthReadsMode = (
  raw: string | undefined,
): IdentityAuthReadsMode =>
  raw === 'postgres' || raw === 'compare' ? raw : 'd1'

/**
 * Parse the KS-8.18 migration flags from Worker vars. Dual-write defaults
 * ON (this lane lands with the mirror active wherever the binding exists);
 * reads default to D1 authority — read flips are OWNER-GATED ops decisions
 * (#8282), never a code default, and for AUTH they are the highest-risk
 * step of all. Unknown read values fall back to 'd1' — never fail open
 * into an unproven read path on a typo. `nonGateReads` is parsed the same
 * way but is a SEPARATE env var (#8362 follow-up) governing only the
 * bounded non-gate reads.
 */
export const identityAuthFlagsFromEnv = (
  env: IdentityAuthFlagEnv,
): IdentityAuthFlags => {
  const dualWriteRaw = env.KHALA_SYNC_IDENTITY_DUAL_WRITE?.trim().toLowerCase()

  return {
    dualWrite:
      dualWriteRaw === undefined || !FLAG_OFF_VALUES.has(dualWriteRaw),
    nonGateReads: parseIdentityAuthReadsMode(
      env.KHALA_SYNC_IDENTITY_NON_GATE_READS?.trim().toLowerCase(),
    ),
    reads: parseIdentityAuthReadsMode(
      env.KHALA_SYNC_IDENTITY_READS?.trim().toLowerCase(),
    ),
  }
}

// ---------------------------------------------------------------------------
// Diagnostics (the drift metric)
// ---------------------------------------------------------------------------

export type IdentityAuthDiagnosticEvent =
  | 'khala_sync_identity_dual_write_failed'
  | 'khala_sync_identity_postgres_reads_deferred'
  // #8362 follow-up: the bounded non-gate read allowlist's OWN
  // diagnostics — deliberately distinct event names so a dashboard can
  // never conflate auth-decision drift with this display-only surface.
  | 'khala_sync_identity_non_gate_read_compare_mismatch'
  | 'khala_sync_identity_non_gate_postgres_read_failed'
  | 'khala_sync_identity_non_gate_postgres_read_fallback'

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
// Bounded non-gate read allowlist (#8362 follow-up)
// ---------------------------------------------------------------------------

/**
 * The single row shape `provider-account-usage-routes.ts`'s `listPoolState`
 * reads: a live pool-state snapshot for one provider account, scoped to its
 * owning user. No custody columns (token ciphertext/IVs/key ids never live
 * on `provider_accounts`). Field names match the D1 column names exactly —
 * this is the SAME shape the call site's own `PoolStateRow` type declares,
 * kept structurally compatible on purpose so the call site needs no import
 * cycle into this module.
 */
export type ProviderAccountPoolStateRow = Readonly<{
  provider_account_ref: string
  provider: string
  account_label: string | null
  operator_label: string | null
  status: string
  health: string
  low_credit_flag: number
  cooldown_until: string | null
}>

/**
 * The bounded non-gate read surface (#8362 follow-up). Both D1 and Postgres
 * implement this interface; the routing wrapper below serves it per
 * KHALA_SYNC_IDENTITY_NON_GATE_READS, fully independent of the (currently
 * unrouted) auth-decision gate reads. Every function here is a pure
 * display/reporting projection that never influences an allow/deny, lease
 * acquire/release, or dedupe decision — see the module header for the full
 * audit trail of what was considered and demoted.
 */
export type IdentityAuthNonGateReads = Readonly<{
  /**
   * `provider_accounts` pool-state rows for one operator's own accounts
   * (admin-only route, `provider-account-usage-routes.ts`'s
   * `listPoolState`). Bounded by `limit` (the call site's own
   * `ACCOUNT_USAGE_LIMIT` ceiling); never unbounded.
   */
  providerAccountPoolStateByUserId: (
    userId: string,
    limit: number,
  ) => Promise<ReadonlyArray<ProviderAccountPoolStateRow>>
}>

/**
 * The D1 implementation — the SAME statement
 * `provider-account-usage-routes.ts`'s `listPoolState` runs inline today.
 * Kept here so the compare/postgres router has a D1 side without a runtime
 * import cycle into that route module.
 */
export const makeD1IdentityAuthNonGateReads = (
  db: D1Database,
): IdentityAuthNonGateReads => ({
  providerAccountPoolStateByUserId: async (userId, limit) => {
    const rows = await db
      .prepare(
        `SELECT pa.provider_account_ref,
                pa.provider,
                pa.account_label,
                pa.operator_label,
                pa.status,
                pa.health,
                COALESCE(pa.low_credit_flag, 0) AS low_credit_flag,
                pa.cooldown_until
           FROM provider_accounts pa
          WHERE pa.user_id = ?
            AND pa.deleted_at IS NULL
          LIMIT ?`,
      )
      .bind(userId, limit)
      .all<ProviderAccountPoolStateRow>()
    return rows.results ?? []
  },
})

const toNonGatePoolStateRow = (
  row: Record<string, unknown>,
): ProviderAccountPoolStateRow => ({
  account_label: row.account_label === null ? null : String(row.account_label),
  cooldown_until:
    row.cooldown_until === null ? null : String(row.cooldown_until),
  health: String(row.health),
  low_credit_flag: Number(row.low_credit_flag),
  operator_label:
    row.operator_label === null ? null : String(row.operator_label),
  provider: String(row.provider),
  provider_account_ref: String(row.provider_account_ref),
  status: String(row.status),
})

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
    /**
     * Delete rows by composite PK from the Postgres twin ONLY — never
     * touches D1. Used exclusively by `mirrorDeleteByKey` for the narrow
     * set of explicit-delete writers in this domain (today:
     * `openauth_storage.remove()`). Idempotent.
     */
    deleteRows: (
      table: IdentityAuthDomainTable,
      keys: ReadonlyArray<IdentityAuthKey>,
    ) => Promise<number>
    /** The Postgres side of the bounded non-gate reads (#8362 follow-up). */
    nonGateReads: IdentityAuthNonGateReads
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
    deleteRows: (table, keys) =>
      withSql(sql => deleteIdentityAuthRows(sql, table, keys)),
    nonGateReads: {
      providerAccountPoolStateByUserId: (userId, limit) =>
        withSql(async sql => {
          const rows = await requireIdentityAuthUnsafe(sql)(
            `SELECT provider_account_ref, provider, account_label,
                    operator_label, status, health,
                    COALESCE(low_credit_flag, 0) AS low_credit_flag,
                    cooldown_until
               FROM provider_accounts
              WHERE user_id = $1
                AND deleted_at IS NULL
              LIMIT $2`,
            [userId, limit],
          )
          return rows.map(toNonGatePoolStateRow)
        }),
    },
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
  /**
   * Delete the given composite keys from the Postgres twin ONLY (D1 is
   * unaffected — this is called AFTER an authoritative D1 delete/expiry to
   * converge the mirror). Reserved for genuine explicit-delete write call
   * sites (today: `openauth_storage.remove()`), never for incidental
   * read-path cleanup or unbounded bulk-expiry sweeps.
   */
  mirrorDeleteByKey: (
    table: IdentityAuthDomainTable,
    keys: ReadonlyArray<IdentityAuthKey>,
  ) => Promise<void>
}>

export type MakeIdentityAuthMirrorDependencies = Readonly<{
  db: D1Database
  // Needs `deleteRows` (only on `PostgresIdentityAuthStore`) for
  // `mirrorDeleteByKey`.
  postgres: PostgresIdentityAuthStore
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

    mirrorDeleteByKey: (table, keys) =>
      keys.length === 0
        ? Promise.resolve()
        : guarded(
            `mirror-delete:${table}`,
            keys.slice(0, 10).map(key => key.join('/')),
            async () => {
              await postgres.deleteRows(table, keys)
            },
          ),
  }
}

// ---------------------------------------------------------------------------
// Non-gate read routing (d1 | compare | postgres-with-D1-fallback)
// ---------------------------------------------------------------------------

const stableStringify = (value: unknown): string =>
  JSON.stringify(value, (_key, val: unknown) =>
    val !== null && typeof val === 'object' && !Array.isArray(val)
      ? Object.fromEntries(
          Object.entries(val as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : a > b ? 1 : 0,
          ),
        )
      : val,
  )

export type MakeRoutedIdentityAuthNonGateReadsDependencies = Readonly<{
  d1: IdentityAuthNonGateReads
  postgres: IdentityAuthNonGateReads
  flags: IdentityAuthFlags
  log?: IdentityAuthLog | undefined
  /**
   * Fire-safe scheduler for compare-mode shadow reads (production: leave
   * default — the shadow promise runs detached; tests inject a collector).
   */
  schedule?: ((work: Promise<void>) => void) | undefined
}>

/**
 * Route the bounded non-gate reads per KHALA_SYNC_IDENTITY_NON_GATE_READS
 * (#8362 follow-up) — a flag fully independent of KHALA_SYNC_IDENTITY_READS.
 * NEVER constructed in 'd1' mode (the env factory below returns no
 * `nonGateReads` then, so the call site runs its untouched inline D1 read).
 *
 * compare — serve D1 immediately; schedule a detached Postgres shadow read
 * + comparison, logging the non-gate-scoped drift diagnostic. ZERO
 * blocking latency.
 *
 * postgres — ONE real Postgres attempt, then D1 fallback + diagnostic on
 * ANY error. Safe to actually serve here (unlike a real auth gate) because
 * this read never decides an allow/deny/lease outcome.
 */
export const makeRoutedIdentityAuthNonGateReads = (
  deps: MakeRoutedIdentityAuthNonGateReadsDependencies,
): IdentityAuthNonGateReads => {
  const { d1, flags, postgres } = deps
  const log = deps.log ?? defaultLog
  const schedule =
    deps.schedule ??
    ((work: Promise<void>) => {
      void work
    })

  const route = async <A>(
    op: string,
    d1Read: () => Promise<A>,
    postgresRead: () => Promise<A>,
  ): Promise<A> => {
    if (flags.nonGateReads === 'postgres') {
      try {
        return await postgresRead()
      } catch (error) {
        log('khala_sync_identity_non_gate_postgres_read_fallback', {
          messageSafe: safeMessage(error),
          op,
          refs: [],
        })
        return d1Read()
      }
    }

    // compare: serve D1; shadow-compare off the response path.
    const d1Result = await d1Read()
    schedule(
      postgresRead()
        .then(postgresResult => {
          if (stableStringify(d1Result) !== stableStringify(postgresResult)) {
            log('khala_sync_identity_non_gate_read_compare_mismatch', {
              messageSafe: 'postgres non-gate read differs from d1',
              op,
              refs: [],
            })
          }
        })
        .catch((error: unknown) => {
          log('khala_sync_identity_non_gate_postgres_read_failed', {
            messageSafe: safeMessage(error),
            op,
            refs: [],
          })
        }),
    )
    return d1Result
  }

  return {
    providerAccountPoolStateByUserId: (userId, limit) =>
      route(
        'providerAccountPoolStateByUserId',
        () => d1.providerAccountPoolStateByUserId(userId, limit),
        () => postgres.providerAccountPoolStateByUserId(userId, limit),
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

/**
 * The production bounded non-gate reads for this env (#8362 follow-up), or
 * undefined when KHALA_SYNC_IDENTITY_NON_GATE_READS is 'd1' (the default) or
 * the KHALA_SYNC_DB binding is absent — in both cases the call site keeps
 * its byte-identical inline D1 read. Fully independent of
 * `identityAuthMirrorFromEnv`/dual-write and of the (unrouted)
 * `KHALA_SYNC_IDENTITY_READS` gate-read flag.
 */
export const identityAuthNonGateReadsForEnv = (
  env: IdentityAuthStoreEnv,
  options: MakeIdentityAuthStoreOptions = {},
): IdentityAuthNonGateReads | undefined => {
  const flags = identityAuthFlagsFromEnv(env)
  if (flags.nonGateReads === 'd1') {
    return undefined
  }
  const postgres = postgresIdentityAuthStoreForEnv(env, options)
  if (postgres === undefined) {
    return undefined
  }
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const log = options.log ?? defaultLog
  return makeRoutedIdentityAuthNonGateReads({
    d1: makeD1IdentityAuthNonGateReads(db),
    flags,
    log,
    postgres: postgres.nonGateReads,
  })
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

// ---------------------------------------------------------------------------
// OpenAuth storage drop-in (#8362 follow-up wiring)
// ---------------------------------------------------------------------------

/**
 * Drop-in for `makeD1Storage(openAgentsDatabase(env), runtime)` (the
 * OpenAuth `StorageAdapter`, `openauth_storage` — every session/refresh
 * payload and email-OTP rate-limit bucket lives here; SECOND writer is
 * `auth/email-otp-hardening.ts` `reserveAuthEmailOtpSend`, wired
 * separately below). Wraps the WRITE surface:
 *   - `set` (upsert): read-back mirrors the row by `key` after the D1
 *     write, same as every other writer in this file.
 *   - `remove` (explicit delete): the ONLY hard-delete write call site in
 *     this domain. The D1 delete runs unchanged, then
 *     `mirror.mirrorDeleteByKey` removes the same `key` from the Postgres
 *     twin — this is the one writer that needed the new delete-mirror
 *     capability above.
 *
 * KNOWN, DOCUMENTED DRIFT (not a bug): `get()` also deletes a row
 * internally when it discovers the row already expired
 * (`auth/openauth-storage.ts` lines ~39-46) — a lazy-expiry side effect of
 * a READ. This wrapper deliberately does NOT mirror that path: `get()` is
 * the single hottest call in the entire identity/auth domain (every
 * OpenAuth session/code lookup), and adding a Postgres round-trip there
 * would be exactly the "per-request read storm" RUNBOOK.md's identity/auth
 * cutover section warns against inheriting. The result is that the
 * Postgres twin can accumulate expired-but-undeleted `openauth_storage`
 * rows the mirror never proactively removes, so this table's row COUNT
 * will not converge to exact equality the way `users`/`auth_identities`
 * do — a structural property of a read-back-only mirror over a lazily
 * TTL'd table, not incomplete wiring. See RUNBOOK.md's identity/auth
 * section for the explicit callout and the recommended remediation before
 * any future read cutover (an active TTL-based prune of Postgres's own
 * expired rows, independent of D1, or accept it the same way the
 * tokens-served projection documents its own expected drift sources).
 */
export const makeOpenAuthStorageForEnv = (
  env: IdentityAuthStoreEnv,
  runtime: OpenAuthStorageRuntime = systemOpenAuthStorageRuntime,
  options: MakeIdentityAuthStoreOptions = {},
): StorageAdapter => {
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const base = makeD1Storage(db, runtime)
  const mirror = identityAuthMirrorFromEnv(env, { ...options, db })
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    set: async (key, value, expiry) => {
      await base.set(key, value, expiry)
      await mirror.mirrorRowsByKey('openauth_storage', [[joinKey(key)]])
    },
    remove: async key => {
      await base.remove(key)
      await mirror.mirrorDeleteByKey('openauth_storage', [[joinKey(key)]])
    },
  }
}

// ---------------------------------------------------------------------------
// GitHub write-connections drop-in (#8362 follow-up wiring)
// ---------------------------------------------------------------------------

/**
 * Drop-in for `makeD1GitHubWriteRepository(openAgentsDatabase(env))`.
 * Wraps all six WRITE methods on `GitHubWriteRepository`; the authoritative
 * D1 write (or `db.batch`) runs unchanged, then the affected rows read-back
 * mirror by their PKs (all three `github_write_*` tables key on `id`).
 * `disconnectConnection` additionally scan-mirrors
 * `github_write_auth_grants` on `connection_id` because it can revoke an
 * unbounded number of issued grants without ever resolving their
 * individual ids (not a custody column, so the scan ref is safe to log).
 */
export const makeGitHubWriteRepositoryForEnv = (
  env: IdentityAuthStoreEnv,
  options: MakeIdentityAuthStoreOptions = {},
): GitHubWriteRepository => {
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const base = makeD1GitHubWriteRepository(db)
  const mirror = identityAuthMirrorFromEnv(env, { ...options, db })
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    createAttempt: async attempt => {
      const result = await base.createAttempt(attempt)
      await mirror.mirrorRowsByKey('github_write_connection_attempts', [
        [result.id],
      ])
      return result
    },
    markAttemptFailed: async (attempt, status, reason, now) => {
      const result = await base.markAttemptFailed(attempt, status, reason, now)
      await mirror.mirrorRowsByKey('github_write_connection_attempts', [
        [result.id],
      ])
      return result
    },
    recordConnectedAttempt: async input => {
      const result = await base.recordConnectedAttempt(input)
      await mirror.mirrorRowsByKey('github_write_connections', [[result.id]])
      await mirror.mirrorRowsByKey('github_write_connection_attempts', [
        [input.attempt.id],
      ])
      return result
    },
    disconnectConnection: async input => {
      const result = await base.disconnectConnection(input)
      if (result !== undefined) {
        await mirror.mirrorRowsByKey('github_write_connections', [
          [result.id],
        ])
        await mirror.mirrorRowsWhere(
          'github_write_auth_grants',
          ['connection_id'],
          [result.id],
        )
      }
      return result
    },
    createGrant: async grant => {
      const result = await base.createGrant(grant)
      await mirror.mirrorRowsByKey('github_write_auth_grants', [[result.id]])
      return result
    },
    markGrantUsed: async grant => {
      const result = await base.markGrantUsed(grant)
      await mirror.mirrorRowsByKey('github_write_auth_grants', [[result.id]])
      return result
    },
  }
}

// ---------------------------------------------------------------------------
// Provider-account repository drop-in (#8362 follow-up wiring)
// ---------------------------------------------------------------------------

/**
 * Drop-in for `makeD1ProviderAccountRepository(openAgentsDatabase(env))`.
 * Wraps all seven WRITE methods; every one of them already writes 2-3
 * tables per call (`provider_accounts`, `provider_account_connection_attempts`,
 * `provider_account_auth_grants`, `provider_account_events` — all keyed on
 * `id`), so each wrapper issues one `mirrorRowsByKey` per touched table
 * after the unchanged D1 write. `recordAccountHealth` locates its account
 * row by `provider_account_ref` in D1 but the input `account` record still
 * carries `.id`, so the mirror key is `account.id` there too.
 * `disconnectAccount` additionally scan-mirrors
 * `provider_account_auth_grants` on `provider_account_id` (can revoke an
 * unbounded number of issued grants with no individual ids in scope).
 * `provider_account_token_custody`/`_audit` are NOT part of this
 * repository (separate flagship drop-in above).
 */
export const makeProviderAccountRepositoryForEnv = (
  env: IdentityAuthStoreEnv,
  options: MakeIdentityAuthStoreOptions = {},
): ProviderAccountRepository => {
  const db =
    options.db ?? openAgentsDatabase(env as { OPENAGENTS_DB: D1Database })
  const base = makeD1ProviderAccountRepository(db)
  const mirror = identityAuthMirrorFromEnv(env, { ...options, db })
  if (mirror === undefined) {
    return base
  }
  return {
    ...base,
    saveStartedDeviceLogin: async (
      account,
      attempt,
      event,
      accountAlreadyExists,
    ) => {
      await base.saveStartedDeviceLogin(
        account,
        attempt,
        event,
        accountAlreadyExists,
      )
      await mirror.mirrorRowsByKey('provider_accounts', [[account.id]])
      await mirror.mirrorRowsByKey('provider_account_connection_attempts', [
        [attempt.id],
      ])
      await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
    },
    recordConnectedAttempt: async (account, attempt, event) => {
      const result = await base.recordConnectedAttempt(account, attempt, event)
      await mirror.mirrorRowsByKey('provider_accounts', [[result.id]])
      await mirror.mirrorRowsByKey('provider_account_connection_attempts', [
        [attempt.id],
      ])
      await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
      return result
    },
    recordFailedAttempt: async (account, attempt, event) => {
      const result = await base.recordFailedAttempt(account, attempt, event)
      await mirror.mirrorRowsByKey('provider_accounts', [[result.id]])
      await mirror.mirrorRowsByKey('provider_account_connection_attempts', [
        [attempt.id],
      ])
      await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
      return result
    },
    recordAccountHealth: async (providerAccountRef, account, event) => {
      const result = await base.recordAccountHealth(
        providerAccountRef,
        account,
        event,
      )
      await mirror.mirrorRowsByKey('provider_accounts', [[account.id]])
      await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
      return result
    },
    createAuthGrant: async (grant, event) => {
      const result = await base.createAuthGrant(grant, event)
      await mirror.mirrorRowsByKey('provider_account_auth_grants', [
        [result.id],
      ])
      await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
      return result
    },
    markGrantUsed: async (grant, event) => {
      const result = await base.markGrantUsed(grant, event)
      await mirror.mirrorRowsByKey('provider_account_auth_grants', [
        [result.id],
      ])
      await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
      return result
    },
    disconnectAccount: async (userId, providerAccountRef, now, metadataJson, event) => {
      const result = await base.disconnectAccount(
        userId,
        providerAccountRef,
        now,
        metadataJson,
        event,
      )
      if (result !== undefined) {
        await mirror.mirrorRowsByKey('provider_accounts', [[result.id]])
        await mirror.mirrorRowsWhere(
          'provider_account_auth_grants',
          ['provider_account_id'],
          [result.id],
        )
        await mirror.mirrorRowsByKey('provider_account_events', [[event.id]])
      }
      return result
    },
  }
}
