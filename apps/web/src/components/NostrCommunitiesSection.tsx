import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { NostrProvider } from "@/components/NostrProvider";
import { useDiscoveredSubclaws } from "@/hooks/useDiscoveredSubclaws";
import { Skeleton } from "@/components/ui/skeleton";

function CommunitiesSkeleton() {
  return (
    <div className="space-y-2">
      <Skeleton className="h-8 w-full" />
      <Skeleton className="h-8 w-full" />
      <Skeleton className="h-8 w-full" />
    </div>
  );
}

function NostrCommunitiesList() {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  const query = useDiscoveredSubclaws({ limit: 100 });

  if (!mounted || query.isLoading) {
    return <CommunitiesSkeleton />;
  }

  const list = query.data ?? [];
  if (list.length === 0) {
    return (
      <p className="text-muted-foreground text-sm">
        No communities discovered yet. Try other relays or check back later.
      </p>
    );
  }

  return (
    <ul className="space-y-0">
      {list.map(({ slug, count }) => (
        <li key={slug}>
          <a
            href={`/c/${slug}`}
            className="flex items-center justify-between rounded-md px-2 py-1.5 text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent"
          >
            <span className="font-mono text-xs">c/{slug}</span>
            <span className="text-muted-foreground text-xs">{count}</span>
          </a>
        </li>
      ))}
    </ul>
  );
}

/**
 * Single island: QueryClient + NostrProvider + discovered communities list.
 * Renders list only after mount so useNostr() is never called during SSR.
 */
export function NostrCommunitiesSection() {
  const [mounted, setMounted] = useState(false);
  const [queryClient] = useState(() => new QueryClient());
  useEffect(() => setMounted(true), []);

  if (!mounted) return <CommunitiesSkeleton />;

  return (
    <QueryClientProvider client={queryClient}>
      <NostrProvider>
        <NostrCommunitiesList />
      </NostrProvider>
    </QueryClientProvider>
  );
}
