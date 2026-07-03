import { describe, expect, test } from "bun:test"

import {
  khalaCodeQaSwarmPanelStateFromRunProjection,
  khalaCodeQaSwarmPanelView,
} from "../src/ui/qa-swarm-panel"
import type { QaSwarmBoardRunProjection } from "@openagentsinc/arbiter-effect/qa-swarm"

const projection = (): QaSwarmBoardRunProjection => ({
  coverageFrontier: [
    {
      current: 42,
      frontier: 58,
      label: "Seed corpus coverage",
      receiptRef: "coverage.qa_swarm.khala_code.seed_corpus.desktop",
    },
    {
      current: 17,
      frontier: 24,
      label: "Desktop state frontier",
      receiptRef: "frontier.qa_swarm.khala_code.desktop_state.desktop",
    },
  ],
  distilledTests: [
    {
      href: "/docs/qa/khala-code-mechanical-corpus",
      label: "Mechanical corpus",
      receiptRef: "test.qa_swarm.khala_code.mechanical_corpus.desktop",
    },
  ],
  generatedAt: "2026-07-02T17:00:00.000Z",
  perfBudgets: [
    {
      actualMs: 82,
      budgetMs: 100,
      label: "Thread switch p95",
      receiptRef: "perf.qa_swarm.khala_code.thread_switch_p95.desktop",
      verdict: "passed",
    },
  ],
  projectionRef: "projection.qa_swarm.run.khala_code.desktop",
  publicSafetyRefs: [
    "check.public_projection.qa_swarm_board.no_receipt_no_lit_edge",
  ],
  runRef: "qa-run.khala-code-nightly.desktop",
  target: {
    label: "Khala Code Desktop",
    ref: "artifact.qa_swarm.target.opaque.customer_one",
    visibility: "opaque",
  },
  title: "Khala Code nightly QA swarm",
  traceRefs: ["trace.public.qa_swarm.khala_code.seed_corpus.desktop"],
  verdict: "warning",
  verdictWall: [
    {
      label: "Login and workspace routing",
      receiptRef: "artifact.qa_swarm.verdict.login_workspace.desktop",
      summary: "Core entrypoints passed.",
      verdict: "passed",
    },
    {
      label: "Desktop command palette",
      receiptRef: "artifact.qa_swarm.verdict.command_palette.desktop",
      summary: "Explorer found one warning.",
      verdict: "warning",
    },
  ],
})

describe("Khala Code QA Swarm panel", () => {
  test("consumes the shared QA Swarm Arbiter GraphSpec projection", () => {
    const state = khalaCodeQaSwarmPanelStateFromRunProjection(projection())

    expect(state.boardGraph.schemaVersion).toBe("openagents.arbiter.graph_spec.v0")
    expect(state.boardGraph.nodes.map(node => node.id)).toContain("scenario-runner")
    expect(state.boardGraph.links.find(link => link.id === "scenario-to-target")?.status)
      .toBe("evidence_backed")
  })

  test("exposes a read-only desktop panel view with accessible graph mirror", () => {
    const view = khalaCodeQaSwarmPanelView(
      khalaCodeQaSwarmPanelStateFromRunProjection(projection()),
    )

    expect(view).not.toBeNull()
    expect(JSON.stringify(view)).toContain("khala-code-qa-swarm-panel")
    expect(JSON.stringify(view)).toContain("QA Swarm board text mirror")
  })

  test("counter-only receipts remain inactive in the desktop projection", () => {
    const state = khalaCodeQaSwarmPanelStateFromRunProjection({
      ...projection(),
      perfBudgets: [
        {
          actualMs: 1,
          budgetMs: 2,
          label: "Counter-only p95",
          receiptRef: "counter.khala_tokens_served.total=123",
          verdict: "passed",
        },
      ],
    })

    expect(state.boardGraph.links.find(link => link.id === "perf-to-target")?.status)
      .toBe("inactive")
    expect(JSON.stringify(state)).not.toContain("counter.khala_tokens_served")
  })
})
