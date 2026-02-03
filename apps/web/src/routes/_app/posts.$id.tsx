import { createFileRoute } from '@tanstack/react-router';
import { NostrPostSection } from '@/components/nostr/NostrPostSection';

export const Route = createFileRoute('/_app/posts/$id')({
  component: PostPage,
});

function PostPage() {
  const { id } = Route.useParams();
  return (
    <div className="p-4 md:p-6">
      <NostrPostSection eventId={id} />
    </div>
  );
}
