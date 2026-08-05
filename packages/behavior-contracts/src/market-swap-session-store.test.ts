import { readFile } from "node:fs/promises"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import { decodeBehaviorContractRegistryDocument } from "./contract"
import { checkBehaviorContractCoverageFromFiles } from "./coverage"
import { marketSwapSessionStoreContractRegistry } from "./market-swap-session-store"
import { validateBehaviorContractRegistry } from "./registry"

const repoRoot = resolve(import.meta.dirname, "../../..")

describe("market swap session store contract registry", () => {
  test("decodes and validates with zero issues", () => {
    const decoded = decodeBehaviorContractRegistryDocument(
      marketSwapSessionStoreContractRegistry,
    )
    const validation = validateBehaviorContractRegistry(decoded)
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("carries exactly the two laws issue #9320 requires", () => {
    expect(
      marketSwapSessionStoreContractRegistry.contracts.map(c => c.contractId),
    ).toEqual([
      "openagents_web.swap_history.export_import_round_trip.v1",
      "openagents_web.swap_history.resume_after_reload.v1",
    ])
  })

  test("oracle coverage links against the real test sources on disk", async () => {
    const report = await checkBehaviorContractCoverageFromFiles(
      marketSwapSessionStoreContractRegistry,
      path => readFile(path, "utf8"),
      ref => resolve(repoRoot, ref),
    )
    expect(report.ok).toBe(true)
    for (const result of report.results) {
      expect(result.status).toBe("covered")
    }
  })
})
