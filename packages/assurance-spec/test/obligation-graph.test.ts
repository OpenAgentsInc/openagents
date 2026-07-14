import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Obligation dependency-graph projection (#8761, GAP_ANALYSIS §6 / §11 item 5):
 * cycle detection with the designed stable codes, the pure designable-now vs
 * blocked vs gated projection, and its CLI/MCP exposure.
 */
import { describe, expect, test } from "vite-plus/test"
import { createHash } from "node:crypto"
import { readFileSync, writeFileSync } from "node:fs"
import { join, resolve } from "node:path"

import {
  ASSURANCE_SECTION_LABELS,
  ASSURANCE_SPEC_FORMAT_VERSION,
  MANDATORY_ASSURANCE_SECTION_IDS,
  absentRepositoryInventory,
  analyzeObligationDependencies,
  getObligationGraph,
  getObligations,
  handleMcpRequest,
  parseAssuranceSpec,
  parseAssuranceSpecDocument,
  projectObligationGraph,
  runTool,
  serializeAssuranceSpec,
  validateAssuranceSpec,
  type AssuranceGate,
  type AssuranceObligation,
  type AssuranceSpecDocument,
  type ObligationGraph,
  type ToolOutcome,
} from "../src/index.ts"
import { MVP_SPEC, makeFixtureRoot, repoRoot } from "./fixture.ts"

const ok = <A>(outcome: ToolOutcome<A>): A => {
  if (!outcome.ok) throw new Error(`expected success, received ${outcome.code}: ${outcome.message}`)
  return outcome.value
}

const err = <A>(outcome: ToolOutcome<A>): { code: string; message: string } => {
  if (outcome.ok) throw new Error("expected failure, received success")
  return outcome
}

const sha256 = (value: string): string =>
  `sha256:${createHash("sha256").update(value, "utf8").digest("hex")}`

type ObligationSeed = Readonly<{
  dependency_refs?: ReadonlyArray<string>
  activation_gate?: string
}>

const obligationId = (index: number): string => `AO-EX-AC-${index}-01`

/**
 * Minimal, structurally valid document: one criterion per obligation, plus
 * whatever dependencies and gates the seed declares.
 */
const makeDocument = (seeds: ReadonlyArray<ObligationSeed>): AssuranceSpecDocument => {
  const obligations: AssuranceObligation[] = seeds.map((seed, position) => {
    const index = position + 1
    const criterion = `EX-AC-${index}`
    const claim = `Example criterion ${index} holds.`
    return {
      id: obligationId(index),
      title: `Assure ${criterion}`,
      criterion_refs: [criterion],
      source_claim_snapshot: claim,
      source_claim_digest: sha256(`${criterion}\n${claim}`),
      disposition: "required",
      candidate_artifact_refs: [],
      ...(seed.dependency_refs === undefined ? {} : { dependency_refs: seed.dependency_refs }),
      ...(seed.activation_gate === undefined ? {} : { activation_gate: seed.activation_gate }),
    }
  })
  const gates: AssuranceGate[] = [...new Set(
    seeds.flatMap((seed) => (seed.activation_gate === undefined ? [] : [seed.activation_gate])),
  )].map((id) => ({ id, expression: "all required obligations are ready" }))
  const risks = "The declared example behavior may not be present."
  return {
    frontmatter: {
      assurance_spec_format_version: ASSURANCE_SPEC_FORMAT_VERSION,
      assurance_spec_id: "assurance.test.obligation.graph",
      assurance_revision: 1,
      title: "Obligation graph test spec",
      artifact_type: "product_assurance",
      lifecycle_state: "proposed",
      author: "obligation-graph tests",
    },
    unknownFrontmatter: [],
    customSections: [],
    sections: MANDATORY_ASSURANCE_SECTION_IDS.map((id) => ({
      id,
      label: ASSURANCE_SECTION_LABELS[id],
      content: "",
    })),
    subject: {
      product_spec: {
        profile: "openagents_executable_v0.1_exact_document",
        path: "docs/example.product-spec.md",
        spec_format_version: "0.1",
        spec_revision: 1,
        document_digest: sha256("obligation graph test subject placeholder"),
        criterion_refs: seeds.map((_, position) => `EX-AC-${position + 1}`),
      },
    },
    riskModel: { source_snapshot: risks, source_digest: sha256(risks), risks: [] },
    environments: { profiles: [], repository_inventory: absentRepositoryInventory() },
    obligations,
    gates,
    evidencePolicy: {
      links_are_verdicts: false,
      missing_evidence_verdict: "INCONCLUSIVE",
      required_for_ready_obligation: ["oracle_observation", "falsifier_observation", "environment_binding", "independent_review"],
      policy_state: "needs_design",
    },
    authority: {
      proposal_may_self_admit: false,
      proposal_may_execute: false,
      proposal_may_verify: false,
      proposal_may_release: false,
      proposal_may_change_public_promises: false,
      admitted_roles: [],
      verifier_roles: [],
      release_roles: [],
      policy_state: "needs_design",
    },
  }
}

const errorCodes = (markdown: string): ReadonlyArray<string> =>
  validateAssuranceSpec(markdown).errors.map((error) => error.code)

describe("dependency cycle detection (stable structural codes)", () => {
  test("a dependency cycle fails validation with cyclic_obligation_dependency", () => {
    const document = makeDocument([
      { dependency_refs: [obligationId(2)] },
      { dependency_refs: [obligationId(3)] },
      { dependency_refs: [obligationId(1)] },
    ])
    const validation = validateAssuranceSpec(serializeAssuranceSpec(document))
    expect(validation.valid).toBe(false)
    expect(validation.errors.map((error) => error.code)).toEqual(["cyclic_obligation_dependency"])
    expect(validation.errors[0]?.message).toBe(
      `Obligation dependency cycle among: ${obligationId(1)}, ${obligationId(2)}, ${obligationId(3)}.`,
    )
    expect(validation.errors[0]?.path).toBe("obligations")
    expect(validation.errors[0]?.obligation_id).toBe(obligationId(1))
  })

  test("the crafted conformance corpus fixture is invalid with exactly the cycle code", () => {
    const fixture = resolve(
      import.meta.dirname,
      "../conformance/invalid/cyclic-obligation-dependency.assurance-spec.md",
    )
    const markdown = readFileSync(fixture, "utf8")
    const validation = validateAssuranceSpec(markdown)
    expect(validation.valid).toBe(false)
    expect(validation.errors.map((error) => error.code)).toEqual(["cyclic_obligation_dependency"])
    // The fixture is committed in canonical serialized form. Referential
    // integrity now runs at parse time (#8760), so the strict parse throws on
    // the cycle; the lenient parse still yields the document for byte checks.
    expect(serializeAssuranceSpec(parseAssuranceSpecDocument(markdown).document)).toBe(markdown)
  })

  test("a self-dependency fails validation with self_obligation_dependency", () => {
    const document = makeDocument([{ dependency_refs: [obligationId(1)] }])
    const codes = errorCodes(serializeAssuranceSpec(document))
    expect(codes).toEqual(["self_obligation_dependency"])
  })

  test("a dependency on an unknown obligation fails validation with dangling_dependency_ref", () => {
    const document = makeDocument([{ dependency_refs: ["AO-NOPE-01"] }])
    const validation = validateAssuranceSpec(serializeAssuranceSpec(document))
    expect(validation.valid).toBe(false)
    expect(validation.errors.map((error) => error.code)).toEqual(["dangling_dependency_ref"])
    expect(validation.errors[0]?.path).toBe(`obligations.${obligationId(1)}.dependency_refs`)
  })

  test("two independent cycles are each reported once, in document order", () => {
    const analysis = analyzeObligationDependencies(makeDocument([
      { dependency_refs: [obligationId(2)] },
      { dependency_refs: [obligationId(1)] },
      { dependency_refs: [obligationId(4)] },
      { dependency_refs: [obligationId(3)] },
    ]).obligations)
    expect(analysis.cycles).toEqual([
      [obligationId(1), obligationId(2)],
      [obligationId(3), obligationId(4)],
    ])
    expect(analysis.issues.map((issue) => issue.code))
      .toEqual(["cyclic_obligation_dependency", "cyclic_obligation_dependency"])
  })

  test("duplicate dependency refs are deduplicated without becoming a false cycle", () => {
    const analysis = analyzeObligationDependencies(makeDocument([
      {},
      { dependency_refs: [obligationId(1), obligationId(1)] },
    ]).obligations)
    expect(analysis.edges).toEqual([{ from: obligationId(2), to: obligationId(1) }])
    expect(analysis.issues).toHaveLength(0)
  })

  test("empty dependency_refs stays valid: no dependencies is a legitimate state", () => {
    const document = makeDocument([{ dependency_refs: [] }, {}])
    expect(validateAssuranceSpec(serializeAssuranceSpec(document)).valid).toBe(true)
  })
})

describe("designable-now vs blocked vs gated projection (pure, deterministic)", () => {
  test("diamond dependencies project waits_on and a dependency-respecting design_order", () => {
    const graph = projectObligationGraph(makeDocument([
      {},
      { dependency_refs: [obligationId(1)] },
      { dependency_refs: [obligationId(1)] },
      { dependency_refs: [obligationId(2), obligationId(3)] },
    ]))
    expect(graph.designable_now).toEqual([obligationId(1)])
    expect(graph.blocked).toEqual([
      { obligation_id: obligationId(2), waits_on: [obligationId(1)] },
      { obligation_id: obligationId(3), waits_on: [obligationId(1)] },
      { obligation_id: obligationId(4), waits_on: [obligationId(2), obligationId(3)] },
    ])
    expect(graph.gated).toEqual([])
    expect(graph.design_order).toEqual([obligationId(1), obligationId(2), obligationId(3), obligationId(4)])
    expect(graph.edges).toEqual([
      { from: obligationId(2), to: obligationId(1) },
      { from: obligationId(3), to: obligationId(1) },
      { from: obligationId(4), to: obligationId(2) },
      { from: obligationId(4), to: obligationId(3) },
    ])
    expect(graph.warnings).toEqual([])
    expect(graph.message).toContain("proof-design order")
  })

  test("activation gates project as gated; unresolved dependencies win over gates", () => {
    const graph = projectObligationGraph(makeDocument([
      { activation_gate: "GATE-EX" },
      { dependency_refs: [obligationId(1)], activation_gate: "GATE-EX" },
      {},
    ]))
    expect(graph.gated).toEqual([{ obligation_id: obligationId(1), activation_gate: "GATE-EX" }])
    expect(graph.blocked).toEqual([{ obligation_id: obligationId(2), waits_on: [obligationId(1)] }])
    expect(graph.designable_now).toEqual([obligationId(3)])
    expect(graph.nodes.map((node) => node.status)).toEqual(["gated", "blocked", "designable_now"])
    expect(graph.nodes[1]?.activation_gate).toBe("GATE-EX")
    // The three statuses partition the obligations; nothing is double-counted.
    expect(graph.designable_now.length + graph.blocked.length + graph.gated.length).toBe(3)
  })

  test("topological design_order is stable: document order among independents, declared order in waits_on", () => {
    const document = makeDocument([
      {},
      {},
      { dependency_refs: [obligationId(2), obligationId(1)] },
    ])
    const graph = projectObligationGraph(document)
    expect(graph.design_order).toEqual([obligationId(1), obligationId(2), obligationId(3)])
    expect(graph.blocked[0]?.waits_on).toEqual([obligationId(2), obligationId(1)])
    expect(JSON.stringify(projectObligationGraph(document))).toBe(JSON.stringify(graph))
  })

  test("obligations inside or downstream of a cycle are omitted from design_order with a typed warning", () => {
    const graph = projectObligationGraph(makeDocument([
      { dependency_refs: [obligationId(2)] },
      { dependency_refs: [obligationId(1)] },
      { dependency_refs: [obligationId(1)] },
    ]))
    expect(graph.design_order).toEqual([])
    expect(graph.warnings.map((warning) => warning.code)).toEqual(["cyclic_obligation_dependency"])
    expect(graph.warnings[0]?.cycle).toEqual([obligationId(1), obligationId(2)])
  })

  test("reports no blended score or percentage (Law 7)", () => {
    const graph = projectObligationGraph(makeDocument([{}, { dependency_refs: [obligationId(1)] }]))
    expect(Object.keys(graph)).toEqual([
      "designable_now",
      "blocked",
      "gated",
      "design_order",
      "nodes",
      "edges",
      "warnings",
      "message",
    ])
    expect(JSON.stringify(graph)).not.toMatch(/"(?:[a-z_]*(?:percent|score|blended)[a-z_]*)"\s*:/)
  })

  test("dependency_refs round-trip through serialize and parse byte-identically", () => {
    const document = makeDocument([
      {},
      { dependency_refs: [obligationId(1)], activation_gate: "GATE-EX" },
    ])
    const markdown = serializeAssuranceSpec(document)
    const parsed = parseAssuranceSpec(markdown)
    expect(parsed.obligations[1]?.dependency_refs).toEqual([obligationId(1)])
    expect(serializeAssuranceSpec(parsed)).toBe(markdown)
    expect(projectObligationGraph(parsed)).toEqual(projectObligationGraph(document))
  })
})

describe("handler exposure (shared by CLI and MCP)", () => {
  const writeSpec = (root: string, name: string, document: AssuranceSpecDocument): string => {
    const path = `docs/mvp/${name}`
    writeFileSync(join(root, path), serializeAssuranceSpec(document))
    return path
  }

  test("get_obligation_graph on the MVP spec: 18 designable now, none blocked, none gated", () => {
    const graph = ok(runTool(getObligationGraph({ root: repoRoot, path: MVP_SPEC })))
    expect(graph.designable_now).toHaveLength(18)
    expect(graph.blocked).toEqual([])
    expect(graph.gated).toEqual([])
    expect(graph.edges).toEqual([])
    expect(graph.design_order).toHaveLength(18)
    expect(graph.design_order[3]).toBe("AO-CW-AC-04-01")
  })

  test("get_obligation_graph is deterministic and fails typed on a cyclic spec", () => {
    const root = makeFixtureRoot()
    const path = writeSpec(root, "graph.assurance-spec.md", makeDocument([
      {},
      { dependency_refs: [obligationId(1)] },
    ]))
    const first = ok(runTool(getObligationGraph({ root, path })))
    const second = ok(runTool(getObligationGraph({ root, path })))
    expect(JSON.stringify(first)).toBe(JSON.stringify(second))
    expect(first.blocked).toEqual([{ obligation_id: obligationId(2), waits_on: [obligationId(1)] }])

    const cyclic = writeSpec(root, "cyclic.assurance-spec.md", makeDocument([
      { dependency_refs: [obligationId(2)] },
      { dependency_refs: [obligationId(1)] },
    ]))
    expect(err(runTool(getObligationGraph({ root, path: cyclic }))).code).toBe("cyclic_obligation_dependency")
  })

  test("obligation summaries expose declared dependency_refs and activation_gate", () => {
    const root = makeFixtureRoot()
    const path = writeSpec(root, "graph.assurance-spec.md", makeDocument([
      {},
      { dependency_refs: [obligationId(1)], activation_gate: "GATE-EX" },
    ]))
    const summaries = ok(runTool(getObligations({ root, path })))
    expect(summaries[0]?.dependency_refs).toEqual([])
    expect(summaries[0]?.activation_gate).toBeNull()
    expect(summaries[1]?.dependency_refs).toEqual([obligationId(1)])
    expect(summaries[1]?.activation_gate).toBe("GATE-EX")
  })

  test("the MCP get_obligation_graph tool returns the same projection", () => {
    const response = handleMcpRequest(
      {
        jsonrpc: "2.0",
        id: 1,
        method: "tools/call",
        params: { name: "get_obligation_graph", arguments: { path: MVP_SPEC } },
      },
      repoRoot,
    )
    if (response === null || response.error !== undefined) {
      throw new Error(`tools/call failed: ${JSON.stringify(response)}`)
    }
    const result = response.result as { content: Array<{ type: string; text: string }> }
    const graph = JSON.parse(result.content[0]!.text) as ObligationGraph
    expect(graph.designable_now).toHaveLength(18)
    expect(graph.blocked).toEqual([])
    expect(graph).toEqual(ok(runTool(getObligationGraph({ root: repoRoot, path: MVP_SPEC }))))
  })
})

describe("CLI graph command", () => {
  const cli = resolve(import.meta.dirname, "../src/cli.ts")

  const run = (
    args: ReadonlyArray<string>,
    cwd: string = repoRoot,
  ): Readonly<{ exitCode: number; stdout: string; stderr: string }> => {
    const result = Runtime.spawnSync([process.execPath, "--import", "tsx", cli, ...args], { cwd, stdout: "pipe", stderr: "pipe" })
    return {
      exitCode: result.exitCode,
      stdout: result.stdout.toString("utf8"),
      stderr: result.stderr.toString("utf8"),
    }
  }

  test("graph --json is deterministic and matches the shared handler", () => {
    const first = run(["graph", MVP_SPEC, "--json"])
    const second = run(["graph", MVP_SPEC, "--json"])
    expect(first.exitCode).toBe(0)
    expect(first.stdout).toBe(second.stdout)
    const graph = JSON.parse(first.stdout) as ObligationGraph
    expect(graph.designable_now).toHaveLength(18)
    expect(graph.design_order).toHaveLength(18)
  })

  test("graph human output names blocked and gated obligations without a blended score", () => {
    const root = makeFixtureRoot()
    const document = makeDocument([
      {},
      { dependency_refs: [obligationId(1)] },
      { activation_gate: "GATE-EX" },
    ])
    writeFileSync(join(root, "docs/mvp/graph.assurance-spec.md"), serializeAssuranceSpec(document))
    const result = run(["graph", "docs/mvp/graph.assurance-spec.md", "--root", root])
    expect(result.exitCode).toBe(0)
    expect(result.stdout).toContain(`designable_now (1): ${obligationId(1)}`)
    expect(result.stdout).toContain(`blocked ${obligationId(2)} waits_on ${obligationId(1)}`)
    expect(result.stdout).toContain(`gated ${obligationId(3)} gate=GATE-EX`)
    expect(result.stdout).toContain(`design_order: ${obligationId(1)} -> ${obligationId(2)} -> ${obligationId(3)}`)
    expect(result.stdout).not.toMatch(/\d+(?:\.\d+)?\s*%/)
  })

  test("graph exit codes: 2 without a path, 1 with the stable code on a cyclic spec", () => {
    expect(run(["graph"]).exitCode).toBe(2)
    const root = makeFixtureRoot()
    const document = makeDocument([
      { dependency_refs: [obligationId(2)] },
      { dependency_refs: [obligationId(1)] },
    ])
    writeFileSync(join(root, "docs/mvp/cyclic.assurance-spec.md"), serializeAssuranceSpec(document))
    const failure = run(["graph", "docs/mvp/cyclic.assurance-spec.md", "--root", root])
    expect(failure.exitCode).toBe(1)
    expect(failure.stderr).toContain("cyclic_obligation_dependency")
  })
})
