import { describe, expect, test } from "bun:test"

import {
  CLAUDE_PLAN_FANOUT_DAG_SCHEMA,
  CLAUDE_PLAN_FANOUT_REVIEW_SCHEMA,
  claudePlanFanoutDagToWorkSource,
  claudePlanFanoutPlanModeInstructions,
  claudePlanFanoutReviewAdvisorySignal,
  decodeClaudePlanFanoutDag,
  decodeClaudePlanFanoutReview,
} from "../src/bun/claude-plan-fanout.js"

const validDag = () => ({
  schema: CLAUDE_PLAN_FANOUT_DAG_SCHEMA,
  planRef: "plan.t9_4.fixture",
  source: "claude_plan_mode",
  generatedAt: "2026-07-02T10:00:00.000Z",
  objective: "Plan a bounded implementation of public issue #7873.",
  repo: "OpenAgentsInc/openagents",
  branch: "main",
  baseCommit: "0123456789abcdef0123456789abcdef01234567",
  verify: "bun test clients/khala-code-desktop/tests/claude-plan-fanout.test.ts",
  evidenceRefs: ["issue#7873", "docs/fable/ROADMAP.md"],
  nodes: [
    {
      nodeRef: "contract",
      title: "Define typed contract",
      objective: "Add schema-backed Claude plan fan-out contracts.",
      issue: 7873,
      evidenceRefs: ["docs/fable/ROADMAP.md"],
    },
    {
      nodeRef: "adapter",
      title: "Wire FleetRun adapter",
      objective: "Convert the typed plan DAG into FleetRun work units.",
      dependsOn: ["contract"],
    },
  ],
})

describe("Claude plan-then-fan-out contract", () => {
  test("decodes valid plan-mode JSON and maps it to a plan_dag work source", () => {
    const dag = decodeClaudePlanFanoutDag(validDag())
    const source = claudePlanFanoutDagToWorkSource(dag)

    expect(source).toMatchObject({
      kind: "plan_dag",
      planRef: "plan.t9_4.fixture",
      repo: "OpenAgentsInc/openagents",
      baseCommit: "0123456789abcdef0123456789abcdef01234567",
      verify: "bun test clients/khala-code-desktop/tests/claude-plan-fanout.test.ts",
    })
    expect(source.nodes.map(node => ({
      ref: node.ref,
      dependsOn: node.dependsOn,
      objective: node.objective,
    }))).toEqual([
      {
        ref: "contract",
        dependsOn: undefined,
        objective: "Add schema-backed Claude plan fan-out contracts.",
      },
      {
        ref: "adapter",
        dependsOn: ["contract"],
        objective: "Convert the typed plan DAG into FleetRun work units.",
      },
    ])
  })

  test("rejects prose-only, cyclic, unknown-dependency, and unsafe plan output", () => {
    expect(() => decodeClaudePlanFanoutDag("first do A, then B")).toThrow()

    expect(() => decodeClaudePlanFanoutDag({
      ...validDag(),
      nodes: [
        { nodeRef: "a", title: "A", objective: "A.", dependsOn: ["b"] },
        { nodeRef: "b", title: "B", objective: "B.", dependsOn: ["a"] },
      ],
    })).toThrow(/cycle/)

    expect(() => decodeClaudePlanFanoutDag({
      ...validDag(),
      nodes: [
        { nodeRef: "a", title: "A", objective: "A.", dependsOn: ["missing"] },
      ],
    })).toThrow(/unknown node/)

    expect(() => decodeClaudePlanFanoutDag({
      ...validDag(),
      nodes: [
        { nodeRef: "secret", title: "Secret", objective: "Read /Users/operator/.secrets/token." },
      ],
    })).toThrow(/public-safe/)
  })

  test("decodes Claude review verdicts as advisory signals only", () => {
    const review = decodeClaudePlanFanoutReview({
      schema: CLAUDE_PLAN_FANOUT_REVIEW_SCHEMA,
      reviewRef: "review.t9_4.1",
      planRef: "plan.t9_4.fixture",
      generatedAt: "2026-07-02T10:05:00.000Z",
      verdict: "request_changes",
      summary: "The adapter node needs a dependency-ordering regression.",
      targetNodeRefs: ["adapter"],
      changeRequests: ["Add a regression where the dependent node waits for root closeout."],
      evidenceRefs: ["pr#7994"],
    }, { knownNodeRefs: ["contract", "adapter"] })

    expect(claudePlanFanoutReviewAdvisorySignal(review)).toEqual({
      advisory: true,
      controlFlowAuthority: "khala.fleet.delegate",
      deterministicGateRequired: true,
      planRef: "plan.t9_4.fixture",
      reviewRef: "review.t9_4.1",
      targetNodeRefs: ["adapter"],
      verdict: "request_changes",
    })
    expect(() => decodeClaudePlanFanoutReview({
      ...review,
      targetNodeRefs: ["missing"],
    }, { knownNodeRefs: ["contract", "adapter"] })).toThrow(/unknown node/)
  })

  test("plan-mode instructions name the schema and deterministic authority", () => {
    const instructions = claudePlanFanoutPlanModeInstructions()
    expect(instructions).toContain(CLAUDE_PLAN_FANOUT_DAG_SCHEMA)
    expect(instructions).toContain("Do not edit files")
    expect(instructions).toContain("deterministic FleetRun supervision owns control flow")
  })
})
