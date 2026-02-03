import { Button } from '@/components/ui/button';
import type { FlowNode } from './types';

type NodeDetailsPanelProps = {
  node: FlowNode | null;
  onClose: () => void;
};

export function NodeDetailsPanel({ node, onClose }: NodeDetailsPanelProps) {
  if (node == null) {
    return null;
  }

  return (
    <div className="pointer-events-auto absolute right-4 top-4 w-72 rounded-lg border border-border bg-card p-4 shadow-lg">
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium text-card-foreground">{node.label}</p>
          <p className="mt-1 truncate text-xs text-muted-foreground" title={node.id}>
            {node.id}
          </p>
        </div>
        <Button variant="ghost" size="icon" className="shrink-0" onClick={onClose} aria-label="Close">
          ×
        </Button>
      </div>
    </div>
  );
}
