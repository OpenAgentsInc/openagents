/**
 * AFS-11 claim→rung→proof ledger validation (GitHub issue #9089).
 *
 * These tests are the mechanical guard behind the AFS-11 promise that no
 * product claim sits above its actual evidence. They validate the typed ledger
 * in `afs-11-claim-evidence.ts` against the real repository:
 *
 *   - every version-one capability is covered,
 *   - every cited proof file exists on disk,
 *   - every achieved-rung claim earns its rung from a passing proof,
 *   - the reserved signed-release outcome stays at owner-signing-pending and is
 *     never asserted as a passing signing proof.
 */
import { existsSync } from "node:fs"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  afs11ClaimLedger,
  citedProofRefs,
  ledgerViolations,
  missingCapabilityIds,
  releaseOutcomeClaimId,
  rungOrder,
  strongestPassingRung,
  versionOneCapabilityIds,
} from "./afs-11-claim-evidence.ts"

// apps/openagents-desktop/src -> repository root
const repositoryRoot = path.resolve(import.meta.dirname, "..", "..", "..")

describe("AFS-11 claim evidence ledger (#9089)", () => {
  test("covers every version-one capability from the cut line", () => {
    expect(missingCapabilityIds(afs11ClaimLedger)).toEqual([])
  })

  test("has no honesty violation — no claim above its evidence", () => {
    expect(ledgerViolations(afs11ClaimLedger)).toEqual([])
  })

  test("every cited proof file exists in the repository", () => {
    const missing = citedProofRefs(afs11ClaimLedger).filter(
      (ref) => !existsSync(path.join(repositoryRoot, ref)),
    )
    expect(missing).toEqual([])
  })

  test("every claim cites at least one proof", () => {
    const empty = afs11ClaimLedger.filter((record) => record.proofs.length === 0)
    expect(empty).toEqual([])
  })

  test("every achieved-rung claim earns its rung from a passing proof", () => {
    for (const record of afs11ClaimLedger) {
      if (record.rung === "owner-signing-pending") continue
      expect(rungOrder[record.rung]).toBeLessThanOrEqual(strongestPassingRung(record))
    }
  })

  test("the signed-release outcome stays owner-reserved and unasserted", () => {
    const outcome = afs11ClaimLedger.find((record) => record.id === releaseOutcomeClaimId)
    expect(outcome).toBeDefined()
    expect(outcome?.rung).toBe("owner-signing-pending")
    expect(outcome?.blockedOnOwner).toBe(true)
    expect((outcome?.ownerReservedStepRef ?? "").length).toBeGreaterThan(0)
    const asserted = (outcome?.proofs ?? []).some(
      (proof) => proof.kind === "owner-signing" && proof.ran && proof.result === "pass",
    )
    expect(asserted).toBe(false)
  })

  test("no capability claim is asserted at the owner-signing rung", () => {
    const overreach = afs11ClaimLedger.filter(
      (record) =>
        record.rung === "owner-signing-pending" &&
        (versionOneCapabilityIds as ReadonlyArray<string>).includes(record.id),
    )
    expect(overreach).toEqual([])
  })
})
