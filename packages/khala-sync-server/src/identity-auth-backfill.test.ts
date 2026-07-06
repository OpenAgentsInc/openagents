// KS-8.18 (#8329): identity/auth backfill core — idempotency + verify
// fidelity + schema/registry alignment for the LAST, most sensitive domain.
//
// Load-bearing properties: converge upserts are IDEMPOTENT over the PKs (a
// re-run with the same D1 page converges to the identical Postgres state)
// and converge to the LATEST D1 snapshot (token rotation, status flips),
// the row hash canonicalizes D1 numbers and postgres.js bigint strings to
// the same digest, and every one of the SEVENTEEN twins accepts a full-row
// converge (schema ≡ registry). SECRETS: no assertion and no helper output
// prints a custody column value (token ciphertext/IVs/key ids, openauth
// value_json, device user_code, OAuth state) — keys and sha256 hashes only.

import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import {
  buildIdentityAuthVerifyReport,
  d1IdentityAuthNewestHashes,
  IDENTITY_AUTH_DOMAIN_TABLE_SPECS,
  IDENTITY_AUTH_DOMAIN_TABLES,
  IDENTITY_AUTH_SCALAR_TALLIES,
  identityAuthRowHash,
  identityAuthRowKey,
  identityAuthVerifyReportClean,
  postgresIdentityAuthNewestHashes,
  postgresIdentityAuthRowCount,
  postgresIdentityAuthScalar,
  upsertIdentityAuthRows,
  type D1IdentityAuthSourceRow,
  type IdentityAuthDomainTable,
} from "./identity-auth-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const T0 = "2026-07-04T00:00:00.000Z"
const T1 = "2026-07-04T01:00:00.000Z"
const OWNER = "user_owner"
const REF = "provider_account_ref_1"

// ---------------------------------------------------------------------------
// One full-row sample per table (snake_case, exactly as wrangler returns).
// ---------------------------------------------------------------------------

const custodyRow = (
  overrides: Partial<Record<string, unknown>> = {},
): D1IdentityAuthSourceRow => ({
  access_ciphertext_b64: "CIPHERTEXT-access",
  access_expires_at: T1,
  access_iv_b64: "iv-access",
  access_key_id: "key.v1",
  account_id: "acct_1",
  created_at: T0,
  id_token_ciphertext_b64: null,
  id_token_iv_b64: null,
  id_token_key_id: null,
  last_refreshed_at: null,
  owner_user_id: OWNER,
  provider: "chatgpt_codex",
  provider_account_ref: REF,
  refresh_ciphertext_b64: "CIPHERTEXT-refresh",
  refresh_iv_b64: "iv-refresh",
  refresh_key_id: "key.v1",
  secret_ref: "secret_ref_1",
  updated_at: T0,
  ...overrides,
})

