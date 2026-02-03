import { createFileRoute } from '@tanstack/react-router';
import { NostrGridHome } from '@/components/nostr-grid/NostrGridHome';

export const Route = createFileRoute('/_app/')({
  component: NostrGridHome,
});
