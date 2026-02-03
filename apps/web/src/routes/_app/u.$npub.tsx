import { createFileRoute } from '@tanstack/react-router';
import { NostrProfileSection } from '@/components/nostr/NostrProfileSection';

export const Route = createFileRoute('/_app/u/$npub')({
  component: ProfilePage,
});

function ProfilePage() {
  const { npub } = Route.useParams();
  return (
    <div className="p-4 md:p-6">
      <NostrProfileSection npub={npub} />
    </div>
  );
}
