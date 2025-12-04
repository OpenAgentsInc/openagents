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
  readonly opacity?: number
  readonly stroke?: string
  readonly strokeWidth?: number
  readonly strokeOpacity?: number
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
  readonly strokeOpacity?: number
  readonly opacity?: number
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
  readonly connectionStrokeMuted: string
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
  cornerRadius: 14,
  nodeCornerRadius: 12,
  connectionStroke: "rgba(255, 98, 90, 0.95)",
  connectionStrokeMuted: "rgba(255, 98, 90, 0.35)",
  connectionStrokeWidth: 3,
  nodeFill: "#0d0f16",
  nodeStroke: "rgba(255, 255, 255, 0.12)",
  nodeStrokeWidth: 1.25,
  textColor: "#f5f7fb",
  fontSize: 12,
  fontFamily: "'Berkeley Mono', 'JetBrains Mono', monospace",
  pathConfig: { cornerRadius: 14 },
  statusColors: {
    idle: "#30323f",
    busy: "#f59e0b",
    error: "#ef4444",
    blocked: "#8b5cf6",
    completed: "#16a34a",
  },
}

interface NodeTheme {
  readonly fill: string
  readonly stroke: string
  readonly header: string
  readonly accent: string
  readonly mutedText: string
  readonly glow: string
}

// Node themes inspired by Unkey’s palette
const NODE_THEMES: Record<string, NodeTheme> = {
  root: {
    fill: "#111324",
    stroke: "rgba(129, 140, 248, 0.35)",
    header: "rgba(129, 140, 248, 0.18)",
    accent: "rgba(167, 139, 250, 0.9)",
    mutedText: "rgba(229, 231, 235, 0.75)",
    glow: "rgba(129, 140, 248, 0.3)",
  },
  agent: {
    fill: "#141017",
    stroke: "rgba(245, 158, 11, 0.25)",
    header: "rgba(251, 191, 36, 0.18)",
    accent: "rgba(251, 146, 60, 0.9)",
    mutedText: "rgba(255, 237, 213, 0.8)",
    glow: "rgba(251, 146, 60, 0.25)",
  },
  repo: {
    fill: "#0f1620",
    stroke: "rgba(59, 130, 246, 0.25)",
    header: "rgba(59, 130, 246, 0.12)",
    accent: "rgba(96, 165, 250, 0.9)",
    mutedText: "rgba(191, 219, 254, 0.8)",
    glow: "rgba(59, 130, 246, 0.25)",
  },
  task: {
    fill: "#0f1a12",
    stroke: "rgba(34, 197, 94, 0.25)",
    header: "rgba(34, 197, 94, 0.12)",
    accent: "rgba(74, 222, 128, 0.9)",
    mutedText: "rgba(187, 247, 208, 0.85)",
    glow: "rgba(34, 197, 94, 0.2)",
  },
  workflow: {
    fill: "#111019",
    stroke: "rgba(168, 85, 247, 0.25)",
    header: "rgba(168, 85, 247, 0.14)",
    accent: "rgba(232, 121, 249, 0.9)",
    mutedText: "rgba(240, 171, 252, 0.75)",
    glow: "rgba(168, 85, 247, 0.2)",
  },
  phase: {
    fill: "#0e0f14",
    stroke: "rgba(255, 255, 255, 0.14)",
    header: "rgba(255, 255, 255, 0.05)",
    accent: "rgba(255, 255, 255, 0.3)",
    mutedText: "rgba(229, 231, 235, 0.65)",
    glow: "rgba(255, 255, 255, 0.12)",
  },
  // Terminal-Bench node themes
  "tb-root": {
    fill: "#0f1a12",
    stroke: "rgba(34, 197, 94, 0.35)",
    header: "rgba(34, 197, 94, 0.18)",
    accent: "rgba(74, 222, 128, 0.9)",
    mutedText: "rgba(187, 247, 208, 0.8)",
    glow: "rgba(34, 197, 94, 0.25)",
  },
  "tb-controls": {
    fill: "#111019",
    stroke: "rgba(34, 197, 94, 0.25)",
    header: "rgba(34, 197, 94, 0.12)",
    accent: "rgba(34, 197, 94, 0.9)",
    mutedText: "rgba(187, 247, 208, 0.75)",
    glow: "rgba(34, 197, 94, 0.2)",
  },
  "tb-timeline": {
    fill: "#0d0f16",
    stroke: "rgba(255, 255, 255, 0.12)",
    header: "rgba(255, 255, 255, 0.06)",
    accent: "rgba(255, 255, 255, 0.3)",
    mutedText: "rgba(229, 231, 235, 0.6)",
    glow: "rgba(255, 255, 255, 0.1)",
  },
  "tb-run-summary": {
    fill: "#0f1620",
    stroke: "rgba(34, 197, 94, 0.3)",
    header: "rgba(34, 197, 94, 0.15)",
    accent: "rgba(74, 222, 128, 0.9)",
    mutedText: "rgba(187, 247, 208, 0.8)",
    glow: "rgba(34, 197, 94, 0.2)",
  },
  "tb-run-expanded": {
    fill: "#0a1520",
    stroke: "rgba(59, 130, 246, 0.3)",
    header: "rgba(59, 130, 246, 0.15)",
    accent: "rgba(96, 165, 250, 0.9)",
    mutedText: "rgba(191, 219, 254, 0.8)",
    glow: "rgba(59, 130, 246, 0.2)",
  },
  "tb-task": {
    fill: "#0a0f15",
    stroke: "rgba(255, 255, 255, 0.1)",
    header: "rgba(255, 255, 255, 0.05)",
    accent: "rgba(255, 255, 255, 0.3)",
    mutedText: "rgba(229, 231, 235, 0.6)",
    glow: "rgba(255, 255, 255, 0.08)",
  },
}

