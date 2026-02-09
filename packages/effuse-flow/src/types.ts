import type { TreeNode } from "./layout-engine.js"

export type FlowNodeType = "root" | "leaf" | "skeleton"

export type FlowNodeStatus = "ok" | "live" | "running" | "pending" | "error"

export type FlowNodeBadgeTone =
  | "neutral"
  | "info"
  | "success"
  | "warning"
  | "destructive"

export type FlowNodeBadge = {
  readonly label: string
  readonly tone?: FlowNodeBadgeTone
}

export type FlowNodeMetadata = {
  readonly type: FlowNodeType
  readonly kind?: string
  readonly subtitle?: string
  readonly detail?: string
  readonly status?: FlowNodeStatus
  readonly updatedAt?: string
  readonly badge?: FlowNodeBadge
  readonly [key: string]: unknown
}

export type FlowNode = TreeNode & {
  readonly id: string
  readonly label: string
  readonly direction?: "vertical" | "horizontal"
  readonly children?: ReadonlyArray<FlowNode>
  readonly metadata?: FlowNodeMetadata
}

export function isRootNode(node: FlowNode): node is FlowNode & { readonly metadata: { readonly type: "root" } } {
  return node.metadata?.type === "root"
}

export function isLeafNode(node: FlowNode): node is FlowNode & { readonly metadata: { readonly type: "leaf" } } {
  return node.metadata?.type === "leaf"
}

export function isSkeletonNode(
  node: FlowNode,
): node is FlowNode & { readonly metadata: { readonly type: "skeleton" } } {
  return node.metadata?.type === "skeleton"
}

const DEFAULT_NODE_WIDTH = 180
const DEFAULT_NODE_HEIGHT = 56

export const NODE_SIZES: Record<FlowNodeType, { readonly width: number; readonly height: number }> = {
  root: { width: 140, height: 36 },
  leaf: { width: DEFAULT_NODE_WIDTH, height: DEFAULT_NODE_HEIGHT },
  skeleton: { width: DEFAULT_NODE_WIDTH, height: DEFAULT_NODE_HEIGHT },
}

/** Loading-state tree: root + skeleton children. Use when apiTree is not yet available. */
export const SKELETON_TREE: FlowNode = {
  id: "root",
  label: "OpenAgents",
  direction: "horizontal",
  metadata: { type: "root" },
  children: [
    { id: "skeleton-1", label: "", metadata: { type: "skeleton" } },
    { id: "skeleton-2", label: "", metadata: { type: "skeleton" } },
    { id: "skeleton-3", label: "", metadata: { type: "skeleton" } },
  ],
}

