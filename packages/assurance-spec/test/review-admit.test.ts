import { readFileSync } from "node:fs"
import { existsSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, test } from "vite-plus/test"

import {
  admitAssuranceFrontmatter,
  assessStructuralHonesty,
  buildAuthorityDecisionReceipt,
  classifyReviewTiers,
  decideReviewAdmission,
  decodeAuthorityDecisionReceipt,
  parseAssuranceSpec,
  planOracleReproduction,
  validateAssuranceSpec,
  type BatchReproduction,
  type OracleBatch,
} from "../src/index.ts"

const repoRoot = resolve(import.meta.dirname, "../../..")

/**
 * The live Full Auto proof design. `specs/desktop/full-auto.assurance-spec.md`
 * was retired on 2026-08-04 when the owner deleted `apps/openagents-desktop`
 * (OpenAgentsInc/openagents#9325); Full Auto's surviving implementation and
 * proof design live in `packages/omega-effectd` and this spec.
 */
const LIVE_SPEC = "specs/omega/full-auto.assurance-spec.md"
/** The retired Desktop companion, exercised below as a retired document. */
const RETIRED_DESKTOP_SPEC = "specs/desktop/full-auto.assurance-spec.md"

const read = (path: string): string => readFileSync(resolve(repoRoot, path), "utf8")
// Normalize to a proposed baseline so these tests are independent of whether
// the live spec has already been admitted on disk.
const markdown = read(LIVE_SPEC).replace(/^lifecycle_state:\s*"admitted"\s*$/m, 'lifecycle_state: "proposed"')
const document = parseAssuranceSpec(markdown)
const fileExists = (path: string): boolean => existsSync(resolve(repoRoot, path))
const greenReproducer = (batch: OracleBatch): BatchReproduction => ({
  batch_id: batch.batch_id,
  ok: true,
  exit_code: 0,
  tests_passed: 100,
  tests_failed: 0,
  files: batch.file_args.length,
})

