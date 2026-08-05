import { readFile } from "node:fs/promises"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { decodeBehaviorContractRegistryDocument } from "./contract"
import { checkBehaviorContractCoverageFromFiles } from "./coverage"
import { marketSwapCompareContractRegistry } from "./market-swap-compare"
import { validateBehaviorContractRegistry } from "./registry"

const repoRoot = resolve(import.meta.dirname, "../../..")

describe("market swap compare contract registry", () => {
  test("decodes and validates with zero issues", () => {
    const decoded = decodeBehaviorContractRegistryDocument(
      marketSwapCompareContractRegistry,
    )
    const validation = validateBehaviorContractRegistry(decoded)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("carries exactly the four laws issue #9318 requires", () => {
    expect(
      marketSwapCompareContractRegistry.contracts.map(c => c.contractId),
    ).toEqual([
      "openagents_web.swap_compare.firm_indicative_distinct.v1",
      "openagents_web.swap_compare.reservation_proof_class_distinct.v1",
      "openagents_web.swap_compare.quote_expiry_enforced.v1",
      "openagents_web.swap_compare.funding_disabled_until_checks_pass.v1",
    ])
  })

  test("oracle coverage links against the real test sources on disk", async () => {
    const report = await checkBehaviorContractCoverageFromFiles(
      marketSwapCompareContractRegistry,
      path => readFile(path, "utf8"),
      ref => resolve(repoRoot, ref),
    )
    expect(report.ok).toBe(true)
    for (const result of report.results) {
      expect(result.status).toBe("covered")
    }
  })
})
