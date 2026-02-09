export type Point = { readonly x: number; readonly y: number }

export type TreeNode = {
  readonly id: string
  readonly direction?: "vertical" | "horizontal"
  readonly children?: ReadonlyArray<TreeNode>
  readonly size?: { readonly width: number; readonly height: number }
  readonly [key: string]: unknown
}

export type PositionedNode<T extends TreeNode> = {
  readonly node: T
  readonly position: Point
  readonly level: number
}

/**
 * Dimensions of a rendered node in pixels
 */
type NodeDimensions = {
  readonly width: number
  readonly height: number
}

/**
 * Connection line between parent and child nodes
 */
type Connection<T extends TreeNode> = {
  readonly parent: T
  readonly child: T
  readonly path: ReadonlyArray<Point>
}

/**
 * Configuration for tree layout calculation
 */
export type LayoutConfig = {
  /** Space between adjacent nodes */
  readonly spacing: { readonly x: number; readonly y: number }
  /** Tree growth direction - defaults to horizontal (children spread left-to-right) */
  readonly direction?: "vertical" | "horizontal"
  /** Layout tuning for different orientations */
  readonly layout?: {
    readonly horizontalIndent?: number
    readonly verticalOffset?: number
    readonly subtreeOverlap?: number
    readonly verticalSiblingSpacing?: number
  }
  /** Connection line configuration */
  readonly connections?: {
    readonly vertical?: {
      readonly trunkOffset?: number
      readonly trunkAdjust?: number
    }
  }
}

/**
 * Complete layout calculation result containing positioned nodes and their connections
 */
type LayoutResult<T extends TreeNode> = {
  readonly nodes: ReadonlyArray<PositionedNode<T>>
  readonly connections: ReadonlyArray<Connection<T>>
}

/**
 * Pure layout calculation engine for tree structures.
 * Requires all node dimensions before calculating positions.
 * Throws immediately on missing data or invalid state.
 */
export class LayoutEngine<T extends TreeNode> {
  private readonly config: Required<LayoutConfig> & {
    readonly layout: Required<NonNullable<LayoutConfig["layout"]>>
    readonly connections: Required<NonNullable<LayoutConfig["connections"]>> & {
      readonly vertical: Required<NonNullable<NonNullable<LayoutConfig["connections"]>["vertical"]>>
    }
  }
  private readonly dimensions: Map<string, NodeDimensions>

  constructor(config: LayoutConfig) {
    this.config = {
      spacing: config.spacing,
      direction: config.direction ?? "horizontal",
      layout: {
        horizontalIndent: config.layout?.horizontalIndent ?? 60,
        verticalOffset: config.layout?.verticalOffset ?? -25,
        subtreeOverlap: config.layout?.subtreeOverlap ?? 0.4,
        verticalSiblingSpacing: config.layout?.verticalSiblingSpacing ?? 0.833,
      },
      connections: {
        vertical: {
          trunkOffset: config.connections?.vertical?.trunkOffset ?? 0,
          trunkAdjust: config.connections?.vertical?.trunkAdjust ?? 20,
        },
      },
    }

    invariant(
      this.config.layout.horizontalIndent !== undefined,
      "Layout horizontalIndent must be defined",
    )
    invariant(
      this.config.layout.verticalOffset !== undefined,
      "Layout verticalOffset must be defined",
    )
    invariant(
      this.config.layout.subtreeOverlap !== undefined,
      "Layout subtreeOverlap must be defined",
    )
    invariant(
      this.config.layout.verticalSiblingSpacing !== undefined,
      "Layout verticalSiblingSpacing must be defined",
    )
    invariant(
      this.config.connections.vertical.trunkAdjust !== undefined,
      "Connection trunkAdjust must be defined",
    )

    this.dimensions = new Map()
  }

  setNodeDimension(id: string, dimensions: NodeDimensions): void {
    this.dimensions.set(id, dimensions)
  }

  hasAllDimensions(root: T): boolean {
    const allNodes = this.flattenTree(root)
    return allNodes.every((node) => this.dimensions.has(node.id))
  }