const SAMPLE_ROWS: Record<IdentityAuthDomainTable, D1IdentityAuthSourceRow> = {
  users: {
    avatar_url: null,
    created_at: T0,
    deleted_at: null,
    display_name: "Owner",
    id: OWNER,
    kind: "human",
    // CFG-4 Domain 2 (#8519): the registry's users spec carries the full
    // worker column set including the 0025 onboarding columns (NOT NULL
    // onboarding_step) — a real D1 snapshot always has them.
    onboarding_billing_skipped_at: null,
    onboarding_completed_at: null,
    onboarding_goal: null,
    onboarding_repository_default_branch: null,
    onboarding_repository_description: null,
    onboarding_repository_full_name: null,
    onboarding_repository_html_url: null,
    onboarding_repository_id: null,
    onboarding_repository_name: null,
    onboarding_repository_owner: null,
    onboarding_repository_private: null,
    onboarding_repository_provider: null,
    onboarding_repository_selected_at: null,
    onboarding_repository_skipped_at: null,
    onboarding_step: "repository",
    onboarding_updated_at: null,
    primary_email: "owner@contract.test",
    status: "active",
    updated_at: T0,
  },
  auth_identities: {
    created_at: T0,
    deleted_at: null,
    email: "owner@contract.test",
    id: "identity_1",
    provider: "github",
    provider_subject: "gh_123",
    provider_username: "owner",
    updated_at: T0,
    user_id: OWNER,
  },
  openauth_storage: {
    expires_at: 1893456000000,
    key: "oauth:refresh:owner",
    updated_at: T0,
    value_json: '{"refreshToken":"SECRET-SESSION-PAYLOAD"}',
  },
  openauth_agent_links: {
    agent_credential_id: "cred_1",
    agent_user_id: "agent_1",
    created_at: T0,
    id: "link_1",
    link_kind: "claim_approval",
    openauth_user_id: OWNER,
    revoked_at: null,
    status: "active",
    updated_at: T0,
  },
  github_write_connections: {
    connected_at: T0,
    connection_ref: "conn_ref_1",
    created_at: T0,
    deleted_at: null,
    disconnected_at: null,
    github_id: "gh_123",
    github_login: "owner",
    health: "healthy",
    id: "conn_1",
    last_status_at: T0,
    metadata_json: null,
    scopes_json: '["repo"]',
    secret_ref: "secret_ref_gh",
    status: "connected",
    updated_at: T0,
    user_id: OWNER,
  },
  github_write_connection_attempts: {
    completed_at: null,
    created_at: T0,
    expected_github_id: "gh_123",
    expected_github_login: "owner",
    expires_at: T1,
    failed_at: null,
    failure_reason: null,
    id: "attempt_1",
    redirect_after: "/settings",
    scopes_json: '["repo"]',
    state: "OAUTH-CSRF-NONCE",
    status: "pending",
    updated_at: T0,
    user_id: OWNER,
  },
  github_write_auth_grants: {
    connection_id: "conn_1",
    connection_ref: "conn_ref_1",
    created_at: T0,
    expires_at: T1,
    failed_at: null,
    grant_ref: "grant_ref_1",
    id: "grant_1",
    metadata_json: null,
    requested_action: "push_commit",
    revoked_at: null,
    runner_session_id: "runner_1",
    secret_ref: "secret_ref_gh",
    status: "issued",
    updated_at: T0,
    used_at: null,
    user_id: OWNER,
  },
  provider_accounts: {
    account_label: "Codex",
    auth_mode: "chatgpt_device_code",
    connected_at: T0,
    cooldown_until: null,
    created_at: T0,
    deleted_at: null,
    denied_at: null,
    disconnected_at: null,
    health: "healthy",
    id: "pa_1",
    last_failed_launch_at: null,
    last_parallel_probe_at: null,
    last_parallel_probe_result: null,
    last_sanity_check_at: null,
    last_sanity_check_result: null,
    last_selected_at: null,
    last_status_at: T0,
    last_successful_launch_at: null,
    lease_limit: 1,
    low_credit_flag: 0,
    metadata_json: null,
    operator_label: null,
    operator_note: null,
    operator_priority: 100,
    plan_type: "plus",
    provider: "chatgpt_codex",
    provider_account_ref: REF,
    reauth_required_reason: null,
    recent_failure_class: null,
    refill_note: null,
    secret_ref: "secret_ref_pa",
    status: "connected",
    team_id: null,
    updated_at: T0,
    user_id: OWNER,
  },
  provider_account_connection_attempts: {
    completed_at: null,
    created_at: T0,
    expires_at: T1,
    failed_at: null,
    id: "pac_1",
    login_ref: "login_1",
    metadata_json: null,
    method: "chatgpt_device_code",
    provider: "chatgpt_codex",
    provider_account_id: "pa_1",
    source: "worker_device_code",
    status: "pending",
    team_id: null,
    updated_at: T0,
    user_code: "DEVICE-CODE-1234",
    user_id: OWNER,
    verification_url: "https://example.test/device",
  },
  provider_account_auth_grants: {
    created_at: T0,
    expires_at: T1,
    failed_at: null,
    grant_ref: "pa_grant_ref_1",
    id: "pag_1",
    metadata_json: null,
    provider: "chatgpt_codex",
    provider_account_id: "pa_1",
    provider_account_ref: REF,
    provider_secret_ref: "secret_ref_pa",
    requested_action: "launch",
    revoked_at: null,
    runner_session_id: "runner_1",
    status: "issued",
    team_id: null,
    thread_id: null,
    updated_at: T0,
    used_at: null,
    user_id: OWNER,
    workroom_id: null,
  },
  provider_account_events: {
    actor_id: "owner",
    auth_grant_id: null,
    created_at: T0,
    evidence_refs_json: "[]",
    id: "pae_1",
    kind: "login_connected",
    metadata_json: null,
    provider_account_id: "pa_1",
    runner_session_id: null,
    source_refs_json: "[]",
    summary: "connected",
    target_ref: null,
    team_id: null,
    thread_id: null,
    user_id: OWNER,
    workroom_id: null,
  },
  provider_account_sanity_checks: {
    classification: "healthy",
    created_at: T0,
    grant_ref: null,
    id: "pasc_1",
    metadata_json: null,
    provider: "chatgpt_codex",
    provider_account_id: "pa_1",
    provider_account_ref: REF,
    summary: "ok",
    team_id: null,
    user_id: OWNER,
  },
  provider_account_parallel_probe_receipts: {
    classification: "healthy",
    collision_class: "none",
    finished_at: T1,
    id: "papr_1",
    lease_id: "lease_1",
    metadata_json: null,
    probe_id: "probe_1",
    probe_run_id: "run_1",
    provider_account_id: "pa_1",
    provider_account_ref: REF,
    started_at: T0,
    team_id: null,
    terminal_status: "passed",
    user_id: OWNER,
  },
  provider_account_leases: {
    assignment_id: null,
    expires_at: T1,
    failure_class: null,
    id: "lease_1",
    last_touched_at: null,
    lease_ref: "lease_ref_1",
    metadata_json: null,
    order_id: null,
    provider: "chatgpt_codex",
    provider_account_id: "pa_1",
    provider_account_ref: REF,
    released_at: null,
    requested_action: "launch",
    run_id: null,
    selected_by_actor: "system",
    selected_by_policy_version: "provider-account-lease-policy:v1",
    selection_reason: "priority",
    started_at: T0,
    status: "active",
    team_id: null,
    terminal_outcome: null,
    user_id: OWNER,
  },
  provider_account_failover_receipts: {
    account_state_action: "cooldown",
    assignment_id: null,
    attempt_number: 1,
    cooldown_until: null,
    created_at: T0,
    customer_safe_status: "retrying",
    customer_safe_summary: null,
    failure_class: "rate_limited",
    id: "pafr_1",
    max_attempts: 3,
    metadata_json: null,
    next_lease_ref: null,
    next_provider_account_ref: null,
    operator_summary: "Provider account failover was recorded.",
    order_id: null,
    outcome: "retrying",
    policy_version: "provider-account-lease-policy:v1",
    previous_lease_ref: "lease_ref_1",
    previous_provider_account_ref: REF,
    requested_action: "launch",
    run_id: null,
  },
  provider_account_token_custody: custodyRow(),
  provider_account_token_custody_audit: {
    actor_ref: "owner",
    created_at: T0,
    error_message: null,
    error_tag: null,
    event_kind: "auth_stored",
    id: "audit_1",
    metadata_json: null,
    owner_user_id: OWNER,
    provider: "chatgpt_codex",
    provider_account_ref: REF,
    source_ref: null,
    status: "succeeded",
  },
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

describe("registry (pure)", () => {
  test("all seventeen identity/auth tables are registered with PKs inside columns", () => {
    expect(IDENTITY_AUTH_DOMAIN_TABLES.length).toBe(17)
    for (const table of IDENTITY_AUTH_DOMAIN_TABLES) {
      const spec = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table]
      expect(spec.keyColumns.length).toBeGreaterThan(0)
      for (const key of spec.keyColumns) {
        expect(spec.columns).toContain(key)
      }
      expect(spec.columns).toContain(spec.orderColumn)
    }
  })

  test("custody columns are declared for the secret-bearing tables", () => {
    expect(
      IDENTITY_AUTH_DOMAIN_TABLE_SPECS.provider_account_token_custody
        .custodyColumns,
    ).toContain("refresh_ciphertext_b64")
    expect(
      IDENTITY_AUTH_DOMAIN_TABLE_SPECS.openauth_storage.custodyColumns,
    ).toEqual(["value_json"])
    expect(
      IDENTITY_AUTH_DOMAIN_TABLE_SPECS.github_write_connection_attempts
        .custodyColumns,
    ).toEqual(["state"])
    expect(
      IDENTITY_AUTH_DOMAIN_TABLE_SPECS.provider_account_connection_attempts
        .custodyColumns,
    ).toEqual(["user_code"])
    // A PK is never custody-bearing (keys are always safe to print).
    for (const table of IDENTITY_AUTH_DOMAIN_TABLES) {
      const spec = IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table]
      for (const key of spec.keyColumns) {
        expect(spec.custodyColumns ?? []).not.toContain(key)
      }
    }
  })

  test("no scalar tally selects a custody column value", () => {
    const custodyNames = new Set<string>()
    for (const table of IDENTITY_AUTH_DOMAIN_TABLES) {
      for (const column of
        IDENTITY_AUTH_DOMAIN_TABLE_SPECS[table].custodyColumns ?? []) {
        custodyNames.add(column)
      }
    }
    for (const table of IDENTITY_AUTH_DOMAIN_TABLES) {
      for (const tally of IDENTITY_AUTH_SCALAR_TALLIES[table]) {
        for (const custody of custodyNames) {
          // No tally references any custody column at all (not even in a
          // NULL predicate): the stronger, cleaner invariant.
          expect(tally.sql).not.toContain(custody)
        }
      }
    }
  })
})

