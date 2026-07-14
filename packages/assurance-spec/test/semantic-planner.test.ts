import { readFile } from "node:fs/promises"
import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"
import { resolve } from "node:path"

import {
  SEMANTIC_PLANNER_OUTPUT_VERSION,
  compileSemanticPlannerProposal,
  fixtureSemanticPlanner,
  prepareSemanticPlannerInput,
  runSemanticPlannerProposal,
  type ProductSpecSubject,
  type SemanticPlannerInput,
  type SemanticPlannerOutput,
} from "../src/index.ts"

const mvpPath = resolve(import.meta.dir, "../../../docs/mvp/openagents-codex-workroom-mvp.product-spec.md")
const subjectPath = "docs/mvp/openagents-codex-workroom-mvp.product-spec.md"
const acceptedSubject: ProductSpecSubject = {
  profile: "openagents_executable_v0.1_exact_document",
  path: subjectPath,
  spec_format_version: "0.1",
  spec_revision: 6,
  document_digest: "sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1",
  criterion_refs: Array.from({ length: 18 }, (_, index) => `CW-AC-${String(index + 1).padStart(2, "0")}`),
}

const prepare = async (): Promise<SemanticPlannerInput> => {
  const result = prepareSemanticPlannerInput({
    acceptedSubject,
    productSpecPath: subjectPath,
    productSpecMarkdown: await readFile(mvpPath, "utf8"),
  })
  if (!result.ok) throw new Error(result.diagnostics.map((entry) => entry.code).join(", "))
  return result.input
}

const fixtureOutput = (input: SemanticPlannerInput): SemanticPlannerOutput =>
  Effect.runSync(fixtureSemanticPlanner(input)) as SemanticPlannerOutput

const compile = (input: SemanticPlannerInput, output: unknown) =>
  compileSemanticPlannerProposal(input, output, { author: "Observer fixture" })

const designedOutput = (input: SemanticPlannerInput): SemanticPlannerOutput => ({
  format_version: SEMANTIC_PLANNER_OUTPUT_VERSION,
  input_digest: input.input_digest,
  subject: input.subject,
  criterion_dispositions: input.criteria.map((criterion) => ({
    state: "designed" as const,
    criterion_ref: criterion.criterion_ref,
    obligation_id: `AO-${criterion.criterion_ref}-SEMANTIC`,
    title: `Prove ${criterion.criterion_ref}`,
    candidate_artifact_refs: ["packages/assurance-spec/test/semantic-planner.test.ts"],
    domains: ["contract"],
    technique: "deterministic_test",
    environment_refs: ["ENV-OBSERVER-FIXTURE"],
    oracle: {
      statement: "The exact source-bound behavior is observed.",
      evaluator_ref: "packages/assurance-spec/test/semantic-planner.test.ts",
    },
    falsifier: {
      kind: "known_bad_fixture",
      ref: "packages/assurance-spec/conformance/invalid/missing-frontmatter.assurance-spec.md",
      expected_verdict: "REFUTED" as const,
    },
    evidence: { required_kinds: ["test_receipt"], proof_rung: "deterministic_fixture" },
    independence: { producer_may_verify: false },
    dependency_refs: [],
    activation_gate: "GATE-OBSERVER-FIXTURE",
  })),
  risks: [{ id: "RISK-SEMANTIC-DRIFT", statement: "A proof design may drift from the accepted product intent." }],
  environments: [{ id: "ENV-OBSERVER-FIXTURE", status: "proposed" }],
  gates: [{ id: "GATE-OBSERVER-FIXTURE", expression: "every exact semantic obligation has qualifying evidence" }],
  evidence_policy: {
    required_for_ready_obligation: ["oracle_observation", "falsifier_observation", "environment_binding", "independent_review"],
    policy_state: "designed",
  },
  proposed_roles: {
    admitted_roles: ["assurance_reviewer"],
    verifier_roles: ["independent_verifier"],
    release_roles: ["release_owner"],
    policy_state: "designed",
  },
})

