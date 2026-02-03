import { useEffect, useState } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import {
  RelayConfigProvider,
  useRelayConfigContext,
} from '@/contexts/RelayConfigContext';
import { NostrProvider } from '@/components/nostr/NostrProvider';
import { NostrPostView } from '@/components/nostr/NostrPostView';
// import { AIToggle } from '@/components/nostr/AIToggle';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { getQueryClient } from '@/lib/queryClient';

function PostSkeleton() {
  return (
    <Card>
      <CardHeader>
        <Skeleton className="h-6 w-3/4" />
        <Skeleton className="h-4 w-1/2" />
      </CardHeader>
      <CardContent className="space-y-2">
        <Skeleton className="h-20 w-full" />
      </CardContent>
    </Card>
  );
}

interface NostrPostSectionProps {
  eventId: string;
  community?: string;
  showAll?: boolean;
}

function NostrPostSectionInner({
  eventId,
  community,
  showAllInitial,
}: NostrPostSectionProps & { showAllInitial: boolean }) {
  const [showAll, setShowAll] = useState(showAllInitial);
  const { relayMetadata } = useRelayConfigContext();
  return (
    <NostrProvider relayMetadata={relayMetadata}>
      <div className="flex flex-col gap-3">
        {/* <AIToggle showAll={showAll} onChange={setShowAll} source="post" /> */}
        <NostrPostView
          eventId={eventId}
          community={community}
          showAll={showAll}
        />
      </div>
    </NostrProvider>
  );
}

export function NostrPostSection({
  eventId,
  community,
  showAll: showAllInitial = false,
}: NostrPostSectionProps) {
  const [mounted, setMounted] = useState(false);
  const [queryClient] = useState(() => getQueryClient());
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? (
          <NostrPostSectionInner
            eventId={eventId}
            community={community}
            showAllInitial={showAllInitial}
          />
        ) : (
          <PostSkeleton />
        )}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
