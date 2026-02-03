import { createFileRoute } from '@tanstack/react-router';
import { NostrCommunitiesSection } from '@/components/nostr/NostrCommunitiesSection';

export const Route = createFileRoute('/_app/c/')({
  component: CommunitiesPage,
});

function CommunitiesPage() {
  return (
    <div className="p-4 md:p-6">
      <h1 className="text-xl font-semibold mb-4">Communities</h1>
      <NostrCommunitiesSection />
    </div>
  );
}
