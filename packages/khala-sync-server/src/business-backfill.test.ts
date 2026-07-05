// KS-8.14 (#8325): business funnel / orders / referrals backfill core —
// idempotency + acceptance-reconciliation fidelity.
//
// Load-bearing properties:
//   - running the same page twice yields an IDENTICAL Postgres state
//     (converge is byte-stable; the backfill idempotency contract);
//   - a converge sweep brings a stale mirror row forward to the D1
//     snapshot (pipeline stage / attribution policy_state updates
//     re-converge);
//   - ATTRIBUTION SET EQUALITY: the payout-feeding tuple digest catches
//     set inequality even at equal counts (a swapped referral_source_id is
//     a mismatch);
//   - PROMISE-RECEIPT HASH EQUALITY: the full-row set digest over
//     promise_transition_receipts catches a single changed check payload;
//   - FUNNEL COUNTS PER COHORT: per-(stage, source_kind) and
//     per-(source_ref, stage) tallies catch drift exactly;
//   - row hashes normalize identically for a D1 export row and its
//     Postgres twin (extra export columns like d1_rowid are ignored).

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
  BUSINESS_GROUPED_TALLIES,
  BUSINESS_SET_DIGEST_COLUMNS,
  businessNewestHashesFromRows,
  businessRowHash,
  businessSetDigestKeyFromRow,
  compareGroupedTallies,
  compareNewestHashes,
  keySetDigestFromKeys,
  postgresBusinessNewestHashes,
  postgresBusinessRowCount,
  postgresBusinessSetDigest,
  postgresGroupedTally,
  upsertBusinessRows,
  type D1SourceRow,
  type GroupedTallySpec,
} from "./business-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const funnelEventRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  event_ref: `business.funnel.backfill.${n}`,
  id: `funnel_backfill_${n}`,
  observed_at: `2026-07-01T0${n}:00:01.000Z`,
  occurred_at: `2026-07-01T0${n}:00:00.000Z`,
  source_kind: n % 2 === 0 ? "referral" : "direct",
  source_ref: n % 2 === 0 ? "affiliate_alpha" : null,
  stage: n % 2 === 0 ? "signup" : "visit",
  ...overrides,
})

const userAttributionRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  archived_at: null,
  capture_path: "human",
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  first_verified_at: `2026-07-01T0${n}:00:00.000Z`,
  policy_state: "active",
  referral_attribution_id: `attr_${n}`,
  referral_invite_id: null,
  referral_source_id: `source_${n}`,
  target: "home",
  updated_at: `2026-07-01T0${n}:00:00.000Z`,
  user_id: `user-${n}`,
  ...overrides,
})

const promiseReceiptRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  checked_at: `2026-07-01T0${n}:00:00.000Z`,
  checks_json: `[{"check":"c${n}","ok":true}]`,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  evidence_refs_json: "[]",
  exception_json: null,
  from_state: "yellow",
  id: `ptr_backfill_${n}`,
  promise_id: `promise.${n % 2}`,
  registry_version: "v1",
  result: "pass",
  to_state: "green",
  ...overrides,
})

const pipelineRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  blocker_ref: null,
  business_signup_request_id: null,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  next_action_due_at: null,
  owner_role: "operator",
  partner_approval_receipt_ref: null,
  partner_budget_range_ref: null,
  partner_due_window_ref: null,
  partner_offer_ref: null,
  partner_peer_ref: null,
  partner_privacy_tier_ref: null,
  partner_route_flag: 0,
  partner_route_state: "none",
  partner_route_updated_at: null,
  partner_scope_summary_ref: null,
  pipeline_ref: `pipeline_backfill_${n}`,
  quoted_band_label: "unquoted",
  quoted_max_usd_cents: 250_000 * n,
  quoted_min_usd_cents: 100_000 * n,
  receipt_refs_json: "[]",
  source_ref: "direct",
  stage: "intake_received",
  stage_updated_at: `2026-07-01T0${n}:00:00.000Z`,
  updated_at: `2026-07-01T0${n}:00:00.000Z`,
  vertical: "vertical.test",
  ...overrides,
})

