/**
 * Flow — public API
 *
 * Consumers can build a view that matches Unkey's deployment-network-view using only
 * these exports: CanvasBoundary, InfiniteCanvas, GridPattern, TreeLayout, overlay
 * components (NodeDetailsPanel, ProjectDetails, LiveIndicator, DevTreeGenerator),
 * tree-generate (buildFlowTree, PRESETS), SKELETON_TREE, node type guards, and node
 * components (RootNode, LeafNode, SkeletonNode).
 */

// Canvas
export { CanvasBoundary } from './CanvasBoundary';
export { InfiniteCanvas } from './InfiniteCanvas';
export { GridPattern } from './GridPattern';

// Overlay (deployment-network-view parity)
export { NodeDetailsPanel } from './NodeDetailsPanel';
export { ProjectDetails } from './ProjectDetails';
export { LiveIndicator } from './LiveIndicator';
export { DevTreeGenerator } from './DevTreeGenerator';

// Tree layout and connection
export { TreeLayout } from './TreeLayout';
export { TreeElementNode } from './TreeElementNode';
export {
  TreeConnectionLine,
  ANIMATION_PRESETS,
  type AnimationConfig,
  type PresetName,
} from './TreeConnectionLine';

// Layout engine
export type { Point, TreeNode, LayoutConfig, PositionedNode } from './layout-engine';
export { LayoutEngine, invariant } from './layout-engine';

// Types, type guards, and node components
export type { FlowNode, FlowNodeType } from './types';
export {
  NODE_SIZES,
  SKELETON_TREE,
  isRootNode,
  isLeafNode,
  isSkeletonNode,
} from './types';
export { RootNode, LeafNode, SkeletonNode } from './nodes';

// Tree-generate (simulate)
export { buildFlowTree, PRESETS } from './tree-generate';
export type { TreeGenerateConfig } from './tree-generate';
