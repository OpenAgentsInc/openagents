/**
 * SVG Rendering Functions for TestGen Graph
 * Factorio-inspired aesthetic with status-based coloring
 */

import type { TestGenNode, TestGenConnection } from "./types.js"

// ============================================================================
// Status Colors
// ============================================================================

function getStatusColor(status: TestGenNode["status"]): {
  fill: string
  stroke: string
  strokeWidth: number
  opacity: number
} {
  switch (status) {
    case "waiting":
      return {
        fill: "#050505",
        stroke: "#666666",
        strokeWidth: 1.5,
        opacity: 0.5,
      }
    case "running":
      return {
        fill: "#0a0a0a",
        stroke: "#888888",
        strokeWidth: 2,
        opacity: 0.8,
      }
    case "completed":
      // Green only for succeeded/completed
      return {
        fill: "#0a1f0a",
        stroke: "#22c55e",
        strokeWidth: 2,
        opacity: 0.9,
      }
    case "failed":
      return {
        fill: "#0a0a0a",
        stroke: "#888888",
        strokeWidth: 2,
        opacity: 0.6,
      }
    case "partial":
      return {
        fill: "#0a0a0a",
        stroke: "#aaaaaa",
        strokeWidth: 2,
        opacity: 0.7,
      }
  }
}

// ============================================================================
// Node Rendering
// ============================================================================

/**
 * Render a TestGen node with status-based styling
 */
export function renderNode(node: TestGenNode, hovered: boolean): string {
  const colors = getStatusColor(node.status)
  const fill = hovered ? "#0f0f0f" : colors.fill
  const strokeOpacity = hovered ? 1.0 : colors.opacity

  // Build label with data
  let label = node.label
  if (node.data) {
    if (node.data.testCount !== undefined) {
      label = `TestGen\n${node.data.testCount} tests`
    } else if (node.data.subtaskCount !== undefined) {
      label = `Decomposer\n${node.data.subtaskCount} subtasks`
    } else if (node.data.categoryName) {
      label = `${node.data.categoryName}\n${node.data.categoryTestCount || 0} tests`
    } else if (node.data.subtaskName) {
      label = node.data.subtaskName
    } else if (node.data.taskName) {
      label = `Task\n${node.data.taskName}`
    } else if (node.data.percentage !== undefined) {
      label = `Progress\n${node.data.percentage.toFixed(1)}%`
      if (node.data.bestPercentage !== undefined) {
        label += `\n(best: ${node.data.bestPercentage.toFixed(1)}%)`
      }
    } else if (node.data.passed !== undefined && node.data.total !== undefined) {
      label = `Verifier\n${node.data.passed}/${node.data.total}`
    } else if (node.data.action) {
      label = `FM\n${node.data.action}`
      if (node.data.toolName) {
        label += `\n${node.data.toolName}`
      }
    }
  }

  // Split label into lines
  const lines = label.split("\n")
  const lineHeight = 14
  const totalHeight = lines.length * lineHeight
  const startY = node.y - totalHeight / 2 + lineHeight

  // Add pulsing animation for running nodes
  const pulseClass = node.status === "running" ? 'class="pulse-node"' : ""

  return `
    <g ${pulseClass}>
      <rect
        x="${node.x - node.width / 2}"
        y="${node.y - node.height / 2}"
        width="${node.width}"
        height="${node.height}"
        fill="${fill}"
        stroke="${colors.stroke}"
        stroke-width="${colors.strokeWidth}"
        stroke-opacity="${strokeOpacity}"
        data-node-id="${node.id}"
        style="cursor: pointer; user-select: none;"
        rx="4"
      />
      ${lines
        .map(
          (line, i) => `
        <text
          x="${node.x}"
          y="${startY + i * lineHeight}"
          text-anchor="middle"
          dominant-baseline="middle"
          fill="white"
          font-family="Berkeley Mono, monospace"
          font-size="11"
          pointer-events="none"
          style="user-select: none;"
        >
          ${line}
        </text>
      `
        )
        .join("")}
    </g>
  `
}

// ============================================================================
// Connection Rendering
// ============================================================================

/**
 * Render a connection between two nodes
 */
export function renderConnection(
  from: TestGenNode,
  to: TestGenNode,
  dashOffset: number,
  style: "normal" | "feedback" = "normal"
): string {
  // Connection goes from right edge of 'from' to left edge of 'to'
  const fx = from.x + from.width / 2
  const fy = from.y
  const tx = to.x - to.width / 2
  const ty = to.y

  if (style === "feedback") {
    // Curved feedback connection
    const midX = (fx + tx) / 2
    const midY = Math.min(fy, ty) - 50 // Curve upward
    const path = `M ${fx} ${fy} Q ${midX} ${midY} ${tx} ${ty}`

    return `
      <path
        d="${path}"
        fill="none"
        stroke="#999999"
        stroke-width="2"
        stroke-opacity="0.4"
        stroke-dasharray="8 6"
        stroke-dashoffset="${dashOffset}"
        style="user-select: none;"
      />
    `
  } else {
    // Straight connection
    return `
      <line
        x1="${fx}" y1="${fy}"
        x2="${tx}" y2="${ty}"
        stroke="#999999"
        stroke-width="2"
        stroke-opacity="0.6"
        stroke-dasharray="8 6"
        stroke-dashoffset="${dashOffset}"
        style="user-select: none;"
      />
    `
  }
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
      <pattern id="testgen-grid" width="24" height="24" patternUnits="userSpaceOnUse">
        <circle cx="12" cy="12" r="1.2" fill="white" opacity="0.08"/>
      </pattern>
      <style>
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.6; }
        }
        .pulse-node {
          animation: pulse 1.5s ease-in-out infinite;
        }
      </style>
    </defs>
    <rect width="100%" height="100%" fill="url(#testgen-grid)"/>
  `
}

// ============================================================================
// Full Graph Rendering
// ============================================================================

/**
 * Render the complete TestGen graph
 */
export function renderGraph(
  nodes: TestGenNode[],
  connections: TestGenConnection[],
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
      return renderConnection(from, to, dashOffset, conn.style || "normal")
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
