import { useEffect, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { RelayConfigProvider, useRelayConfigContext } from "@/contexts/RelayConfigContext";
import { NostrProvider } from "@/components/nostr/NostrProvider";
import { NostrEventView } from "@/components/nostr/NostrEventView";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { getQueryClient } from "@/lib/queryClient";

function EventSkeleton() {
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

interface NostrEventSectionProps {
  eventId: string;
}

function NostrEventSectionInner({ eventId }: NostrEventSectionProps) {
  const { relayUrls } = useRelayConfigContext();
  return (
    <NostrProvider relayUrls={relayUrls}>
      <NostrEventView eventId={eventId} />
    </NostrProvider>
  );
}

export function NostrEventSection({ eventId }: NostrEventSectionProps) {
  const [mounted, setMounted] = useState(false);
  const [queryClient] = useState(() => getQueryClient());
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? <NostrEventSectionInner eventId={eventId} /> : <EventSkeleton />}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
