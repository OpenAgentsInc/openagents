import { describe, expect, it } from "vitest"

import { renderToString } from "@openagentsinc/effuse"

import { renderDeck } from "../../src/effuse-deck/render"
import type { DeckDocument } from "../../src/effuse-deck/dsl"

describe("apps/web deck: Graph node rendering", () => {
  it("renders positioned Graph nodes + routed edges with a stable viewBox", () => {
    const doc: DeckDocument = {
      dsl: "effuse.slide-deck",
      version: "0.1.0",
      meta: { title: "Deck Test" },
      deck: {
        aspectRatio: "16:9",
        slides: [
          {
            id: "title",
            content: [
              {
                type: "Graph",
                props: { opacity: 0.55, preset: "dots-slow" },
                children: [
                  { type: "GraphNode", props: { nodeId: "a", label: "Runtime", nodeType: "leaf", x: "15%", y: "12%" } },
                  { type: "GraphNode", props: { nodeId: "b", label: "Compiler", nodeType: "leaf", x: "78%", y: "16%" } },
                  { type: "GraphNode", props: { nodeId: "c", label: "Autopilot", nodeType: "root", anchor: "center", x: "50%", y: "52%" } },
                  { type: "GraphEdge", props: { from: "c", to: "a" } },
                  { type: "GraphEdge", props: { from: "c", to: "b" } },
                ],
              },
              { type: "Text", props: { style: "h1" }, children: ["OpenAgents"] },
            ],
          },
        ],
      },
    }

    const out = renderDeck({ doc, slideIndex: 0, stepIndex: 1, presenting: false })
    const html = renderToString(out.template)

    expect(html).toContain('data-deck-graph="1"')
    expect(html).toContain('viewBox="0 0 1920 1080"')
    expect(html).toContain('data-node-id="a"')
    expect(html).toContain('data-node-id="b"')
    expect(html).toContain('data-node-id="c"')
    expect(html).toContain("stroke-dasharray")
  })
})

