import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import {
  InfiniteCanvas,
  isSkeletonNode,
  NodeDetailsPanel,
  SKELETON_TREE,
  TreeLayout,
  type FlowNode,
} from '@/components/flow';
import { Skeleton } from '@/components/ui/skeleton';

export const Route = createFileRoute('/_app/')({
  component: Home,
});

const HOME_TREE: FlowNode = {
  id: 'root',
  label: 'OpenAgents',
  direction: 'horizontal',
  metadata: { type: 'root' },
  children: [
    {
      id: 'runtime',
      label: 'Runtime',
      metadata: { type: 'leaf' },
    },
    {
      id: 'agents',
      label: 'Agents',
      metadata: { type: 'leaf' },
    },
    {
      id: 'protocol',
      label: 'Protocol',
      metadata: { type: 'leaf' },
    },
  ],
};

function Home() {
  const [selectedNode, setSelectedNode] = useState<FlowNode | null>(null);
  const generatedTree: FlowNode | null = null;
  const apiTree: FlowNode | null = HOME_TREE;
  const currentTree = generatedTree ?? apiTree ?? SKELETON_TREE;
  const isShowingSkeleton = currentTree === SKELETON_TREE;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <InfiniteCanvas
        defaultZoom={1}
        overlay={
          <NodeDetailsPanel node={selectedNode} onClose={() => setSelectedNode(null)} />
        }
      >
        <TreeLayout
          data={currentTree}
          nodeSpacing={{ x: 24, y: 60 }}
          layoutConfig={{ direction: 'vertical' }}
          onNodeClick={isShowingSkeleton ? undefined : (node) => setSelectedNode(node)}
          renderNode={(node) =>
            isSkeletonNode(node) ? (
              <div className="rounded-lg border border-border bg-card px-4 py-2 shadow-sm">
                <Skeleton className="h-4 w-24 rounded" />
              </div>
            ) : (
              <div className="rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-card-foreground shadow-sm">
                {node.label}
              </div>
            )
          }
        />
      </InfiniteCanvas>
    </div>
  );
}
