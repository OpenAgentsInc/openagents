import type {
  KhalaGymGraphLink,
  KhalaGymGraphNode,
  KhalaGymGraphProjection,
} from "./gym-graph-projection"

export type KhalaGymGraphRenderOptions = Readonly<{
  reducedMotion?: boolean
}>

export type KhalaGymGraphRenderOutput = Readonly<{
  html: string
  mirrorHtml: string
  svg: string
}>

const graphWidth = 1480
const graphHeight = 430
const nodeWidth = 166
const nodeHeight = 78

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

const nodeById = (
  projection: KhalaGymGraphProjection,
): ReadonlyMap<string, KhalaGymGraphNode> =>
  new Map(projection.nodes.map(node => [node.id, node]))

const linkPath = (
  link: KhalaGymGraphLink,
  nodes: ReadonlyMap<string, KhalaGymGraphNode>,
): string | null => {
  const from = nodes.get(link.from.nodeId)
  const to = nodes.get(link.to.nodeId)
  if (from === undefined || to === undefined) return null

  const startX = from.position.x + nodeWidth
  const startY = from.position.y + nodeHeight / 2
  const endX = to.position.x
  const endY = to.position.y + nodeHeight / 2
  const tension = Math.max(72, Math.abs(endX - startX) * 0.42)
  return [
    `M ${startX} ${startY}`,
    `C ${startX + tension} ${startY}, ${endX - tension} ${endY}, ${endX} ${endY}`,
  ].join(" ")
}

const firstRef = (refs: ReadonlyArray<string>): string =>
  refs[0] ?? "no public ref"

const nodeDatumText = (node: KhalaGymGraphNode): string => {
  const datum = node.datum[0]
  if (datum === undefined) return firstRef(node.evidenceRefs)
  const value = datum.unit === undefined ? datum.value : `${datum.value} ${datum.unit}`
  return `${datum.label}: ${value}`
}

const renderLink = (
  link: KhalaGymGraphLink,
  nodes: ReadonlyMap<string, KhalaGymGraphNode>,
): string => {
  const path = linkPath(link, nodes)
  if (path === null) return ""
  return [
    `<path class="khala-gym-edge" data-link-id="${escapeHtml(link.id)}" data-status="${escapeHtml(link.status)}" d="${escapeHtml(path)}" />`,
    `<text class="khala-gym-edge-label" data-status="${escapeHtml(link.status)}">`,
    `<textPath href="#${escapeHtml(link.id)}-path" startOffset="50%">${escapeHtml(link.label)}</textPath>`,
    "</text>",
  ].join("")
}

const renderLinkPathDef = (
  link: KhalaGymGraphLink,
  nodes: ReadonlyMap<string, KhalaGymGraphNode>,
): string => {
  const path = linkPath(link, nodes)
  if (path === null) return ""
  return `<path id="${escapeHtml(link.id)}-path" d="${escapeHtml(path)}" />`
}

const renderNode = (node: KhalaGymGraphNode): string => {
  const ref = firstRef(node.evidenceRefs.length > 0 ? node.evidenceRefs : node.blockerRefs)
  const datum = nodeDatumText(node)
  return [
    `<g class="khala-gym-node" data-node-id="${escapeHtml(node.id)}" data-status="${escapeHtml(node.status)}" transform="translate(${node.position.x} ${node.position.y})">`,
    `<rect class="khala-gym-node-card" width="${nodeWidth}" height="${nodeHeight}" rx="6" />`,
    `<text class="khala-gym-node-label" x="12" y="21">${escapeHtml(short(node.label, 24))}</text>`,
    `<text class="khala-gym-node-status" x="12" y="39">${escapeHtml(node.status)}</text>`,
    `<text class="khala-gym-node-datum" x="12" y="56">${escapeHtml(short(datum, 30))}</text>`,
    `<text class="khala-gym-node-ref" x="12" y="70">${escapeHtml(short(ref, 34))}</text>`,
    "</g>",
  ].join("")
}

const renderMirror = (
  projection: KhalaGymGraphProjection,
  nodes: ReadonlyMap<string, KhalaGymGraphNode>,
): string => {
  const rows = projection.links
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
    `<div class="khala-gym-graph-mirror" aria-label="Gym graph text mirror">`,
    `<ol class="khala-gym-graph-mirror-list">${rows}</ol>`,
    "</div>",
  ].join("")
}

export const renderKhalaGymGraphHtml = (
  projection: KhalaGymGraphProjection,
  options: KhalaGymGraphRenderOptions = {},
): KhalaGymGraphRenderOutput => {
  const nodes = nodeById(projection)
  const defs = projection.links.map(link => renderLinkPathDef(link, nodes)).join("")
  const edges = projection.links.map(link => renderLink(link, nodes)).join("")
  const renderedNodes = projection.nodes.map(renderNode).join("")
  const reducedMotion = options.reducedMotion === true

  const svg = [
    `<svg class="khala-gym-graph-svg" data-reduced-motion="${reducedMotion ? "true" : "false"}" viewBox="0 0 ${graphWidth} ${graphHeight}" role="img" aria-label="${escapeHtml(projection.title)}">`,
    `<defs>${defs}</defs>`,
    `<g class="khala-gym-edge-layer">${edges}</g>`,
    `<g class="khala-gym-node-layer">${renderedNodes}</g>`,
    "</svg>",
  ].join("")
  const mirrorHtml = renderMirror(projection, nodes)
  const html = [
    `<figure class="khala-gym-graph" data-status="${escapeHtml(projection.status)}">`,
    svg,
    mirrorHtml,
    "</figure>",
  ].join("")

  return { html, mirrorHtml, svg }
}
