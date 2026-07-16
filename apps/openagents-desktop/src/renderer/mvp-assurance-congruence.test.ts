import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import {
  mvpAssuranceCongruenceViolations,
  mvpAssuranceCoverageMatrix,
  renderMvpAssuranceCoverageMarkdown,
} from "./mvp-assurance-congruence.ts"

describe("MVP assurance/visible-surface congruence (UX-5 #8791)", () => {
  test("the expected-working allowlist is covered exactly and every item has proof", () => {
    expect(mvpAssuranceCongruenceViolations()).toEqual([])
    const contractIds = new Set(openAgentsDesktopUxContractRegistry.contracts.filter(contract => contract.state === "enforced").map(contract => contract.contractId))
    for (const item of mvpAssuranceCoverageMatrix) {
      for (const contractRef of item.contractRefs) expect(contractIds.has(contractRef)).toBe(true)
      for (const oracleRef of item.oracleRefs) expect(readFileSync(resolve(import.meta.dirname, "../../../..", oracleRef), "utf8").length).toBeGreaterThan(0)
    }
  })

  test("the checked-in coverage document is the exact matrix projection", () => {
    const path = resolve(import.meta.dirname, "../../../..", "docs/mvp/openagents-codex-workroom-mvp-assurance-coverage-matrix.md")
    expect(readFileSync(path, "utf8")).toBe(renderMvpAssuranceCoverageMarkdown())
  })

  test("FALSIFIER: an uncovered expected-working surface fails", () => {
    const reduced = mvpAssuranceCoverageMatrix.filter(item => item.surfaceId !== "shell-settings-toggle")
    expect(mvpAssuranceCongruenceViolations(reduced)).toContain('allowlisted surface "shell-settings-toggle" has no assurance item')
  })

  test("FALSIFIER: assurance coverage for a non-MVP surface fails", () => {
    const overCovered = [...mvpAssuranceCoverageMatrix, { ...mvpAssuranceCoverageMatrix[0]!, surfaceId: "workspace-fleet" }]
    expect(mvpAssuranceCongruenceViolations(overCovered)).toContain('assurance matrix over-covers non-MVP surface "workspace-fleet"')
  })
})
