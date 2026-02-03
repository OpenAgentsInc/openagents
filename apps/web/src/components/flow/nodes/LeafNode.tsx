import { cn } from '@/lib/utils';
import type { FlowNode, FlowNodeMetadata } from '../types';
import { Pill, StatusDot } from '../ui';

type LeafNodeProps = {
  node: FlowNode & { metadata: FlowNodeMetadata & { type: 'leaf' } };
  selected?: boolean;
};

export function LeafNode({ node, selected = false }: LeafNodeProps) {
  const subtitle = node.metadata?.subtitle;
  const status = node.metadata?.status;
  const badge = node.metadata?.badge;

  return (
    <div
      className={cn(
        'relative h-[56px] w-[180px] select-none rounded-lg border border-border bg-card px-3 py-2 shadow-sm',
        selected && 'border-primary/60 ring-2 ring-primary/20',
      )}
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold text-card-foreground">
            {node.label}
          </div>
          <div className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
            {subtitle ?? ''}
          </div>
        </div>
        <div className="mt-0.5 flex shrink-0 flex-col items-end gap-1">
          <StatusDot status={status} />
          {badge?.label ? (
            <Pill tone={badge.tone} className="text-[10px]">
              {badge.label}
            </Pill>
          ) : null}
        </div>
      </div>
    </div>
  );
}
