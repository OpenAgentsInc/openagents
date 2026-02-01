import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { NostrProvider } from "@/components/NostrProvider";
import { NostrPostView } from "@/components/NostrPostView";
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
 */
export function NostrPostSection({ eventId, subclaw, showAll = false }: NostrPostSectionProps) {
  const [mounted, setMounted] = useState(false);
  const [queryClient] = useState(() => new QueryClient());
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <NostrProvider>
        {mounted ? (
          <NostrPostView eventId={eventId} subclaw={subclaw} showAll={showAll} />
        ) : (
          <PostSkeleton />
        )}
      </NostrProvider>
    </QueryClientProvider>
  );
}
