// KS-8.16 (#8327): forge backfill core — idempotency + verify fidelity.
//
// Load-bearing properties: converge upserts are IDEMPOTENT over the
// COMPOSITE PKs (a re-run with the same D1 page converges to the
// identical Postgres state) and converge to the LATEST D1 snapshot (ref
// tip moves, lease state flips, mirror attempt bumps), the per-repository
// ref-set digest catches any single-ref drift (the storage twin of
// `git ls-remote`), the merge-queue replay digest catches any
// promotion-chain drift, and the row hash canonicalizes D1 numbers and
// postgres.js bigint strings to the same digest. SECRETS: no assertion
// and no helper output prints a custody column value (token_hash /
// token_prefix) — keys and sha256 hashes only, same as the CLI.

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
  buildForgeDomainVerifyReport,
  compareMergeQueueReplays,
  compareRefSets,
  d1ForgeDomainNewestHashes,
  FORGE_DOMAIN_SCALAR_TALLIES,
  FORGE_DOMAIN_TABLE_SPECS,
  FORGE_DOMAIN_TABLES,
  forgeDomainRowHash,
  forgeDomainRowKey,
  forgeDomainVerifyReportClean,
  mergeQueueReplayFromRows,
  postgresForgeDomainNewestHashes,
  postgresForgeDomainRowCount,
  postgresForgeDomainScalar,
  postgresMergeQueueReplay,
  postgresRefSetTally,
  refSetTallyFromRows,
  upsertForgeDomainRows,
  type D1ForgeSourceRow,
} from "./forge-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Fixtures (snake_case rows exactly as `wrangler d1 execute --json` returns)
// ---------------------------------------------------------------------------

const T0 = "2026-07-04T00:00:00.000Z"
const T1 = "2026-07-04T01:00:00.000Z"

const TENANT = "tenant_alpha"
const REPO = "repo_core"

const refRow = (
  name: string,
  overrides: Partial<Record<string, unknown>> = {},
): D1ForgeSourceRow => ({
  created_at: T0,
  object_format: "sha1",
  object_id: "a".repeat(40),
  previous_object_id: null,
  ref_name: name,
  repository_ref: REPO,
  source_refs_json: '["intake.rp_1"]',
  state: "active",
  tenant_ref: TENANT,
  updated_at: T0,
  updated_by_change_ref: "change_1",
  updated_by_packfile_ref: "pack_1",
  updated_by_receive_pack_ref: "rp_1",
  ...overrides,
})

const leaseRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1ForgeSourceRow => ({
  acquired_at: T0,
  expires_at: T1,
  heartbeat_at: T0,
  idempotency_key_hash: `hash_${n}`,
  lease_ref: `lease_${n}`,
  owner_agent_ref: "agent_raynor",
  released_at: null,
  source_refs_json: "[]",
  state: "active",
  tenant_ref: TENANT,
  work_ref: `work_${n}`,
  ...overrides,
})

const tokenRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1ForgeSourceRow => ({
  created_at: T0,
  expires_at: T1,
  last_used_at: null,
  ref_restrictions_json: "[]",
  repository_ref: REPO,
  revoked_at: null,
  source_refs_json: "[]",
  state: "active",
  subject_ref: "agent_raynor",
  tenant_ref: TENANT,
  token_hash: "e3".repeat(32),
  token_prefix: "oa_forge_git_ab",
  token_ref: `token_${n}`,
  ...overrides,
})

const decisionRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1ForgeSourceRow => ({
  base_head: "b".repeat(40),
  blocker_refs_json: "[]",
  candidate_head: "c".repeat(40),
  change_ref: `change_${n}`,
  created_at: T0,
  decided_at: T0,
  decided_by_ref: "forge.merge_queue",
  decision: "approved",
  gate_refs_json: "[]",
  gate_results_json: "[]",
  promoted_head: "d".repeat(40),
  promotion_ref: `promo_${n}`,
  queue_position: n,
  queue_ref: "queue_main",
  redacted: 1,
  source_refs_json: "[]",
  target_ref: "refs/heads/main",
  tenant_ref: TENANT,
  verification_ref: null,
  ...overrides,
})

const ledgerRow = (
  overrides: Partial<Record<string, unknown>> = {},
): D1ForgeSourceRow => ({
  actual_head: "b".repeat(40),
  base_head: "b".repeat(40),
  blocked_json: "[]",
  created_at: T0,
  next_promotion_ref: "promo_1",
  queue_ref: "queue_main",
  ready_json: '["change_1"]',
  source_refs_json: "[]",
  state: "projected",
  tenant_ref: TENANT,
  updated_at: T0,
  virtual_head: "d".repeat(40),
  ...overrides,
})

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

