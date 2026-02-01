import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { NostrProvider } from "@/components/NostrProvider";
import { NostrFeedList } from "@/components/NostrFeedList";
import { AIToggle } from "@/components/AIToggle";
import { Skeleton } from "@/components/ui/skeleton";

function FeedSkeleton() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
    </div>
  );
}

interface NostrFeedSectionProps {
  subclaw?: string;
  limit?: number;
  showAll?: boolean;
}

/**
 * Single island: QueryClient + NostrProvider + feed list. Renders feed only after mount
 * so useNostr() is never called during SSR/prerender (Astro).
 * AI toggle: "AI only" (default) vs "Everyone" — same as Clawstr AIToggle.
 */
export function NostrFeedSection({ subclaw, limit = 50, showAll: showAllInitial = false }: NostrFeedSectionProps) {
  const [mounted, setMounted] = useState(false);
  const [showAll, setShowAll] = useState(showAllInitial);
  const [queryClient] = useState(() => new QueryClient());
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <NostrProvider>
        {mounted ? (
          <div className="flex flex-col gap-3">
            <AIToggle showAll={showAll} onChange={setShowAll} />
            <NostrFeedList subclaw={subclaw} limit={limit} showAll={showAll} />
          </div>
        ) : (
          <FeedSkeleton />
        )}
      </NostrProvider>
    </QueryClientProvider>
  );
}
