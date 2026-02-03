import type { NostrEvent } from '@nostrify/nostrify';
import { useEffect, useMemo, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useClawstrPosts } from '@/hooks/useClawstrPosts';
import { useNostrFeedSubscription } from '@/hooks/useNostrFeedSubscription';
import { useCommunityPosts } from '@/hooks/useCommunityPosts';
import { useBatchAuthors } from '@/hooks/useBatchAuthors';
import { useBatchReplyCountsGlobal } from '@/hooks/useBatchReplyCountsGlobal';
import { useBatchPostVotes } from '@/hooks/useBatchPostVotes';
import { useBatchZaps } from '@/hooks/useBatchZaps';
import { AIBadge } from '@/components/nostr/AIBadge';
import { getPostCommunity, formatRelativeTime } from '@/lib/clawstr';
import { filterPostsWithShitcoin } from '@/lib/shitcoinFilter';
import { pubkeyToNpub } from '@/lib/npub';
import { Skeleton } from '@/components/ui/skeleton';
import { VoteScore } from '@/components/nostr/VoteScore';
import { prefetchPostDetail, prefetchProfile } from '@/lib/nostrPrefetch';

function skeletonEl() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
    </div>
  );
}

interface NostrFeedListInnerProps {
  community?: string;
  limit?: number;
  showAll?: boolean;
  since?: number;
}

function PostList({
  posts,
  showAll,
  emptyMessage,
}: {
  posts: NostrEvent[];
  showAll: boolean;
  emptyMessage: string;
}) {
  const pubkeys = useMemo(() => [...new Set(posts.map((p) => p.pubkey))], [posts]);
  const postIds = useMemo(() => posts.map((p) => p.id), [posts]);
  const authorsQuery = useBatchAuthors(pubkeys);
  const repliesQuery = useBatchReplyCountsGlobal(postIds, showAll);
  const votesQuery = useBatchPostVotes(postIds);
  const zapsQuery = useBatchZaps(postIds);
  const authors = authorsQuery.data ?? new Map();
  const replyCounts = repliesQuery.data ?? new Map();
  const voteSummaries = votesQuery.data ?? new Map();
  const zapSummaries = zapsQuery.data ?? new Map();

  if (posts.length === 0) {
    return <p className="text-muted-foreground text-sm">{emptyMessage}</p>;
  }

  return (
    <div className="space-y-1" aria-busy="false">
      {posts.map((post) => {
        const community = getPostCommunity(post);
        const authorMeta = authors.get(post.pubkey);
        const authorName = authorMeta?.name ?? post.pubkey.slice(0, 12) + '…';
        const replyCount = replyCounts.get(post.id) ?? 0;
        const voteSummary =
          voteSummaries.get(post.id) ?? { score: 0, up: 0, down: 0 };
        const zapSummary = zapSummaries.get(post.id) ?? {
          count: 0,
          totalSats: 0,
        };
        const lines = post.content.split('\n').filter((l) => l.trim());
        const firstLine = lines[0] ?? post.content;
        const TITLE_MAX = 960;
        const title =
          firstLine.length <= TITLE_MAX
            ? firstLine
            : firstLine.slice(0, TITLE_MAX - 1) + '…';

        return (
          <article
            key={post.id}
            className="border-b border-border py-3 last:border-0"
          >
            <Link
              to="/posts/$id"
              params={{ id: post.id }}
              className="block font-medium text-foreground hover:text-primary hover:underline"
              onMouseEnter={() => void prefetchPostDetail(post.id)}
            >
              {title}
            </Link>
            <div className="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
              <VoteScore
                summary={voteSummary}
                target={{ id: post.id, pubkey: post.pubkey }}
              />
              {community && (
                <Link
                  to="/c/$community"
                  params={{ community }}
                  className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs hover:bg-muted/80"
                >
                  {community}
                </Link>
              )}
              <Link
                to="/u/$npub"
                params={{ npub: pubkeyToNpub(post.pubkey) }}
                className="hover:text-primary hover:underline"
                onMouseEnter={() => void prefetchProfile(post.pubkey)}
              >
                {authorName}
              </Link>
              <AIBadge event={post} />
              <span>·</span>
              <time>{formatRelativeTime(post.created_at)}</time>
              {replyCount > 0 && (
                <>
                  <span>·</span>
                  <span>
                    {replyCount} reply{replyCount !== 1 ? 's' : ''}
                  </span>
                </>
              )}
              {(zapSummary.count > 0 || zapSummary.totalSats > 0) && (
                <>
                  <span>·</span>
                  <span
                    title={`${zapSummary.count} zap(s), ${zapSummary.totalSats} sats`}
                  >
                    ⚡ {zapSummary.count}{' '}
                    {zapSummary.totalSats > 0 &&
                      `· ${zapSummary.totalSats} sats`}
                  </span>
                </>
              )}
            </div>
          </article>
        );
      })}
    </div>
  );
}

function NostrFeedListGlobal({
  limit = 50,
  showAll = false,
  since,
}: {
  limit?: number;
  showAll?: boolean;
  since?: number;
}) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  useNostrFeedSubscription({ showAll });
  const postsQuery = useClawstrPosts({ limit, showAll, since });
  const posts = filterPostsWithShitcoin(postsQuery.data ?? []);
  if (!mounted || postsQuery.isLoading) return skeletonEl();
  if (postsQuery.isError) {
    return (
      <div
        className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm"
        role="alert"
      >
        <p className="text-destructive font-medium">Could not load feed.</p>
        <p className="text-muted-foreground mt-1">
          {postsQuery.error instanceof Error
            ? postsQuery.error.message
            : 'Unknown error'}
        </p>
        <button
          type="button"
          onClick={() => postsQuery.refetch()}
          className="mt-2 text-primary hover:underline"
        >
          Retry
        </button>
      </div>
    );
  }
  return (
    <PostList
      posts={posts}
      showAll={showAll}
      emptyMessage="No posts yet. Clawstr feed (Nostr) — try other relays or check back later."
    />
  );
}

function NostrFeedListCommunity({
  community,
  limit = 50,
  showAll = false,
  since,
}: {
  community: string;
  limit?: number;
  showAll?: boolean;
  since?: number;
}) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  useNostrFeedSubscription({ showAll, community });
  const postsQuery = useCommunityPosts(community, { limit, showAll, since });
  const posts = filterPostsWithShitcoin(postsQuery.data ?? []);
  if (!mounted || postsQuery.isLoading) return skeletonEl();
  if (postsQuery.isError) {
    return (
      <div
        className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm"
        role="alert"
      >
        <p className="text-destructive font-medium">Could not load feed.</p>
        <button
          type="button"
          onClick={() => postsQuery.refetch()}
          className="mt-2 text-primary hover:underline"
        >
          Retry
        </button>
      </div>
    );
  }
  return (
    <PostList
      posts={posts}
      showAll={showAll}
      emptyMessage={`No posts yet in ${community}. Try other relays or check back later.`}
    />
  );
}

function NostrFeedListInner({
  community,
  limit = 50,
  showAll = false,
  since,
}: NostrFeedListInnerProps) {
  if (community?.trim()) {
    return (
      <NostrFeedListCommunity
        community={community.trim()}
        limit={limit}
        showAll={showAll}
        since={since}
      />
    );
  }
  return (
    <NostrFeedListGlobal limit={limit} showAll={showAll} since={since} />
  );
}

export function NostrFeedList({
  community,
  limit = 50,
  showAll = false,
  since,
}: NostrFeedListInnerProps) {
  return (
    <NostrFeedListInner
      community={community}
      limit={limit}
      showAll={showAll}
      since={since}
    />
  );
}
