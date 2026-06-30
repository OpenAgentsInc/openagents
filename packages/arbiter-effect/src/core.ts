import { Schema as S } from "effect"

export const ArbiterGraphSpecSchemaVersion =
  "openagents.arbiter.graph_spec.v0"

export const GraphNodeStatus = S.Literals([
  "idle",
  "active",
  "blocked",
  "complete",
  "proposal_ready",
])
export type GraphNodeStatus = typeof GraphNodeStatus.Type

export const GraphLinkStatus = S.Literals([
  "inactive",
  "active",
  "blocked",
  "evidence_backed",
])
export type GraphLinkStatus = typeof GraphLinkStatus.Type

export const GraphPinDirection = S.Literals(["input", "output"])
export type GraphPinDirection = typeof GraphPinDirection.Type

export const GraphPoint = S.Struct({
  x: S.Number,
  y: S.Number,
})
export type GraphPoint = typeof GraphPoint.Type

export const GraphPin = S.Struct({
  id: S.String,
  name: S.String,
  direction: GraphPinDirection,
  type: S.String,
})
export type GraphPin = typeof GraphPin.Type

export const GraphDatumValue = S.Union([S.String, S.Number, S.Boolean])
export type GraphDatumValue = typeof GraphDatumValue.Type

export const GraphDatum = S.Struct({
  label: S.String,
  value: GraphDatumValue,
  unit: S.optional(S.String),
  evidenceRefs: S.Array(S.String),
})
export type GraphDatum = typeof GraphDatum.Type

export const GraphNode = S.Struct({
  id: S.String,
  label: S.String,
  kind: S.String,
  status: GraphNodeStatus,
  inputs: S.Array(GraphPin),
  outputs: S.Array(GraphPin),
  datum: S.Array(GraphDatum),
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  position: GraphPoint,
})
export type GraphNode = typeof GraphNode.Type

export const GraphPinRef = S.Struct({
  nodeId: S.String,
  pinId: S.String,
})
export type GraphPinRef = typeof GraphPinRef.Type

export const GraphLink = S.Struct({
  id: S.String,
  label: S.String,
  status: GraphLinkStatus,
  from: GraphPinRef,
  to: GraphPinRef,
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
})
export type GraphLink = typeof GraphLink.Type

export const GraphSpec = S.Struct({
  schemaVersion: S.String,
  title: S.String,
  generatedAt: S.String,
  status: GraphNodeStatus,
  nodes: S.Array(GraphNode),
  links: S.Array(GraphLink),
  evidenceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
})
export type GraphSpec = typeof GraphSpec.Type

export const decodeGraphSpec = S.decodeUnknownSync(GraphSpec)

export type GraphLayout = Readonly<{
  width: number
  height: number
  nodeWidth: number
  nodeHeight: number
  edgeTensionMin: number
  edgeTensionRatio: number
}>

export const defaultGraphLayout: GraphLayout = {
  width: 1480,
  height: 430,
  nodeWidth: 166,
  nodeHeight: 78,
  edgeTensionMin: 72,
  edgeTensionRatio: 0.42,
}

const unsafeRefValue =
  /\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer |authorization:|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|http:\/\/|https:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|preimage|private[_-]?(endpoint|repo|source)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|fixture|log|payload|prompt|provider|runner|source|trace|traces)|secret|(?:^|[^A-Za-z0-9])sk-[a-z0-9]|scratch[_-]?log|token|wallet/i

export const graphSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
export const graphCounterOnlyRefPattern =
  /(^|[.:/-])counter([.:/-]|$)|=\d+$/
export const graphPublicSafetyCheckRefPattern =
  /^check\.(public_projection|public_safe)\.[A-Za-z0-9_.:/-]+$/

export const isDereferenceableGraphRef = (ref: string): boolean => {
  if (graphCounterOnlyRefPattern.test(ref)) return false
  if (
    !graphSafeRefPattern.test(ref) ||
    (!graphPublicSafetyCheckRefPattern.test(ref) && unsafeRefValue.test(ref))
  ) {
    return false
  }
  return /^[A-Za-z][A-Za-z0-9_-]*(\.[A-Za-z0-9_:/-]+){1,}$/.test(ref)
}

export const graphLinkStatusForRefs = (
  evidenceRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
): GraphLinkStatus => {
  if (blockerRefs.length > 0) return "blocked"
  if (evidenceRefs.length > 0) return "evidence_backed"
  return "inactive"
}

export const graphNodeById = (
  spec: GraphSpec,
): ReadonlyMap<string, GraphNode> =>
  new Map(spec.nodes.map(node => [node.id, node]))

export const graphPinAnchor = (
  node: GraphNode,
  direction: GraphPinDirection,
  layout: GraphLayout = defaultGraphLayout,
): GraphPoint => ({
  x: direction === "output" ? node.position.x + layout.nodeWidth : node.position.x,
  y: node.position.y + layout.nodeHeight / 2,
})

export const graphLinkPath = (
  link: GraphLink,
  nodes: ReadonlyMap<string, GraphNode>,
  layout: GraphLayout = defaultGraphLayout,
): string | null => {
  const from = nodes.get(link.from.nodeId)
  const to = nodes.get(link.to.nodeId)
  if (from === undefined || to === undefined) return null

  const start = graphPinAnchor(from, "output", layout)
  const end = graphPinAnchor(to, "input", layout)
  const tension = Math.max(
    layout.edgeTensionMin,
    Math.abs(end.x - start.x) * layout.edgeTensionRatio,
  )
  return [
    `M ${start.x} ${start.y}`,
    `C ${start.x + tension} ${start.y}, ${end.x - tension} ${end.y}, ${end.x} ${end.y}`,
  ].join(" ")
}
