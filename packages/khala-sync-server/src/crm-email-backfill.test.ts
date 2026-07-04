// KS-8.11 (#8322): CRM/email/enrichment backfill core — idempotency,
// mirror-wins semantics, PII discipline, and verify fidelity.
//
// Load-bearing properties: `ON CONFLICT ... DO NOTHING` upserts are
// IDEMPOTENT (a re-run inserts 0) and NEVER overwrite a row the live
// dual-write mirror already owns; the campaign-send dedupe key
// (enrollment × step idempotency_key) ports as a REAL unique constraint
// (the double-email compliance gate); the whole-set digest proves
// suppression-set equality without emitting a single address; and no
// assertion here prints row contents — keys/hashes/counts only, same as
// the CLI.

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
  CRM_EMAIL_BACKFILL_TABLES,
  CRM_EMAIL_TABLE_SPECS,
  compareCrmEmailTallies,
  crmEmailRowHash,
  crmEmailSetDigest,
  d1CrmEmailNewestRowHashes,
  d1CrmEmailSetDigest,
  piiSafeKey,
  postgresCrmEmailNewestRowHashes,
  postgresCrmEmailSetDigest,
  postgresCrmEmailTally,
  upsertCrmEmailRows,
  type D1SourceRow,
} from "./crm-email-backfill.js"
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

const suppressionRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  active: 1,
  archived_at: null,
  created_at: T0,
  email: `person${n}@example.com`,
  id: `sup_${n}`,
  note: null,
  provider_event_id: null,
  reason: "hard_bounce",
  scope: "marketing",
  source_authority_ref: "authority.email.suppression.v1",
  updated_at: T0,
  ...overrides,
})

const campaignSendRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  attempt_count: 0,
  campaign_id: "camp_1",
  claimed_at: null,
  created_at: T0,
  due_at: T0,
  email: `person${n}@example.com`,
  email_message_id: null,
  enrollment_id: `enr_${n}`,
  error_message: null,
  error_name: null,
  failed_at: null,
  id: `send_${n}`,
  idempotency_key: `email_campaign_send:enr_${n}:step_1`,
  metadata_json: "{}",
  next_attempt_at: null,
  provider_event_id: null,
  sent_at: null,
  skipped_at: null,
  source_authority_ref: "authority.email.campaign.v1",
  status: "scheduled",
  step_id: "step_1",
  updated_at: T0,
  user_id: null,
  ...overrides,
})

// ---------------------------------------------------------------------------
// Pure: registry, hashes, PII discipline, comparators
// ---------------------------------------------------------------------------

describe("registry discipline (pure)", () => {
  test("covers all 36 canonical tables; status/order/conflict columns are real columns", () => {
    expect(CRM_EMAIL_BACKFILL_TABLES).toHaveLength(36)
    for (const table of CRM_EMAIL_BACKFILL_TABLES) {
      const spec = CRM_EMAIL_TABLE_SPECS[table]
      expect(spec.columns).toContain(spec.conflictKey)
      expect(spec.columns).toContain(spec.orderColumn)
      expect(spec.columns).toContain(spec.statusColumn)
    }
  })

  test("the compliance-bearing tables carry the whole-set digest flag", () => {
    const fullSet = CRM_EMAIL_BACKFILL_TABLES.filter(
      (table) => CRM_EMAIL_TABLE_SPECS[table].fullSetDigest === true,
    ).sort()
    expect(fullSet).toContain("email_suppression_entries")
    expect(fullSet).toContain("email_preferences")
    expect(fullSet).toContain("business_outreach_suppressions")
  })

  test("no statusColumn is a PII column (tallies group by it in verify output)", () => {
    for (const table of CRM_EMAIL_BACKFILL_TABLES) {
      const status = CRM_EMAIL_TABLE_SPECS[table].statusColumn
      expect(status).not.toMatch(
        /email|note|body|subject|first_name|last_name|full_name|address/,
      )
    }
  })
})