function getNodeTheme(node: PositionedNode): NodeTheme {
  return NODE_THEMES[node.type] ?? {
    fill: "#0d0f16",
    stroke: "rgba(255, 255, 255, 0.12)",
    header: "rgba(255, 255, 255, 0.06)",
    accent: "rgba(255, 255, 255, 0.3)",
    mutedText: "rgba(229, 231, 235, 0.6)",
    glow: "rgba(255, 255, 255, 0.1)",
  }
}

function getStatus(node: PositionedNode): Status | undefined {
  const status = node.metadata?.status
  if (status === "idle" || status === "busy" || status === "error" || status === "blocked" || status === "completed") {
    return status
  }
  return undefined
}

function getSubtitle(node: PositionedNode): string {
  if (node.type === "repo") {
    const tasks = (node.metadata?.taskCount as number | undefined) ?? null
    const open = (node.metadata?.openCount as number | undefined) ?? null
    if (tasks !== null && open !== null) {
      return `${tasks} tasks • ${open} open`
    }
    return "Repository"
  }
  if (node.type === "task") {
    const priority = node.metadata?.priority as number | undefined
    const kind = node.metadata?.taskType as string | undefined
    const parts = []
    if (kind) parts.push(kind)
    if (priority !== undefined) parts.push(`P${priority}`)
    return parts.join(" • ") || "Task"
  }
  if (node.type === "agent") {
    return "Desktop agent loop"
  }
  if (node.type === "workflow") {
    return "Loop phases"
  }
  if (node.type === "phase") {
    return "Phase"
  }
  if (node.type === "root") {
    return "OpenAgents"
  }
  // Terminal-Bench node types
  if (node.type === "tb-root") {
    const totalRuns = node.metadata?.totalRuns as number | undefined
    return totalRuns ? `${totalRuns} runs` : "Terminal-Bench"
  }
  if (node.type === "tb-controls") {
    return "Controls"
  }
  if (node.type === "tb-timeline") {
    return "Run history"
  }
  if (node.type === "tb-run-summary") {
    const suiteName = node.metadata?.suiteName as string | undefined
    const taskCount = node.metadata?.taskCount as number | undefined
    const parts = []
    if (suiteName) parts.push(suiteName)
    if (taskCount) parts.push(`${taskCount} tasks`)
    return parts.join(" • ") || "Run"
  }
  if (node.type === "tb-run-expanded") {
    const timestamp = node.metadata?.timestamp as string | undefined
    if (timestamp) {
      const date = new Date(timestamp)
      return date.toLocaleString("en-US", {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit"
      })
    }
    return "Expanded run"
  }
  if (node.type === "tb-task") {
    const difficulty = node.metadata?.difficulty as string | undefined
    const durationMs = node.metadata?.durationMs as number | undefined
    const parts = []
    if (difficulty) parts.push(difficulty)
    if (durationMs) parts.push(`${(durationMs / 1000).toFixed(1)}s`)
    return parts.join(" • ") || "Task"
  }
  return node.metadata?.path as string ?? ""
}

