import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

import {
  type LayoutConfig,
  LayoutEngine,
  type Point,
  type TreeNode,
} from "./layout-engine.js"
import type { FlowNode } from "./types.js"
import { NODE_SIZES } from "./types.js"
import type { AnimationConfig } from "./TreeConnectionLine.js"
import { TreeConnectionLine } from "./TreeConnectionLine.js"
import { TreeElementNode } from "./TreeElementNode.js"

export type TreeLayoutProps = {
  readonly data: FlowNode
  readonly nodeSpacing?: { readonly x: number; readonly y: number }
  readonly layoutConfig?: Omit<LayoutConfig, "spacing" | "direction"> & {
    readonly direction?: "vertical" | "horizontal"
  }
  readonly connectionAnimation?: AnimationConfig
  readonly renderNode: (node: FlowNode, parent?: FlowNode) => TemplateResult
  readonly renderConnection?: (path: ReadonlyArray<Point>, parent: FlowNode, child: FlowNode) => TemplateResult
}

const buildParentMap = (root: TreeNode): Map<string, FlowNode> => {
  const map = new Map<string, FlowNode>()
  const walk = (node: TreeNode) => {
    if (node.children) {
      for (const child of node.children) {
        map.set(child.id, node as FlowNode)
        walk(child)
      }
    }
  }
  walk(root)
  return map
}

export function TreeLayout({
  data,
  nodeSpacing = { x: 50, y: 50 },
  layoutConfig,
  connectionAnimation,
  renderNode,
  renderConnection,
}: TreeLayoutProps): TemplateResult {
  const layoutEngine = new LayoutEngine<FlowNode>({
    spacing: nodeSpacing,
    direction: layoutConfig?.direction ?? "vertical",
    layout: layoutConfig?.layout,
    connections: layoutConfig?.connections,
  })

  const parentMap = buildParentMap(data)
  const allNodes = layoutEngine.flattenTree(data)

  for (const node of allNodes) {
    const type = node.metadata?.type ?? "leaf"
    const size = NODE_SIZES[type]
    layoutEngine.setNodeDimension(node.id, size)
  }

  const layout = layoutEngine.calculate(data)

  return html`
    <g class="oa-flow-tree-layout" data-oa-flow-tree-layout="1">
      ${layout.connections.map((conn) =>
        renderConnection
          ? html`<g data-oa-flow-conn="${`${conn.parent.id}-${conn.child.id}`}">
              ${renderConnection(conn.path, conn.parent, conn.child)}
            </g>`
          : TreeConnectionLine({ path: conn.path, animation: connectionAnimation })
      )}
      ${layout.nodes.map((positioned) => {
        const parent = parentMap.get(positioned.node.id)
        return TreeElementNode({
          id: positioned.node.id,
          position: positioned.position,
          children: renderNode(positioned.node, parent),
        })
      })}
    </g>
  `
}
