import type { PositionedNode, Point, NodeId, Status } from "../flow/model.js"
import type { LayoutOutput } from "../flow/layout.js"
import type { CanvasState } from "../flow/canvas.js"
import { buildRoundedPath, type PathConfig } from "../flow/path.js"

// SVG element descriptors (pure data, no DOM)

export interface SVGRect {
  readonly type: "rect"
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  readonly rx?: number
  readonly ry?: number
  readonly fill?: string
  readonly stroke?: string
  readonly strokeWidth?: number
  readonly className?: string
  readonly dataNodeId?: string
}

export interface SVGText {
  readonly type: "text"
  readonly x: number
  readonly y: number
  readonly text: string
  readonly fontSize?: number
  readonly fontFamily?: string
  readonly fill?: string
  readonly textAnchor?: "start" | "middle" | "end"
  readonly dominantBaseline?: "auto" | "middle" | "hanging"
  readonly className?: string
}

export interface SVGPath {
  readonly type: "path"
  readonly d: string
  readonly fill?: string
  readonly stroke?: string
  readonly strokeWidth?: number
  readonly strokeDasharray?: string
  readonly className?: string
  readonly dataParentId?: string
  readonly dataChildId?: string
}

export interface SVGGroup {
  readonly type: "g"
  readonly transform?: string
  readonly children: readonly SVGElement[]
  readonly className?: string
}

export type SVGElement = SVGRect | SVGText | SVGPath | SVGGroup

export interface RenderConfig {
  readonly cornerRadius: number
  readonly nodeCornerRadius: number
  readonly connectionStroke: string
  readonly connectionStrokeWidth: number
  readonly nodeFill: string
  readonly nodeStroke: string
  readonly nodeStrokeWidth: number
  readonly textColor: string
  readonly fontSize: number
  readonly fontFamily: string
  readonly pathConfig: PathConfig
  readonly statusColors: Record<Status, string>
}

export const DEFAULT_RENDER_CONFIG: RenderConfig = {
  cornerRadius: 12,
  nodeCornerRadius: 10,
  connectionStroke: "#dc2626",
  connectionStrokeWidth: 2.5,
  nodeFill: "#0a0a0f",
  nodeStroke: "rgba(255, 255, 255, 0.1)",
  nodeStrokeWidth: 1,
  textColor: "#ffffff",
  fontSize: 11,
  fontFamily: "'Berkeley Mono', 'JetBrains Mono', monospace",
  pathConfig: { cornerRadius: 12 },
  statusColors: {
    idle: "#1a1a2e",
    busy: "#78350f",
    error: "#7f1d1d",
    blocked: "#3b0764",
    completed: "#14532d",
  },
}

// Node type colors - darker, more subtle
const NODE_TYPE_COLORS: Record<string, { fill: string; stroke: string }> = {
  root: { fill: "#1a0a2e", stroke: "rgba(139, 92, 246, 0.3)" },
  agent: { fill: "#1a1408", stroke: "rgba(245, 158, 11, 0.3)" },
  repo: { fill: "#0a1628", stroke: "rgba(59, 130, 246, 0.3)" },
  task: { fill: "#0a1a0f", stroke: "rgba(34, 197, 94, 0.25)" },
  workflow: { fill: "#0f0a1a", stroke: "rgba(168, 85, 247, 0.25)" },
  phase: { fill: "#0a0a0f", stroke: "rgba(255, 255, 255, 0.15)" },
}

// Get fill color based on node type and status
function getNodeFill(node: PositionedNode, config: RenderConfig): string {
  const status = node.metadata?.status as Status | undefined
  if (status && config.statusColors[status]) {
    return config.statusColors[status]
  }
  const typeColors = NODE_TYPE_COLORS[node.type]
  if (typeColors) {
    return typeColors.fill
  }
  return config.nodeFill
}

// Get stroke color based on node type
function getNodeStroke(node: PositionedNode, config: RenderConfig): string {
  const typeColors = NODE_TYPE_COLORS[node.type]
  if (typeColors) {
    return typeColors.stroke
  }
  return config.nodeStroke
}

// Render a single node as a rect + text
function renderNode(node: PositionedNode, config: RenderConfig): SVGGroup {
  const { x, y, size, label, id } = node
  
  // Node position is top-left corner
  const rect: SVGRect = {
    type: "rect",
    x,
    y,
    width: size.width,
    height: size.height,
    rx: config.nodeCornerRadius,
    ry: config.nodeCornerRadius,
    fill: getNodeFill(node, config),
    stroke: getNodeStroke(node, config),
    strokeWidth: config.nodeStrokeWidth,
    className: `flow-node flow-node-${node.type}`,
    dataNodeId: id,
  }

  // Center text in the node
  const text: SVGText = {
    type: "text",
    x: x + size.width / 2,
    y: y + size.height / 2,
    text: label,
    fontSize: config.fontSize,
    fontFamily: config.fontFamily,
    fill: config.textColor,
    textAnchor: "middle",
    dominantBaseline: "middle",
    className: "flow-node-label",
  }

  return {
    type: "g",
    className: "flow-node-group",
    children: [rect, text],
  }
}

// Render a connection as a path
function renderConnection(
  parentId: NodeId,
  childId: NodeId,
  waypoints: readonly Point[],
  config: RenderConfig
): SVGPath {
  const d = buildRoundedPath(waypoints, config.pathConfig)
  
  return {
    type: "path",
    d,
    fill: "none",
    stroke: config.connectionStroke,
    strokeWidth: config.connectionStrokeWidth,
    className: "flow-connection",
    dataParentId: parentId,
    dataChildId: childId,
  }
}

// Apply canvas transform (pan + zoom) to get the transform string
export function getCanvasTransform(canvas: CanvasState): string {
  return `translate(${canvas.panX}, ${canvas.panY}) scale(${canvas.scale})`
}