describe("independent-admission verifier", () => {
  test("classifies the live Omega Full Auto spec into its exact tiers", () => {
    const classifications = classifyReviewTiers(document, fileExists)
    const counts = { executable: 0, smoke_gated: 0, receipt_backed: 0, designed_only: 0, release_blocked: 0, unclassified: 0 }
    for (const entry of classifications) counts[entry.tier] += 1
    expect(counts).toEqual({ executable: 4, smoke_gated: 0, receipt_backed: 0, designed_only: 0, release_blocked: 0, unclassified: 4 })
    // The 4 unclassified criteria are exactly the ones whose oracle is a real
    // but non-test artifact (an engine source file or a design document) armed
    // by an Omega-specific gate the desktop tier table does not name. They are
    // honestly unobserved, never counted as executable.
    const unclassified = classifications.filter((entry) => entry.tier === "unclassified")
    expect(unclassified.map((entry) => entry.criterion_ref).sort()).toEqual([
      "OMEGA-FA-AC-01",
      "OMEGA-FA-AC-03",
      "OMEGA-FA-AC-07",
      "OMEGA-FA-AC-08",
    ])
    expect(unclassified.every((entry) => entry.oracle_exists && !entry.oracle_is_test_file)).toBe(true)
  })

  test("plans reproduction as one repository-root Omega engine batch", () => {
    const batches = planOracleReproduction(classifyReviewTiers(document, fileExists))
    // `apps/openagents-desktop` is deleted, so there is no second working
    // directory and no app-local vp binary to borrow: one root batch remains.
    expect(batches.map((batch) => batch.batch_id)).toEqual(["repo-oracles"])
    const repo = batches[0]
    expect(repo?.cwd).toBe(".")
    expect(repo?.binary).toBe("./node_modules/.bin/vp")
    expect(repo?.root).toBe(".")
    expect(repo?.file_args).toEqual([
      "packages/omega-effectd/src/engine/full-auto-capacity.test.ts",
      "packages/omega-effectd/src/engine/full-auto-routing.test.ts",
      "packages/omega-effectd/src/engine/full-auto-run-report.test.ts",
      "packages/omega-effectd/src/protocol/server.host-bridge.test.ts",
    ])
    // Every planned file argument is a file that actually exists.
    expect(repo?.file_args.every(fileExists)).toBe(true)
  })

  test("admits when every armed oracle reproduces green and no tier is overclaimed", () => {
    const decision = decideReviewAdmission({ document, fileExists, reproduce: greenReproducer })
    expect(decision.admit).toBe(true)
    expect(decision.outcome).toBe("succeeded")
    expect(decision.blockers).toEqual([])
    expect(decision.executable_green).toBe(4)
    expect(decision.structural.ok).toBe(true)
  })

  test("refuses when an armed local-unit oracle reproduces red (no rounding up)", () => {
    const redReproducer = (batch: OracleBatch): BatchReproduction =>
      batch.batch_id === "repo-oracles"
        ? { batch_id: batch.batch_id, ok: false, exit_code: 1, tests_passed: 500, tests_failed: 3 }
        : greenReproducer(batch)
    const decision = decideReviewAdmission({ document, fileExists, reproduce: redReproducer })
    expect(decision.admit).toBe(false)
    expect(decision.outcome).toBe("refused")
    expect(decision.blockers.some((blocker) => blocker.code === "oracle_red")).toBe(true)
  })

  test("refuses when a manifest overclaims an unobserved criterion as executable", () => {
    const decision = decideReviewAdmission({
      document,
      fileExists,
      reproduce: greenReproducer,
      claimedTiers: { "OMEGA-FA-AC-01": "executable" },
    })
    expect(decision.admit).toBe(false)
    const roundUp = decision.blockers.find((blocker) => blocker.code === "tier_round_up")
    expect(roundUp?.criterion_ref).toBe("OMEGA-FA-AC-01")
  })

  test("refuses to admit a document that is not proposed", () => {
    const admittedMarkdown = markdown.replace(/^lifecycle_state:\s*"proposed"\s*$/m, 'lifecycle_state: "admitted"')
    const admittedDocument = parseAssuranceSpec(admittedMarkdown)
    const structural = assessStructuralHonesty(admittedDocument)
    expect(structural.ok).toBe(false)
    expect(structural.issues.some((issue) => issue.code === "not_proposed")).toBe(true)
    const decision = decideReviewAdmission({ document: admittedDocument, fileExists, reproduce: greenReproducer })
    expect(decision.admit).toBe(false)
  })

  test("builds a schema-valid authority_decision_receipt.v1 on admission", () => {
    const decision = decideReviewAdmission({ document, fileExists, reproduce: greenReproducer })
    const receipt = buildAuthorityDecisionReceipt({
      decision,
      targetRef: LIVE_SPEC,
      targetDigest: "sha256:" + "a".repeat(64),
      reviewerRef: "authority_delegated_independent_reviewer",
      producerRef: "assurance_packet_producer",
      triggerRef: "owner_directive.independent_admission",
      startedAt: "2026-07-21T00:00:00Z",
      settledAt: "2026-07-21T00:00:01Z",
      evidenceRefs: [LIVE_SPEC],
      scopeNotes: ["Admission overclaims no tier."],
    })
    // Round-trips through its own schema decoder.
    expect(() => decodeAuthorityDecisionReceipt(receipt)).not.toThrow()
    expect(receipt.outcome).toBe("succeeded")
    expect(receipt.action).toBe("admit_assurance_revision_when_source_spec_allows_owner_designated_independent_reviewer")
    expect(receipt.independence.distinct).toBe(true)
    expect(receipt.grant_ref).toBe("grant.independent_assurance")
    expect(receipt.reproduction_summary.executable_green).toBe(4)
    const verification = receipt.condition_results.find((entry) => entry.condition_ref === "condition.verification")
    expect(verification?.result).toBe("satisfied")
  })

  test("records not_satisfied independence when reviewer equals producer", () => {
    const decision = decideReviewAdmission({ document, fileExists, reproduce: greenReproducer })
    const receipt = buildAuthorityDecisionReceipt({
      decision,
      targetRef: LIVE_SPEC,
      targetDigest: "sha256:" + "b".repeat(64),
      reviewerRef: "same_identity",
      producerRef: "same_identity",
      triggerRef: "owner_directive.independent_admission",
      startedAt: "2026-07-21T00:00:00Z",
      settledAt: "2026-07-21T00:00:01Z",
      evidenceRefs: [LIVE_SPEC],
      scopeNotes: ["note"],
    })
    expect(receipt.independence.distinct).toBe(false)
    const independence = receipt.condition_results.find((entry) => entry.condition_ref === "condition.independence")
    expect(independence?.result).toBe("not_satisfied")
  })

  test("flips only the frontmatter lifecycle and keeps the document valid", () => {
    const flipped = admitAssuranceFrontmatter({
      markdown,
      reviewerRef: "authority_delegated_independent_reviewer",
      receiptRef: "authority.decision.deadbeef",
      receiptPath: "docs/assurance/receipts/authority.decision.deadbeef.json",
      admittedAt: "2026-07-21T00:00:01Z",
    })
    const flippedDocument = parseAssuranceSpec(flipped)
    expect(flippedDocument.frontmatter.lifecycle_state).toBe("admitted")
    expect(validateAssuranceSpec(flipped).valid).toBe(true)
    // Body is byte-identical after the frontmatter fence.
    const bodyOf = (text: string): string => text.slice(text.indexOf("\n---\n") + "\n---\n".length)
    expect(bodyOf(flipped)).toBe(bodyOf(markdown))
    // Refuses to re-flip an already-admitted document.
    expect(() => admitAssuranceFrontmatter({ markdown: flipped, reviewerRef: "r", receiptRef: "x", receiptPath: "y", admittedAt: "2026-07-21T00:00:02Z" })).toThrow()
  })
})

