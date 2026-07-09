import { describe, expect, test } from "bun:test"

import {
  checkBehaviorContractCoverageFromFiles,
  validateBehaviorContractRegistry,
} from "@openagentsinc/behavior-contracts"
import {
  SARAH_FLEET_COMMAND_CONTRACTS_DOC_PATH,
  sarahFleetCommandContractRegistry,
} from "./fleet-command-contracts.ts"

const repoPath = (ref: string): string =>
  new URL(`../../../../${ref}`, import.meta.url).pathname

describe("Sarah Fleet Command contract registry", () => {
  test("records the owner expectation as pending behind the live fleet proof", async () => {
    const validation = validateBehaviorContractRegistry(
      sarahFleetCommandContractRegistry,
    )
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)

    const coverage = await checkBehaviorContractCoverageFromFiles(
      sarahFleetCommandContractRegistry,
      (path) => Bun.file(path).text(),
      repoPath,
    )
    expect(coverage.ok).toBe(true)
    expect(coverage.results.every((result) => result.status === "skipped_state")).toBe(
      true,
    )

    const [contract] = sarahFleetCommandContractRegistry.contracts
    expect(contract?.state).toBe("pending")
    expect(contract?.enforcementTier).toBe("unenforced")
    expect(contract?.blockerRefs).toEqual([
      "issue:#8637",
      "issue:#8633",
      "issue:#8639",
      "issue:#8640",
    ])

    const doc = await Bun.file(
      repoPath(SARAH_FLEET_COMMAND_CONTRACTS_DOC_PATH),
    ).text()
    expect(doc).toContain(
      `Registry version: \`${sarahFleetCommandContractRegistry.version}\``,
    )
    expect(doc).toContain(contract?.contractId ?? "missing contract id")
    expect(doc).toContain(contract?.statement ?? "missing statement")
  })
})
