import { Button } from '@/components/ui/button';
import type { FlowNode } from './types';
import { buildFlowTree, PRESETS } from './tree-generate';

type DevTreeGeneratorProps = {
  onGenerate: (tree: FlowNode) => void;
  onReset: () => void;
};

/**
 * Dev-only overlay: preset buttons (Small, Medium, Large) and Reset to generate or clear a flow tree.
 * Renders nothing when not in dev (import.meta.env.DEV).
 */
export function DevTreeGenerator({ onGenerate, onReset }: DevTreeGeneratorProps) {
  if (!import.meta.env.DEV) {
    return null;
  }

  return (
    <div className="pointer-events-auto absolute bottom-4 left-4 flex flex-col gap-2 rounded-lg border border-border bg-card p-3 shadow-lg">
      <span className="text-xs font-medium text-muted-foreground">Dev: tree</span>
      <div className="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => onGenerate(buildFlowTree(PRESETS.small))}
        >
          Small
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => onGenerate(buildFlowTree(PRESETS.medium))}
        >
          Medium
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => onGenerate(buildFlowTree(PRESETS.large))}
        >
          Large
        </Button>
        <Button variant="ghost" size="sm" onClick={onReset}>
          Reset
        </Button>
      </div>
    </div>
  );
}
