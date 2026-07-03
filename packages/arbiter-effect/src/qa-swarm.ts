import {
  ArbiterGraphSpecSchemaVersion,
  graphLinkStatusForRefs,
  isDereferenceableGraphRef,
  type GraphDatum,
  type GraphLink,
  type GraphNode,
  type GraphPin,
  type GraphPinDirection,
  type GraphSpec,
} from "./core"

export const QaSwarmBoardProjectionSchemaVersion =
  "openagents.qa_swarm.board_graph_projection.v1"

export type QaSwarmBoardVerdict =
  | "passed"
  | "failed"
  | "warning"
  | "inconclusive"

export type QaSwarmBoardVerdictItem = Readonly<{
  label: string
  receiptRef: string
  summary: string
  verdict: QaSwarmBoardVerdict
}>

export type QaSwarmBoardCoverageItem = Readonly<{
  current: number
  frontier: number
  label: string
  receiptRef: string
}>

export type QaSwarmBoardPerfItem = Readonly<{
  actualMs: number
  budgetMs: number
  label: string
  receiptRef: string
  verdict: QaSwarmBoardVerdict
}>

export type QaSwarmBoardDistilledTest = Readonly<{
  href: string
  label: string
  receiptRef: string
}>

export type QaSwarmBoardRunProjection = Readonly<{
  coverageFrontier: ReadonlyArray<QaSwarmBoardCoverageItem>
  distilledTests: ReadonlyArray<QaSwarmBoardDistilledTest>
  generatedAt: string
  perfBudgets: ReadonlyArray<QaSwarmBoardPerfItem>
  projectionRef: string
  publicSafetyRefs: ReadonlyArray<string>
  runRef: string
  target: Readonly<{
    label: string
    ref: string
    visibility: "public" | "opaque"
  }>
  title: string
  traceRefs: ReadonlyArray<string>
  verdict: QaSwarmBoardVerdict
  verdictWall: ReadonlyArray<QaSwarmBoardVerdictItem>
}>

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.flatMap(ref => {
    if (ref === null || ref === undefined) return []
    const trimmed = ref.trim()
    return trimmed.length === 0 ? [] : [trimmed]
  }))].sort()

const dereferenceableRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  uniqueRefs(refs).filter(isDereferenceableGraphRef)

const pin = (
  direction: GraphPinDirection,
  id: string,
  name: string,
  type: string,
): GraphPin => ({
  direction,
  id,
  name,
  type,
})

const datum = (
  label: string,
  value: string | number | boolean | undefined,
  evidenceRefs: ReadonlyArray<string>,
  unit?: string,
): ReadonlyArray<GraphDatum> =>
  value === undefined
    ? []
    : [{
        label,
        value,
        evidenceRefs,
        ...(unit === undefined ? {} : { unit }),
      }]

const node = (input: {
  id: string
  label: string
  kind: string
  status: GraphNode["status"]
  inputs?: ReadonlyArray<GraphPin>
  outputs?: ReadonlyArray<GraphPin>
  datum?: ReadonlyArray<GraphDatum>
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
  x: number
  y: number
}): GraphNode => ({
  id: input.id,
  label: input.label,
  kind: input.kind,
  status: input.status,
  inputs: [...(input.inputs ?? [])],
  outputs: [...(input.outputs ?? [])],
  datum: [...(input.datum ?? [])],
  evidenceRefs: [...(input.evidenceRefs ?? [])],
  blockerRefs: [...(input.blockerRefs ?? [])],
  caveatRefs: [...(input.caveatRefs ?? [])],
  position: { x: input.x, y: input.y },
})

const link = (input: {
  id: string
  label: string
  fromNodeId: string
  fromPinId: string
  toNodeId: string
  toPinId: string
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
}): GraphLink => {
  const evidenceRefs = dereferenceableRefs(input.evidenceRefs ?? [])
  const blockerRefs = uniqueRefs(input.blockerRefs ?? [])
  return {
    id: input.id,
    label: input.label,
    status: graphLinkStatusForRefs(evidenceRefs, blockerRefs),
    from: { nodeId: input.fromNodeId, pinId: input.fromPinId },
    to: { nodeId: input.toNodeId, pinId: input.toPinId },
    evidenceRefs,
    blockerRefs,
    caveatRefs: uniqueRefs(input.caveatRefs ?? []),
  }
}

const firstVerdictRef = (
  projection: QaSwarmBoardRunProjection,
  needle: RegExp,
): string | undefined =>
  projection.verdictWall.find(item => needle.test(item.label))?.receiptRef

const boardStatus = (
  verdict: QaSwarmBoardVerdict,
): GraphNode["status"] => {
  switch (verdict) {
    case "failed":
      return "blocked"
    case "passed":
      return "complete"
    case "warning":
      return "active"
    case "inconclusive":
      return "idle"
  }
}

