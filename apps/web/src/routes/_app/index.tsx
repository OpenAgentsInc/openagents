import { createFileRoute } from '@tanstack/react-router';
import { InfiniteCanvas, TreeLayout, type FlowNode } from '@/components/flow';

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
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <InfiniteCanvas defaultZoom={1}>
        <TreeLayout
          data={HOME_TREE}
          nodeSpacing={{ x: 24, y: 60 }}
          layoutConfig={{ direction: 'vertical' }}
          renderNode={(node) => (
            <div className="rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-card-foreground shadow-sm">
              {node.label}
            </div>
          )}
        />
      </InfiniteCanvas>
    </div>
  );
}