describe("registry (pure)", () => {
  test("all sixteen forge tables are registered with composite keys inside columns", () => {
    expect(FORGE_DOMAIN_TABLES.length).toBe(16)
    for (const table of FORGE_DOMAIN_TABLES) {
      const spec = FORGE_DOMAIN_TABLE_SPECS[table]
      expect(spec.keyColumns.length).toBeGreaterThan(0)
      for (const key of spec.keyColumns) {
        expect(spec.columns).toContain(key)
      }
      expect(spec.columns).toContain(spec.orderColumn)
    }
  })

  test("custody columns are declared for the token table only", () => {
    expect(FORGE_DOMAIN_TABLE_SPECS.forge_git_access_tokens.custodyColumns).toEqual([
      "token_hash",
      "token_prefix",
    ])
    for (const table of FORGE_DOMAIN_TABLES) {
      if (table !== "forge_git_access_tokens") {
        expect(FORGE_DOMAIN_TABLE_SPECS[table].custodyColumns).toBeUndefined()
      }
    }
  })

  test("no scalar tally selects a custody column value", () => {
    for (const table of FORGE_DOMAIN_TABLES) {
      for (const tally of FORGE_DOMAIN_SCALAR_TALLIES[table]) {
        expect(tally.sql).not.toContain("token_hash")
        expect(tally.sql).not.toContain("token_prefix")
      }
    }
  })
})

describe("forgeDomainRowHash (pure)", () => {
  test("identical rows hash identically; any column drift changes the hash", () => {
    const a = refRow("refs/heads/main")
    const b = refRow("refs/heads/main")
    expect(forgeDomainRowHash("forge_git_refs", a)).toBe(
      forgeDomainRowHash("forge_git_refs", b),
    )
    const drifted = refRow("refs/heads/main", { object_id: "f".repeat(40) })
    expect(forgeDomainRowHash("forge_git_refs", drifted)).not.toBe(
      forgeDomainRowHash("forge_git_refs", a),
    )
  })

  test("D1 numbers and postgres.js bigint strings canonicalize equal", () => {
    const d1Side = decisionRow(1, { queue_position: 7 })
    const pgSide = decisionRow(1, { queue_position: "7" })
    expect(forgeDomainRowHash("forge_promotion_decisions", d1Side)).toBe(
      forgeDomainRowHash("forge_promotion_decisions", pgSide),
    )
  })

  test("row keys are the composite PK values, never custody values", () => {
    const key = forgeDomainRowKey("forge_git_access_tokens", tokenRow(1))
    expect(key).toBe(`${TENANT}/token_1`)
    expect(key).not.toContain("e3e3")
    expect(key).not.toContain("oa_forge_git_")
  })
})

describe("ref-set digests (pure)", () => {
  const toScanRow = (row: D1ForgeSourceRow) => ({
    ref_name: row["ref_name"],
    repository_ref: row["repository_ref"],
    state: row["state"],
    tenant_ref: row["tenant_ref"],
    tip: row["object_id"] ?? "<deleted>",
  })

  test("equal ref sets produce no mismatches; a single tip drift is caught", () => {
    const rows = [refRow("refs/heads/main"), refRow("refs/heads/dev")]
    const left = refSetTallyFromRows(rows.map(toScanRow))
    const right = refSetTallyFromRows(rows.map(toScanRow))
    expect(compareRefSets(left, right)).toEqual([])

    const drifted = refSetTallyFromRows(
      [refRow("refs/heads/main", { object_id: "f".repeat(40) }), refRow("refs/heads/dev")].map(
        toScanRow,
      ),
    )
    const mismatches = compareRefSets(left, drifted)
    expect(mismatches.length).toBe(1)
    expect(mismatches[0]?.repository).toBe(`${TENANT}/${REPO}`)
  })

  test("a missing ref changes the count and the digest", () => {
    const full = refSetTallyFromRows(
      [refRow("refs/heads/main"), refRow("refs/heads/dev")].map(toScanRow),
    )
    const short = refSetTallyFromRows([refRow("refs/heads/main")].map(toScanRow))
    const mismatches = compareRefSets(full, short)
    expect(mismatches.length).toBe(1)
    expect(mismatches[0]?.d1?.refs).toBe(2)
    expect(mismatches[0]?.postgres?.refs).toBe(1)
  })
})

