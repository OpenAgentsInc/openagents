import type { TreeNode } from './layout-engine';

export type FlowNodeType = 'root' | 'leaf' | 'skeleton';

export type FlowNode = TreeNode & {
  id: string;
  label: string;
  direction?: 'vertical' | 'horizontal';
  children?: FlowNode[];
  metadata?: { type: FlowNodeType };
};

export function isSkeletonNode(
  node: FlowNode,
): node is FlowNode & { metadata: { type: 'skeleton' } } {
  return node.metadata?.type === 'skeleton';
}

const DEFAULT_NODE_WIDTH = 180;
const DEFAULT_NODE_HEIGHT = 56;

export const NODE_SIZES: Record<FlowNodeType, { width: number; height: number }> = {
  root: { width: 140, height: 36 },
  leaf: { width: DEFAULT_NODE_WIDTH, height: DEFAULT_NODE_HEIGHT },
  skeleton: { width: DEFAULT_NODE_WIDTH, height: DEFAULT_NODE_HEIGHT },
};

/** Loading-state tree: root + skeleton children. Use when apiTree is not yet available. */
export const SKELETON_TREE: FlowNode = {
  id: 'root',
  label: 'OpenAgents',
  direction: 'horizontal',
  metadata: { type: 'root' },
  children: [
    { id: 'skeleton-1', label: '', metadata: { type: 'skeleton' } },
    { id: 'skeleton-2', label: '', metadata: { type: 'skeleton' } },
    { id: 'skeleton-3', label: '', metadata: { type: 'skeleton' } },
  ],
};
