// KS-8.12 (#8323): sites content backfill core — idempotency + verify
// fidelity.
//
// Load-bearing properties: converge upserts are IDEMPOTENT (a re-run with
// the same D1 page converges to the identical Postgres state) and
// converge to the LATEST D1 snapshot (deployment status flips, project
// archival), the per-site version-chain comparator catches
// missing/extra/reordered chains exactly, the deployment state-machine
// census comparator catches any per-(site,status) count drift, the
// builder sequence chains behave the same over messages/events/phase
// runs, and the row hash canonicalizes D1 numbers and postgres.js bigint
// strings to the same digest. Privacy: no assertion prints prompts,
// message bodies, or preview text — hashes and keys only, same as the
// CLI.

import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { runMigrations } from "./migrate.js"
import {
  buildSitesContentVerifyReport,
  compareDeploymentStates,
  compareGroupChains,
  deploymentStateRowsFromRaw,
  deploymentStateSql,
  groupChainRowsFromRaw,
  groupChainSql,
  postgresDeploymentStates,
  postgresGroupChains,
  postgresSitesContentNewestHashes,
  postgresSitesContentRowCount,
  postgresSitesContentScalar,
  SITES_CONTENT_CHAINS,
  SITES_CONTENT_SCALAR_TALLIES,
  sitesContentRowHash,
  sitesContentVerifyReportClean,
  upsertSitesContentRows,
  d1SitesContentNewestHashes,
  type D1SourceRow,
} from "./sites-content-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Fixtures (snake_case rows exactly as `wrangler d1 execute --json` returns)
// ---------------------------------------------------------------------------

const T0 = "2026-07-04T00:00:00.000Z"
const T1 = "2026-07-04T01:00:00.000Z"

const projectRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  access_mode: "customer_owner",
  active_deployment_id: null,
  active_version_id: null,
  archived_at: null,
  created_at: T0,
  id: `site_${n}`,
  owner_user_id: "user_owner",
  project_id: null,
  prompt: `prompt for site ${n}`,
  slug: `site-${n}`,
  software_order_id: null,
  source_repository_name: null,
  source_repository_owner: null,
  source_repository_provider: null,
  source_repository_ref: null,
  status: "approved",
  team_id: null,
  title: `Site ${n}`,
  updated_at: T0,
  visibility: "private",
  ...overrides,
})

const versionRow = (
  siteN: number,
  versionN: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  artifact_manifest_r2_key: null,
  build_command: null,
  build_log_r2_key: null,
  build_status: "saved",
  created_at: `2026-07-04T00:0${versionN}:00.000Z`,
  created_by_run_id: null,
  created_by_user_id: "user_owner",
  d1_binding_name: null,
  id: `version_${siteN}_${versionN}`,
  metadata_json: "{}",
  r2_binding_name: null,
  rejected_at: null,
  saved_at: T0,
  site_id: `site_${siteN}`,
  source_archive_r2_key: null,
  source_commit_sha: null,
  source_kind: "autopilot_generated",
  static_assets_manifest_json: '{"assets":[]}',
  worker_module_r2_key: null,
  ...overrides,
})

const deploymentRow = (
  siteN: number,
  deployN: number,
  status: string,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  activated_at: status === "active" ? T0 : null,
  created_at: T0,
  deployed_by_user_id: "user_owner",
  disabled_at: status === "disabled" ? T0 : null,
  dispatch_namespace: null,
  external_deployment_id: null,
  failed_at: null,
  id: `deployment_${siteN}_${deployN}`,
  rolled_back_at: status === "rolled_back" ? T0 : null,
  runtime_kind: "workers_for_platforms",
  runtime_script_name: null,
  site_id: `site_${siteN}`,
  slug: `site-${siteN}`,
  started_at: T0,
  status,
  updated_at: T0,
  url: `https://site-${siteN}.openagents.com`,
  version_id: `version_${siteN}_1`,
  ...overrides,
})

const messageRow = (
  sessionN: number,
  sequence: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  actor_kind: "agent",
  archived_at: null,
  body: `message ${sequence} in session ${sessionN}`,
  created_at: T0,
  id: `message_${sessionN}_${sequence}`,
  idempotency_key: `message-key-${sessionN}-${sequence}`,
  metadata_json: "{}",
  sequence,
  session_id: `session_${sessionN}`,
  visibility: "customer",
  ...overrides,
})

const sessionRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  active_artifact_id: null,
  active_preview_id: null,
  archived_at: null,
  created_at: T0,
  created_by_actor_ref: "agent_raynor",
  customer_user_id: null,
  id: `session_${n}`,
  idempotency_key: `session-key-${n}`,
  metadata_json: "{}",
  order_id: null,
  owner_user_id: "user_owner",
  prompt_summary: `session ${n} summary`,
  site_id: `site_${n}`,
  source_revision_id: null,
  source_site_version_id: null,
  status: "building",
  updated_at: T0,
  workroom_id: null,
  ...overrides,
})

// ---------------------------------------------------------------------------
// Pure comparators
// ---------------------------------------------------------------------------

describe("sitesContentRowHash (pure)", () => {
  test("identical rows hash identically; any column drift changes the hash", () => {
    const base = projectRow(1)
    expect(sitesContentRowHash("site_projects", base)).toBe(
      sitesContentRowHash("site_projects", { ...base }),
    )
    expect(sitesContentRowHash("site_projects", base)).not.toBe(
      sitesContentRowHash("site_projects", { ...base, status: "disabled" }),
    )
  })

  test("D1 numbers and postgres.js bigint strings canonicalize equal", () => {
    const d1Side = messageRow(1, 7)
    const pgSide = messageRow(1, 7, { sequence: "7" })
    expect(sitesContentRowHash("site_builder_messages", d1Side)).toBe(
      sitesContentRowHash("site_builder_messages", pgSide),
    )
  })

  test("NULL and empty string hash differently", () => {
    const withNull = projectRow(2, { software_order_id: null })
    const withEmpty = projectRow(2, { software_order_id: "" })
    expect(sitesContentRowHash("site_projects", withNull)).not.toBe(
      sitesContentRowHash("site_projects", withEmpty),
    )
  })

  test("output is a hash, never customer content", () => {
    const hash = sitesContentRowHash("site_projects", projectRow(3))
    expect(hash).toMatch(/^[0-9a-f]{64}$/)
    expect(hash).not.toContain("prompt for site")
  })
})

describe("group chain comparison (pure)", () => {
  const chains = (
    entries: ReadonlyArray<[string, number, number, string, string]>,
  ) =>
    groupChainRowsFromRaw(
      entries.map(([groupKey, members, distinct, min, max]) => ({
        distinct_orders: distinct,
        group_key: groupKey,
        max_order: max,
        members,
        min_order: min,
      })),
    )

  test("equal chains produce no mismatches", () => {
    const tally = chains([
      ["site_1", 3, 3, T0, T1],
      ["site_2", 1, 1, T0, T0],
    ])
    expect(compareGroupChains(tally, tally)).toEqual([])
  })

  test("missing group, short chain, and reordered bounds are all caught", () => {
    const d1 = chains([
      ["site_1", 3, 3, T0, T1],
      ["site_2", 1, 1, T0, T0],
      ["site_3", 2, 2, T0, T1],
    ])
    const pg = chains([
      ["site_1", 2, 2, T0, T1], // short chain
      ["site_2", 1, 1, T1, T1], // shifted bounds
      // site_3 missing entirely
      ["site_4", 1, 1, T0, T0], // extra on pg
    ])
    const mismatches = compareGroupChains(d1, pg)
    expect(mismatches.map((m) => m.groupKey).sort()).toEqual([
      "site_1",
      "site_2",
      "site_3",
      "site_4",
    ])
  })

  test("the chain SQL groups by the registered chain columns", () => {
    expect(groupChainSql("site_versions", "site_id", "created_at")).toContain(
      "GROUP BY site_id",
    )
    expect(
      SITES_CONTENT_CHAINS.map((chain) => chain.name).sort(),
    ).toEqual([
      "event_chain_per_session",
      "message_chain_per_session",
      "phase_run_chain_per_session",
      "version_chain_per_site",
    ])
  })
})

