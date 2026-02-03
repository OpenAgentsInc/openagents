import { useEffect } from 'react';
import { createFileRoute } from '@tanstack/react-router';
import { CommunitiesGraph } from '@/components/nostr-grid/CommunitiesGraph';
import { posthogCapture } from '@/lib/posthog';

export const Route = createFileRoute('/_app/c/')({
  component: CommunitiesPage,
});

function CommunitiesPage() {
  useEffect(() => {
    posthogCapture('communities_view', { view: 'communities_graph' });
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <CommunitiesGraph />
    </div>
  );
}
