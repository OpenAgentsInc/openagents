import { describe, expect, test } from "bun:test"

import {
  buildCompareSoakReport,
  compareSoakSql,
  KNOWN_COMPARE_SOAK_DOMAINS,
  queryCompareSoak,
  renderCompareSoakReportTable,
  type FetchLike,
} from "./query-compare-soak.js"

describe("compareSoakSql", () => {
  test("builds the expected SUM/GROUP BY query against the named dataset over the window", () => {
    const sql = compareSoakSql("khala_sync_compare_soak", 6)
    expect(sql).toContain("FROM khala_sync_compare_soak")
    expect(sql).toContain("INTERVAL '6' HOUR")
    expect(sql).toContain("GROUP BY blob1")
    expect(sql).toContain("SUM(double1) AS total_reads")
    expect(sql).toContain("SUM(double3) AS mismatches")
  })
})

describe("buildCompareSoakReport", () => {
  test("reports a known domain with traffic as non-vacuous", () => {
    const report = buildCompareSoakReport({
      dataset: "khala_sync_compare_soak",
      hours: 6,
      knownDomains: ["supervision"],
      queriedAt: "2026-07-05T00:00:00.000Z",
      rows: [
        {
          domain: "supervision",
          errors: 0,
          matches: 42,
          mismatches: 0,
          total_reads: 42,
          window_end: "2026-07-05T00:00:00Z",
          window_start: "2026-07-04T18:00:00Z",
        },
      ],
    })
    expect(report.domains).toEqual([
      {
        domain: "supervision",
        errors: 0,
        matches: 42,
        mismatches: 0,
        totalReads: 42,
        vacuous: false,
        windowEnd: "2026-07-05T00:00:00Z",
        windowStart: "2026-07-04T18:00:00Z",
      },
    ])
  })

  test("flags a known domain absent from the query result as VACUOUS — the #8361 zero-traffic case", () => {
    const report = buildCompareSoakReport({
      dataset: "khala_sync_compare_soak",
      hours: 6,
      knownDomains: ["supervision", "artanis"],
      queriedAt: "2026-07-05T00:00:00.000Z",
      rows: [
        {
          domain: "artanis",
          errors: 0,
          matches: 10,
          mismatches: 0,
          total_reads: 10,
          window_end: null,
          window_start: null,
        },
      ],
    })
    const supervision = report.domains.find(d => d.domain === "supervision")
    expect(supervision).toEqual({
      domain: "supervision",
      errors: 0,
      matches: 0,
      mismatches: 0,
      totalReads: 0,
      vacuous: true,
      windowEnd: null,
      windowStart: null,
    })
  })

  test("surfaces a domain the query returned that isn't in the known list yet", () => {
    const report = buildCompareSoakReport({
      dataset: "khala_sync_compare_soak",
      hours: 6,
      knownDomains: ["supervision"],
      queriedAt: "2026-07-05T00:00:00.000Z",
      rows: [
        {
          domain: "some_new_domain",
          errors: 0,
          matches: 5,
          mismatches: 0,
          total_reads: 5,
          window_end: null,
          window_start: null,
        },
      ],
    })
    expect(report.domains.map(d => d.domain).sort()).toEqual([
      "some_new_domain",
      "supervision",
    ])
  })

  test("mismatches and errors are counted independently of the total", () => {
    const report = buildCompareSoakReport({
      dataset: "khala_sync_compare_soak",
      hours: 24,
      knownDomains: ["entitlements_gate"],
      queriedAt: "2026-07-05T00:00:00.000Z",
      rows: [
        {
          domain: "entitlements_gate",
          errors: "3",
          matches: "95",
          mismatches: "2",
          total_reads: "100",
          window_end: "2026-07-05T00:00:00Z",
          window_start: "2026-07-04T00:00:00Z",
        },
      ],
    })
    expect(report.domains[0]).toEqual({
      domain: "entitlements_gate",
      errors: 3,
      matches: 95,
      mismatches: 2,
      totalReads: 100,
      vacuous: false,
      windowEnd: "2026-07-05T00:00:00Z",
      windowStart: "2026-07-04T00:00:00Z",
    })
  })

  test("defaults to the full KNOWN_COMPARE_SOAK_DOMAINS list when none is passed", () => {
    const report = buildCompareSoakReport({
      dataset: "khala_sync_compare_soak",
      hours: 6,
      queriedAt: "2026-07-05T00:00:00.000Z",
      rows: [],
    })
    expect(report.domains.map(d => d.domain).sort()).toEqual(
      [...KNOWN_COMPARE_SOAK_DOMAINS].sort(),
    )
    expect(report.domains.every(d => d.vacuous)).toBe(true)
  })
})

