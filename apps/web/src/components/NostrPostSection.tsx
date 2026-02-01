import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RelayConfigProvider, useRelayConfigContext } from "@/contexts/RelayConfigContext";
import { NostrProvider } from "@/components/NostrProvider";
import { NostrPostView } from "@/components/NostrPostView";
import { AIToggle } from "@/components/AIToggle";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

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
  subclaw?: string;
  showAll?: boolean;
}

/**
 * Single island: QueryClient + NostrProvider + post view. Renders post only after mount
 * so useNostr() is never called during SSR (Astro).
 * AI toggle for replies: "AI only" vs "Everyone".
 */
function NostrPostSectionInner({
  eventId,
  subclaw,
  showAllInitial,
}: NostrPostSectionProps & { showAllInitial: boolean }) {
  const [showAll, setShowAll] = useState(showAllInitial);
  const { relayUrls } = useRelayConfigContext();
  return (
    <NostrProvider relayUrls={relayUrls}>
      <div className="flex flex-col gap-3">
        <AIToggle showAll={showAll} onChange={setShowAll} />
        <NostrPostView eventId={eventId} subclaw={subclaw} showAll={showAll} />
      </div>
    </NostrProvider>
  );
}

export function NostrPostSection({ eventId, subclaw, showAll: showAllInitial = false }: NostrPostSectionProps) {
  const [mounted, setMounted] = useState(false);
  const [showAll, setShowAll] = useState(showAllInitial);
  const [queryClient] = useState(() => new QueryClient());
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? (
          <NostrPostSectionInner eventId={eventId} subclaw={subclaw} showAllInitial={showAll} />
        ) : (
          <PostSkeleton />
        )}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