describe("identityAuthRowHash (pure)", () => {
  test("identical rows hash identically; any column drift changes the hash", () => {
    const a = custodyRow()
    const b = custodyRow()
    expect(identityAuthRowHash("provider_account_token_custody", a)).toBe(
      identityAuthRowHash("provider_account_token_custody", b),
    )
    const rotated = custodyRow({ refresh_ciphertext_b64: "CIPHERTEXT-rotated" })
    expect(
      identityAuthRowHash("provider_account_token_custody", rotated),
    ).not.toBe(identityAuthRowHash("provider_account_token_custody", a))
  })

  test("D1 numbers and postgres.js bigint strings canonicalize equal", () => {
    const d1Side = SAMPLE_ROWS.provider_accounts
    const pgSide = { ...SAMPLE_ROWS.provider_accounts, operator_priority: "100" }
    expect(identityAuthRowHash("provider_accounts", d1Side)).toBe(
      identityAuthRowHash("provider_accounts", pgSide),
    )
  })

  test("row keys are the PK values, never custody values", () => {
    const key = identityAuthRowKey(
      "provider_account_token_custody",
      custodyRow(),
    )
    expect(key).toBe(REF)
    expect(key).not.toContain("CIPHERTEXT")
    const attemptKey = identityAuthRowKey(
      "provider_account_connection_attempts",
      SAMPLE_ROWS.provider_account_connection_attempts,
    )
    expect(attemptKey).not.toContain("DEVICE-CODE")
  })
})

