import { createFileRoute } from '@tanstack/react-router';
import { NostrFeedSection } from '@/components/nostr/NostrFeedSection';

export const Route = createFileRoute('/_app/feed')({
  component: FeedPage,
});

function FeedPage() {
  return (
    <div className="p-4 md:p-6">
      <NostrFeedSection />
    </div>
  );
}
