import { cn } from '@/lib/utils';
import type { FlowNode, FlowNodeMetadata } from '../types';
import { StatusDot } from '../ui';

type RootNodeProps = {
  node: FlowNode & { metadata: FlowNodeMetadata & { type: 'root' } };
  selected?: boolean;
};

export function RootNode({ node, selected = false }: RootNodeProps) {
  return (
    <div
      className={cn(
        'relative flex h-[36px] w-[140px] select-none items-center justify-between gap-2 rounded-lg border border-border bg-card px-3 shadow-sm',
        selected && 'border-primary/60 ring-2 ring-primary/20',
      )}
    >
      <div className="min-w-0 truncate text-sm font-semibold text-card-foreground">
        {node.label}
      </div>
      <StatusDot status={node.metadata?.status} />
    </div>
  );
}
