import "./foldkit-window-shim"

import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"

import {
  decodeGraphSpec,
  defaultGraphLayout,
  graphLinkPath,
  graphNodeById,
  type GraphLayout,
  type GraphLink,
  type GraphNode,
  type GraphSpec,
} from "./core"

export type ArbiterGraphRenderOptions = Readonly<{
  reducedMotion?: boolean
  layout?: Partial<GraphLayout>
  mirrorLabel?: string
}>

export type ArbiterGraphRenderOutput = Readonly<{
  html: string
  mirrorHtml: string
  svg: string
}>

export type ArbiterGraphFoldkitInput<Message> = Readonly<{
  spec: GraphSpec
  options?: ArbiterGraphRenderOptions
  attrs?: ReadonlyArray<Attribute<Message>>
}>

const layoutFromOptions = (
  options: ArbiterGraphRenderOptions = {},
): GraphLayout => ({
  ...defaultGraphLayout,
  ...(options.layout ?? {}),
})

const escapeHtml = (value: string | number | boolean): string =>
  String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")

const short = (value: string | number | boolean, length = 40): string => {
  const text = String(value)
  if (text.length <= length) return text
  return `${text.slice(0, Math.max(0, length - 3))}...`
}

const firstRef = (refs: ReadonlyArray<string>): string =>
  refs[0] ?? "no public ref"

const nodeDatumText = (node: GraphNode): string => {
  const datum = node.datum[0]
  if (datum === undefined) return firstRef(node.evidenceRefs)
  const value = datum.unit === undefined ? datum.value : `${datum.value} ${datum.unit}`
  return `${datum.label}: ${value}`
}

const linkPath = (
  link: GraphLink,
  nodes: ReadonlyMap<string, GraphNode>,
  layout: GraphLayout,
): string | null => graphLinkPath(link, nodes, layout)

const renderLinkPathDef = (
  link: GraphLink,
  nodes: ReadonlyMap<string, GraphNode>,
  layout: GraphLayout,
): string => {
  const path = linkPath(link, nodes, layout)
  if (path === null) return ""
  return `<path id="${escapeHtml(link.id)}-path" d="${escapeHtml(path)}" />`
}

const renderLink = (
  link: GraphLink,
  nodes: ReadonlyMap<string, GraphNode>,
  layout: GraphLayout,
): string => {
  const path = linkPath(link, nodes, layout)
  if (path === null) return ""
  return [
    `<path class="khala-gym-edge" data-link-id="${escapeHtml(link.id)}" data-status="${escapeHtml(link.status)}" d="${escapeHtml(path)}" />`,
    `<text class="khala-gym-edge-label" data-status="${escapeHtml(link.status)}">`,
    `<textPath href="#${escapeHtml(link.id)}-path" startOffset="50%">${escapeHtml(link.label)}</textPath>`,
    "</text>",
  ].join("")
}

const renderNode = (node: GraphNode, layout: GraphLayout): string => {
  const ref = firstRef(
    node.evidenceRefs.length > 0 ? node.evidenceRefs : node.blockerRefs,
  )
  const datum = nodeDatumText(node)
  return [
    `<g class="khala-gym-node" data-node-id="${escapeHtml(node.id)}" data-status="${escapeHtml(node.status)}" transform="translate(${node.position.x} ${node.position.y})">`,
    `<rect class="khala-gym-node-card" width="${layout.nodeWidth}" height="${layout.nodeHeight}" rx="6" />`,
    `<text class="khala-gym-node-label" x="12" y="21">${escapeHtml(short(node.label, 24))}</text>`,
    `<text class="khala-gym-node-status" x="12" y="39">${escapeHtml(node.status)}</text>`,
    `<text class="khala-gym-node-datum" x="12" y="56">${escapeHtml(short(datum, 30))}</text>`,
    `<text class="khala-gym-node-ref" x="12" y="70">${escapeHtml(short(ref, 34))}</text>`,
    "</g>",
  ].join("")
}

