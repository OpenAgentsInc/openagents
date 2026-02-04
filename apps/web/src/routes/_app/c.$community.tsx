import { createFileRoute } from '@tanstack/react-router';
import { NostrFeedSection } from '@/components/nostr/NostrFeedSection';
import { isCommunityBlacklisted } from '@/lib/communityBlacklist';

export const Route = createFileRoute('/_app/c/$community')({
  component: CommunityFeedPage,
});

function CommunityFeedPage() {
  const { community } = Route.useParams();
  if (isCommunityBlacklisted(community)) {
    return (
      <div className="p-4 md:p-6 text-muted-foreground">
        This community is not available.
      </div>
    );
  }
  return (
    <div className="p-4 md:p-6">
      <NostrFeedSection community={community} />
    </div>
  );
}
