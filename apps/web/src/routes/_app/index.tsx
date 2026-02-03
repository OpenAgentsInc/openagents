import { useState } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import {
  DevTreeGenerator,
  InfiniteCanvas,
  isLeafNode,
  isRootNode,
  isSkeletonNode,
  LeafNode,
  LiveIndicator,
  NodeDetailsPanel,
  ProjectDetails,
  RootNode,
  SKELETON_TREE,
  SkeletonNode,
  TreeLayout,
  type FlowNode,
} from '@/components/flow';

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

function renderFlowNode(node: FlowNode) {
  if (isRootNode(node)) return <RootNode node={node} />;
  if (isLeafNode(node)) return <LeafNode node={node} />;
  if (isSkeletonNode(node)) return <SkeletonNode node={node} />;
  return (
    <div className="rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-card-foreground shadow-sm">
      {node.label}
    </div>
  );
}

function Home() {
  const [selectedNode, setSelectedNode] = useState<FlowNode | null>(null);
  const [generatedTree, setGeneratedTree] = useState<FlowNode | null>(null);
  const apiTree: FlowNode | null = HOME_TREE;
  const currentTree = generatedTree ?? apiTree ?? SKELETON_TREE;
  const isShowingSkeleton = currentTree === SKELETON_TREE;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <InfiniteCanvas
        defaultZoom={1}
        overlay={
          <>
            <NodeDetailsPanel node={selectedNode} onClose={() => setSelectedNode(null)} />
            <ProjectDetails projectId={undefined} />
            <LiveIndicator />
            <DevTreeGenerator
              onGenerate={setGeneratedTree}
              onReset={() => setGeneratedTree(null)}
            />
          </>
        }
      >
        <TreeLayout
          data={currentTree}
          nodeSpacing={{ x: 24, y: 60 }}
          layoutConfig={{ direction: 'vertical' }}
          onNodeClick={isShowingSkeleton ? undefined : (node) => setSelectedNode(node)}
          renderNode={renderFlowNode}
        />
      </InfiniteCanvas>
    </div>
  );
}