// Render layout output to SVG element descriptors
export function renderLayout(
  layout: LayoutOutput,
  config: RenderConfig = DEFAULT_RENDER_CONFIG
): SVGGroup {
  const connectionElements: SVGPath[] = layout.connections.map(conn =>
    renderConnection(conn.parentId, conn.childId, conn.waypoints, config)
  )

  const nodeElements: SVGGroup[] = layout.nodes.map(node =>
    renderNode(node, config)
  )

  // Connections rendered first (behind nodes)
  return {
    type: "g",
    className: "flow-content",
    children: [...connectionElements, ...nodeElements],
  }
}

// Render full SVG with canvas transform
export function renderFlowSVG(
  layout: LayoutOutput,
  canvas: CanvasState,
  config: RenderConfig = DEFAULT_RENDER_CONFIG
): SVGGroup {
  const content = renderLayout(layout, config)
  
  return {
    type: "g",
    transform: getCanvasTransform(canvas),
    className: "flow-canvas",
    children: [content],
  }
}

// Convert SVG element descriptor to SVG string (for debugging/testing)
export function svgElementToString(element: SVGElement, indent: number = 0): string {
  const pad = "  ".repeat(indent)
  
  switch (element.type) {
    case "rect": {
      const attrs = [
        `x="${element.x}"`,
        `y="${element.y}"`,
        `width="${element.width}"`,
        `height="${element.height}"`,
      ]
      if (element.rx !== undefined) attrs.push(`rx="${element.rx}"`)
      if (element.ry !== undefined) attrs.push(`ry="${element.ry}"`)
      if (element.fill) attrs.push(`fill="${element.fill}"`)
      if (element.stroke) attrs.push(`stroke="${element.stroke}"`)
      if (element.strokeWidth) attrs.push(`stroke-width="${element.strokeWidth}"`)
      if (element.className) attrs.push(`class="${element.className}"`)
      if (element.dataNodeId) attrs.push(`data-node-id="${element.dataNodeId}"`)
      return `${pad}<rect ${attrs.join(" ")} />`
    }
    
    case "text": {
      const attrs = [
        `x="${element.x}"`,
        `y="${element.y}"`,
      ]
      if (element.fontSize) attrs.push(`font-size="${element.fontSize}"`)
      if (element.fontFamily) attrs.push(`font-family="${element.fontFamily}"`)
      if (element.fill) attrs.push(`fill="${element.fill}"`)
      if (element.textAnchor) attrs.push(`text-anchor="${element.textAnchor}"`)
      if (element.dominantBaseline) attrs.push(`dominant-baseline="${element.dominantBaseline}"`)
      if (element.className) attrs.push(`class="${element.className}"`)
      return `${pad}<text ${attrs.join(" ")}>${escapeXml(element.text)}</text>`
    }
    
    case "path": {
      const attrs = [`d="${element.d}"`]
      if (element.fill) attrs.push(`fill="${element.fill}"`)
      if (element.stroke) attrs.push(`stroke="${element.stroke}"`)
      if (element.strokeWidth) attrs.push(`stroke-width="${element.strokeWidth}"`)
      if (element.strokeDasharray) attrs.push(`stroke-dasharray="${element.strokeDasharray}"`)
      if (element.className) attrs.push(`class="${element.className}"`)
      if (element.dataParentId) attrs.push(`data-parent-id="${element.dataParentId}"`)
      if (element.dataChildId) attrs.push(`data-child-id="${element.dataChildId}"`)
      return `${pad}<path ${attrs.join(" ")} />`
    }
    
    case "g": {
      const attrs: string[] = []
      if (element.transform) attrs.push(`transform="${element.transform}"`)
      if (element.className) attrs.push(`class="${element.className}"`)
      const attrStr = attrs.length > 0 ? " " + attrs.join(" ") : ""
      const children = element.children.map(c => svgElementToString(c, indent + 1)).join("\n")
      return `${pad}<g${attrStr}>\n${children}\n${pad}</g>`
    }
  }
}

function escapeXml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
}

// Generate grid pattern defs
function generateGridDefs(gridSize: number = 20, dotRadius: number = 1.5): string {
  return `
  <defs>
    <pattern id="dot-grid" x="0" y="0" width="${gridSize}" height="${gridSize}" patternUnits="userSpaceOnUse">
      <circle cx="${gridSize / 2}" cy="${gridSize / 2}" r="${dotRadius}" fill="rgba(255, 255, 255, 0.06)">
        <animate attributeName="opacity" values="0.4;0.8;0.4" dur="4s" repeatCount="indefinite" />
      </circle>
    </pattern>
    <radialGradient id="vignette" cx="50%" cy="50%" r="70%">
      <stop offset="0%" stop-color="transparent" />
      <stop offset="100%" stop-color="rgba(0,0,0,0.4)" />
    </radialGradient>
  </defs>
  <rect x="-5000" y="-5000" width="10000" height="10000" fill="url(#dot-grid)" />`
}

// Generate a complete SVG document string
export function renderToSVGString(
  layout: LayoutOutput,
  canvas: CanvasState,
  config: RenderConfig = DEFAULT_RENDER_CONFIG
): string {
  const root = renderFlowSVG(layout, canvas, config)
  const content = svgElementToString(root, 1)
  const gridDefs = generateGridDefs()
  
  return `<svg 
  xmlns="http://www.w3.org/2000/svg"
  width="${canvas.viewportWidth}"
  height="${canvas.viewportHeight}"
  viewBox="0 0 ${canvas.viewportWidth} ${canvas.viewportHeight}"
  class="flow-svg"
  style="background: #000"
>
${gridDefs}
${content}
</svg>`
}