describe("Observer semantic planner boundary", () => {
  test("requires an explicit accepted identity that exactly matches the ProductSpec bytes", async () => {
    const markdown = await readFile(mvpPath, "utf8")
    const drifted = prepareSemanticPlannerInput({
      acceptedSubject: { ...acceptedSubject, spec_revision: acceptedSubject.spec_revision + 1 },
      productSpecPath: subjectPath,
      productSpecMarkdown: markdown,
    })
    expect(drifted.ok).toBe(false)
    if (!drifted.ok) expect(drifted.diagnostics[0]?.code).toBe("semantic_planner_subject_drift")

    const pathDrift = prepareSemanticPlannerInput({
      acceptedSubject,
      productSpecPath: "docs/mvp/different.product-spec.md",
      productSpecMarkdown: markdown,
    })
    expect(pathDrift.ok).toBe(false)
    if (!pathDrift.ok) expect(pathDrift.diagnostics[0]?.code).toBe("semantic_planner_subject_drift")

    const invalid = prepareSemanticPlannerInput({
      acceptedSubject: { ...acceptedSubject, criterion_refs: [] } as ProductSpecSubject,
      productSpecPath: subjectPath,
      productSpecMarkdown: markdown,
      repositoryInventory: { state: "clean" } as never,
    })
    expect(invalid.ok).toBe(false)
    if (!invalid.ok) expect(invalid.diagnostics[0]?.code).toBe("invalid_semantic_planner_input")
  })

  test("runs an injected provider-neutral fixture planner and preserves needs-design", async () => {
    const input = await prepare()
    const result = Effect.runSync(runSemanticPlannerProposal(input, fixtureSemanticPlanner, { author: "Observer fixture" }))
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.document.frontmatter.lifecycle_state).toBe("proposed")
    expect(result.adequacy.coverage).toEqual({ criteria: 18, obligations: 18, ready: 0, needs_design: 18 })
    expect(result.document.authority).toMatchObject({
      proposal_may_self_admit: false,
      proposal_may_execute: false,
      proposal_may_verify: false,
      proposal_may_release: false,
      proposal_may_change_public_promises: false,
    })

    const unavailable = Effect.runSync(runSemanticPlannerProposal(
      input,
      () => Effect.fail("provider unavailable"),
      { author: "Observer fixture" },
    ))
    expect(unavailable.ok).toBe(false)
    if (!unavailable.ok) expect(unavailable.diagnostics[0]?.code).toBe("semantic_planner_unavailable")
  })

  test("rejects missing, duplicate, and stale criterion dispositions", async () => {
    const input = await prepare()
    const base = fixtureOutput(input)
    const cases = [
      [{ ...base, criterion_dispositions: base.criterion_dispositions.slice(1) }, "semantic_planner_missing_disposition"],
      [{ ...base, criterion_dispositions: [...base.criterion_dispositions, base.criterion_dispositions[0]!] }, "semantic_planner_duplicate_disposition"],
      [{ ...base, criterion_dispositions: base.criterion_dispositions.map((entry, index) => index === 0 ? { ...entry, criterion_ref: "CW-AC-99" } : entry) }, "semantic_planner_stale_criterion"],
    ] as const
    for (const [output, expectedCode] of cases) {
      const result = compile(input, output)
      expect(result.ok).toBe(false)
      if (!result.ok) expect(result.diagnostics[0]?.code).toBe(expectedCode)
    }
  })

  test("rejects input/output binding drift and malformed designed proof", async () => {
    const input = await prepare()
    const output = fixtureOutput(input)
    const forgedInput = { ...input, risk_source_snapshot: `${input.risk_source_snapshot} invented` }
    const inputDrift = compile(forgedInput, output)
    expect(inputDrift.ok).toBe(false)
    if (!inputDrift.ok) expect(inputDrift.diagnostics[0]?.code).toBe("semantic_planner_input_drift")

    const outputDrift = compile(input, { ...output, input_digest: `sha256:${"0".repeat(64)}` })
    expect(outputDrift.ok).toBe(false)
    if (!outputDrift.ok) expect(outputDrift.diagnostics[0]?.code).toBe("semantic_planner_output_drift")

    const outputSubjectDrift = compile(input, {
      ...output,
      subject: { ...output.subject, spec_revision: output.subject.spec_revision + 1 },
    })
    expect(outputSubjectDrift.ok).toBe(false)
    if (!outputSubjectDrift.ok) expect(outputSubjectDrift.diagnostics[0]?.code).toBe("semantic_planner_output_drift")

    const malformed = designedOutput(input)
    const first = malformed.criterion_dispositions[0]!
    const withoutOracle = { ...first } as Record<string, unknown>
    delete withoutOracle.oracle
    const weak = compile(input, {
      ...malformed,
      criterion_dispositions: [withoutOracle, ...malformed.criterion_dispositions.slice(1)],
    })
    expect(weak.ok).toBe(false)
    if (!weak.ok) expect(weak.diagnostics[0]?.code).toBe("invalid_semantic_planner_output")
  })

  test("rejects self-verification and weak seam claims", async () => {
    const input = await prepare()
    const base = designedOutput(input)
    const first = base.criterion_dispositions[0]!
    if (first.state !== "designed") throw new Error("designed fixture mismatch")
    const selfVerified = compile(input, {
      ...base,
      criterion_dispositions: [{ ...first, independence: { producer_may_verify: true } }, ...base.criterion_dispositions.slice(1)],
    })
    expect(selfVerified.ok).toBe(false)
    if (!selfVerified.ok) expect(selfVerified.diagnostics[0]?.code).toBe("semantic_planner_weak_proof")

    const weakSeam = compile(input, {
      ...base,
      criterion_dispositions: [{ ...first, domains: ["seam"] }, ...base.criterion_dispositions.slice(1)],
    })
    expect(weakSeam.ok).toBe(false)
    if (!weakSeam.ok) expect(weakSeam.diagnostics[0]?.code).toBe("semantic_planner_weak_proof")
  })

  test("compiles a complete design as proposed-only and byte-deterministically", async () => {
    const input = await prepare()
    const output = designedOutput(input)
    const inventedSnapshot = {
      ...output,
      criterion_dispositions: output.criterion_dispositions.map((entry) => ({
        ...entry,
        source_claim_snapshot: "invented planner prose",
        source_claim_digest: `sha256:${"0".repeat(64)}`,
      })),
    }
    const first = compile(input, inventedSnapshot)
    const second = compile(input, JSON.parse(JSON.stringify(inventedSnapshot)))
    expect(first.ok && second.ok).toBe(true)
    if (!first.ok || !second.ok) return
    expect(first.markdown).toBe(second.markdown)
    expect(first.document.obligations[0]?.source_claim_snapshot).toBe(input.criteria[0]?.source_claim_snapshot)
    expect(first.document.obligations[0]?.source_claim_digest).toBe(input.criteria[0]?.source_claim_digest)
    expect(first.document.obligations[0]?.source_claim_snapshot).not.toBe("invented planner prose")
    expect(first.adequacy.coverage).toEqual({ criteria: 18, obligations: 18, ready: 18, needs_design: 0 })
    expect(first.document.frontmatter.lifecycle_state).toBe("proposed")
    expect(first.document.authority.proposal_may_self_admit).toBe(false)
    expect(first.document.authority.proposal_may_execute).toBe(false)
  })
})
