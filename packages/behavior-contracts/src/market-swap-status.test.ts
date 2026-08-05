import { readFile } from "node:fs/promises"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { decodeBehaviorContractRegistryDocument } from "./contract"
import { checkBehaviorContractCoverageFromFiles } from "./coverage"
import { marketSwapStatusContractRegistry } from "./market-swap-status"
import { validateBehaviorContractRegistry } from "./registry"

const repoRoot = resolve(import.meta.dirname, "../../..")

describe("market swap status contract registry", () => {
  test("decodes and validates with zero issues", () => {
    const decoded = decodeBehaviorContractRegistryDocument(
      marketSwapStatusContractRegistry,
    )
    const validation = validateBehaviorContractRegistry(decoded)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("carries exactly the three laws issue #9321 requires", () => {
    expect(
      marketSwapStatusContractRegistry.contracts.map(c => c.contractId),
    ).toEqual([
      "openagents_web.swap_status.gap_renders_unknown.v1",
      "openagents_web.swap_status.fork_retained_loud.v1",
      "openagents_web.swap_status.rung_never_inferred_upward.v1",
    ])
  })

  test("oracle coverage links against the real test sources on disk", async () => {
    const report = await checkBehaviorContractCoverageFromFiles(
      marketSwapStatusContractRegistry,
      path => readFile(path, "utf8"),
      ref => resolve(repoRoot, ref),
    )
    expect(report.ok).toBe(true)
    for (const result of report.results) {
      expect(result.status).toBe("covered")
    }
  })
})