describe("deployment state census comparison (pure)", () => {
  const states = (
    entries: ReadonlyArray<[string, string, number]>,
  ) =>
    deploymentStateRowsFromRaw(
      entries.map(([siteId, status, deployments]) => ({
        deployments,
        site_id: siteId,
        status,
      })),
    )

  test("equal censuses produce no mismatches", () => {
    const tally = states([
      ["site_1", "active", 1],
      ["site_1", "rolled_back", 2],
    ])
    expect(compareDeploymentStates(tally, tally)).toEqual([])
  })

  test("count drift, missing cells, and extra cells are all caught", () => {
    const d1 = states([
      ["site_1", "active", 1],
      ["site_1", "rolled_back", 2],
      ["site_2", "disabled", 1],
    ])
    const pg = states([
      ["site_1", "active", 2], // drifted count
      ["site_1", "rolled_back", 2],
      // site_2/disabled missing
      ["site_3", "queued", 1], // extra on pg
    ])
    const mismatches = compareDeploymentStates(d1, pg)
    expect(
      mismatches.map((m) => `${m.siteId}:${m.status}`).sort(),
    ).toEqual(["site_1:active", "site_2:disabled", "site_3:queued"])
  })
})

describe("verify report (pure)", () => {
  test("clean report on matching inputs; drift flips it", () => {
    const clean = buildSitesContentVerifyReport({
      d1Newest: [{ hash: "abc", key: "site_1" }],
      d1Total: 1,
      postgresNewest: [{ hash: "abc", key: "site_1" }],
      postgresTotal: 1,
      scalars: [{ d1: 5, metric: "active_projects", postgres: 5 }],
      table: "site_projects",
    })
    expect(sitesContentVerifyReportClean(clean)).toBe(true)

    const drifted = buildSitesContentVerifyReport({
      d1Newest: [{ hash: "abc", key: "site_1" }],
      d1States: deploymentStateRowsFromRaw([
        { deployments: 1, site_id: "site_1", status: "active" },
      ]),
      d1Total: 2,
      postgresNewest: [{ hash: "def", key: "site_1" }],
      postgresStates: deploymentStateRowsFromRaw([
        { deployments: 2, site_id: "site_1", status: "active" },
      ]),
      postgresTotal: 1,
      scalars: [{ d1: 5, metric: "active_projects", postgres: 4 }],
      table: "site_deployments",
    })
    expect(sitesContentVerifyReportClean(drifted)).toBe(false)
    expect(drifted.countsMatch).toBe(false)
    expect(drifted.scalarMismatches).toHaveLength(1)
    expect(drifted.stateMismatches).toHaveLength(1)
    expect(drifted.newestHashMismatches).toHaveLength(1)
  })
})

