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

/**
 * Measure the full subtree dimensions (including all descendants).
 * Returns the bounding box needed to render this node and all its children.
 */
function measureSubtree(
  node: FlowNode,
  nodeSizes: Readonly<Record<NodeId, NodeSize>>,
  spacing: number
): { width: number; height: number } {
  const size = nodeSizes[node.id]!
  const children = node.children ?? []

  if (children.length === 0) {
    return { width: size.width, height: size.height }
  }

  const dir = node.direction ?? 'vertical'
  const childMeasures = children.map(child => measureSubtree(child, nodeSizes, spacing))

  if (dir === 'horizontal') {
    // Children laid out horizontally below this node
    const childrenWidth = childMeasures.reduce((sum, m, i) => sum + m.width + (i > 0 ? spacing : 0), 0)
    const childrenHeight = Math.max(...childMeasures.map(m => m.height))
    return {
      width: Math.max(size.width, childrenWidth),
      height: size.height + spacing + childrenHeight,
    }
  } else {
    // Children laid out vertically below this node
    const childrenWidth = Math.max(...childMeasures.map(m => m.width))
    const childrenHeight = childMeasures.reduce((sum, m, i) => sum + m.height + (i > 0 ? spacing : 0), 0)
    return {
      width: Math.max(size.width, childrenWidth),
      height: size.height + spacing + childrenHeight,
    }
  }
}

/**
 * Layout a subtree with children positioned BELOW/BESIDE the parent (flow graph style),
 * not contained inside the parent bounds.
 */
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
  const spacing = config.spacing

  // Children start below this node
  const childStartY = y + size.height + spacing

  if (dir === 'horizontal') {
    // Lay out children horizontally, centered under this node
    const childMeasures = children.map(child => measureSubtree(child, nodeSizes, spacing))
    const totalWidth = childMeasures.reduce((sum, m, i) => sum + m.width + (i > 0 ? spacing : 0), 0)
    const startX = x + size.width / 2 - totalWidth / 2

    let curX = startX
    for (let i = 0; i < children.length; i++) {
      const child = children[i]
      const childMeasure = childMeasures[i]
      // Center child within its measured subtree width
      const childSize = nodeSizes[child.id]!
      const childX = curX + (childMeasure.width - childSize.width) / 2
      layoutSubtree(child, childX, childStartY, nodeSizes, config, allNodes, node.id)
      curX += childMeasure.width + spacing
    }
  } else {
    // Lay out children vertically, centered under this node
    let curY = childStartY
    for (const child of children) {
      const childSize = nodeSizes[child.id]!
      // Center child horizontally relative to parent
      const childX = x + (size.width - childSize.width) / 2
      layoutSubtree(child, childX, curY, nodeSizes, config, allNodes, node.id)
      const childMeasure = measureSubtree(child, nodeSizes, spacing)
      curY += childMeasure.height + spacing
    }
  }

  return positioned
}

function computeConnections(
  node: FlowNode,
  positionedNodes: Map<NodeId, PositionedNode>,
  conns: Connection[]
): void {
  for (const child of node.children ?? []) {
    const parentPos = positionedNodes.get(node.id)!
    const childPos = positionedNodes.get(child.id)!

    // For external flow layout:
    // Parent exit: bottom center
    // Child entry: top center
    const exitX = parentPos.x + parentPos.size.width / 2
    const exitY = parentPos.y + parentPos.size.height
    const entryX = childPos.x + childPos.size.width / 2
    const entryY = childPos.y

    // Create an elbow connection with a midpoint
    const midY = (exitY + entryY) / 2
    conns.push({
      parentId: node.id,
      childId: child.id,
      waypoints: [
        { x: exitX, y: exitY },
        { x: exitX, y: midY },
        { x: entryX, y: midY },
        { x: entryX, y: entryY }
      ]
    })

    computeConnections(child, positionedNodes, conns)
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
  computeConnections(root, allNodes, connections)

  return {
    nodes: Array.from(allNodes.values()),
    connections
  }
}
