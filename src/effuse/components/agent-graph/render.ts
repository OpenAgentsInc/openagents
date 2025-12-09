/**
 * SVG Rendering Functions for Agent Graph
 * Factorio-inspired aesthetic: dark squares, white borders, dotted connections
 */

import type { SimNode, GraphConnection } from "./types.js"

// ============================================================================
// Node Rendering
// ============================================================================

/**
 * Render a node as an SVG rect with label
 */
export function renderNode(node: SimNode, hovered: boolean): string {
  const fill = hovered ? "#0a0a0a" : "#050505"
  const strokeOpacity = hovered ? 1.0 : 0.9

  return `
    <rect
      x="${node.x - node.width / 2}"
      y="${node.y - node.height / 2}"
      width="${node.width}"
      height="${node.height}"
      fill="${fill}"
      stroke="white"
      stroke-width="2"
      stroke-opacity="${strokeOpacity}"
      data-node-id="${node.id}"
      style="cursor: pointer; user-select: none;"
    />
    <text
      x="${node.x}"
      y="${node.y}"
      text-anchor="middle"
      dominant-baseline="middle"
      fill="white"
      font-family="Berkeley Mono, monospace"
      font-size="11"
      pointer-events="none"
      style="user-select: none;"
    >
      ${node.label}
    </text>
  `
}

// ============================================================================
// Connection Rendering
// ============================================================================

/**
 * Render a connection between two nodes as a dotted line
 */
export function renderConnection(
  from: SimNode,
  to: SimNode,
  dashOffset: number
): string {
  // Connection goes from right edge of 'from' to left edge of 'to'
  const fx = from.x + from.width / 2
  const fy = from.y
  const tx = to.x - to.width / 2
  const ty = to.y

  return `
    <line
      x1="${fx}" y1="${fy}"
      x2="${tx}" y2="${ty}"
      stroke="#999999"
      stroke-width="2"
      stroke-opacity="0.6"
      stroke-dasharray="8 6"
      stroke-dashoffset="${dashOffset}"
    />
  `
}

// ============================================================================
// Background Pattern
// ============================================================================

/**
 * Render grid background pattern
 */
export function renderGridPattern(): string {
  return `
    <defs>
      <pattern id="grid" width="24" height="24" patternUnits="userSpaceOnUse">
        <circle cx="12" cy="12" r="1.2" fill="white" opacity="0.08"/>
      </pattern>
    </defs>
    <rect width="100%" height="100%" fill="url(#grid)"/>
  `
}

// ============================================================================
// Full Graph Rendering
// ============================================================================

/**
 * Render the complete graph with connections and nodes
 */
export function renderGraph(
  nodes: SimNode[],
  connections: GraphConnection[],
  hoveredNodeId: string | null,
  dashOffset: number,
  pan: { x: number; y: number },
  zoom: number
): string {
  // Find node by ID helper
  const nodeById = (id: string) => nodes.find((n) => n.id === id)

  // Render connections
  const connectionsHtml = connections
    .map((conn) => {
      const from = nodeById(conn.from)
      const to = nodeById(conn.to)
      if (!from || !to) return ""
      return renderConnection(from, to, dashOffset)
    })
    .filter((s) => s !== "")
    .join("\n")

  // Render nodes
  const nodesHtml = nodes
    .map((node) => renderNode(node, node.id === hoveredNodeId))
    .join("\n")

  return `
    ${renderGridPattern()}
    <g transform="translate(${pan.x},${pan.y}) scale(${zoom})">
      ${connectionsHtml}
      ${nodesHtml}
    </g>
  `
}
