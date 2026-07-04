// KS-8.7 (#8318): billing / Stripe / pay-ins backfill core — idempotency +
// money-reconciliation fidelity.
//
// Load-bearing properties:
//   - running the same page twice yields an IDENTICAL Postgres state
//     (converge is byte-stable; the backfill idempotency contract);
//   - a converge sweep brings a stale mirror row forward to the D1
//     snapshot (webhook status / pay-in state updates re-converge);
//   - the grouped money tallies (per-user balance map, per-(currency,
//     source) cents, per-(type, state) msat) catch drift EXACTLY — a
//     single cent / msat / row difference is a mismatch;
//   - the webhook event-id key-set digest catches set inequality even at
//     equal counts;
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
  BILLING_GROUPED_TALLIES,
  billingNewestHashesFromRows,
  billingRowHash,
  compareGroupedTallies,
  compareNewestHashes,
  groupedTallyFromRows,
  keySetDigestFromKeys,
  postgresBillingNewestHashes,
  postgresBillingRowCount,
  postgresGroupedTally,
  postgresKeySetDigest,
  upsertBillingRows,
  type D1SourceRow,
  type GroupedTallySpec,
} from "./billing-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const ledgerRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  amount_cents: n % 2 === 0 ? -25 * n : 100 * n,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  currency: "USD",
  description: `entry ${n}`,
  id: `bill_backfill_${n}`,
  idempotency_key: `billing:backfill:${n}`,
  metadata_json: "{}",
  quantity: null,
  run_id: null,
  source: n % 2 === 0 ? "container_usage" : "stripe_checkout",
  team_id: null,
  unit: null,
  unit_rate_cents: null,
  user_id: `user-${1 + (n % 2)}`,
  ...overrides,
})

const webhookRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  checkout_session_id: `cs_backfill_${n}`,
  event_id: `evt_backfill_${n}`,
  processed_at: null,
  processing_status: "received",
  received_at: `2026-07-01T0${n}:00:00.000Z`,
  type: "checkout.session.completed",
  ...overrides,
})

const payInRow = (
  n: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  context_ref: null,
  cost_msat: 1_000 * n,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  failure_reason: null,
  genesis_id: null,
  id: `payin_backfill_${n}`,
  idempotency_key: `payin:backfill:${n}`,
  pay_in_type: "tip",
  payer_ref: `agent:payer-${n}`,
  public_receipt_ref: null,
  rung: null,
  state: "pending",
  state_changed_at: `2026-07-01T0${n}:00:00.000Z`,
  successor_id: null,
  ...overrides,
})

const ledgerTallySpecs = BILLING_GROUPED_TALLIES["billing_ledger_entries"] ?? []
const perUserBalanceSpec = ledgerTallySpecs.find(spec =>
  spec.groupColumns.length === 1 && spec.groupColumns[0] === "user_id",
) as GroupedTallySpec

