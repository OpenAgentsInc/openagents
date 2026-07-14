import { describe, expect, test } from "vite-plus/test"

import { parseVitePlusTestSummary } from "../src/full-gate.ts"

describe("Vite Plus full-gate summary", () => {
  test("accepts a green summary whose zero-failure axis is omitted", () => {
    expect(parseVitePlusTestSummary(" Test Files 133 passed\n Tests 1308 passed | 39 skipped (1347)\n")).toEqual({
      passed: 1308,
      skipped: 39,
      failed: 0,
    })
  })

  test("retains an explicit failure axis and refuses missing summaries", () => {
    expect(parseVitePlusTestSummary(" Tests 2 failed | 9 passed | 1 skipped (12)\n")).toEqual({
      passed: 9,
      skipped: 1,
      failed: 2,
    })
    expect(parseVitePlusTestSummary("no test summary")).toBeNull()
  })
})