describe("crmEmailRowHash / piiSafeKey (pure)", () => {
  test("identical rows hash identically; any column drift changes the hash", () => {
    const base = suppressionRow(1)
    expect(crmEmailRowHash("email_suppression_entries", base)).toBe(
      crmEmailRowHash("email_suppression_entries", { ...base }),
    )
    expect(crmEmailRowHash("email_suppression_entries", base)).not.toBe(
      crmEmailRowHash("email_suppression_entries", {
        ...base,
        reason: "unsubscribe",
      }),
    )
  })

  test("D1 numbers and postgres.js numeric strings canonicalize equal", () => {
    const d1Side = suppressionRow(2, { active: 1 })
    const pgSide = suppressionRow(2, { active: "1" })
    expect(crmEmailRowHash("email_suppression_entries", d1Side)).toBe(
      crmEmailRowHash("email_suppression_entries", pgSide),
    )
  })

  test("NULL and empty string hash differently", () => {
    expect(
      crmEmailRowHash("email_suppression_entries", suppressionRow(3, { note: null })),
    ).not.toBe(
      crmEmailRowHash("email_suppression_entries", suppressionRow(3, { note: "" })),
    )
  })

  test("piiSafeKey hashes email-shaped keys and passes opaque ids through", () => {
    expect(piiSafeKey("sup_1")).toBe("sup_1")
    const safe = piiSafeKey("person@example.com")
    expect(safe).toMatch(/^sha256:[0-9a-f]{12}$/)
    expect(safe).not.toContain("@")
  })
})

describe("crmEmailSetDigest (pure)", () => {
  test("order-independent set equality; any element drift changes the digest", () => {
    const a = crmEmailRowHash("email_suppression_entries", suppressionRow(1))
    const b = crmEmailRowHash("email_suppression_entries", suppressionRow(2))
    expect(crmEmailSetDigest([a, b])).toBe(crmEmailSetDigest([b, a]))
    const drifted = crmEmailRowHash(
      "email_suppression_entries",
      suppressionRow(2, { reason: "unsubscribe" }),
    )
    expect(crmEmailSetDigest([a, b])).not.toBe(crmEmailSetDigest([a, drifted]))
  })
})

describe("compareCrmEmailTallies (pure)", () => {
  const newest = (rows: ReadonlyArray<D1SourceRow>) =>
    d1CrmEmailNewestRowHashes("email_suppression_entries", rows)

  test("clean report on identical sides", () => {
    const rows = [suppressionRow(1), suppressionRow(2)]
    const tally = { byStatus: { hard_bounce: 2 }, total: 2 }
    const digest = d1CrmEmailSetDigest("email_suppression_entries", rows)
    const report = compareCrmEmailTallies(
      "email_suppression_entries",
      tally,
      tally,
      newest(rows),
      newest(rows),
      { d1: digest.digest, postgres: digest.digest },
    )
    expect(report.countsMatch).toBe(true)
    expect(report.statusMismatches).toEqual([])
    expect(report.newestHashMismatches).toEqual([])
    expect(report.setDigestsMatch).toBe(true)
  })

  test("count, per-status, newest-hash, and set-digest drift are all caught — with PII-safe keys", () => {
    const d1Rows = [suppressionRow(1), suppressionRow(2)]
    const pgRows = [suppressionRow(1), suppressionRow(2, { reason: "unsubscribe" })]
    const report = compareCrmEmailTallies(
      "email_suppression_entries",
      { byStatus: { hard_bounce: 2 }, total: 2 },
      { byStatus: { hard_bounce: 1, unsubscribe: 1 }, total: 2 },
      newest(d1Rows),
      newest(pgRows),
      {
        d1: d1CrmEmailSetDigest("email_suppression_entries", d1Rows).digest,
        postgres: d1CrmEmailSetDigest("email_suppression_entries", pgRows)
          .digest,
      },
    )
    expect(report.countsMatch).toBe(true)
    expect(report.statusMismatches.map((m) => m.status).sort()).toEqual([
      "hard_bounce",
      "unsubscribe",
    ])
    expect(report.newestHashMismatches).toHaveLength(1)
    expect(report.setDigestsMatch).toBe(false)
    // PII discipline: nothing in the report carries an address.
    expect(JSON.stringify(report)).not.toContain("@example.com")
  })
})

