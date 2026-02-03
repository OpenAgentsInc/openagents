import { createFileRoute } from '@tanstack/react-router';
import { NostrFeedSection } from '@/components/nostr/NostrFeedSection';

export const Route = createFileRoute('/_app/c/$community')({
  component: CommunityFeedPage,
});

function CommunityFeedPage() {
  const { community } = Route.useParams();
  return (
    <div className="p-4 md:p-6">
      <NostrFeedSection community={community} />
    </div>
  );
}