export const buildQaSwarmBoardGraphSpec = (
  projection: QaSwarmBoardRunProjection,
): GraphSpec => {
  const scenarioRefs = dereferenceableRefs([
    firstVerdictRef(projection, /login|workspace|scenario/i),
    ...projection.traceRefs,
  ])
  const monkeyRefs = dereferenceableRefs(
    projection.coverageFrontier
      .filter(item => /seed|monkey|corpus/i.test(item.label))
      .map(item => item.receiptRef),
  )
  const explorerRefs = dereferenceableRefs(
    projection.coverageFrontier
      .filter(item => /frontier|desktop|explor/i.test(item.label))
      .map(item => item.receiptRef),
  )
  const perfRefs = dereferenceableRefs(
    projection.perfBudgets.map(item => item.receiptRef),
  )
  const headedAxRefs = dereferenceableRefs([
    firstVerdictRef(projection, /command|palette|headed|ax|desktop/i),
  ])
  const oracleRefs = dereferenceableRefs([
    ...projection.verdictWall.map(item => item.receiptRef),
    ...projection.coverageFrontier.map(item => item.receiptRef),
    ...projection.perfBudgets.map(item => item.receiptRef),
  ])
  const verdictRefs = dereferenceableRefs(
    projection.verdictWall.map(item => item.receiptRef),
  )
  const distilledRefs = dereferenceableRefs(
    projection.distilledTests.map(item => item.receiptRef),
  )
  const safetyRefs = uniqueRefs(projection.publicSafetyRefs)
  const allEvidenceRefs = dereferenceableRefs([
    projection.projectionRef,
    projection.target.ref,
    ...projection.traceRefs,
    ...projection.verdictWall.map(item => item.receiptRef),
    ...projection.coverageFrontier.map(item => item.receiptRef),
    ...projection.perfBudgets.map(item => item.receiptRef),
    ...projection.distilledTests.map(item => item.receiptRef),
    ...projection.publicSafetyRefs,
  ])
  const graphStatus = boardStatus(projection.verdict)

  const nodes: ReadonlyArray<GraphNode> = [
    node({
      id: "scenario-runner",
      label: "Scenario runner",
      kind: "qa_agent",
      status: scenarioRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "run", "run", "qa.scenario.receipt")],
      datum: datum("traces", projection.traceRefs.length, scenarioRefs),
      evidenceRefs: scenarioRefs,
      x: 40,
      y: 45,
    }),
    node({
      id: "seeded-monkey",
      label: "Seeded monkey",
      kind: "qa_agent",
      status: monkeyRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "coverage", "coverage", "qa.coverage.receipt")],
      datum: datum("coverage rows", projection.coverageFrontier.length, monkeyRefs),
      evidenceRefs: monkeyRefs,
      x: 40,
      y: 155,
    }),
    node({
      id: "llm-explorer",
      label: "LLM explorer",
      kind: "qa_agent",
      status: explorerRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "frontier", "frontier", "qa.frontier.receipt")],
      datum: datum("frontier", projection.coverageFrontier.length, explorerRefs),
      evidenceRefs: explorerRefs,
      x: 40,
      y: 265,
    }),
    node({
      id: "perf-probe",
      label: "Perf probe",
      kind: "qa_agent",
      status: perfRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "budget", "budget", "qa.perf.receipt")],
      datum: datum("budgets", projection.perfBudgets.length, perfRefs),
      evidenceRefs: perfRefs,
      x: 255,
      y: 45,
    }),
    node({
      id: "headed-ax",
      label: "Headed AX",
      kind: "qa_agent",
      status: headedAxRefs.length > 0 ? "complete" : "idle",
      outputs: [pin("output", "desktop", "desktop", "qa.ax.receipt")],
      datum: datum("driver", "native", headedAxRefs),
      evidenceRefs: headedAxRefs,
      x: 255,
      y: 265,
    }),
    node({
      id: "target-surface",
      label: projection.target.label,
      kind: "target_surface",
      status: allEvidenceRefs.length > 0 ? "complete" : "idle",
      inputs: [
        pin("input", "run", "run", "qa.scenario.receipt"),
        pin("input", "coverage", "coverage", "qa.coverage.receipt"),
        pin("input", "frontier", "frontier", "qa.frontier.receipt"),
        pin("input", "budget", "budget", "qa.perf.receipt"),
        pin("input", "desktop", "desktop", "qa.ax.receipt"),
      ],
      outputs: [pin("output", "observed", "observed", "qa.observed.receipt")],
      datum: datum("target", projection.target.visibility, allEvidenceRefs),
      evidenceRefs: dereferenceableRefs([projection.target.ref, ...allEvidenceRefs]),
      x: 470,
      y: 155,
    }),
    node({
      id: "oracle-families",
      label: "Oracle families",
      kind: "oracle_stage",
      status: oracleRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "observed", "observed", "qa.observed.receipt")],
      outputs: [pin("output", "verdict", "verdict", "qa.verdict.receipt")],
      datum: datum("checks", projection.verdictWall.length, oracleRefs),
      evidenceRefs: oracleRefs,
      x: 685,
      y: 155,
    }),
    node({
      id: "verdict-wall",
      label: "Verdict wall",
      kind: "verdict_stage",
      status: graphStatus,
      inputs: [pin("input", "verdict", "verdict", "qa.verdict.receipt")],
      outputs: [pin("output", "finding", "finding", "qa.finding.receipt")],
      datum: datum("verdict", projection.verdict, verdictRefs),
      evidenceRefs: verdictRefs,
      x: 900,
      y: 155,
    }),
    node({
      id: "distiller",
      label: "Distiller",
      kind: "distiller_stage",
      status: distilledRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "finding", "finding", "qa.finding.receipt")],
      outputs: [pin("output", "test", "test", "qa.distilled_test.receipt")],
      datum: datum("merged tests", projection.distilledTests.length, distilledRefs),
      evidenceRefs: distilledRefs,
      x: 1115,
      y: 155,
    }),
    node({
      id: "public-safe-share",
      label: "Share URL",
      kind: "public_projection",
      status: safetyRefs.length > 0 ? "complete" : "idle",
      inputs: [pin("input", "test", "test", "qa.distilled_test.receipt")],
      datum: datum("schema", QaSwarmBoardProjectionSchemaVersion, safetyRefs),
      evidenceRefs: dereferenceableRefs(safetyRefs),
      caveatRefs: safetyRefs.filter(ref => !isDereferenceableGraphRef(ref)),
      x: 1330,
      y: 155,
    }),
  ]

  const links: ReadonlyArray<GraphLink> = [
    link({
      id: "scenario-to-target",
      label: "scenario receipts",
      fromNodeId: "scenario-runner",
      fromPinId: "run",
      toNodeId: "target-surface",
      toPinId: "run",
      evidenceRefs: scenarioRefs,
    }),
    link({
      id: "monkey-to-target",
      label: "seed coverage",
      fromNodeId: "seeded-monkey",
      fromPinId: "coverage",
      toNodeId: "target-surface",
      toPinId: "coverage",
      evidenceRefs: monkeyRefs,
    }),
    link({
      id: "explorer-to-target",
      label: "frontier",
      fromNodeId: "llm-explorer",
      fromPinId: "frontier",
      toNodeId: "target-surface",
      toPinId: "frontier",
      evidenceRefs: explorerRefs,
    }),
    link({
      id: "perf-to-target",
      label: "p95 budgets",
      fromNodeId: "perf-probe",
      fromPinId: "budget",
      toNodeId: "target-surface",
      toPinId: "budget",
      evidenceRefs: perfRefs,
    }),
    link({
      id: "headed-ax-to-target",
      label: "desktop receipts",
      fromNodeId: "headed-ax",
      fromPinId: "desktop",
      toNodeId: "target-surface",
      toPinId: "desktop",
      evidenceRefs: headedAxRefs,
    }),
    link({
      id: "target-to-oracles",
      label: "observed run",
      fromNodeId: "target-surface",
      fromPinId: "observed",
      toNodeId: "oracle-families",
      toPinId: "observed",
      evidenceRefs: oracleRefs,
    }),
    link({
      id: "oracles-to-verdict",
      label: "verdict receipts",
      fromNodeId: "oracle-families",
      fromPinId: "verdict",
      toNodeId: "verdict-wall",
      toPinId: "verdict",
      evidenceRefs: verdictRefs,
    }),
    link({
      id: "verdict-to-distiller",
      label: "merged tests",
      fromNodeId: "verdict-wall",
      fromPinId: "finding",
      toNodeId: "distiller",
      toPinId: "finding",
      evidenceRefs: distilledRefs,
    }),
    link({
      id: "distiller-to-share",
      label: "public mirror",
      fromNodeId: "distiller",
      fromPinId: "test",
      toNodeId: "public-safe-share",
      toPinId: "test",
      evidenceRefs: [...distilledRefs, ...safetyRefs],
    }),
  ]

  return {
    schemaVersion: ArbiterGraphSpecSchemaVersion,
    title: `QA Swarm board - ${projection.title}`,
    generatedAt: projection.generatedAt,
    status: graphStatus,
    nodes,
    links,
    evidenceRefs: allEvidenceRefs,
    blockerRefs: [],
    caveatRefs: safetyRefs.filter(ref => !isDereferenceableGraphRef(ref)),
    sourceRefs: uniqueRefs([
      projection.runRef,
      projection.projectionRef,
      QaSwarmBoardProjectionSchemaVersion,
    ]),
  }
}