describe("verify report (pure)", () => {
  test("clean report on matching inputs; drift flips it", () => {
    const rows = [custodyRow(), custodyRow({ provider_account_ref: "ref_2" })]
    const clean = buildIdentityAuthVerifyReport({
      d1Newest: d1IdentityAuthNewestHashes(
        "provider_account_token_custody",
        rows,
      ),
      d1Total: 2,
      postgresNewest: d1IdentityAuthNewestHashes(
        "provider_account_token_custody",
        rows,
      ),
      postgresTotal: 2,
      scalars: [{ d1: 2, metric: "distinct_owners", postgres: 2 }],
      table: "provider_account_token_custody",
    })
    expect(identityAuthVerifyReportClean(clean)).toBe(true)

    const drifted = buildIdentityAuthVerifyReport({
      d1Newest: d1IdentityAuthNewestHashes(
        "provider_account_token_custody",
        rows,
      ),
      d1Total: 2,
      postgresNewest: d1IdentityAuthNewestHashes(
        "provider_account_token_custody",
        [custodyRow({ refresh_ciphertext_b64: "DRIFT" }), rows[1]!],
      ),
      postgresTotal: 2,
      scalars: [{ d1: 2, metric: "distinct_owners", postgres: 1 }],
      table: "provider_account_token_custody",
    })
    expect(identityAuthVerifyReportClean(drifted)).toBe(false)
    expect(drifted.scalarMismatches.length).toBe(1)
    expect(drifted.newestHashMismatches.length).toBe(1)
  })
})

