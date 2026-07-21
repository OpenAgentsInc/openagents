/**
 * AFS-11 claim→rung→proof ledger validation (GitHub issue #9089).
 *
 * These tests are the mechanical, structural guard behind the AFS-11 promise
 * that no product claim sits above its honest rung. They do NOT re-run the
 * cited proofs (an earlier audit, orrery #9089, found the prior wording
 * overstated this). They validate the typed ledger in
 * `afs-11-claim-evidence.ts` against the real repository:
 *
 *   - every version-one capability is covered,
 *   - each version-one capability claim text is present verbatim in the plan
 *     cut line, so the transcription cannot drift from its source,
 *   - every cited proof file exists on disk,
 *   - runnable proofs are sweep files or check scripts, ceremony proofs are
 *     scripts or runbooks, and each achieved-rung claim's rung matches the
 *     strongest proof kind it records as passing,
 *   - the reserved signed-release outcome stays at owner-signing-pending, is
 *     never asserted as a passing signing proof, and its reserved-step
 *     reference resolves to a real in-repository file,
 *   - the reserved status is not ranked above an achieved rung,
 *   - the committed ceremony evidence record exists on disk.
 *
 * The pass or refuse verdicts of the runnable proofs come from `pnpm run check`,
 * which executes those `.test.ts`/`.test.tsx` files and `check-` scripts. The
 * ceremony verdicts come from the committed evidence record, not from this
 * guard.
 */
import { existsSync, readFileSync } from "node:fs"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  afs11ClaimLedger,
  afs11EvidenceDocRef,
  citedProofRefs,
  ledgerViolations,
  missingCapabilityIds,
  releaseOutcomeClaimId,
  reservedRung,
  rungOrder,
  strongestPassingRung,
  versionOneCapabilityIds,
  versionOneCutLineSourceRef,
} from "./afs-11-claim-evidence.ts"

// apps/openagents-desktop/src -> repository root
const repositoryRoot = path.resolve(import.meta.dirname, "..", "..", "..")

/** Normalize a claim or cut-line bullet for comparison: drop inline-code
 *  backticks and collapse whitespace. */
const normalizeClaim = (text: string): string => text.replace(/`/g, "").replace(/\s+/g, " ").trim()

/** The capability claim bullets from the plan's "Version-one cut line" section,
 *  normalized. */
const readCutLineClaims = (): ReadonlyArray<string> => {
  const source = readFileSync(path.join(repositoryRoot, versionOneCutLineSourceRef), "utf8")
  const start = source.indexOf("## Version-one cut line")
  const end = source.indexOf("Version one does not include", start)
  const section = source.slice(start, end === -1 ? undefined : end)
  return section
    .split("\n")
    .filter((line) => line.startsWith("- "))
    .map((line) => normalizeClaim(line.slice(2)))
}

describe("AFS-11 claim evidence ledger (#9089)", () => {
  test("covers every version-one capability from the cut line", () => {
    expect(missingCapabilityIds(afs11ClaimLedger)).toEqual([])
  })

  test("every version-one capability claim text is present in the plan cut line", () => {
    const cutLineClaims = readCutLineClaims()
    const capabilityRecords = afs11ClaimLedger.filter((record) =>
      (versionOneCapabilityIds as ReadonlyArray<string>).includes(record.id),
    )
    const orphaned = capabilityRecords
      .filter((record) => !cutLineClaims.includes(normalizeClaim(record.claim)))
      .map((record) => record.id)
    expect(orphaned).toEqual([])
  })

  test("the packaging and staging claim stays present in the ledger", () => {
    const staging = afs11ClaimLedger.find((record) => record.id === "C-PACKAGE-STAGING")
    expect(staging).toBeDefined()
    expect(staging?.rung).toBe("packaged-proven")
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
      if (record.rung === reservedRung) continue
      expect(rungOrder[record.rung]).toBeLessThanOrEqual(strongestPassingRung(record))
    }
  })

  test("the reserved status is not ranked above an achieved rung", () => {
    expect(rungOrder[reservedRung]).toBeLessThanOrEqual(rungOrder["packaged-proven"])
  })

  test("the signed-release outcome stays owner-reserved and unasserted", () => {
    const outcome = afs11ClaimLedger.find((record) => record.id === releaseOutcomeClaimId)
    expect(outcome).toBeDefined()
    expect(outcome?.rung).toBe(reservedRung)
    expect(outcome?.blockedOnOwner).toBe(true)
    expect((outcome?.ownerReservedStepRef ?? "").length).toBeGreaterThan(0)
    const asserted = (outcome?.proofs ?? []).some(
      (proof) => proof.kind === "owner-signing" && proof.ran && proof.result === "pass",
    )
    expect(asserted).toBe(false)
  })

  test("the reserved-step reference resolves to a real in-repository file", () => {
    const outcome = afs11ClaimLedger.find((record) => record.id === releaseOutcomeClaimId)
    const ref = outcome?.ownerReservedStepRef ?? ""
    const filePart = ref.split("#")[0] ?? ""
    expect(filePart.length).toBeGreaterThan(0)
    expect(existsSync(path.join(repositoryRoot, filePart))).toBe(true)
  })

  test("the committed ceremony evidence record exists on disk", () => {
    expect(existsSync(path.join(repositoryRoot, afs11EvidenceDocRef))).toBe(true)
  })

  test("no capability claim is asserted at the owner-signing rung", () => {
    const overreach = afs11ClaimLedger.filter(
      (record) =>
        record.rung === reservedRung &&
        (versionOneCapabilityIds as ReadonlyArray<string>).includes(record.id),
    )
    expect(overreach).toEqual([])
  })
})