// Render a single node as layered card closer to Unkey styling
function renderNode(node: PositionedNode, config: RenderConfig): SVGGroup {
  const { x, y, size, label, id } = node
  const theme = getNodeTheme(node)
  const status = getStatus(node)
  const statusColor = status ? config.statusColors[status] : theme.accent
  const headerHeight = Math.max(24, Math.min(32, size.height * 0.32))
  const padding = 14
  const pillHeight = 16
  const pillWidth = 70

  const glow: SVGRect = {
    type: "rect",
    x: x - 6,
    y: y - 6,
    width: size.width + 12,
    height: size.height + 12,
    rx: config.nodeCornerRadius + 6,
    ry: config.nodeCornerRadius + 6,
    fill: theme.glow,
    opacity: 0.35,
  }
  
  const base: SVGRect = {
    type: "rect",
    x,
    y,
    width: size.width,
    height: size.height,
    rx: config.nodeCornerRadius,
    ry: config.nodeCornerRadius,
    fill: theme.fill,
    stroke: theme.stroke || config.nodeStroke,
    strokeWidth: config.nodeStrokeWidth,
    className: `flow-node flow-node-${node.type}`,
    dataNodeId: id,
  }

  const header: SVGRect = {
    type: "rect",
    x: x + 1,
    y: y + 1,
    width: size.width - 2,
    height: headerHeight,
    rx: config.nodeCornerRadius - 4,
    ry: config.nodeCornerRadius - 4,
    fill: theme.header,
    stroke: "transparent",
    className: "flow-node-header",
  }

  const accentBar: SVGRect = {
    type: "rect",
    x: x + 1,
    y: y + headerHeight - 2,
    width: size.width - 2,
    height: 2,
    fill: statusColor,
    opacity: 0.9,
    className: "flow-node-accent",
  }

  const labelText: SVGText = {
    type: "text",
    x: x + padding,
    y: y + headerHeight / 2 + 2,
    text: label,
    fontSize: config.fontSize + 1,
    fontFamily: config.fontFamily,
    fill: config.textColor,
    textAnchor: "start",
    dominantBaseline: "middle",
    className: "flow-node-label",
  }

  const subtitle: SVGText = {
    type: "text",
    x: x + padding,
    y: y + headerHeight + (size.height - headerHeight) / 2,
    text: getSubtitle(node),
    fontSize: config.fontSize - 1,
    fontFamily: config.fontFamily,
    fill: theme.mutedText,
    textAnchor: "start",
    dominantBaseline: "middle",
    className: "flow-node-subtitle",
  }

  const statusPill: SVGRect = {
    type: "rect",
    x: x + size.width - padding - pillWidth,
    y: y + (headerHeight - pillHeight) / 2,
    width: pillWidth,
    height: pillHeight,
    rx: pillHeight / 2,
    ry: pillHeight / 2,
    fill: statusColor,
    stroke: config.connectionStrokeMuted,
    strokeWidth: 1,
    opacity: 0.9,
    className: "flow-node-status-pill",
  }

  const statusText: SVGText = {
    type: "text",
    x: statusPill.x + pillWidth / 2,
    y: statusPill.y + pillHeight / 2 + 0.5,
    text: status ?? "idle",
    fontSize: config.fontSize - 3,
    fontFamily: config.fontFamily,
    fill: "#0b0b0f",
    textAnchor: "middle",
    dominantBaseline: "middle",
    className: "flow-node-status-text",
  }

  return {
    type: "g",
    className: "flow-node-group",
    children: [
      glow,
      base,
      header,
      accentBar,
      labelText,
      subtitle,
      statusPill,
      statusText,
    ],
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
    strokeOpacity: 0.9,
    strokeDasharray: "2 14",
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
      if (element.opacity !== undefined) attrs.push(`opacity="${element.opacity}"`)
      if (element.stroke) attrs.push(`stroke="${element.stroke}"`)
      if (element.strokeWidth) attrs.push(`stroke-width="${element.strokeWidth}"`)
      if (element.strokeOpacity !== undefined) attrs.push(`stroke-opacity="${element.strokeOpacity}"`)
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
      if (element.strokeOpacity !== undefined) attrs.push(`stroke-opacity="${element.strokeOpacity}"`)
      if (element.opacity !== undefined) attrs.push(`opacity="${element.opacity}"`)
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
function generateGridDefs(gridSize: number = 24, dotRadius: number = 1.2): string {
  return `
  <defs>
    <pattern id="dot-grid" x="0" y="0" width="${gridSize}" height="${gridSize}" patternUnits="userSpaceOnUse">
      <circle cx="${gridSize / 2}" cy="${gridSize / 2}" r="${dotRadius}" fill="rgba(255, 255, 255, 0.08)">
        <animate attributeName="opacity" values="0.3;0.8;0.3" dur="4s" repeatCount="indefinite" />
      </circle>
      <circle cx="${(gridSize / 2) + 6}" cy="${(gridSize / 2) + 6}" r="${dotRadius}" fill="rgba(255, 255, 255, 0.05)">
        <animate attributeName="opacity" values="0.2;0.6;0.2" dur="5s" repeatCount="indefinite" />
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