describe("queryCompareSoak", () => {
  const fakeFetch = (body: unknown, ok = true, status = 200): FetchLike => async () => ({
    ok,
    status,
    text: async () => JSON.stringify(body),
  })

  test("parses a ClickHouse-style JSON response into the cross-referenced report", async () => {
    const report = await queryCompareSoak({
      accountId: "acct-1",
      apiToken: "token-1",
      dataset: "khala_sync_compare_soak",
      fetchImpl: fakeFetch({
        data: [
          {
            domain: "supervision",
            errors: 0,
            matches: 12,
            mismatches: 0,
            total_reads: 12,
            window_end: "2026-07-05T00:00:00Z",
            window_start: "2026-07-04T18:00:00Z",
          },
        ],
        meta: [],
        rows: 1,
      }),
      hours: 6,
      knownDomains: ["supervision"],
      now: () => new Date("2026-07-05T00:05:00.000Z"),
    })
    expect(report.queriedAt).toBe("2026-07-05T00:05:00.000Z")
    expect(report.domains).toEqual([
      {
        domain: "supervision",
        errors: 0,
        matches: 12,
        mismatches: 0,
        totalReads: 12,
        vacuous: false,
        windowEnd: "2026-07-05T00:00:00Z",
        windowStart: "2026-07-04T18:00:00Z",
      },
    ])
  })

  test("an empty data array reports every known domain as VACUOUS rather than throwing", async () => {
    const report = await queryCompareSoak({
      accountId: "acct-1",
      apiToken: "token-1",
      dataset: "khala_sync_compare_soak",
      fetchImpl: fakeFetch({ data: [], meta: [], rows: 0 }),
      hours: 6,
      knownDomains: ["supervision", "artanis"],
    })
    expect(report.domains.every(d => d.vacuous)).toBe(true)
  })

  test("throws with the response body on a non-2xx status — never silently reports empty as clean", async () => {
    await expect(
      queryCompareSoak({
        accountId: "acct-1",
        apiToken: "bad-token",
        dataset: "khala_sync_compare_soak",
        fetchImpl: fakeFetch({ errors: [{ message: "Authentication error" }] }, false, 403),
        hours: 6,
      }),
    ).rejects.toThrow(/403/)
  })

  test("throws a clear error on a non-JSON response body", async () => {
    const brokenFetch: FetchLike = async () => ({
      ok: true,
      status: 200,
      text: async () => "not json",
    })
    await expect(
      queryCompareSoak({
        accountId: "acct-1",
        apiToken: "token-1",
        dataset: "khala_sync_compare_soak",
        fetchImpl: brokenFetch,
        hours: 6,
      }),
    ).rejects.toThrow(/non-JSON/)
  })
})

describe("renderCompareSoakReportTable", () => {
  test("flags mismatches and vacuous domains distinctly in the rendered table", () => {
    const table = renderCompareSoakReportTable({
      dataset: "khala_sync_compare_soak",
      domains: [
        {
          domain: "artanis",
          errors: 0,
          matches: 98,
          mismatches: 2,
          totalReads: 100,
          vacuous: false,
          windowEnd: "2026-07-05T00:00:00Z",
          windowStart: "2026-07-04T18:00:00Z",
        },
        {
          domain: "supervision",
          errors: 0,
          matches: 0,
          mismatches: 0,
          totalReads: 0,
          vacuous: true,
          windowEnd: null,
          windowStart: null,
        },
      ],
      hours: 6,
      queriedAt: "2026-07-05T00:05:00.000Z",
    })
    expect(table).toContain("MISMATCHES — do NOT flip")
    expect(table).toContain("VACUOUS — no compare-mode traffic in window")
  })
})