// ---------------------------------------------------------------------------
// Postgres integration (skipped without local server binaries)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("identity/auth backfill — Postgres", () => {
  let pg: LocalPostgres
  let rawSql: SQL
  let sql: SyncSql

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_identity_auth_backfill")
    await admin.end()
    const url = pg.urlFor("khala_identity_auth_backfill")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0028_identity_auth_domain.sql")
    rawSql = new SQL({ url, max: 4 })
    sql = rawSql as unknown as SyncSql
  })

  afterAll(async () => {
    await rawSql?.end()
    await pg?.stop()
  })

  test("all seventeen twins accept a full-row converge (schema ≡ registry)", async () => {
    for (const table of IDENTITY_AUTH_DOMAIN_TABLES) {
      expect(await upsertIdentityAuthRows(sql, table, [SAMPLE_ROWS[table]])).toBe(
        1,
      )
      // Re-running the same row converges without duplication.
      expect(await upsertIdentityAuthRows(sql, table, [SAMPLE_ROWS[table]])).toBe(
        1,
      )
      expect(await postgresIdentityAuthRowCount(sql, table)).toBe(1)
    }
  })

  test("token custody rotation converges to the latest D1 snapshot", async () => {
    await upsertIdentityAuthRows(sql, "provider_account_token_custody", [
      custodyRow({
        access_ciphertext_b64: "CIPHERTEXT-access-rotated",
        last_refreshed_at: T1,
        refresh_ciphertext_b64: "CIPHERTEXT-refresh-rotated",
        updated_at: T1,
      }),
    ])
    const unsafe = (
      sql as unknown as {
        unsafe: (
          q: string,
          p: Array<unknown>,
        ) => Promise<Array<Record<string, unknown>>>
      }
    ).unsafe
    const rows = await unsafe(
      `SELECT refresh_ciphertext_b64, last_refreshed_at FROM provider_account_token_custody WHERE provider_account_ref = $1`,
      [REF],
    )
    expect(rows[0]?.["refresh_ciphertext_b64"]).toBe("CIPHERTEXT-refresh-rotated")
    expect(rows[0]?.["last_refreshed_at"]).toBe(T1)
  })

  test("scalar tallies + newest hashes agree with the D1-side helpers", async () => {
    // A custody-safe tally runs the SAME SQL text on both engines.
    expect(
      await postgresIdentityAuthScalar(
        sql,
        IDENTITY_AUTH_SCALAR_TALLIES.provider_account_token_custody[0]!.sql,
      ),
    ).toBe(1)

    // Newest-N hashes: the Postgres row hashes identically to the (rotated)
    // D1 source row, and the key carries no custody value.
    const newest = await postgresIdentityAuthNewestHashes(
      sql,
      "provider_account_token_custody",
      10,
    )
    const d1Newest = d1IdentityAuthNewestHashes(
      "provider_account_token_custody",
      [
        custodyRow({
          access_ciphertext_b64: "CIPHERTEXT-access-rotated",
          last_refreshed_at: T1,
          refresh_ciphertext_b64: "CIPHERTEXT-refresh-rotated",
          updated_at: T1,
        }),
      ],
    )
    expect(newest).toEqual(d1Newest)
    for (const entry of newest) {
      expect(entry.key).not.toContain("CIPHERTEXT")
    }
  })
})