describe("billing backfill pure comparators", () => {
  test("per-user balance map: a single-cent drift on one account is a mismatch", () => {
    const d1 = { "user-1": { rows: 3, sums: [1_000] } }
    const postgres = { "user-1": { rows: 3, sums: [999] } }
    const mismatches = compareGroupedTallies(d1, postgres)
    expect(mismatches).toHaveLength(1)
    expect(mismatches[0]?.groupKey).toBe("user-1")
  })

  test("a group present on only one side is a mismatch", () => {
    const d1 = { "user-1": { rows: 1, sums: [100] } }
    const mismatches = compareGroupedTallies(d1, {})
    expect(mismatches).toHaveLength(1)
    expect(compareGroupedTallies(d1, d1)).toHaveLength(0)
  })

  test("key-set digest: equal counts but different id sets differ", () => {
    const left = keySetDigestFromKeys(["evt_1", "evt_2"])
    const right = keySetDigestFromKeys(["evt_1", "evt_3"])
    expect(left.count).toBe(right.count)
    expect(left.digest).not.toBe(right.digest)
    // order-insensitive
    expect(keySetDigestFromKeys(["evt_2", "evt_1"]).digest).toBe(left.digest)
  })

  test("row hash ignores extra D1 export columns (d1_rowid)", () => {
    const row = ledgerRow(1)
    expect(
      billingRowHash("billing_ledger_entries", { ...row, d1_rowid: 42 }),
    ).toBe(billingRowHash("billing_ledger_entries", row))
  })

  test("newest-hash compare flags a byte-level amount difference", () => {
    const row = ledgerRow(1)
    const d1Newest = billingNewestHashesFromRows("billing_ledger_entries", [
      row,
    ])
    const pgNewest = billingNewestHashesFromRows("billing_ledger_entries", [
      { ...row, amount_cents: 101 },
    ])
    expect(compareNewestHashes(d1Newest, pgNewest)).toHaveLength(1)
    expect(compareNewestHashes(d1Newest, d1Newest)).toHaveLength(0)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "billing backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_billing_backfill")
      await admin.end()
      const url = pg.urlFor("khala_billing_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0015_billing_pay_ins.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("ledger entries: run twice → identical state (idempotency)", async () => {
      const page = [ledgerRow(1), ledgerRow(2), ledgerRow(3)]
      await upsertBillingRows(sql, "billing_ledger_entries", page)
      const hashesAfterFirst = await postgresBillingNewestHashes(
        sql,
        "billing_ledger_entries",
        10,
      )
      await upsertBillingRows(sql, "billing_ledger_entries", page)
      const hashesAfterSecond = await postgresBillingNewestHashes(
        sql,
        "billing_ledger_entries",
        10,
      )
      expect(hashesAfterSecond).toEqual(hashesAfterFirst)
      expect(
        await postgresBillingRowCount(sql, "billing_ledger_entries"),
      ).toBe(3)
    })

    test("per-user balance map reconciles exactly against the source rows", async () => {
      const sourceRows = [ledgerRow(1), ledgerRow(2), ledgerRow(3)]
      // Build the D1-side tally from the raw rows (what the CLI computes
      // via SQL on D1 — here computed structurally for the fixture).
      const perUser = new Map<string, { rows: number; sum: number }>()
      for (const row of sourceRows) {
        const key = String(row["user_id"])
        const entry = perUser.get(key) ?? { rows: 0, sum: 0 }
        entry.rows += 1
        entry.sum += Number(row["amount_cents"])
        perUser.set(key, entry)
      }
      const d1Tally = Object.fromEntries(
        [...perUser.entries()].map(([key, entry]) => [
          key,
          { rows: entry.rows, sums: [entry.sum] },
        ]),
      )
      const postgresTally = await postgresGroupedTally(
        sql,
        "billing_ledger_entries",
        perUserBalanceSpec,
      )
      expect(compareGroupedTallies(d1Tally, postgresTally)).toHaveLength(0)
    })

    test("webhook events: converge brings a stale status forward; id set digest matches", async () => {
      const received = [webhookRow(1), webhookRow(2)]
      await upsertBillingRows(sql, "stripe_webhook_events", received)

      // D1 later marks event 1 processed — the catch-up sweep converges it.
      const processed = webhookRow(1, {
        processed_at: "2026-07-01T01:05:00.000Z",
        processing_status: "processed",
      })
      await upsertBillingRows(sql, "stripe_webhook_events", [processed])

      const statusSpec = (BILLING_GROUPED_TALLIES["stripe_webhook_events"] ??
        [])[0] as GroupedTallySpec
      const tally = await postgresGroupedTally(
        sql,
        "stripe_webhook_events",
        statusSpec,
      )
      expect(tally["processed"]?.rows).toBe(1)
      expect(tally["received"]?.rows).toBe(1)

      const digest = await postgresKeySetDigest(sql, "stripe_webhook_events")
      expect(digest).toEqual(
        keySetDigestFromKeys(["evt_backfill_1", "evt_backfill_2"]),
      )
    })

    test("pay_ins: converge re-runs are stable and state transitions converge", async () => {
      await upsertBillingRows(sql, "pay_ins", [payInRow(1), payInRow(2)])
      // D1-side state machine moved pay-in 1 to paid; the sweep converges.
      await upsertBillingRows(sql, "pay_ins", [
        payInRow(1, {
          state: "paid",
          state_changed_at: "2026-07-01T01:10:00.000Z",
        }),
      ])
      const spec = (BILLING_GROUPED_TALLIES["pay_ins"] ??
        [])[0] as GroupedTallySpec
      const tally = await postgresGroupedTally(sql, "pay_ins", spec)
      expect(tally["tip:paid"]).toEqual({ rows: 1, sums: [1_000] })
      expect(tally["tip:pending"]).toEqual({ rows: 1, sums: [2_000] })

      // and the same D1 snapshot compares clean
      const d1Rows = [
        payInRow(1, {
          state: "paid",
          state_changed_at: "2026-07-01T01:10:00.000Z",
        }),
        payInRow(2),
      ]
      const d1Tally = groupedTallyFromRows(spec, [
        {
          group_key: "tip:paid",
          row_count: 1,
          sum_0: 1_000,
        },
        {
          group_key: "tip:pending",
          row_count: 1,
          sum_0: 2_000,
        },
      ])
      expect(compareGroupedTallies(d1Tally, tally)).toHaveLength(0)
      // hash parity for the newest snapshot rows
      const d1Newest = billingNewestHashesFromRows("pay_ins", [
        d1Rows[1] as D1SourceRow,
        d1Rows[0] as D1SourceRow,
      ])
      const pgNewest = await postgresBillingNewestHashes(sql, "pay_ins", 10)
      expect(compareNewestHashes(d1Newest, pgNewest)).toHaveLength(0)
    })
  },
)