  calculate(root: T): LayoutResult<T> {
    invariant(
      this.hasAllDimensions(root),
      `Cannot calculate layout: missing dimensions for some nodes. Have ${this.dimensions.size} dimensions.`,
    )

    const positioned = this.buildNodeLayout(root, 0, { x: 0, y: 0 })
    const connections = this.buildConnections(positioned)

    return { nodes: positioned, connections }
  }

  flattenTree(root: T): T[] {
    const result: T[] = [root]
    if (root.children) {
      root.children.forEach((child) => {
        result.push(...this.flattenTree(child as T))
      })
    }
    return result
  }

  private getNodeDirection(node: T): "vertical" | "horizontal" {
    return node.direction ?? this.config.direction
  }

  private buildNodeLayout(
    node: T,
    level: number,
    parentPosition: Point,
  ): PositionedNode<T>[] {
    const positioned: PositionedNode<T>[] = []
    const nodeDim = this.dimensions.get(node.id)
    invariant(nodeDim, `Missing dimensions for node ${node.id}`)

    const nodePosition = level === 0 ? { x: 0, y: 0 } : parentPosition
    positioned.push({ node, position: nodePosition, level })

    if (node.children && node.children.length > 0) {
      const parentDirection = this.getNodeDirection(node)

      if (parentDirection === "vertical") {
        const subtreeHeights = node.children.map((child) =>
          this.calculateSubtreeHeight(child as T),
        )

        node.children.forEach((child, index) => {
          const childDim = this.dimensions.get(child.id)
          invariant(childDim, `Missing dimensions for child ${child.id}`)

          const childX = nodePosition.x - nodeDim.width / 2
          const childY = this.calculateChildYPosition(
            nodePosition.y + nodeDim.height / 2 + this.config.spacing.y,
            index,
            subtreeHeights,
          )

          const childPositioned = this.buildNodeLayout(child as T, level + 1, {
            x: childX + childDim.width / 2 + this.config.layout.horizontalIndent,
            y: childY + this.config.layout.verticalOffset,
          })

          positioned.push(...childPositioned)
        })
      } else {
        const subtreeWidths = node.children.map((child) =>
          this.calculateSubtreeWidth(child as T),
        )

        node.children.forEach((child, index) => {
          const childX = this.calculateChildXPosition(nodePosition.x, index, subtreeWidths)
          const childY = nodePosition.y + nodeDim.height / 2 + this.config.spacing.y

          const childDim = this.dimensions.get(child.id)
          invariant(childDim, `Missing dimensions for child ${child.id}`)

          const childPositioned = this.buildNodeLayout(child as T, level + 1, {
            x: childX,
            y: childY + childDim.height / 2,
          })

          positioned.push(...childPositioned)
        })
      }
    }

    return positioned
  }

  private calculateSubtreeWidth(node: T): number {
    const nodeDim = this.dimensions.get(node.id)
    invariant(nodeDim, `Missing dimensions for node ${node.id}`)

    if (!node.children || node.children.length === 0) {
      return nodeDim.width
    }

    const parentDirection = this.getNodeDirection(node)

    if (parentDirection === "horizontal") {
      const childWidths = node.children.map((child) =>
        this.calculateSubtreeWidth(child as T),
      )
      const totalChildWidth = childWidths.reduce((sum, w) => sum + w, 0)
      const spacing = (node.children.length - 1) * this.config.spacing.x
      return Math.max(nodeDim.width, totalChildWidth + spacing)
    }

    const childSubtreeWidths = node.children.map((child) =>
      this.calculateSubtreeWidth(child as T),
    )
    const maxChildWidth = Math.max(...childSubtreeWidths)
    return (
      nodeDim.width + this.config.spacing.x + maxChildWidth * this.config.layout.subtreeOverlap
    )
  }

  private calculateSubtreeHeight(node: T): number {
    const nodeDim = this.dimensions.get(node.id)
    invariant(nodeDim, `Missing dimensions for node ${node.id}`)

    if (!node.children || node.children.length === 0) {
      return nodeDim.height
    }

    const childDirection = this.getNodeDirection(node)

    if (childDirection === "vertical") {
      const childHeights = node.children.map((child) =>
        this.calculateSubtreeHeight(child as T),
      )
      const totalChildHeight = childHeights.reduce((sum, h) => sum + h, 0)
      const spacing = (node.children.length - 1) * this.config.spacing.y
      return Math.max(nodeDim.height, totalChildHeight + spacing)
    }

    const maxChildHeight = Math.max(
      ...node.children.map((child) => this.calculateSubtreeHeight(child as T)),
    )
    return nodeDim.height + this.config.spacing.y + maxChildHeight
  }

