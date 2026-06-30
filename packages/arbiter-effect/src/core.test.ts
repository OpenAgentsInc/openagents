import { describe, expect, test } from "bun:test"
import { Schema as S } from "effect"

import {
  GraphSpec,
  defaultGraphLayout,
  graphLinkPath,
  graphLinkStatusForRefs,
  graphNodeById,
  graphPinAnchor,
  isDereferenceableGraphRef,
  type GraphSpec as GraphSpecType,
} from "./core"

export const graphFixture = (): GraphSpecType => ({
  schemaVersion: "openagents.khala_code.gym_graph_projection.v0",
  title: "Fixture Arbiter graph",
  generatedAt: "time.fixture",
  status: "proposal_ready",
  nodes: [
    {
      id: "source",
      label: "source",
      kind: "operator_request",
      status: "complete",
      inputs: [],
      outputs: [
        {
          id: "out",
          name: "out",
          direction: "output",
          type: "fixture.output",
        },
      ],
      datum: [],
      evidenceRefs: ["trace.fixture.source.v1"],
      blockerRefs: [],
      caveatRefs: [],
      position: { x: 40, y: 70 },
    },
    {
      id: "sink",
      label: "sink",
      kind: "proposal",
      status: "proposal_ready",
      inputs: [
        {
          id: "in",
          name: "in",
          direction: "input",
          type: "fixture.output",
        },
      ],
      outputs: [],
      datum: [
        {
          label: "metric",
          value: 211,
          unit: "bps",
          evidenceRefs: ["eval_result.fixture.v1"],
        },
      ],
      evidenceRefs: ["action_submission.proposal.fixture.v1"],
      blockerRefs: [],
      caveatRefs: [],
      position: { x: 315, y: 70 },
    },
  ],
  links: [
    {
      id: "source-to-sink",
      label: "proposal",
      status: "evidence_backed",
      from: { nodeId: "source", pinId: "out" },
      to: { nodeId: "sink", pinId: "in" },
      evidenceRefs: ["action_submission.proposal.fixture.v1"],
      blockerRefs: [],
      caveatRefs: [],
    },
  ],
  evidenceRefs: [
    "trace.fixture.source.v1",
    "action_submission.proposal.fixture.v1",
  ],
  blockerRefs: [],
  caveatRefs: [],
  sourceRefs: ["github.issue.openagents.7761"],
})

describe("arbiter-effect core", () => {
  test("decodes a Khala Code projection-compatible graph spec", () => {
    const decoded = S.decodeUnknownSync(GraphSpec)(graphFixture())

    expect(decoded.schemaVersion).toBe(
      "openagents.khala_code.gym_graph_projection.v0",
    )
    expect(decoded.nodes.map(node => node.id)).toEqual(["source", "sink"])
    expect(decoded.links[0]?.status).toBe("evidence_backed")
  })

  test("rejects invalid node and link statuses", () => {
    const invalid = {
      ...graphFixture(),
      nodes: [{ ...graphFixture().nodes[0], status: "done" }],
    }

    expect(() => S.decodeUnknownSync(GraphSpec)(invalid)).toThrow()
  })

  test("computes stable node anchors and cubic link paths", () => {
    const spec = graphFixture()
    const nodes = graphNodeById(spec)
    const source = nodes.get("source")
    const link = spec.links[0]

    if (source === undefined || link === undefined) {
      throw new Error("fixture missing source/link")
    }

    expect(graphPinAnchor(source, "output")).toEqual({ x: 206, y: 109 })
    expect(graphLinkPath(link, nodes)).toBe(
      "M 206 109 C 278 109, 243 109, 315 109",
    )
    expect(defaultGraphLayout.width).toBe(1480)
  })

  test("classifies evidence-bound and blocked link states", () => {
    expect(graphLinkStatusForRefs([], [])).toBe("inactive")
    expect(graphLinkStatusForRefs(["eval_result.fixture.v1"], [])).toBe(
      "evidence_backed",
    )
    expect(graphLinkStatusForRefs(["eval_result.fixture.v1"], ["blocker.fixture"])).toBe(
      "blocked",
    )
  })

  test("keeps counter-only and unsafe refs from being dereferenceable", () => {
    expect(isDereferenceableGraphRef("eval_result.fixture.v1")).toBe(true)
    expect(isDereferenceableGraphRef("counter.khala_tokens_served.total=123")).toBe(
      false,
    )
    expect(isDereferenceableGraphRef("/Users/operator/private.json")).toBe(false)
  })
})
