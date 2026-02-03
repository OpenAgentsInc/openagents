import { Skeleton } from '@/components/ui/skeleton';
import type { FlowNode } from '../types';

type SkeletonNodeProps = {
  node: FlowNode & { metadata: { type: 'skeleton' } };
};

export function SkeletonNode({ node: _node }: SkeletonNodeProps) {
  return (
    <div className="rounded-lg border border-border bg-card px-4 py-2 shadow-sm">
      <Skeleton className="h-4 w-24 rounded" />
    </div>
  );
}
