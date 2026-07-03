import { GraphSpec } from "@openagentsinc/arbiter-effect/core"
import { arbiterGraphFigure } from "@openagentsinc/arbiter-effect/foldkit"
import {
  buildQaSwarmBoardGraphSpec,
  type QaSwarmBoardRunProjection,
} from "@openagentsinc/arbiter-effect/qa-swarm"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"

export type KhalaCodeQaSwarmPanelState = Readonly<{
  boardGraph: GraphSpec
  generatedAt: string
  runRef: string
  status: string
  title: string
}>

export const khalaCodeQaSwarmPanelStateFromRunProjection = (
  projection: QaSwarmBoardRunProjection,
): KhalaCodeQaSwarmPanelState => ({
  boardGraph: buildQaSwarmBoardGraphSpec(projection),
  generatedAt: projection.generatedAt,
  runRef: projection.runRef,
  status: projection.verdict,
  title: projection.title,
})

export const khalaCodeQaSwarmPanelView = <Message>(
  state: KhalaCodeQaSwarmPanelState,
): Html => {
  const h = html<Message>()

  return h.section([
    h.Class("khala-qa-swarm-panel"),
    h.DataAttribute("component", "khala-code-qa-swarm-panel"),
    h.DataAttribute("status", state.status),
  ], [
    h.header([h.Class("khala-qa-swarm-header")], [
      h.div([h.Class("khala-qa-swarm-title-group")], [
        h.h2([h.Class("khala-qa-swarm-title")], ["QA Swarm"]),
        h.p([h.Class("khala-qa-swarm-subtitle")], [state.title]),
      ]),
      h.div([h.Class("khala-qa-swarm-meta")], [
        h.span([h.Class("khala-qa-swarm-status")], [state.status]),
        h.span([h.Class("khala-qa-swarm-run-ref")], [state.runRef]),
      ]),
    ]),
    h.div([h.Class("khala-qa-swarm-board")], [
      arbiterGraphFigure<Message>({
        spec: state.boardGraph,
        options: {
          layout: { height: 395 },
          mirrorLabel: "QA Swarm board text mirror",
        },
      }),
    ]),
    h.p([h.Class("khala-qa-swarm-boundary")], [
      `Generated ${state.generatedAt}. Edges light only when the GraphSpec carries a dereferenceable receipt ref.`,
    ]),
  ])
}
