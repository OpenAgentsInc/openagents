import { createFileRoute } from '@tanstack/react-router';
import { lazy, Suspense } from 'react';

const NodeCanvas = lazy(() =>
  import('@/components/three/NodeCanvas').then((m) => ({ default: m.NodeCanvas })),
);

export const Route = createFileRoute('/_app/')({
  component: Home,
});

function Home() {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Suspense fallback={<div className="flex-1 bg-background" />}>
        <NodeCanvas />
      </Suspense>
    </div>
  );
}
