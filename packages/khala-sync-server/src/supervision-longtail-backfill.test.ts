import { describe, expect, test } from "bun:test"
import {
  buildSupervisionLongtailVerifyReport,
  compareIdempotencyKeySets,
  compareProofBundleDigests,
  d1SupervisionLongtailNewestHashes,
  idempotencyKeySetFromRows,
  proofBundleDigestFromRows,
  supervisionLongtailRowHash,
  supervisionLongtailVerifyReportClean,
  SUPERVISION_LONGTAIL_TABLE_SPECS,
  SUPERVISION_LONGTAIL_TABLES,
} from "./supervision-longtail-backfill.js"

describe("supervision-longtail registry", () => {
  test("29 tables, every spec's keyColumns and orderColumn are real columns", () => {
    expect(SUPERVISION_LONGTAIL_TABLES.length).toBe(29)
    for (const table of SUPERVISION_LONGTAIL_TABLES) {
      const spec = SUPERVISION_LONGTAIL_TABLE_SPECS[table]
      expect(spec.keyColumns.length).toBeGreaterThan(0)
      for (const key of spec.keyColumns) {
        expect(spec.columns).toContain(key)
      }
      expect(spec.columns).toContain(spec.orderColumn)
      for (const custody of spec.custodyColumns ?? []) {
        expect(spec.columns).toContain(custody)
      }
    }
  })
})

describe("row hash — D1 number vs postgres bigint-string parity", () => {
  test("smallint/bigint columns hash identically across engine representations", () => {
    // D1 returns numbers; postgres.js returns bigint columns as strings and
    // smallint as numbers. The hash must be identical.
    const d1Row = {
      account_ref: null,
      cache_read_tokens: 0,
      cache_write_1h_tokens: 0,
      cache_write_5m_tokens: 0,
      created_at: "2026-07-04T00:00:00.000Z",
      event_id: "e1",
      id: "t1",
      input_tokens: 100,
      model: "m",
      output_tokens: 200,
      provider: "p",
      reasoning_tokens: 0,
      run_id: "r1",
      source: "s",
      source_ref: "sr1",
      team_id: null,
      total_tokens: 300,
      user_id: "u1",
    }
    const pgRow = {
      ...d1Row,
      input_tokens: "100", // bigint from postgres.js
      output_tokens: "200",
      total_tokens: "300",
    }
    expect(supervisionLongtailRowHash("autopilot_token_usage", d1Row)).toBe(
      supervisionLongtailRowHash("autopilot_token_usage", pgRow),
    )
  })
})

describe("idempotency key-set equality (omni_idempotency_keys)", () => {
  test("equal sets pass; a missing / extra key is reported", () => {
    const d1 = idempotencyKeySetFromRows([
      { key: "a" },
      { key: "b" },
      { key: "c" },
    ])
    expect(compareIdempotencyKeySets(d1, new Set(["a", "b", "c"])).equal).toBe(
      true,
    )
    const missing = compareIdempotencyKeySets(d1, new Set(["a", "b"]))
    expect(missing.equal).toBe(false)
    expect(missing.missingInPostgres).toEqual(["c"])
    const extra = compareIdempotencyKeySets(d1, new Set(["a", "b", "c", "d"]))
    expect(extra.extraInPostgres).toEqual(["d"])
  })
})

describe("public proof-bundle digests", () => {
  test("identical projections match; a status drift is detected", () => {
    const rows = [
      {
        acceptance_state_ref: "acc",
        economics_caveat_ref: "econ",
        id: "b1",
        legal_sensitive: 0,
        no_settlement_implication: 1,
        privacy_caveat_ref: "priv",
        public_receipt_ref: "rcpt",
        review_state_ref: "rev",
        status: "ready",
        updated_at: "t",
        work_kind: "site",
        workroom_id: "w1",
      },
    ]
    const d1 = proofBundleDigestFromRows(rows)
    const same = proofBundleDigestFromRows(rows.map(r => ({ ...r })))
    expect(compareProofBundleDigests(d1, same).length).toBe(0)
    const drifted = proofBundleDigestFromRows([
      { ...rows[0], status: "blocked" },
    ])
    expect(compareProofBundleDigests(d1, drifted).length).toBe(1)
  })
})

describe("verify report", () => {
  test("clean when counts + scalars + newest hashes all match", () => {
    const newest = d1SupervisionLongtailNewestHashes("relay_health_probes", [
      {
        created_at: "t",
        id: "p1",
        nip11_http_status: 200,
        nip11_latency_ms: 1,
        nip11_outcome: "ok",
        nip11_relay_name: "m",
        probed_at: "t",
        relay_url: "wss",
        status: "healthy",
        ws_latency_ms: 2,
        ws_outcome: "ok",
      },
    ])
    const report = buildSupervisionLongtailVerifyReport({
      d1Newest: newest,
      d1Total: 1,
      postgresNewest: newest,
      postgresTotal: 1,
      scalars: [{ d1: 1, metric: "healthy_probes", postgres: 1 }],
      table: "relay_health_probes",
    })
    expect(supervisionLongtailVerifyReportClean(report)).toBe(true)

    const drifted = buildSupervisionLongtailVerifyReport({
      d1Newest: newest,
      d1Total: 1,
      postgresNewest: newest,
      postgresTotal: 2,
      scalars: [],
      table: "relay_health_probes",
    })
    expect(supervisionLongtailVerifyReportClean(drifted)).toBe(false)
  })
})
