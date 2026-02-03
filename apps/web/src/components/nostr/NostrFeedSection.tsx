import { useEffect, useRef, useState } from 'react';
import { QueryClientProvider } from '@tanstack/react-query';
import {
  RelayConfigProvider,
  useRelayConfigContext,
} from '@/contexts/RelayConfigContext';
import { NostrProvider } from '@/components/nostr/NostrProvider';
import { NostrFeedList } from '@/components/nostr/NostrFeedList';
import { NostrPostForm } from '@/components/nostr/NostrPostForm';
import { RelaySettings } from '@/components/nostr/RelaySettings';
import { AIToggle } from '@/components/nostr/AIToggle';
import { Skeleton } from '@/components/ui/skeleton';
import { getQueryClient } from '@/lib/queryClient';
import { posthogCapture } from '@/lib/posthog';

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
  community?: string;
  limit?: number;
  showAll?: boolean;
}

const SINCE_OPTIONS: { label: string; key: 'all' | '24h' | '7d' | '30d' }[] = [
  { label: 'All', key: 'all' },
  { label: '24h', key: '24h' },
  { label: '7d', key: '7d' },
  { label: '30d', key: '30d' },
];

function sinceKeyToTimestamp(
  key: 'all' | '24h' | '7d' | '30d',
): number | undefined {
  if (key === 'all') return undefined;
  const now = Math.floor(Date.now() / 1000);
  if (key === '24h') return now - 86400;
  if (key === '7d') return now - 604800;
  if (key === '30d') return now - 2592000;
  return undefined;
}

function NostrFeedSectionInner({
  community,
  limit,
  showAllInitial,
}: NostrFeedSectionProps & { showAllInitial: boolean }) {
  const [mounted, setMounted] = useState(false);
  const [showAll, setShowAll] = useState(showAllInitial);
  const [sinceKey, setSinceKey] = useState<'all' | '24h' | '7d' | '30d'>('all');
  const since =
    sinceKey === 'all' ? undefined : sinceKeyToTimestamp(sinceKey);
  const { relayMetadata } = useRelayConfigContext();
  const lastViewRef = useRef<string | null>(null);
  const lastSinceRef = useRef<string | null>(null);

  useEffect(() => setMounted(true), []);
  useEffect(() => {
    if (!mounted) return;
    const scope = community ? 'community' : 'global';
    const key = `${scope}:${community ?? 'all'}`;
    if (lastViewRef.current === key) return;
    lastViewRef.current = key;
    posthogCapture('nostr_feed_view', {
      scope,
      community: community ?? null,
      show_all: showAll,
      since: sinceKey,
      limit,
    });
  }, [mounted, community, showAll, sinceKey, limit]);

  useEffect(() => {
    if (!mounted) return;
    if (lastSinceRef.current === sinceKey) return;
    if (lastSinceRef.current !== null) {
      posthogCapture('nostr_feed_since_change', {
        scope: community ? 'community' : 'global',
        community: community ?? null,
        since: sinceKey,
      });
    }
    lastSinceRef.current = sinceKey;
  }, [mounted, sinceKey, community]);

  if (!mounted) return <FeedSkeleton />;

  return (
    <NostrProvider relayMetadata={relayMetadata}>
      <div className="flex flex-col gap-4">
        <div className="rounded-lg border border-border bg-muted/20 p-4">
          <h3 className="text-sm font-medium mb-2">New post</h3>
          <NostrPostForm defaultCommunity={community ?? ''} />
        </div>
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-2 flex-wrap">
            <AIToggle
              showAll={showAll}
              onChange={setShowAll}
              source={community ? 'community_feed' : 'feed'}
            />
            <label className="text-sm text-muted-foreground flex items-center gap-1.5">
              <span>Since:</span>
              <select
                value={sinceKey}
                onChange={(e) =>
                  setSinceKey(e.target.value as 'all' | '24h' | '7d' | '30d')
                }
                className="border-input bg-background rounded-md border px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label="Time range"
              >
                {SINCE_OPTIONS.map((opt) => (
                  <option key={opt.key} value={opt.key}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <NostrFeedList
            community={community}
            limit={limit}
            showAll={showAll}
            since={since}
          />
        </div>
        <RelaySettings />
      </div>
    </NostrProvider>
  );
}

export function NostrFeedSection({
  community,
  limit = 50,
  showAll: showAllInitial = false,
}: NostrFeedSectionProps) {
  const [queryClient] = useState(() => getQueryClient());
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? (
          <NostrFeedSectionInner
            community={community}
            limit={limit}
            showAllInitial={showAllInitial}
          />
        ) : (
          <FeedSkeleton />
        )}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
