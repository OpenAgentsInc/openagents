import { describe, expect, test } from "bun:test"
import { resolve } from "node:path"

import {
  assessAssuranceSpec,
  parseAssuranceSpec,
  proposeAssuranceSpec,
  serializeAssuranceSpec,
  validateAssuranceSpec,
} from "../src/index.ts"

const mvpPath = resolve(import.meta.dir, "../../../docs/mvp/openagents-codex-workroom-mvp.product-spec.md")

describe("AssuranceSpec format and proposal", () => {
  test("proposes exact criterion coverage for the current MVP without inventing proof", async () => {
    const source = await Bun.file(mvpPath).text()
    const result = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source })
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.document.subject.product_spec.document_digest).toBe("sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1")
    expect(result.document.subject.product_spec.criterion_refs).toEqual(Array.from({ length: 18 }, (_, index) => `CW-AC-${String(index + 1).padStart(2, "0")}`))
    expect(result.document.obligations).toHaveLength(18)
    expect(result.document.obligations[3]?.id).toBe("AO-CW-AC-04-01")
    expect(result.document.obligations.every((obligation) => obligation.oracle === undefined && obligation.falsifier === undefined && obligation.technique === undefined && obligation.environment_refs === undefined && obligation.candidate_artifact_refs.length === 0)).toBe(true)
    expect(result.adequacy.coverage).toEqual({ criteria: 18, obligations: 18, ready: 0, needs_design: 18 })
    expect(result.adequacy.diagnostics.filter((diagnostic) => diagnostic.code === "obligation_needs_design")).toHaveLength(18)
    expect(result.adequacy.design_ready).toBe(false)
  })

  test("is byte-deterministic and semantic round trips are stable", async () => {
    const source = await Bun.file(mvpPath).text()
    const options = { productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source }
    const first = proposeAssuranceSpec(options)
    const second = proposeAssuranceSpec(options)
    expect(first.ok && second.ok).toBe(true)
    if (!first.ok || !second.ok) return
    expect(first.markdown).toBe(second.markdown)
    const parsed = parseAssuranceSpec(first.markdown)
    expect(parseAssuranceSpec(serializeAssuranceSpec(parsed))).toEqual(parsed)
    expect(validateAssuranceSpec(first.markdown).valid).toBe(true)
  })

  test("preserves authored section narrative while canonicalizing structured blocks", async () => {
    const source = await Bun.file(mvpPath).text()
    const result = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source })
    if (!result.ok) throw new Error("fixture proposal failed")
    const edited = result.markdown.replace(
      "Every executable ProductSpec criterion is in assurance scope. No criterion is silently excluded or marked not applicable.",
      "This review covers the exact MVP criterion set; it grants no execution authority.",
    )
    const roundTrip = serializeAssuranceSpec(parseAssuranceSpec(edited))
    expect(roundTrip).toContain("This review covers the exact MVP criterion set; it grants no execution authority.")
    expect(parseAssuranceSpec(roundTrip)).toEqual(parseAssuranceSpec(edited))
  })

  test("round trips quoted frontmatter and rejects duplicate keys", async () => {
    const source = await Bun.file(mvpPath).text()
    const result = proposeAssuranceSpec({
      productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
      productSpecMarkdown: source,
      title: "MVP \"proof\" plan",
    })
    if (!result.ok) throw new Error("fixture proposal failed")
    expect(parseAssuranceSpec(result.markdown).frontmatter.title).toBe("MVP \"proof\" plan")
    const duplicate = result.markdown.replace(
      'assurance_spec_id: "assurance.openagents.desktop.codex.workroom.mvp"',
      'assurance_spec_id: "assurance.openagents.desktop.codex.workroom.mvp"\nassurance_spec_id: "duplicate"',
    )
    expect(validateAssuranceSpec(duplicate).errors[0]?.code).toBe("duplicate_frontmatter_key")
  })

  test("refuses a ProductSpec with a missing or duplicate executable criterion ID", async () => {
    const source = await Bun.file(mvpPath).text()
    const missing = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source.replace("**CW-AC-01:**", "**Acceptance:**") })
    expect(missing.ok).toBe(false)
    if (!missing.ok) expect(missing.diagnostics.map((diagnostic) => diagnostic.code)).toContain("missing_acceptance_criterion_id")
    const duplicate = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source.replace("**CW-AC-02:**", "**CW-AC-01:**") })
    expect(duplicate.ok).toBe(false)
    if (!duplicate.ok) expect(duplicate.diagnostics.map((diagnostic) => diagnostic.code)).toContain("duplicate_acceptance_criterion_id")
  })

  test("keeps structural validity separate from assurance adequacy", async () => {
    const source = await Bun.file(mvpPath).text()
    const result = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source })
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(validateAssuranceSpec(result.markdown).valid).toBe(true)
    expect(assessAssuranceSpec(result.document).design_ready).toBe(false)
  })

  test("can represent a fully designed proposal without conflating it with admission", async () => {
    const source = await Bun.file(mvpPath).text()
    const result = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source })
    if (!result.ok) throw new Error("fixture proposal failed")
    const designed = {
      ...result.document,
      riskModel: {
        ...result.document.riskModel,
        risks: [{ id: "RISK-LOCAL", statement: "The declared behavior may not be present." }],
      },
      environments: {
        ...result.document.environments,
        profiles: [{ id: "ENV-LOCAL", status: "proposed" as const }],
      },
      obligations: result.document.obligations.map((obligation) => ({
        ...obligation,
        domains: ["contract"],
        technique: "deterministic_test",
        environment_refs: ["ENV-LOCAL"],
        oracle: { statement: "The exact claim is observed.", evaluator_ref: "tests/oracle.test.ts" },
        falsifier: { kind: "known_bad_fixture", ref: "fixtures/known-bad.json", expected_verdict: "REFUTED" as const },
        evidence: { required_kinds: ["test_receipt"], proof_rung: "local_fixture" },
        independence: { producer_may_verify: false },
        activation_gate: "GATE-LOCAL",
      })),
      gates: [{ id: "GATE-LOCAL", expression: "all required obligations are ready" }],
      evidencePolicy: { ...result.document.evidencePolicy, policy_state: "designed" as const },
      authority: {
        ...result.document.authority,
        admitted_roles: ["assurance_reviewer"],
        verifier_roles: ["independent_verifier"],
        release_roles: ["release_owner"],
        policy_state: "designed" as const,
      },
    }
    const validation = validateAssuranceSpec(serializeAssuranceSpec(designed))
    expect(validation.valid).toBe(true)
    if (validation.document === undefined) throw new Error("designed fixture failed validation")
    expect(assessAssuranceSpec(validation.document)).toMatchObject({
      design_ready: true,
      coverage: { criteria: 18, obligations: 18, ready: 18, needs_design: 0 },
    })
    expect(validation.document.frontmatter.lifecycle_state).toBe("proposed")
  })

  test("rejects missing sections and dangling criterion references", async () => {
    const source = await Bun.file(mvpPath).text()
    const result = proposeAssuranceSpec({ productSpecPath: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md", productSpecMarkdown: source })
    if (!result.ok) throw new Error("fixture proposal failed")
    const missing = result.markdown.replace(/## Gates\n[\s\S]*?(?=\n## Evidence Policy)/, "")
    expect(validateAssuranceSpec(missing).errors[0]?.code).toBe("missing_required_section")
    const danglingDocument = { ...result.document, obligations: result.document.obligations.map((obligation, index) => index === 0 ? { ...obligation, criterion_refs: ["CW-AC-99"] } : obligation) }
    const dangling = validateAssuranceSpec(serializeAssuranceSpec(danglingDocument))
    expect(dangling.valid).toBe(false)
    expect(dangling.errors.map((diagnostic) => diagnostic.code)).toContain("dangling_source_ref")
  })
})
