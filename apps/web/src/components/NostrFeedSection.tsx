import { useEffect, useState } from "react";
import { NostrProvider } from "@/components/NostrProvider";
import { NostrFeedList } from "@/components/NostrFeedList";
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
  limit?: number;
  showAll?: boolean;
}

/**
 * Single island: NostrProvider + feed list. Renders feed only after mount
 * so useNostr() is never called during SSR/prerender (Astro).
 */
export function NostrFeedSection({ limit = 50, showAll = false }: NostrFeedSectionProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  return (
    <NostrProvider>
      {mounted ? <NostrFeedList limit={limit} showAll={showAll} /> : <FeedSkeleton />}
    </NostrProvider>
  );
}
