import type { FlowNode } from '../types';

type LeafNodeProps = {
  node: FlowNode & { metadata: { type: 'leaf' } };
};

export function LeafNode({ node }: LeafNodeProps) {
  return (
    <div className="rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-card-foreground shadow-sm">
      {node.label}
    </div>
  );
}