const funnelStageSpec = (BUSINESS_GROUPED_TALLIES["business_funnel_events"] ??
  [])[0] as GroupedTallySpec

describe("business backfill pure comparators", () => {
  test("attribution set digest: equal counts but a swapped source id differ", () => {
    const columns = BUSINESS_SET_DIGEST_COLUMNS[
      "user_referral_attributions"
    ] as ReadonlyArray<string>
    const left = keySetDigestFromKeys(
      [userAttributionRow(1), userAttributionRow(2)].map((row) =>
        businessSetDigestKeyFromRow(columns, row),
      ),
    )
    const right = keySetDigestFromKeys(
      [
        userAttributionRow(1),
        userAttributionRow(2, { referral_source_id: "source_SWAPPED" }),
      ].map((row) => businessSetDigestKeyFromRow(columns, row)),
    )
    expect(left.count).toBe(right.count)
    expect(left.digest).not.toBe(right.digest)
    // order-insensitive
    const reordered = keySetDigestFromKeys(
      [userAttributionRow(2), userAttributionRow(1)].map((row) =>
        businessSetDigestKeyFromRow(columns, row),
      ),
    )
    expect(reordered.digest).toBe(left.digest)
  })

  test("promise-receipt full-row digest: one changed check payload is a mismatch", () => {
    const columns = BUSINESS_SET_DIGEST_COLUMNS[
      "promise_transition_receipts"
    ] as ReadonlyArray<string>
    const left = keySetDigestFromKeys(
      [promiseReceiptRow(1)].map((row) =>
        businessSetDigestKeyFromRow(columns, row),
      ),
    )
    const right = keySetDigestFromKeys(
      [promiseReceiptRow(1, { checks_json: '[{"check":"c1","ok":false}]' })].map(
        (row) => businessSetDigestKeyFromRow(columns, row),
      ),
    )
    expect(left.digest).not.toBe(right.digest)
  })

  test("funnel cohort tally: a single missing stage receipt is a mismatch", () => {
    const d1 = { "signup:referral": { rows: 3, sums: [] } }
    const postgres = { "signup:referral": { rows: 2, sums: [] } }
    expect(compareGroupedTallies(d1, postgres)).toHaveLength(1)
    expect(compareGroupedTallies(d1, d1)).toHaveLength(0)
  })

  test("row hash ignores extra D1 export columns (d1_rowid)", () => {
    const row = funnelEventRow(1)
    expect(
      businessRowHash("business_funnel_events", { ...row, d1_rowid: 42 }),
    ).toBe(businessRowHash("business_funnel_events", row))
  })

  test("newest-hash compare flags a byte-level cents difference", () => {
    const row = pipelineRow(1)
    const d1Newest = businessNewestHashesFromRows("business_pipeline_rows", [
      row,
    ])
    const pgNewest = businessNewestHashesFromRows("business_pipeline_rows", [
      { ...row, quoted_min_usd_cents: 100_001 },
    ])
    expect(compareNewestHashes(d1Newest, pgNewest)).toHaveLength(1)
    expect(compareNewestHashes(d1Newest, d1Newest)).toHaveLength(0)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "business backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_business_backfill")
      await admin.end()
      const url = pg.urlFor("khala_business_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0023_business_funnel.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("funnel events: run twice → identical state (idempotency) + cohort tallies", async () => {
      const page = [funnelEventRow(1), funnelEventRow(2), funnelEventRow(3)]
      await upsertBusinessRows(sql, "business_funnel_events", page)
      const hashesAfterFirst = await postgresBusinessNewestHashes(
        sql,
        "business_funnel_events",
        10,
      )
      await upsertBusinessRows(sql, "business_funnel_events", page)
      const hashesAfterSecond = await postgresBusinessNewestHashes(
        sql,
        "business_funnel_events",
        10,
      )
      expect(hashesAfterSecond).toEqual(hashesAfterFirst)
      expect(
        await postgresBusinessRowCount(sql, "business_funnel_events"),
      ).toBe(3)

      const tally = await postgresGroupedTally(
        sql,
        "business_funnel_events",
        funnelStageSpec,
      )
      expect(tally["signup:referral"]?.rows).toBe(1)
      expect(tally["visit:direct"]?.rows).toBe(2)
    })

    test("attribution set digest reconciles exactly against the source rows", async () => {
      const rows = [userAttributionRow(1), userAttributionRow(2)]
      await upsertBusinessRows(sql, "user_referral_attributions", rows)
      const columns = BUSINESS_SET_DIGEST_COLUMNS[
        "user_referral_attributions"
      ] as ReadonlyArray<string>
      const d1Digest = keySetDigestFromKeys(
        rows.map((row) => businessSetDigestKeyFromRow(columns, row)),
      )
      const postgresDigest = await postgresBusinessSetDigest(
        sql,
        "user_referral_attributions",
        columns,
      )
      expect(postgresDigest).toEqual(d1Digest)

      // A policy_state transition converges and CHANGES the digest — the
      // catch-up sweep + re-verify keeps the sets honest.
      await upsertBusinessRows(sql, "user_referral_attributions", [
        userAttributionRow(2, {
          policy_state: "disputed",
          updated_at: "2026-07-01T09:00:00.000Z",
        }),
      ])
      const digestAfterTransition = await postgresBusinessSetDigest(
        sql,
        "user_referral_attributions",
        columns,
      )
      expect(digestAfterTransition.count).toBe(d1Digest.count)
      expect(digestAfterTransition.digest).not.toBe(d1Digest.digest)
    })

    test("promise receipts: full-row set digest matches the D1 snapshot exactly", async () => {
      const rows = [promiseReceiptRow(1), promiseReceiptRow(2)]
      await upsertBusinessRows(sql, "promise_transition_receipts", rows)
      const columns = BUSINESS_SET_DIGEST_COLUMNS[
        "promise_transition_receipts"
      ] as ReadonlyArray<string>
      const d1Digest = keySetDigestFromKeys(
        rows.map((row) => businessSetDigestKeyFromRow(columns, row)),
      )
      expect(
        await postgresBusinessSetDigest(
          sql,
          "promise_transition_receipts",
          columns,
        ),
      ).toEqual(d1Digest)
    })

    test("pipeline rows: converge brings a stale stage forward; money tally exact", async () => {
      await upsertBusinessRows(sql, "business_pipeline_rows", [
        pipelineRow(1),
        pipelineRow(2),
      ])
      // D1 later moved pipeline 1 to closed_won — the catch-up sweep
      // converges the mirror forward.
      await upsertBusinessRows(sql, "business_pipeline_rows", [
        pipelineRow(1, {
          stage: "closed_won",
          stage_updated_at: "2026-07-01T09:00:00.000Z",
          updated_at: "2026-07-01T09:00:00.000Z",
        }),
      ])
      const spec = (BUSINESS_GROUPED_TALLIES["business_pipeline_rows"] ??
        [])[0] as GroupedTallySpec
      const tally = await postgresGroupedTally(
        sql,
        "business_pipeline_rows",
        spec,
      )
      expect(tally["closed_won"]).toEqual({
        rows: 1,
        sums: [100_000, 250_000],
      })
      expect(tally["intake_received"]).toEqual({
        rows: 1,
        sums: [200_000, 500_000],
      })

      // hash parity for the converged snapshot
      const d1Newest = businessNewestHashesFromRows("business_pipeline_rows", [
        pipelineRow(1, {
          stage: "closed_won",
          stage_updated_at: "2026-07-01T09:00:00.000Z",
          updated_at: "2026-07-01T09:00:00.000Z",
        }),
        pipelineRow(2),
      ])
      const pgNewest = await postgresBusinessNewestHashes(
        sql,
        "business_pipeline_rows",
        10,
      )
      expect(compareNewestHashes(d1Newest, pgNewest)).toHaveLength(0)
    })
  },
)