const renderMirror = (
  spec: GraphSpec,
  nodes: ReadonlyMap<string, GraphNode>,
  label = "Gym graph text mirror",
): string => {
  const rows = spec.links
    .map(link => {
      const from = nodes.get(link.from.nodeId)
      const to = nodes.get(link.to.nodeId)
      if (from === undefined || to === undefined) return ""
      const refs = link.evidenceRefs.length > 0 ? link.evidenceRefs : link.blockerRefs
      return [
        `<li class="khala-gym-graph-mirror-row" data-status="${escapeHtml(link.status)}">`,
        `<span class="khala-gym-graph-mirror-flow">${escapeHtml(from.label)} -&gt; ${escapeHtml(to.label)}</span>`,
        `<span class="khala-gym-graph-mirror-status">${escapeHtml(link.status)}</span>`,
        `<span class="khala-gym-graph-mirror-refs">${escapeHtml(refs.join(" "))}</span>`,
        "</li>",
      ].join("")
    })
    .join("")

  return [
    `<div class="khala-gym-graph-mirror" aria-label="${escapeHtml(label)}">`,
    `<ol class="khala-gym-graph-mirror-list">${rows}</ol>`,
    "</div>",
  ].join("")
}

export const renderArbiterGraphHtml = (
  spec: GraphSpec,
  options: ArbiterGraphRenderOptions = {},
): ArbiterGraphRenderOutput => {
  const graph = decodeGraphSpec(spec)
  const layout = layoutFromOptions(options)
  const nodes = graphNodeById(graph)
  const defs = graph.links
    .map(link => renderLinkPathDef(link, nodes, layout))
    .join("")
  const edges = graph.links.map(link => renderLink(link, nodes, layout)).join("")
  const renderedNodes = graph.nodes.map(node => renderNode(node, layout)).join("")
  const reducedMotion = options.reducedMotion === true

  const svg = [
    `<svg class="khala-gym-graph-svg" data-reduced-motion="${reducedMotion ? "true" : "false"}" viewBox="0 0 ${layout.width} ${layout.height}" role="img" aria-label="${escapeHtml(graph.title)}">`,
    `<defs>${defs}</defs>`,
    `<g class="khala-gym-edge-layer">${edges}</g>`,
    `<g class="khala-gym-node-layer">${renderedNodes}</g>`,
    "</svg>",
  ].join("")
  const mirrorHtml = renderMirror(graph, nodes, options.mirrorLabel)
  const renderedHtml = [
    `<figure class="khala-gym-graph" data-status="${escapeHtml(graph.status)}">`,
    svg,
    mirrorHtml,
    "</figure>",
  ].join("")

  return { html: renderedHtml, mirrorHtml, svg }
}

const foldkitLinkPathDef = <Message>(
  h: ReturnType<typeof html<Message>>,
  link: GraphLink,
  nodes: ReadonlyMap<string, GraphNode>,
  layout: GraphLayout,
): Html => {
  const path = linkPath(link, nodes, layout)
  if (path === null) return null
  return h.path([
    h.Attribute("id", `${link.id}-path`),
    h.Attribute("d", path),
  ], [])
}

const foldkitLink = <Message>(
  h: ReturnType<typeof html<Message>>,
  link: GraphLink,
  nodes: ReadonlyMap<string, GraphNode>,
  layout: GraphLayout,
): Html => {
  const path = linkPath(link, nodes, layout)
  if (path === null) return null
  return h.g([], [
    h.path([
      h.Class("khala-gym-edge"),
      h.DataAttribute("link-id", link.id),
      h.DataAttribute("status", link.status),
      h.Attribute("d", path),
    ], []),
    h.text([
      h.Class("khala-gym-edge-label"),
      h.DataAttribute("status", link.status),
    ], [
      h.textPath([
        h.Attribute("href", `#${link.id}-path`),
        h.Attribute("startOffset", "50%"),
      ], [link.label]),
    ]),
  ])
}

