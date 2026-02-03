import type { TreeNode } from './layout-engine';

export type FlowNodeType = 'root' | 'leaf';

export type FlowNode = TreeNode & {
  id: string;
  label: string;
  direction?: 'vertical' | 'horizontal';
  children?: FlowNode[];
  metadata?: { type: FlowNodeType };
};

const DEFAULT_NODE_WIDTH = 180;
const DEFAULT_NODE_HEIGHT = 56;

export const NODE_SIZES: Record<FlowNodeType, { width: number; height: number }> = {
  root: { width: 140, height: 36 },
  leaf: { width: DEFAULT_NODE_WIDTH, height: DEFAULT_NODE_HEIGHT },
};
