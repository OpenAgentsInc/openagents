import { describe, expect, test } from "bun:test"

import {
  buildMeasuredReceipt,
  buildNotEligibleReceipt,
  normalizePathPrefix,
  parseGitUiLog,
  parsePrNumbersFromSubject,
  pathMatchesAnyPrefix,
  quantile,
  summarizeUiVelocityWindow,
} from "./ui-velocity-receipt"

describe("ui velocity receipt helpers", () => {
  test("parses PR refs from historical squash subjects", () => {
    expect(parsePrNumbersFromSubject("feat: ship sidebar (#8349)")).toEqual([8349])
    expect(parsePrNumbersFromSubject("Merge remote-tracking branch origin/pr/8258")).toEqual([8258])
    expect(parsePrNumbersFromSubject("docs: issue only")).toEqual([])
  })

  test("normalizes path filters and matches only child files", () => {
    expect(normalizePathPrefix("./apps/openagents.com/apps/start")).toBe(
      "apps/openagents.com/apps/start/",
    )
    expect(pathMatchesAnyPrefix(
      "apps/openagents.com/apps/start/src/routes/index.tsx",
      ["apps/openagents.com/apps/start"],
    )).toBe(true)
    expect(pathMatchesAnyPrefix(
      "apps/openagents.com/apps/web/src/page.ts",
      ["apps/openagents.com/apps/start"],
    )).toBe(false)
  })

  test("parses git UI log rows", () => {
    const entries = parseGitUiLog(
      "abc\t2026-07-04T20:06:51Z\tAdd TanStack Start staging scaffold (#8343)\n" +
        "def\t2026-07-04T21:36:04Z\tTS-7: port sidebar origin/pr/8349\n",
    )

    expect(entries.map((entry) => entry.hash)).toEqual(["abc", "def"])
    expect(entries.map((entry) => entry.prs)).toEqual([[8343], [8349]])
  })

  test("uses the baseline interpolation quantile method", () => {
    expect(quantile([1, 2, 3, 4], 0.5)).toBe(2.5)
    expect(quantile([1, 2, 3, 4], 0.75)).toBe(3.25)
    expect(quantile([], 0.5)).toBeNull()
  })

  test("summarizes windows with direct commit and cycle-time counts", () => {
    const cutoff = new Date("2026-08-04T00:00:00Z")
    const start = new Date("2026-08-03T00:00:00Z")
    const summary = summarizeUiVelocityWindow({
      cutoff,
      start,
      pathFilters: [
        "apps/openagents.com/apps/start",
        "clients/khala-code-desktop",
      ],
      entries: [
        {
          hash: "a",
          date: new Date("2026-08-03T01:00:00Z"),
          subject: "TS-2 follow-up (#9001)",
          prs: [9001],
        },
        {
          hash: "b",
          date: new Date("2026-08-03T02:00:00Z"),
          subject: "direct fix",
          prs: [],
        },
      ],
      pullRequests: [
        {
          number: 9001,
          createdAt: "2026-08-03T00:30:00Z",
          mergedAt: "2026-08-03T01:45:00Z",
          files: ["apps/openagents.com/apps/start/src/routes/index.tsx"],
        },
        {
          number: 9002,
          createdAt: "2026-08-03T00:30:00Z",
          mergedAt: "2026-08-04T00:30:00Z",
          files: ["clients/khala-code-desktop/src/ui/main.tsx"],
        },
      ],
    })

    expect(summary.gitUiFirstParentCommitCount).toBe(2)
    expect(summary.uiPrCount).toBe(1)
    expect(summary.uiPrsByPathPrefix["apps/openagents.com/apps/start/"]).toBe(1)
    expect(summary.uiPrsByPathPrefix["clients/khala-code-desktop/"]).toBe(0)
    expect(summary.directOrNoPrUiCommitCount).toBe(1)
    expect(summary.cycleMinutes).toMatchObject({
      count: 1,
      average: 75,
      median: 75,
      p75: 75,
      min: 75,
      max: 75,
    })
  })

  test("emits an explicit not-eligible receipt before the React-era window matures", () => {
    const receipt = buildNotEligibleReceipt({
      repo: "OpenAgentsInc/openagents",
      ref: "HEAD",
      cutoff: new Date("2026-07-04T22:49:12Z"),
      pathFilters: ["apps/openagents.com/apps/start", "clients/khala-code-desktop"],
      eraStart: new Date("2026-07-04T21:36:04Z"),
      requiredAgeDays: 30,
    })

    expect(receipt.measurementState).toBe("not_eligible")
    expect(receipt.windows).toEqual([])
    expect(receipt.eligibility?.earliestEligibleCutoff).toBe("2026-08-03T21:36:04.000Z")
  })

  test("builds measured receipts from supplied entries and PRs", () => {
    const receipt = buildMeasuredReceipt({
      repo: "OpenAgentsInc/openagents",
      ref: "HEAD",
      cutoff: new Date("2026-08-04T00:00:00Z"),
      pathFilters: ["apps/openagents.com/apps/start"],
      windowDays: [1],
      entries: [
        {
          hash: "a",
          date: new Date("2026-08-03T01:00:00Z"),
          subject: "TS-2 follow-up (#9001)",
          prs: [9001],
        },
      ],
      pullRequests: [
        {
          number: 9001,
          createdAt: "2026-08-03T00:00:00Z",
          mergedAt: "2026-08-03T00:10:00Z",
          files: ["apps/openagents.com/apps/start/src/routes/index.tsx"],
        },
      ],
    })

    expect(receipt.measurementState).toBe("measured")
    expect(receipt.windows[0]?.uiPrCount).toBe(1)
    expect(receipt.windows[0]?.cycleMinutes.median).toBe(10)
  })
})