describe("retired Desktop Full Auto AssuranceSpec", () => {
  const retiredMarkdown = read(RETIRED_DESKTOP_SPEC)
  const retiredDocument = parseAssuranceSpec(retiredMarkdown)

  test("is retired on disk and cannot be admitted", () => {
    expect(retiredDocument.frontmatter.lifecycle_state).toBe("retired")
    const structural = assessStructuralHonesty(retiredDocument)
    expect(structural.ok).toBe(false)
    expect(structural.issues.some((issue) => issue.code === "not_proposed")).toBe(true)
    const decision = decideReviewAdmission({ document: retiredDocument, fileExists, reproduce: greenReproducer })
    expect(decision.admit).toBe(false)
    // Retiring the document changed only its frontmatter, so it still parses
    // and still validates as an exact-subject AssuranceSpec.
    expect(validateAssuranceSpec(retiredMarkdown).valid).toBe(true)
  })

  test("reports its deleted Electron oracles as unobserved instead of retargeting them", () => {
    const classifications = classifyReviewTiers(retiredDocument, fileExists)
    const counts = { executable: 0, smoke_gated: 0, receipt_backed: 0, designed_only: 0, release_blocked: 0, unclassified: 0 }
    for (const entry of classifications) counts[entry.tier] += 1
    // Before #9325 deleted apps/openagents-desktop this document classified as
    // executable 61 / smoke_gated 2 / receipt_backed 5 / designed_only 8. The
    // 43 executable criteria whose oracle was a deleted apps/openagents-desktop
    // test file fell back to the tier their own obligation already declared:
    // 41 to designed_only (8 + 41 = 49) and 2 (FA-AC-07, FA-AC-29) to the
    // GATE-DEV-TWO-PROCESS smoke tier (2 + 2 = 4). The 5 receipt-backed
    // criteria are unaffected: their evidence file still exists.
    expect(counts).toEqual({ executable: 18, smoke_gated: 4, receipt_backed: 5, designed_only: 49, release_blocked: 0, unclassified: 0 })
    // Nothing is rounded up: no surviving oracle sits under the deleted app,
    // and every criterion still counted executable names a file that exists.
    const desktopOracles = classifications.filter((entry) => entry.evaluator_ref?.startsWith("apps/openagents-desktop/") === true)
    // 49 designed-only (41 fallen back plus the 8 MemoHarness criteria whose
    // named seam never existed) and all 4 smoke-gated criteria.
    expect(desktopOracles.length).toBe(53)
    expect(desktopOracles.every((entry) => entry.tier !== "executable" && !entry.oracle_exists)).toBe(true)
    const executable = classifications.filter((entry) => entry.tier === "executable")
    expect(executable.every((entry) => entry.evaluator_ref !== undefined && fileExists(entry.evaluator_ref))).toBe(true)
    // The 18 surviving oracles all live in packages that outlived the purge.
    expect([...new Set(executable.map((entry) => entry.evaluator_ref))].sort()).toEqual([
      "packages/assurance-spec/test/assurance-spec.test.ts",
      "packages/omega-effectd/src/engine/full-auto-control-server.test.ts",
      "packages/omega-effectd/src/engine/full-auto-lane.test.ts",
      "packages/omega-effectd/src/engine/full-auto-liveness.test.ts",
      "packages/omega-effectd/src/engine/full-auto-provider-handoff.test.ts",
      "packages/omega-effectd/src/engine/full-auto-run-report.test.ts",
    ])
  })
})