// ---------------------------------------------------------------------------
// Postgres integration (skipped without local Postgres binaries)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("crm/email backfill — Postgres", () => {
  let pg: LocalPostgres
  let rawSql: SQL
  let sql: SyncSql

  const unsafe = (text: string, params: Array<unknown> = []) =>
    (
      rawSql as unknown as {
        unsafe: (
          q: string,
          p: Array<unknown>,
        ) => Promise<Array<Record<string, unknown>>>
      }
    ).unsafe(text, params)

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_crm_email_backfill")
    await admin.end()
    const url = pg.urlFor("khala_crm_email_backfill")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0021_crm_email_domain.sql")
    rawSql = new SQL({ url, max: 4 })
    sql = rawSql as unknown as SyncSql
  })

  afterAll(async () => {
    await rawSql?.end()
    await pg?.stop()
  })

  test("upsert is idempotent (re-run inserts 0) and NEVER overwrites a mirror-owned row", async () => {
    const first = [suppressionRow(1), suppressionRow(2)]
    expect(
      await upsertCrmEmailRows(sql, "email_suppression_entries", first),
    ).toBe(2)
    // Re-running the SAME page inserts nothing.
    expect(
      await upsertCrmEmailRows(sql, "email_suppression_entries", first),
    ).toBe(0)

    // A stale snapshot page for an existing key does NOT clobber the row
    // the dual-write mirror already converged (DO NOTHING semantics).
    expect(
      await upsertCrmEmailRows(sql, "email_suppression_entries", [
        suppressionRow(1, { reason: "stale_snapshot_value", updated_at: T1 }),
      ]),
    ).toBe(0)
    const rows = await unsafe(
      "SELECT reason FROM email_suppression_entries WHERE id = $1",
      ["sup_1"],
    )
    expect(rows[0]?.["reason"]).toBe("hard_bounce")
  })

  test("the campaign-send dedupe key ports as a REAL unique constraint (double-email gate)", async () => {
    expect(
      await upsertCrmEmailRows(sql, "email_campaign_sends", [
        campaignSendRow(1),
      ]),
    ).toBe(1)
    // Same enrollment × step under a DIFFERENT primary key must be
    // impossible on the Postgres side too.
    await expect(
      upsertCrmEmailRows(sql, "email_campaign_sends", [
        campaignSendRow(1, { id: "send_1_dup" }),
      ]),
    ).rejects.toThrow()
  })

  test("verify surfaces agree across engines and stay PII-safe", async () => {
    const d1Rows = [suppressionRow(1), suppressionRow(2)]

    const tally = await postgresCrmEmailTally(
      sql,
      "email_suppression_entries",
    )
    expect(tally.total).toBe(2)
    expect(tally.byStatus["hard_bounce"]).toBe(2)

    const pgNewest = await postgresCrmEmailNewestRowHashes(
      sql,
      "email_suppression_entries",
      10,
    )
    const d1Newest = d1CrmEmailNewestRowHashes(
      "email_suppression_entries",
      [...d1Rows].sort((a, b) =>
        String(b["id"]).localeCompare(String(a["id"])),
      ),
    )
    const pgDigest = await postgresCrmEmailSetDigest(
      sql,
      "email_suppression_entries",
    )
    const d1Digest = d1CrmEmailSetDigest("email_suppression_entries", d1Rows)

    const report = compareCrmEmailTallies(
      "email_suppression_entries",
      tally,
      tally,
      d1Newest,
      pgNewest,
      { d1: d1Digest.digest, postgres: pgDigest.digest },
    )
    expect(report.countsMatch).toBe(true)
    expect(report.newestHashMismatches).toEqual([])
    expect(report.setDigestsMatch).toBe(true)
    // The suppression-set equality proof never emits an address.
    expect(JSON.stringify({ pgNewest, report })).not.toContain("@example.com")
  })

  test("every registry table has a real Postgres twin with exactly the registry columns", async () => {
    for (const table of CRM_EMAIL_BACKFILL_TABLES) {
      const rows = await unsafe(
        `SELECT column_name FROM information_schema.columns
          WHERE table_schema = 'public' AND table_name = $1
          ORDER BY ordinal_position`,
        [table],
      )
      expect(
        rows.map((row) => row["column_name"]),
        table,
      ).toEqual([...CRM_EMAIL_TABLE_SPECS[table].columns])
    }
  })
})
