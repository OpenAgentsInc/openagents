// CFG-4 Domain 2 (#8519, epic #8515): the identity-core database handle —
// the HARD-CUT replacement for the D1 `users` / `auth_identities` authority.
//
// EXACTLY TWO TABLES live behind this handle: `users` and `auth_identities`.
// They are Cloud SQL Postgres-AUTHORITATIVE (khala-sync-server migration
// `0028_identity_auth_domain.sql` twins + `0042_identity_hard_cut.sql`
// uniques/accelerators); the D1 code path for them is DELETED — no
// dual-write, no mirror, no read flag. This is the owner-approved
// supersession of the old "identity reads stay D1 until the owner-gated
// last step" policy FOR THESE TWO TABLES ONLY: the auth-GATE reads (agent
// bearer-token resolution, session-subject upserts) run here on every
// request now. All fifteen OTHER identity/auth-domain tables
// (openauth_agent_links, github_write_*, provider_account_* — and the
// legacy openauth_storage D1 table) keep their D1 authority and the
// `identityAuthMirrorFromEnv` dual-write machinery in
// `identity-auth-domain-store.ts`.
//
// The handle itself is the SAME generic Hyperdrive executor the credits
// ledger proved in Domain 1 (`payments-ledger-db.ts`): D1-style `?`
// placeholders auto-translated to `$n`, `batch` = one atomic Postgres
// transaction, fail-hard at use when `KHALA_SYNC_DB` is absent (an auth
// write must never silently land in a store nobody reads). Both domains
// talk to the SAME khala_sync Postgres database, so a statement on either
// handle may JOIN `users` × `agent_balances` freely.
//
// Statement portability: keep `?` placeholders, no `INSERT OR IGNORE`/
// `OR REPLACE` (use `ON CONFLICT`), no `datetime()`, no bare `? IS NULL`
// (build SQL variants) — guarded in tests by
// `test/payments-ledger-sqlite.ts`'s `assertPortableLedgerSql`.

import {
  paymentsLedgerDbForEnv,
  type LedgerParam,
  type PaymentsLedgerDb,
} from './payments-ledger-db'

/**
 * The identity-core database handle (`users` + `auth_identities`).
 * Structurally the same executor as the credits ledger — one Postgres
 * database, one driver discipline. Tests back it with
 * `test/payments-ledger-sqlite.ts` adapters sharing the test's SQLite
 * database with its D1 shim.
 */
export type IdentityDb = PaymentsLedgerDb

/** Widened `| undefined` so every env shape in the Worker (they all
 * declare `KHALA_SYNC_DB?: ... | undefined` under
 * exactOptionalPropertyTypes) can flow in unchanged. `IDENTITY_DB` is the
 * test-override slot (same pattern as CFG-3's `AUTH_KV` override): tests
 * that drive the whole Worker inject a SQLite-backed handle here;
 * production never sets it. */
export type IdentityDbEnv = Readonly<{
  IDENTITY_DB?: IdentityDb | undefined
  KHALA_SYNC_DB?: Readonly<{ connectionString: string }> | undefined
}>

/**
 * The production wiring: identity core over the `KHALA_SYNC_DB` Hyperdrive
 * binding. Construction never throws; USE fails hard with
 * `PaymentsLedgerUnavailableError` when the binding is absent (see
 * `paymentsLedgerDbForEnv` — reused, NOT duplicated).
 */
export const identityDbForEnv = (env: IdentityDbEnv): IdentityDb =>
  env.IDENTITY_DB ??
  paymentsLedgerDbForEnv(
    env.KHALA_SYNC_DB === undefined
      ? {}
      : { KHALA_SYNC_DB: env.KHALA_SYNC_DB },
  )

// ---------------------------------------------------------------------------
// Shared identity profile lookup
// ---------------------------------------------------------------------------
//
// The most common shape the hard cut splits out of old D1 JOINs: a batch
// "display fields + GitHub identity for these user ids" read. D1-resident
// tables (team_chat_messages, software_orders, agent_credentials, …) can no
// longer JOIN `users` in one statement, so call sites read their own rows
// from D1, collect the user ids, and enrich through this ONE IN-list query
// (no N+1).

export type IdentityUserProfile = Readonly<{
  userId: string
  kind: string
  displayName: string
  primaryEmail: string | null
  avatarUrl: string | null
  status: string
  createdAt: string
  deletedAt: string | null
  /** `auth_identities.provider_username` for provider='github', live rows. */
  githubUsername: string | null
  /** `auth_identities.provider_subject` for provider='github', live rows. */
  githubId: string | null
}>

/**
 * Batch-read identity profiles by user id. Returns a map keyed by
 * `users.id`; DELETED users are included with `deletedAt` set so call
 * sites keep their own old JOIN semantics (`users.deleted_at IS NULL` ⇒
 * filter; plain LEFT JOIN ⇒ keep).
 */
export const readIdentityUserProfiles = async (
  identityDb: IdentityDb,
  userIds: ReadonlyArray<string>,
): Promise<ReadonlyMap<string, IdentityUserProfile>> => {
  const unique = [...new Set(userIds.filter(id => id.length > 0))]
  if (unique.length === 0) return new Map()
  const placeholders = unique.map(() => '?').join(', ')
  const rows = await identityDb.query(
    `SELECT users.id,
            users.kind,
            users.display_name,
            users.primary_email,
            users.avatar_url,
            users.status,
            users.created_at,
            users.deleted_at,
            (SELECT auth_identities.provider_username
               FROM auth_identities
              WHERE auth_identities.user_id = users.id
                AND auth_identities.provider = 'github'
                AND auth_identities.deleted_at IS NULL
              LIMIT 1) AS github_username,
            (SELECT auth_identities.provider_subject
               FROM auth_identities
              WHERE auth_identities.user_id = users.id
                AND auth_identities.provider = 'github'
                AND auth_identities.deleted_at IS NULL
              LIMIT 1) AS github_id
       FROM users
      WHERE users.id IN (${placeholders})`,
    unique as ReadonlyArray<LedgerParam>,
  )
  return new Map(
    rows.map(row => [
      String(row.id),
      {
        avatarUrl: row.avatar_url === null ? null : String(row.avatar_url),
        createdAt: String(row.created_at),
        deletedAt: row.deleted_at === null ? null : String(row.deleted_at),
        displayName: String(row.display_name),
        githubId: row.github_id === null ? null : String(row.github_id),
        githubUsername:
          row.github_username === null ? null : String(row.github_username),
        kind: String(row.kind),
        primaryEmail:
          row.primary_email === null ? null : String(row.primary_email),
        status: String(row.status),
        userId: String(row.id),
      } satisfies IdentityUserProfile,
    ]),
  )
}