  private calculateChildXPosition(
    parentX: number,
    childIndex: number,
    subtreeWidths: number[],
  ): number {
    const childCount = subtreeWidths.length

    if (childCount === 1) {
      return parentX
    }

    const totalSubtreeWidth = subtreeWidths.reduce((sum, w) => sum + w, 0)
    const totalSpacing = (childCount - 1) * this.config.spacing.x
    const totalWidth = totalSubtreeWidth + totalSpacing

    const startX = parentX - totalWidth / 2

    let x = startX
    for (let i = 0; i < childIndex; i++) {
      x += subtreeWidths[i]! + this.config.spacing.x
    }

    x += subtreeWidths[childIndex]! / 2

    return x
  }

  private calculateChildYPosition(
    startY: number,
    childIndex: number,
    subtreeHeights: number[],
  ): number {
    let y = startY

    y += subtreeHeights[0]! / 2

    for (let i = 0; i < childIndex; i++) {
      y +=
        subtreeHeights[i]! / 2 +
        this.config.spacing.y * this.config.layout.verticalSiblingSpacing +
        subtreeHeights[i + 1]! / 2
    }

    return y
  }

  private buildConnections(positioned: PositionedNode<T>[]): Connection<T>[] {
    const connections: Connection<T>[] = []
    const posMap = new Map(positioned.map((p) => [p.node.id, p] as const))

    for (const pos of positioned) {
      if (!pos.node.children) {
        continue
      }

      const parentDim = this.dimensions.get(pos.node.id)
      invariant(
        parentDim,
        `Parent dimensions cannot be empty or undefined for node ${pos.node.id}`,
      )

      const parentEdges = getNodeEdges(pos.position, parentDim)
      const parentDirection = this.getNodeDirection(pos.node)

      for (const child of pos.node.children) {
        const childPos = posMap.get(child.id)
        invariant(childPos, `Cannot find positioned node for child ${child.id}`)

        const childDim = this.dimensions.get(child.id)
        invariant(
          childDim,
          `Child dimensions cannot be empty or undefined for node ${child.id}`,
        )

        const childEdges = getNodeEdges(childPos.position, childDim)

        const path = this.buildConnectionPath(
          pos.position,
          parentEdges,
          childPos.position,
          childEdges,
          parentDirection,
        )

        connections.push({
          parent: pos.node,
          child: childPos.node as T,
          path,
        })
      }
    }

    return connections
  }

  private buildConnectionPath(
    parentPos: Point,
    parentEdges: ReturnType<typeof getNodeEdges>,
    childPos: Point,
    childEdges: ReturnType<typeof getNodeEdges>,
    direction: "vertical" | "horizontal",
  ): Point[] {
    if (direction === "vertical") {
      const trunkX =
        parentEdges.left -
        this.config.connections.vertical.trunkOffset +
        this.config.connections.vertical.trunkAdjust

      return [
        { x: trunkX, y: parentPos.y },
        { x: trunkX, y: childPos.y },
        { x: childEdges.left, y: childPos.y },
      ]
    }

    const verticalGap = childEdges.top - parentEdges.bottom
    const midY = parentEdges.bottom + verticalGap * 0.5

    return [
      { x: parentPos.x, y: parentEdges.bottom },
      { x: parentPos.x, y: midY },
      { x: childPos.x, y: midY },
      { x: childPos.x, y: childEdges.top },
    ]
  }
}

function getNodeEdges(position: Point, dimensions: { readonly width: number; readonly height: number }) {
  return {
    left: position.x - dimensions.width / 2,
    right: position.x + dimensions.width / 2,
    top: position.y - dimensions.height / 2,
    bottom: position.y + dimensions.height / 2,
  }
}

export function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new Error(message)
  }
}

