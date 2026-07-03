import { describe, expect, test } from "bun:test"

import {
  buildQaSwarmBoardGraphSpec,
  QaSwarmBoardProjectionSchemaVersion,
  type QaSwarmBoardRunProjection,
} from "./qa-swarm"

const projection = (
  overrides: Partial<QaSwarmBoardRunProjection> = {},
): QaSwarmBoardRunProjection => ({
  coverageFrontier: [
    {
      current: 42,
      frontier: 58,
      label: "Seed corpus coverage",
      receiptRef: "coverage.qa_swarm.khala_code.seed_corpus.20260702",
    },
    {
      current: 17,
      frontier: 24,
      label: "Desktop state frontier",
      receiptRef: "frontier.qa_swarm.khala_code.desktop_state.20260702",
    },
  ],
  distilledTests: [
    {
      href: "/docs/qa/khala-code-mechanical-corpus",
      label: "Mechanical corpus",
      receiptRef: "test.qa_swarm.khala_code.mechanical_corpus.20260702",
    },
  ],
  generatedAt: "2026-07-02T17:00:00.000Z",
  perfBudgets: [
    {
      actualMs: 82,
      budgetMs: 100,
      label: "Thread switch p95",
      receiptRef: "perf.qa_swarm.khala_code.thread_switch_p95.20260702",
      verdict: "passed",
    },
  ],
  projectionRef: "projection.qa_swarm.run.khala_code.20260702",
  publicSafetyRefs: [
    "check.public_projection.qa_swarm_board.no_receipt_no_lit_edge",
  ],
  runRef: "qa-run.khala-code-nightly.2026-07-02",
  target: {
    label: "Khala Code Desktop",
    ref: "artifact.qa_swarm.target.opaque.customer_one",
    visibility: "opaque",
  },
  title: "Khala Code nightly QA swarm",
  traceRefs: ["trace.public.qa_swarm.khala_code.seed_corpus.20260702"],
  verdict: "warning",
  verdictWall: [
    {
      label: "Login and workspace routing",
      receiptRef: "artifact.qa_swarm.verdict.login_workspace.20260702",
      summary: "Core entrypoints passed.",
      verdict: "passed",
    },
    {
      label: "Desktop command palette",
      receiptRef: "artifact.qa_swarm.verdict.command_palette.20260702",
      summary: "Explorer found one warning.",
      verdict: "warning",
    },
  ],
  ...overrides,
})

describe("QA Swarm Arbiter board projection", () => {
  test("maps a QA run projection into Arbiter GraphSpec nodes and links", () => {
    const graph = buildQaSwarmBoardGraphSpec(projection())

    expect(graph.schemaVersion).toBe("openagents.arbiter.graph_spec.v0")
    expect(graph.sourceRefs).toContain(QaSwarmBoardProjectionSchemaVersion)
    expect(graph.nodes.map(node => node.id)).toEqual([
      "scenario-runner",
      "seeded-monkey",
      "llm-explorer",
      "perf-probe",
      "headed-ax",
      "target-surface",
      "oracle-families",
      "verdict-wall",
      "distiller",
      "public-safe-share",
    ])
    expect(graph.links.every(link => link.status === "evidence_backed")).toBe(
      true,
    )
  })

  test("does not light edges from missing or counter-only receipts", () => {
    const graph = buildQaSwarmBoardGraphSpec(
      projection({
        distilledTests: [],
        perfBudgets: [
          {
            actualMs: 1,
            budgetMs: 2,
            label: "Counter-only p95",
            receiptRef: "counter.khala_tokens_served.total=123",
            verdict: "passed",
          },
        ],
      }),
    )

    const perfLink = graph.links.find(link => link.id === "perf-to-target")
    const distilledLink = graph.links.find(link => link.id === "verdict-to-distiller")

    expect(perfLink?.status).toBe("inactive")
    expect(perfLink?.evidenceRefs).toEqual([])
    expect(distilledLink?.status).toBe("inactive")
    expect(JSON.stringify(graph)).not.toContain("counter.khala_tokens_served")
  })

  test("ignores unmodeled raw private fields", () => {
    const graph = buildQaSwarmBoardGraphSpec({
      ...projection(),
      rawPrompt: "Bearer sk-local /Users/operator/.codex/auth.json",
    } as unknown as QaSwarmBoardRunProjection)

    expect(JSON.stringify(graph)).not.toMatch(/Bearer|sk-local|\/Users\/|auth\.json/)
  })
})
