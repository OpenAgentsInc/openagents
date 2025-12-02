import type { FlowNode, NodeId, NodeSize, PositionedNode, Point } from './model.js'

export interface LayoutConfig {
  readonly padding: number
  readonly spacing: number
}

export interface LayoutInput {
  readonly root: FlowNode
  readonly nodeSizes: Readonly<Record<NodeId, NodeSize>>
  readonly config: LayoutConfig
}

export interface LayoutOutput {
  readonly nodes: readonly PositionedNode[]
  readonly connections: readonly Connection[]
}

interface Connection {
  readonly parentId: NodeId
  readonly childId: NodeId
  readonly waypoints: readonly Point[]
}

function collectNodeIds(node: FlowNode, ids: Set<NodeId>): void {
  if (ids.has(node.id)) {
    throw new Error(`Duplicate ID or cycle detected: ${node.id}`)
  }
  ids.add(node.id)
  for (const child of node.children ?? []) {
    collectNodeIds(child, ids)
  }
}

function layoutSubtree(
  node: FlowNode,
  x: number,
  y: number,
  nodeSizes: Readonly<Record<NodeId, NodeSize>>,
  config: LayoutConfig,
  allNodes: Map<NodeId, PositionedNode>,
  parentId?: NodeId
): PositionedNode {
  const size = nodeSizes[node.id]!
  const positioned: PositionedNode = { ...node, x, y, size }
  allNodes.set(node.id, positioned)

  const children = node.children ?? []
  if (children.length === 0) {
    return positioned
  }

  const dir = node.direction ?? 'vertical'
  const padding = config.padding
  const contentX = x + padding
  const contentY = y + padding
  const contentWidth = size.width - 2 * padding
  const contentHeight = size.height - 2 * padding

  if (contentWidth < 0 || contentHeight < 0) {
    throw new Error(`Padding too large for node ${node.id}: content ${contentWidth}x${contentHeight}`)
  }

  const childSizes = children.map(child => nodeSizes[child.id]!)
  const maxChildWidth = Math.max(...childSizes.map(s => s.width), 0)
  const maxChildHeight = Math.max(...childSizes.map(s => s.height), 0)

  if (dir === 'horizontal') {
    const childY = contentY + Math.max(0, (contentHeight - maxChildHeight) / 2)
    const totalRowWidth = childSizes.reduce((sum, s, i) => sum + s.width + (i > 0 ? config.spacing : 0), 0)
    const rowStartX = contentX + Math.max(0, (contentWidth - totalRowWidth) / 2)
    let curX = rowStartX
    for (const child of children) {
      layoutSubtree(child, curX, childY, nodeSizes, config, allNodes, node.id)
      curX += nodeSizes[child.id]!.width + config.spacing
    }
  } else {
    // vertical
    const childX = contentX + Math.max(0, (contentWidth - maxChildWidth) / 2)
    let curY = contentY
    for (const child of children) {
      layoutSubtree(child, childX, curY, nodeSizes, config, allNodes, node.id)
      curY += nodeSizes[child.id]!.height + config.spacing
    }
  }

  return positioned
}

function computeConnections(
  node: FlowNode,
  positionedNodes: Map<NodeId, PositionedNode>,
  conns: Connection[],
  config: LayoutConfig
): void {
  for (const child of node.children ?? []) {
    const parentPos = positionedNodes.get(node.id)!
    const childPos = positionedNodes.get(child.id)!
    const dir = node.direction ?? 'vertical'

    // Parent exit point
    let exitX: number, exitY: number
    if (dir === 'horizontal') {
      exitX = parentPos.x + parentPos.size.width - config.padding / 2
      exitY = parentPos.y + parentPos.size.height / 2
    } else {
      exitX = parentPos.x + parentPos.size.width / 2
      exitY = parentPos.y + parentPos.size.height - config.padding / 2
    }

    // Child entry point
    const childDir = child.direction ?? 'vertical'
    let entryX: number, entryY: number
    if (childDir === 'horizontal') {
      entryX = childPos.x + config.padding / 2
      entryY = childPos.y + childPos.size.height / 2
    } else {
      entryX = childPos.x + childPos.size.width / 2
      entryY = childPos.y + config.padding / 2
    }

    conns.push({
      parentId: node.id,
      childId: child.id,
      waypoints: [{ x: exitX, y: exitY }, { x: entryX, y: entryY }]
    })

    computeConnections(child, positionedNodes, conns, config)
  }
}

/**
 * Computes layout positions and connections for a flow tree.
 * Fails fast on cycles/duplicates, missing/invalid sizes, or excessive padding.
 */
export function calculateLayout(input: LayoutInput): LayoutOutput {
  const { root, nodeSizes, config } = input

  // Validate tree structure
  const allIds = new Set<NodeId>()
  collectNodeIds(root, allIds)

  // Validate sizes
  for (const id of allIds) {
    const size = nodeSizes[id]
    if (!size || size.width <= 0 || size.height <= 0) {
      throw new Error(`Invalid or missing size for node "${id}": ${JSON.stringify(size)}`)
    }
  }

  const allNodes = new Map<NodeId, PositionedNode>()
  layoutSubtree(root, 0, 0, nodeSizes, config, allNodes)

  const connections: Connection[] = []
  computeConnections(root, allNodes, connections, config)

  return {
    nodes: Array.from(allNodes.values()),
    connections
  }
}
