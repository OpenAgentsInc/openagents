import { createFileRoute } from '@tanstack/react-router';
import { NostrEventSection } from '@/components/nostr/NostrEventSection';

export const Route = createFileRoute('/_app/event/$id')({
  component: EventPage,
});

function EventPage() {
  const { id } = Route.useParams();
  return (
    <div className="p-4 md:p-6">
      <NostrEventSection eventId={id} />
    </div>
  );
}
