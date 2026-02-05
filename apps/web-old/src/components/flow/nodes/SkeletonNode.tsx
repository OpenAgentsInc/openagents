import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';
import type { FlowNode } from '../types';

type SkeletonNodeProps = {
  node: FlowNode & { metadata: { type: 'skeleton' } };
  selected?: boolean;
};

export function SkeletonNode({ node: _node, selected = false }: SkeletonNodeProps) {
  return (
    <div
      className={cn(
        'h-[56px] w-[180px] select-none rounded-lg border border-border bg-card px-3 py-2 shadow-sm',
        selected && 'border-primary/60 ring-2 ring-primary/20',
      )}
    >
      <Skeleton className="mt-1 h-4 w-28 rounded" />
      <Skeleton className="mt-2 h-3 w-20 rounded" />
    </div>
  );
}
