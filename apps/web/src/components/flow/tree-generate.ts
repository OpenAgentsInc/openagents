import type { FlowNode } from './types';

export type TreeGenerateConfig = {
  rootLabel: string;
  /** Explicit labels for children. If omitted, childCount is used with generated labels. */
  childLabels?: string[];
  /** Number of children when childLabels is not provided. Default 3. */
  childCount?: number;
  direction?: 'vertical' | 'horizontal';
};

/**
 * Builds a FlowNode tree from a simple config. No health/region logic; generic tree only.
 */
export function buildFlowTree(config: TreeGenerateConfig): FlowNode {
  const {
    rootLabel,
    childLabels,
    childCount = 3,
    direction = 'vertical',
  } = config;

  const count = childLabels?.length ?? childCount;
  const labels =
    childLabels ??
    Array.from({ length: count }, (_, i) => `Child ${i + 1}`);

  const children: FlowNode[] = labels.map((label, i) => ({
    id: `child-${i + 1}`,
    label,
    metadata: { type: 'leaf' },
  }));

  return {
    id: 'root',
    label: rootLabel,
    direction,
    metadata: { type: 'root' },
    children,
  };
}

/** Presets for buildFlowTree: small (3 children), medium (5), large (7). */
export const PRESETS = {
  small: {
    rootLabel: 'OpenAgents',
    childCount: 3,
    direction: 'vertical' as const,
  },
  medium: {
    rootLabel: 'OpenAgents',
    childCount: 5,
    direction: 'vertical' as const,
  },
  large: {
    rootLabel: 'OpenAgents',
    childCount: 7,
    direction: 'vertical' as const,
  },
} satisfies Record<string, TreeGenerateConfig>;
