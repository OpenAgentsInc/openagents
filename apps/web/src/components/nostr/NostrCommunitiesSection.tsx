import { useEffect, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { QueryClientProvider } from '@tanstack/react-query';
import {
  RelayConfigProvider,
  useRelayConfigContext,
} from '@/contexts/RelayConfigContext';
import { NostrProvider } from '@/components/nostr/NostrProvider';
import { useDiscoveredCommunities } from '@/hooks/useDiscoveredCommunities';
import { Skeleton } from '@/components/ui/skeleton';
import { formatCount } from '@/lib/clawstr';
import { getQueryClient } from '@/lib/queryClient';
import { prefetchCommunity } from '@/lib/nostrPrefetch';

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
  const query = useDiscoveredCommunities({ limit: 100 });

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
          <Link
            to="/c/$community"
            params={{ community: slug }}
            className="flex items-center justify-between rounded-md px-2 py-1.5 text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent"
            onMouseEnter={() => void prefetchCommunity(slug)}
          >
            <span className="font-mono text-xs">{slug}</span>
            <span className="text-muted-foreground text-xs">
              {formatCount(count)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
}

export function NostrCommunitiesSection() {
  const [mounted, setMounted] = useState(false);
  const [queryClient] = useState(() => getQueryClient());
  useEffect(() => setMounted(true), []);

  if (!mounted) return <CommunitiesSkeleton />;

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        <NostrCommunitiesSectionInner />
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}

function NostrCommunitiesSectionInner() {
  const { relayMetadata } = useRelayConfigContext();
  return (
    <NostrProvider relayMetadata={relayMetadata}>
      <NostrCommunitiesList />
    </NostrProvider>
  );
}