const foldkitNode = <Message>(
  h: ReturnType<typeof html<Message>>,
  node: GraphNode,
  layout: GraphLayout,
): Html => {
  const ref = firstRef(
    node.evidenceRefs.length > 0 ? node.evidenceRefs : node.blockerRefs,
  )
  return h.g([
    h.Class("khala-gym-node"),
    h.DataAttribute("node-id", node.id),
    h.DataAttribute("status", node.status),
    h.Attribute("transform", `translate(${node.position.x} ${node.position.y})`),
  ], [
    h.rect([
      h.Class("khala-gym-node-card"),
      h.Attribute("width", String(layout.nodeWidth)),
      h.Attribute("height", String(layout.nodeHeight)),
      h.Attribute("rx", "6"),
    ], []),
    h.text([
      h.Class("khala-gym-node-label"),
      h.Attribute("x", "12"),
      h.Attribute("y", "21"),
    ], [short(node.label, 24)]),
    h.text([
      h.Class("khala-gym-node-status"),
      h.Attribute("x", "12"),
      h.Attribute("y", "39"),
    ], [node.status]),
    h.text([
      h.Class("khala-gym-node-datum"),
      h.Attribute("x", "12"),
      h.Attribute("y", "56"),
    ], [short(nodeDatumText(node), 30)]),
    h.text([
      h.Class("khala-gym-node-ref"),
      h.Attribute("x", "12"),
      h.Attribute("y", "70"),
    ], [short(ref, 34)]),
  ])
}

const foldkitMirror = <Message>(
  h: ReturnType<typeof html<Message>>,
  spec: GraphSpec,
  nodes: ReadonlyMap<string, GraphNode>,
  label = "Gym graph text mirror",
): Html =>
  h.div([
    h.Class("khala-gym-graph-mirror"),
    h.AriaLabel(label),
  ], [
    h.ol([
      h.Class("khala-gym-graph-mirror-list"),
    ], spec.links.flatMap(link => {
      const from = nodes.get(link.from.nodeId)
      const to = nodes.get(link.to.nodeId)
      if (from === undefined || to === undefined) return []
      const refs = link.evidenceRefs.length > 0 ? link.evidenceRefs : link.blockerRefs
      return [
        h.li([
          h.Class("khala-gym-graph-mirror-row"),
          h.DataAttribute("status", link.status),
        ], [
          h.span([
            h.Class("khala-gym-graph-mirror-flow"),
          ], [`${from.label} -> ${to.label}`]),
          h.span([
            h.Class("khala-gym-graph-mirror-status"),
          ], [link.status]),
          h.span([
            h.Class("khala-gym-graph-mirror-refs"),
          ], [refs.join(" ")]),
        ]),
      ]
    })),
  ])

export const arbiterGraphFigure = <Message>(
  input: ArbiterGraphFoldkitInput<Message>,
): Html => {
  const h = html<Message>()
  const graph = decodeGraphSpec(input.spec)
  const layout = layoutFromOptions(input.options)
  const nodes = graphNodeById(graph)
  const reducedMotion = input.options?.reducedMotion === true

  return h.figure([
    ...(input.attrs ?? []),
    h.Class("khala-gym-graph"),
    h.DataAttribute("status", graph.status),
  ], [
    h.svg([
      h.Class("khala-gym-graph-svg"),
      h.DataAttribute("reduced-motion", reducedMotion ? "true" : "false"),
      h.ViewBox(`0 0 ${layout.width} ${layout.height}`),
      h.Role("img"),
      h.AriaLabel(graph.title),
    ], [
      h.defs([], graph.links.map(link =>
        foldkitLinkPathDef(h, link, nodes, layout),
      )),
      h.g([
        h.Class("khala-gym-edge-layer"),
      ], graph.links.map(link => foldkitLink(h, link, nodes, layout))),
      h.g([
        h.Class("khala-gym-node-layer"),
      ], graph.nodes.map(node => foldkitNode(h, node, layout))),
    ]),
    foldkitMirror(h, graph, nodes, input.options?.mirrorLabel),
  ])
}