describe("merge-queue replay digests (pure)", () => {
  const toDecisionScan = (row: D1ForgeSourceRow) => ({
    base_head: row["base_head"],
    candidate_head: row["candidate_head"],
    decided_at: row["decided_at"],
    decision: row["decision"],
    promoted: row["promoted_head"] ?? "<none>",
    promotion_ref: row["promotion_ref"],
    queue_position: row["queue_position"],
    queue_ref: row["queue_ref"],
    tenant_ref: row["tenant_ref"],
  })
  const toLedgerScan = (row: D1ForgeSourceRow) => ({
    actual_head: row["actual_head"],
    base_head: row["base_head"],
    next_promotion: row["next_promotion_ref"] ?? "<none>",
    queue_ref: row["queue_ref"],
    state: row["state"],
    tenant_ref: row["tenant_ref"],
    virtual_head: row["virtual_head"],
  })

  test("identical chains match; a reordered or altered decision is caught", () => {
    const decisions = [decisionRow(1), decisionRow(2, { decided_at: T1 })]
    const ledgers = [ledgerRow()]
    const left = mergeQueueReplayFromRows(
      decisions.map(toDecisionScan),
      ledgers.map(toLedgerScan),
    )
    const right = mergeQueueReplayFromRows(
      decisions.map(toDecisionScan),
      ledgers.map(toLedgerScan),
    )
    expect(compareMergeQueueReplays(left, right)).toEqual([])

    const altered = mergeQueueReplayFromRows(
      [decisionRow(1), decisionRow(2, { decided_at: T1, decision: "blocked" })].map(
        toDecisionScan,
      ),
      ledgers.map(toLedgerScan),
    )
    const mismatches = compareMergeQueueReplays(left, altered)
    expect(mismatches.length).toBe(1)
    expect(mismatches[0]?.queue).toBe(`${TENANT}/queue_main`)
  })
})

describe("verify report (pure)", () => {
  test("clean report on matching inputs; drift flips it", () => {
    const rows = [leaseRow(1), leaseRow(2)]
    const clean = buildForgeDomainVerifyReport({
      d1Newest: d1ForgeDomainNewestHashes("forge_dispatch_leases", rows),
      d1Total: 2,
      postgresNewest: d1ForgeDomainNewestHashes("forge_dispatch_leases", rows),
      postgresTotal: 2,
      scalars: [{ d1: 2, metric: "active_leases", postgres: 2 }],
      table: "forge_dispatch_leases",
    })
    expect(forgeDomainVerifyReportClean(clean)).toBe(true)

    const drifted = buildForgeDomainVerifyReport({
      d1Newest: d1ForgeDomainNewestHashes("forge_dispatch_leases", rows),
      d1Total: 2,
      postgresNewest: d1ForgeDomainNewestHashes("forge_dispatch_leases", [
        leaseRow(1, { state: "expired" }),
        leaseRow(2),
      ]),
      postgresTotal: 2,
      scalars: [{ d1: 2, metric: "active_leases", postgres: 1 }],
      table: "forge_dispatch_leases",
    })
    expect(forgeDomainVerifyReportClean(drifted)).toBe(false)
    expect(drifted.scalarMismatches.length).toBe(1)
    expect(drifted.newestHashMismatches.length).toBe(1)
  })
})

