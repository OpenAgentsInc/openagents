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
const specPath = resolve(repoRoot, "specs/desktop/full-auto.assurance-spec.md")
// Normalize to a proposed baseline so these tests are independent of whether
// the live spec has already been admitted on disk.
const markdown = readFileSync(specPath, "utf8").replace(
  /^lifecycle_state:\s*"admitted"\s*$/m,
  'lifecycle_state: "proposed"',
)
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
  test("classifies the Full Auto spec into the review packet's exact tiers", () => {
    const classifications = classifyReviewTiers(document, fileExists)
    const counts = { executable: 0, smoke_gated: 0, receipt_backed: 0, designed_only: 0, release_blocked: 0, unclassified: 0 }
    for (const entry of classifications) counts[entry.tier] += 1
    expect(counts).toEqual({ executable: 61, smoke_gated: 2, receipt_backed: 5, designed_only: 8, release_blocked: 0, unclassified: 0 })
    // The 8 designed-only criteria are exactly the MemoHarness cluster.
    const designedOnly = classifications.filter((entry) => entry.tier === "designed_only").map((entry) => entry.criterion_ref).sort()
    expect(designedOnly).toEqual(["FA-AC-69", "FA-AC-70", "FA-AC-71", "FA-AC-72", "FA-AC-73", "FA-AC-74", "FA-AC-75", "FA-AC-76"])
  })

  test("plans reproduction as the documented desktop plus assurance-self-check batches", () => {
    const batches = planOracleReproduction(classifyReviewTiers(document, fileExists))
    expect(batches.map((batch) => batch.batch_id).sort()).toEqual(["desktop-oracles", "repo-oracles"])
    const repo = batches.find((batch) => batch.batch_id === "repo-oracles")
    expect(repo?.file_args).toEqual(["packages/assurance-spec/test/assurance-spec.test.ts"])
    const desktop = batches.find((batch) => batch.batch_id === "desktop-oracles")
    expect(desktop?.file_args.every((path) => !path.startsWith("apps/openagents-desktop/"))).toBe(true)
    expect(desktop?.file_args).toContain("tests/full-auto-restart.e2e.test.ts")
  })

  test("admits when every armed oracle reproduces green and no tier is overclaimed", () => {
    const decision = decideReviewAdmission({ document, fileExists, reproduce: greenReproducer })
    expect(decision.admit).toBe(true)
    expect(decision.outcome).toBe("succeeded")
    expect(decision.blockers).toEqual([])
    expect(decision.executable_green).toBe(61)
    expect(decision.structural.ok).toBe(true)
  })

  test("refuses when an armed local-unit oracle reproduces red (no rounding up)", () => {
    const redReproducer = (batch: OracleBatch): BatchReproduction =>
      batch.batch_id === "desktop-oracles"
        ? { batch_id: batch.batch_id, ok: false, exit_code: 1, tests_passed: 500, tests_failed: 3 }
        : greenReproducer(batch)
    const decision = decideReviewAdmission({ document, fileExists, reproduce: redReproducer })
    expect(decision.admit).toBe(false)
    expect(decision.outcome).toBe("refused")
    expect(decision.blockers.some((blocker) => blocker.code === "oracle_red")).toBe(true)
  })

  test("refuses when a manifest overclaims a designed-only criterion as executable", () => {
    const decision = decideReviewAdmission({
      document,
      fileExists,
      reproduce: greenReproducer,
      claimedTiers: { "FA-AC-69": "executable" },
    })
    expect(decision.admit).toBe(false)
    const roundUp = decision.blockers.find((blocker) => blocker.code === "tier_round_up")
    expect(roundUp?.criterion_ref).toBe("FA-AC-69")
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
      targetRef: "specs/desktop/full-auto.assurance-spec.md",
      targetDigest: "sha256:" + "a".repeat(64),
      reviewerRef: "authority_delegated_independent_reviewer",
      producerRef: "assurance_packet_producer",
      triggerRef: "owner_directive.independent_admission",
      startedAt: "2026-07-21T00:00:00Z",
      settledAt: "2026-07-21T00:00:01Z",
      evidenceRefs: ["specs/desktop/full-auto.assurance-spec.md"],
      scopeNotes: ["Admission overclaims no tier."],
    })
    // Round-trips through its own schema decoder.
    expect(() => decodeAuthorityDecisionReceipt(receipt)).not.toThrow()
    expect(receipt.outcome).toBe("succeeded")
    expect(receipt.action).toBe("admit_assurance_revision_when_source_spec_allows_owner_designated_independent_reviewer")
    expect(receipt.independence.distinct).toBe(true)
    expect(receipt.grant_ref).toBe("grant.independent_assurance")
    expect(receipt.reproduction_summary.executable_green).toBe(61)
    const verification = receipt.condition_results.find((entry) => entry.condition_ref === "condition.verification")
    expect(verification?.result).toBe("satisfied")
  })

  test("records not_satisfied independence when reviewer equals producer", () => {
    const decision = decideReviewAdmission({ document, fileExists, reproduce: greenReproducer })
    const receipt = buildAuthorityDecisionReceipt({
      decision,
      targetRef: "specs/desktop/full-auto.assurance-spec.md",
      targetDigest: "sha256:" + "b".repeat(64),
      reviewerRef: "same_identity",
      producerRef: "same_identity",
      triggerRef: "owner_directive.independent_admission",
      startedAt: "2026-07-21T00:00:00Z",
      settledAt: "2026-07-21T00:00:01Z",
      evidenceRefs: ["specs/desktop/full-auto.assurance-spec.md"],
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
