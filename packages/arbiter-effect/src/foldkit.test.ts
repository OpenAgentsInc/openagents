import { describe, expect, test } from "bun:test"

import { graphFixture } from "./core.test"
import {
  arbiterGraphFigure,
  renderArbiterGraphHtml,
} from "./foldkit"

describe("arbiter-effect foldkit renderer", () => {
  test("renders read-only SVG HTML with evidence-bound link state", () => {
    const rendered = renderArbiterGraphHtml(graphFixture(), {
      reducedMotion: true,
    })

    expect(rendered.html).toContain('class="khala-gym-graph"')
    expect(rendered.svg).toContain('viewBox="0 0 1480 430"')
    expect(rendered.svg).toContain('data-reduced-motion="true"')
    expect(rendered.svg).toContain('data-status="evidence_backed"')
    expect(rendered.svg).toContain('transform="translate(40 70)"')
    expect(rendered.svg).toContain("metric: 211 bps")
    expect(rendered.mirrorHtml).toContain("source -&gt; sink")
    expect(rendered.mirrorHtml).toContain(
      "action_submission.proposal.fixture.v1",
    )
  })

  test("escapes graph labels and refs in string output", () => {
    const fixture = graphFixture()
    const rendered = renderArbiterGraphHtml({
      ...fixture,
      title: "Unsafe <title>",
      nodes: [
        { ...fixture.nodes[0]!, label: "source <script>" },
        ...fixture.nodes.slice(1),
      ],
    })

    expect(rendered.html).toContain("Unsafe &lt;title&gt;")
    expect(rendered.html).toContain("source &lt;script&gt;")
    expect(rendered.html).not.toContain("<script>")
  })

  test("exposes a Foldkit SVG figure renderer", () => {
    const view = arbiterGraphFigure({
      spec: graphFixture(),
      options: { reducedMotion: false },
    })

    expect(view).not.toBeNull()
    expect(typeof view).toBe("object")
  })
})
