import { existsSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { t3MobileComponentCensus } from "../src/contracts/t3-mobile-component-census"
import { mobileWorkspaceLayoutMode } from "../src/screens/mobile-adaptive-workspace"

// Oracle for openagents_mobile.t3_code_full_mobile_parity.v1.
const sourceRoot = join(import.meta.dirname, "../src")

describe("T3M-F2 complete mobile component census", () => {
  test("accounts for every named T3 component once with implementation evidence", () => {
    expect(t3MobileComponentCensus).toHaveLength(43)
    expect(new Set(t3MobileComponentCensus.map(row => row.component)).size).toBe(t3MobileComponentCensus.length)
    expect(new Set(t3MobileComponentCensus.map(row => row.area))).toEqual(new Set([
      "shell", "navigation", "transcript", "runtime", "composer", "workbench", "native_finish",
    ]))
    for (const row of t3MobileComponentCensus) {
      expect(["complete", "adapted"]).toContain(row.implementation)
      if (row.evidence.startsWith("@")) continue
      expect(existsSync(join(sourceRoot, row.evidence)), `${row.component}: ${row.evidence}`).toBe(true)
    }
  })

  test("covers the entire ordered A1-F2 packet ledger", () => {
    expect(new Set(t3MobileComponentCensus.map(row => row.packet))).toEqual(new Set([
      "A1", "A2", "A3", "A4", "B1", "B2", "C1", "C2", "D1", "D2", "E1", "E2", "F1", "F2",
    ]))
  })

  test("pins compact phone and regular tablet layout boundaries", () => {
    expect(mobileWorkspaceLayoutMode(390)).toBe("compact")
    expect(mobileWorkspaceLayoutMode(767)).toBe("compact")
    expect(mobileWorkspaceLayoutMode(768)).toBe("regular")
    expect(mobileWorkspaceLayoutMode(1_366)).toBe("regular")
  })
})
