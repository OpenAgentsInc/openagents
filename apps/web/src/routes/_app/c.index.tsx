import { createFileRoute } from '@tanstack/react-router';
import { CommunitiesGraph } from '@/components/nostr-grid/CommunitiesGraph';

export const Route = createFileRoute('/_app/c/')({
  component: CommunitiesPage,
});

function CommunitiesPage() {
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <CommunitiesGraph />
    </div>
  );
}