// ---------------------------------------------------------------------------
// Postgres integration (skipped without local server binaries)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("forge backfill — Postgres", () => {
  let pg: LocalPostgres
  let rawSql: SQL
  let sql: SyncSql

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_forge_backfill")
    await admin.end()
    const url = pg.urlFor("khala_forge_backfill")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0021_forge_domain.sql")
    rawSql = new SQL({ url, max: 4 })
    sql = rawSql as unknown as SyncSql
  })

  afterAll(async () => {
    await rawSql?.end()
    await pg?.stop()
  })

  test("converge upsert is idempotent over composite PKs and converges to the latest D1 snapshot", async () => {
    const first = [refRow("refs/heads/main"), refRow("refs/heads/dev")]
    expect(await upsertForgeDomainRows(sql, "forge_git_refs", first)).toBe(2)
    // Re-running the SAME page converges without duplication.
    expect(await upsertForgeDomainRows(sql, "forge_git_refs", first)).toBe(2)
    expect(await postgresForgeDomainRowCount(sql, "forge_git_refs")).toBe(2)

    // A later D1 snapshot (fast-forward) converges the ref tip forward.
    await upsertForgeDomainRows(sql, "forge_git_refs", [
      refRow("refs/heads/main", {
        object_id: "f".repeat(40),
        previous_object_id: "a".repeat(40),
        updated_at: T1,
      }),
    ])
    const rows = await (
      sql as unknown as {
        unsafe: (q: string, p: Array<unknown>) => Promise<Array<Record<string, unknown>>>
      }
    ).unsafe(
      `SELECT object_id, previous_object_id FROM forge_git_refs WHERE tenant_ref = $1 AND repository_ref = $2 AND ref_name = $3`,
      [TENANT, REPO, "refs/heads/main"],
    )
    expect(rows[0]?.["object_id"]).toBe("f".repeat(40))
    expect(rows[0]?.["previous_object_id"]).toBe("a".repeat(40))
  })

  test("scalar tallies, ref-set digests, replay digests, and newest hashes agree with the D1-side helpers", async () => {
    await upsertForgeDomainRows(sql, "forge_dispatch_leases", [
      leaseRow(1),
      leaseRow(2, { released_at: T1, state: "expired" }),
    ])
    await upsertForgeDomainRows(sql, "forge_promotion_decisions", [
      decisionRow(1),
      decisionRow(2, { decided_at: T1 }),
    ])
    await upsertForgeDomainRows(sql, "forge_merge_queue_ledger", [ledgerRow()])
    await upsertForgeDomainRows(sql, "forge_git_access_tokens", [
      tokenRow(1),
      tokenRow(2, { revoked_at: T1, state: "revoked" }),
    ])

    // Scalars run the SAME SQL text on both engines.
    expect(
      await postgresForgeDomainScalar(
        sql,
        FORGE_DOMAIN_SCALAR_TALLIES.forge_dispatch_leases[0]!.sql,
      ),
    ).toBe(1)
    expect(
      await postgresForgeDomainScalar(
        sql,
        FORGE_DOMAIN_SCALAR_TALLIES.forge_git_access_tokens[0]!.sql,
      ),
    ).toBe(1)
    expect(
      await postgresForgeDomainScalar(
        sql,
        FORGE_DOMAIN_SCALAR_TALLIES.forge_promotion_decisions[2]!.sql,
      ),
    ).toBe(3)

    // Ref-set digest: the Postgres tally equals the pure tally over the
    // same source rows.
    const pureTally = refSetTallyFromRows([
      {
        ref_name: "refs/heads/dev",
        repository_ref: REPO,
        state: "active",
        tenant_ref: TENANT,
        tip: "a".repeat(40),
      },
      {
        ref_name: "refs/heads/main",
        repository_ref: REPO,
        state: "active",
        tenant_ref: TENANT,
        tip: "f".repeat(40),
      },
    ])
    const pgTally = await postgresRefSetTally(sql)
    expect(compareRefSets(pureTally, pgTally)).toEqual([])

    // Replay digest round-trips through Postgres.
    const pgReplay = await postgresMergeQueueReplay(sql)
    expect(pgReplay.get(`${TENANT}/queue_main`)?.decisions).toBe(2)

    // Newest-N hashes: the Postgres rows hash identically to the D1
    // source rows (bigint canonicalization included), and keys carry no
    // custody values.
    const newest = await postgresForgeDomainNewestHashes(
      sql,
      "forge_git_access_tokens",
      10,
    )
    const d1Newest = d1ForgeDomainNewestHashes("forge_git_access_tokens", [
      tokenRow(2, { revoked_at: T1, state: "revoked" }),
      tokenRow(1),
    ])
    expect(newest).toEqual(d1Newest)
    for (const entry of newest) {
      expect(entry.key).not.toContain("e3e3")
      expect(entry.key).not.toContain("oa_forge_git_")
    }
  })

  test("all sixteen twins accept a full-row converge (schema/registry alignment)", async () => {
    const now = T0
    const sample: Record<string, D1ForgeSourceRow> = {
      forge_coordination_issues: {
        created_at: now,
        git_token_refs_json: "[]",
        github_issue_number: 12,
        issue_ref: "issue_1",
        priority_ref: null,
        source_refs_json: "[]",
        state: "open",
        tenant_ref: TENANT,
        title: "port the forge domain",
        updated_at: now,
      },
      forge_coordination_prs: {
        base_head: "b".repeat(40),
        blocker_refs_json: "[]",
        change_ref: "change_1",
        created_at: now,
        issue_ref: "issue_1",
        patch_head: "c".repeat(40),
        pr_ref: "pr_1",
        source_refs_json: "[]",
        state: "open",
        tenant_ref: TENANT,
        updated_at: now,
        verification_ref: null,
      },
      forge_coordination_status: {
        actor_ref: "agent_raynor",
        created_at: now,
        nip34_kind: 1630,
        source_refs_json: "[]",
        state: "open",
        status_ref: "status_1",
        subject_ref: "issue_1",
        tenant_ref: TENANT,
      },
      forge_git_access_token_scopes: {
        created_at: now,
        scope: "git:receive-pack",
        tenant_ref: TENANT,
        token_ref: "token_1",
      },
      forge_git_objects: {
        first_seen_at: now,
        latest_seen_at: now,
        object_format: "sha1",
        object_id: "9".repeat(40),
        packfile_ref: "pack_1",
        packfile_sha256: "0".repeat(64),
        repository_ref: REPO,
        source_refs_json: "[]",
        tenant_ref: TENANT,
      },
      forge_git_packfile_archives: {
        artifact_r2_key: `forge/git/${TENANT}/${REPO}/pack_1.pack`,
        capabilities_json: "[]",
        change_ref: null,
        command_count: 1,
        content_type: "application/x-git-packed-objects",
        created_at: now,
        object_format: "sha1",
        packfile_bytes: 1024,
        packfile_ref: "pack_1",
        packfile_sha256: "0".repeat(64),
        receive_pack_ref: "rp_1",
        ref_updates_json: "[]",
        repository_ref: REPO,
        source_refs_json: "[]",
        tenant_ref: TENANT,
        updated_at: now,
        visibility: "operator_only",
      },
      forge_git_receive_pack_intakes: {
        change_ref: null,
        command_count: 1,
        created_at: now,
        object_format: "sha1",
        packfile_bytes: 1024,
        packfile_ref: "pack_1",
        packfile_sha256: "0".repeat(64),
        receive_pack_ref: "rp_1",
        ref_updates_json: "[]",
        rejection_code: null,
        rejection_reason: null,
        repository_ref: REPO,
        source_refs_json: "[]",
        state: "accepted",
        subject_ref: "agent_raynor",
        tenant_ref: TENANT,
        token_ref: "token_1",
        updated_at: now,
      },
      forge_git_ref_locks: {
        acquired_at: now,
        action: "update",
        expected_old_object_id: "a".repeat(40),
        lock_ref: "rp_1.refs_heads_main",
        new_object_id: "f".repeat(40),
        receive_pack_ref: "rp_1",
        ref_name: "refs/heads/main",
        released_at: now,
        repository_ref: REPO,
        source_refs_json: "[]",
        state: "applied",
        tenant_ref: TENANT,
      },
      forge_github_mirror_receipts: {
        attempt_count: 1,
        change_ref: "change_1",
        commit_id: "d".repeat(40),
        completed_at: now,
        created_at: now,
        destination_github_ref: "refs/heads/main",
        destination_github_repository: "OpenAgentsInc/openagents",
        error_reason: null,
        first_attempted_at: now,
        last_attempted_at: now,
        mirror_ref: "mirror_1",
        promotion_ref: "promo_1",
        redacted: 1,
        refusal_reason: null,
        repository_ref: REPO,
        source_canonical_ref: "refs/heads/main",
        source_refs_json: "[]",
        status: "mirrored",
        tenant_ref: TENANT,
        updated_at: now,
      },
      forge_tenants: {
        attestation_ref: null,
        confidential_workspace_mode: null,
        created_at: now,
        display_name: "Alpha",
        encrypted_knowledge_pack_ref: null,
        refusal_reason: null,
        retention_policy_ref: null,
        state: "active",
        tenant_ref: TENANT,
        updated_at: now,
      },
      forge_verification_receipts: {
        artifact_refs_json: "[]",
        base_head: "b".repeat(40),
        base_ref: "refs/heads/main",
        change_ref: "change_1",
        command_args_json: "[]",
        command_ref: "verify.suite",
        completed_at: now,
        created_at: now,
        executor_identity_ref: "executor_1",
        exit_code: 0,
        head_head: "c".repeat(40),
        head_ref: "refs/forge/change_1",
        log_sha256: "1".repeat(64),
        packfile_ref: "pack_1",
        packfile_sha256: "0".repeat(64),
        redacted: 1,
        repository_ref: REPO,
        source_refs_json: "[]",
        started_at: now,
        tenant_ref: TENANT,
        verdict: "passed",
        verification_ref: "verify_1",
      },
    }
    for (const [table, row] of Object.entries(sample)) {
      expect(
        await upsertForgeDomainRows(
          sql,
          table as (typeof FORGE_DOMAIN_TABLES)[number],
          [row],
        ),
      ).toBe(1)
    }
    // The remaining five tables were exercised by the earlier tests.
    for (const table of FORGE_DOMAIN_TABLES) {
      expect(
        await postgresForgeDomainRowCount(sql, table),
      ).toBeGreaterThanOrEqual(1)
    }
  })
})
