import type { FlowNode } from '../types';

type RootNodeProps = {
  node: FlowNode & { metadata: { type: 'root' } };
};

export function RootNode({ node }: RootNodeProps) {
  return (
    <div className="rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-card-foreground shadow-sm">
      {node.label}
    </div>
  );
}
