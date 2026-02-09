/**
 * Effuse Flow
 *
 * Effect-native (Effuse template) SVG tree flow graph:
 * - layout engine (pure)
 * - SVG connection paths (rounded + animated)
 * - infinite canvas (pan/zoom) hydration
 * - basic node + overlay templates
 */

export type { Point, TreeNode, LayoutConfig, PositionedNode } from "./layout-engine.js"
export { LayoutEngine, invariant } from "./layout-engine.js"

export type {
  FlowNode,
  FlowNodeType,
  FlowNodeStatus,
  FlowNodeBadgeTone,
  FlowNodeBadge,
  FlowNodeMetadata,
} from "./types.js"
export {
  NODE_SIZES,
  SKELETON_TREE,
  isRootNode,
  isLeafNode,
  isSkeletonNode,
} from "./types.js"

export type { AnimationConfig, PresetName } from "./TreeConnectionLine.js"
export { TreeConnectionLine, ANIMATION_PRESETS } from "./TreeConnectionLine.js"
export { TreeElementNode } from "./TreeElementNode.js"
export { TreeLayout, type TreeLayoutProps } from "./TreeLayout.js"

export { GridPattern } from "./GridPattern.js"
export {
  InfiniteCanvas,
  type InfiniteCanvasProps,
  hydrateInfiniteCanvas,
} from "./InfiniteCanvas.js"

export { Pill, StatusDot, StatusPill } from "./ui.js"
export { RootNode } from "./nodes/RootNode.js"
export { LeafNode } from "./nodes/LeafNode.js"
export { SkeletonNode } from "./nodes/SkeletonNode.js"

export { NodeDetailsPanel, type NodeDetailsPanelProps } from "./NodeDetailsPanel.js"
export { ProjectDetails } from "./ProjectDetails.js"
export { LiveIndicator } from "./LiveIndicator.js"
export { DevTreeGenerator, type DevTreeGeneratorProps } from "./DevTreeGenerator.js"

export { buildFlowTree, PRESETS } from "./tree-generate.js"
export type { TreeGenerateConfig } from "./tree-generate.js"

export { FLOW_STYLES_CSS, FlowStyles } from "./styles.js"