// ---------------------------------------------------------------------------
// Postgres integration (skipped without local Postgres binaries)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "sites content backfill — Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sites_content_backfill")
      await admin.end()
      const url = pg.urlFor("khala_sites_content_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0020_sites_core.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("converge upsert is idempotent and converges to the latest D1 snapshot", async () => {
      const first = [projectRow(1), projectRow(2)]
      expect(
        await upsertSitesContentRows(sql, "site_projects", first),
      ).toBe(2)
      // Re-running the SAME page converges without duplication.
      expect(
        await upsertSitesContentRows(sql, "site_projects", first),
      ).toBe(2)
      expect(await postgresSitesContentRowCount(sql, "site_projects")).toBe(2)

      // A later D1 snapshot (archival) converges the row forward.
      await upsertSitesContentRows(sql, "site_projects", [
        projectRow(1, {
          archived_at: T1,
          status: "archived",
          updated_at: T1,
          visibility: "private",
        }),
      ])
      const rows = await (
        sql as unknown as {
          unsafe: (
            q: string,
            p: Array<unknown>,
          ) => Promise<Array<Record<string, unknown>>>
        }
      ).unsafe(
        `SELECT status, archived_at FROM site_projects WHERE id = $1`,
        ["site_1"],
      )
      expect(rows[0]?.["status"]).toBe("archived")
      expect(rows[0]?.["archived_at"]).toBe(T1)
    })

    test("scalar tallies, version chains, deployment states, and newest hashes agree with the D1-side helpers", async () => {
      const versions = [
        versionRow(1, 1),
        versionRow(1, 2),
        versionRow(1, 3, { build_status: "build_failed", saved_at: null }),
        versionRow(2, 1),
      ]
      await upsertSitesContentRows(sql, "site_versions", versions)
      await upsertSitesContentRows(sql, "site_deployments", [
        deploymentRow(1, 1, "rolled_back"),
        deploymentRow(1, 2, "active"),
        deploymentRow(2, 1, "disabled"),
      ])

      // Scalar tallies (the exact SQL runs on both stores).
      const savedTally = SITES_CONTENT_SCALAR_TALLIES.site_versions.find(
        (tally) => tally.metric === "saved_versions",
      )
      expect(savedTally).toBeDefined()
      expect(await postgresSitesContentScalar(sql, savedTally!.sql)).toBe(3)
      const activeTally = SITES_CONTENT_SCALAR_TALLIES.site_deployments.find(
        (tally) => tally.metric === "active_deployments",
      )
      expect(await postgresSitesContentScalar(sql, activeTally!.sql)).toBe(1)

      // Version chains per site: the Postgres tally equals the tally built
      // from the same rows on the D1 side.
      const chain = SITES_CONTENT_CHAINS.find(
        (entry) => entry.name === "version_chain_per_site",
      )!
      const pgChains = await postgresGroupChains(sql, chain.sql)
      const d1Chains = groupChainRowsFromRaw([
        {
          distinct_orders: 3,
          group_key: "site_1",
          max_order: "2026-07-04T00:03:00.000Z",
          members: 3,
          min_order: "2026-07-04T00:01:00.000Z",
        },
        {
          distinct_orders: 1,
          group_key: "site_2",
          max_order: "2026-07-04T00:01:00.000Z",
          members: 1,
          min_order: "2026-07-04T00:01:00.000Z",
        },
      ])
      expect(compareGroupChains(d1Chains, pgChains)).toEqual([])

      // Deployment state census equality.
      const pgStates = await postgresDeploymentStates(sql)
      const d1States = deploymentStateRowsFromRaw([
        { deployments: 1, site_id: "site_1", status: "active" },
        { deployments: 1, site_id: "site_1", status: "rolled_back" },
        { deployments: 1, site_id: "site_2", status: "disabled" },
      ])
      expect(compareDeploymentStates(d1States, pgStates)).toEqual([])
      expect(deploymentStateSql()).toContain("GROUP BY site_id, status")

      // Newest hashes: hashing the same source rows on both sides matches.
      const pgNewest = await postgresSitesContentNewestHashes(
        sql,
        "site_versions",
        10,
      )
      const d1Newest = d1SitesContentNewestHashes(
        "site_versions",
        [...versions].sort(
          (a, b) =>
            String(b["created_at"]).localeCompare(String(a["created_at"])) ||
            String(b["id"]).localeCompare(String(a["id"])),
        ),
      )
      const report = buildSitesContentVerifyReport({
        d1Newest,
        d1Total: 4,
        postgresNewest: pgNewest,
        postgresTotal: await postgresSitesContentRowCount(
          sql,
          "site_versions",
        ),
        scalars: [],
        table: "site_versions",
      })
      expect(report.countsMatch).toBe(true)
      expect(report.newestHashMismatches).toEqual([])
    })

    test("builder sequence chains behave like version chains over sequence", async () => {
      await upsertSitesContentRows(sql, "site_builder_sessions", [
        sessionRow(1),
      ])
      await upsertSitesContentRows(sql, "site_builder_messages", [
        messageRow(1, 1),
        messageRow(1, 2),
        messageRow(1, 3),
      ])
      const chain = SITES_CONTENT_CHAINS.find(
        (entry) => entry.name === "message_chain_per_session",
      )!
      const pgChains = await postgresGroupChains(sql, chain.sql)
      expect(pgChains).toEqual([
        {
          distinctOrders: 3,
          groupKey: "session_1",
          maxOrder: "3",
          members: 3,
          minOrder: "1",
        },
      ])
      // Sequence-sum scalar agrees.
      const sumTally = SITES_CONTENT_SCALAR_TALLIES.site_builder_messages.find(
        (tally) => tally.metric === "sum_sequence",
      )!
      expect(await postgresSitesContentScalar(sql, sumTally.sql)).toBe(6)
    })

    test("dedupe keys port exactly: duplicate idempotency_key rejected on a DIFFERENT id", async () => {
      await upsertSitesContentRows(sql, "site_builder_sessions", [
        sessionRow(2),
      ])
      await expect(
        upsertSitesContentRows(sql, "site_builder_sessions", [
          sessionRow(3, { idempotency_key: "session-key-2" }),
        ]),
      ).rejects.toThrow()
    })

    test("builder natural keys port exactly: duplicate (session, sequence) rejected", async () => {
      await expect(
        upsertSitesContentRows(sql, "site_builder_messages", [
          messageRow(1, 2, {
            id: "message_1_2_dupe",
            idempotency_key: "message-key-1-2-dupe",
          }),
        ]),
      ).rejects.toThrow()
    })
  },
)
