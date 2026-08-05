import { readFile } from "node:fs/promises"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { decodeBehaviorContractRegistryDocument } from "./contract"
import { checkBehaviorContractCoverageFromFiles } from "./coverage"
import { marketSwapPairContractRegistry } from "./market-swap-pair"
import { validateBehaviorContractRegistry } from "./registry"

const repoRoot = resolve(import.meta.dirname, "../../..")

describe("market swap pair contract registry", () => {
  test("decodes and validates with zero issues", () => {
    const decoded = decodeBehaviorContractRegistryDocument(
      marketSwapPairContractRegistry,
    )
    const validation = validateBehaviorContractRegistry(decoded)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("carries exactly the three laws issue #9316 requires", () => {
    expect(
      marketSwapPairContractRegistry.contracts.map(c => c.contractId),
    ).toEqual([
      "openagents_web.swap_pair.unreachable_direction_disclosed.v1",
      "openagents_web.swap_pair.fee_output_promise.v1",
      "openagents_web.swap_pair.no_auto_unit_switch.v1",
    ])
  })

  test("oracle coverage links against the real test sources on disk", async () => {
    const report = await checkBehaviorContractCoverageFromFiles(
      marketSwapPairContractRegistry,
      path => readFile(path, "utf8"),
      ref => resolve(repoRoot, ref),
    )
    expect(report.ok).toBe(true)
    for (const result of report.results) {
      expect(result.status).toBe("covered")
    }
  })
})
