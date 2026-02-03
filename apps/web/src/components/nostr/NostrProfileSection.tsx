import type { NostrEvent } from '@nostrify/nostrify';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { QueryClientProvider } from '@tanstack/react-query';
import {
  RelayConfigProvider,
  useRelayConfigContext,
} from '@/contexts/RelayConfigContext';
import { NostrProvider } from '@/components/nostr/NostrProvider';
import { useBatchAuthors } from '@/hooks/useBatchAuthors';
import { useAuthorPosts } from '@/hooks/useAuthorPosts';
import { useBatchReplyCountsGlobal } from '@/hooks/useBatchReplyCountsGlobal';
import { useBatchPostVotes } from '@/hooks/useBatchPostVotes';
import {
  getPostCommunity,
  formatRelativeTime,
  formatCount,
} from '@/lib/clawstr';
import { npubDecodeToHex, pubkeyToNpub } from '@/lib/npub';
import { Skeleton } from '@/components/ui/skeleton';
import { VoteScore } from '@/components/nostr/VoteScore';
import { AIBadge } from '@/components/nostr/AIBadge';
import { AIToggle } from '@/components/nostr/AIToggle';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import { getQueryClient } from '@/lib/queryClient';
import { prefetchPostDetail } from '@/lib/nostrPrefetch';
import { posthogCapture } from '@/lib/posthog';

interface NostrProfileSectionProps {
  npub: string;
  limit?: number;
}

const TITLE_MAX = 960;

function ProfilePostList({
  posts,
  showAll,
  profilePubkey,
  profileNpub,
}: {
  posts: NostrEvent[];
  showAll: boolean;
  profilePubkey: string;
  profileNpub: string;
}) {
  const postIds = useMemo(() => posts.map((p) => p.id), [posts]);
  const repliesQuery = useBatchReplyCountsGlobal(postIds, showAll);
  const votesQuery = useBatchPostVotes(postIds);
  const authorsQuery = useBatchAuthors([profilePubkey]);
  const authors = authorsQuery.data ?? new Map();
  const replyCounts = repliesQuery.data ?? new Map();
  const voteSummaries = votesQuery.data ?? new Map();
  const authorMeta = authors.get(profilePubkey);
  const authorName =
    authorMeta?.name ?? profilePubkey.slice(0, 12) + '…';

  if (posts.length === 0) {
    return (
      <p className="text-muted-foreground text-sm py-4">
        No posts yet from this author.
      </p>
    );
  }

  return (
    <div className="space-y-1" aria-busy="false">
      {posts.map((post) => {
        const community = getPostCommunity(post);
        const replyCount = replyCounts.get(post.id) ?? 0;
        const replyLabel = formatCount(replyCount);
        const voteSummary =
          voteSummaries.get(post.id) ?? { score: 0, up: 0, down: 0 };
        const lines = post.content.split('\n').filter((l) => l.trim());
        const firstLine = lines[0] ?? post.content;
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
                params={{ npub: profileNpub }}
                className="hover:text-primary hover:underline"
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
                    {replyLabel} reply{replyCount !== 1 ? 's' : ''}
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

function NostrProfileSectionInner({
  npub,
  limit = 50,
}: NostrProfileSectionProps) {
  const [showAll, setShowAll] = useState(false);
  const lastProfileRef = useRef<string | null>(null);

  const pubkey = useMemo(() => npubDecodeToHex(npub) ?? undefined, [npub]);
  const profileNpub = useMemo(
    () => (pubkey ? pubkeyToNpub(pubkey) : ''),
    [pubkey],
  );

  const authorsQuery = useBatchAuthors(pubkey ? [pubkey] : []);
  const postsQuery = useAuthorPosts(pubkey, { showAll, limit });

  const authors = authorsQuery.data ?? new Map();
  const posts = postsQuery.data ?? [];
  const meta = pubkey ? authors.get(pubkey) : undefined;
  const displayName = meta?.name ?? (pubkey ? pubkey.slice(0, 12) + '…' : '');

  useEffect(() => {
    if (!pubkey) return;
    if (postsQuery.isLoading) return;
    if (lastProfileRef.current === pubkey) return;
    lastProfileRef.current = pubkey;
    posthogCapture('nostr_profile_view', {
      pubkey,
      npub,
      post_count: posts.length,
      has_profile: !!meta,
    });
  }, [pubkey, npub, posts.length, meta, postsQuery.isLoading]);

  if (!pubkey) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-6 text-center">
        <p className="text-muted-foreground">
          Invalid profile (npub). Check the URL and try again.
        </p>
        <Link
          to="/feed"
          className="text-primary hover:underline mt-2 inline-block"
        >
          Back to feed
        </Link>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col gap-3 border-b border-border pb-4">
        <div className="flex items-center gap-4">
          <Avatar className="h-16 w-16">
            <AvatarImage src={meta?.picture} alt="" />
            <AvatarFallback className="text-lg">
              {displayName.slice(0, 2).toUpperCase() || '?'}
            </AvatarFallback>
          </Avatar>
          <div>
            <h1 className="text-xl font-semibold">{displayName}</h1>
            <p className="text-muted-foreground font-mono text-xs break-all">
              {npub}
            </p>
          </div>
        </div>
        {meta?.about && (
          <p className="text-sm text-foreground/90 whitespace-pre-wrap">
            {meta.about}
          </p>
        )}
      </div>
      <div>
        <div className="flex items-center justify-between gap-2 mb-2">
          <h2 className="text-sm font-medium text-muted-foreground">Posts</h2>
          <AIToggle
            showAll={showAll}
            onChange={setShowAll}
            source="profile"
          />
        </div>
        {postsQuery.isLoading ? (
          <div className="space-y-3">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-16 w-full" />
          </div>
        ) : (
          <ProfilePostList
            posts={posts}
            showAll={showAll}
            profilePubkey={pubkey}
            profileNpub={profileNpub}
          />
        )}
      </div>
    </div>
  );
}

export function NostrProfileSection(props: NostrProfileSectionProps) {
  const [queryClient] = useState(() => getQueryClient());
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? (
          <NostrProfileSectionWithRelays {...props} />
        ) : (
          <ProfileSkeleton />
        )}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}

function NostrProfileSectionWithRelays(props: NostrProfileSectionProps) {
  const { relayMetadata } = useRelayConfigContext();
  return (
    <NostrProvider relayMetadata={relayMetadata}>
      <NostrProfileSectionInner {...props} />
    </NostrProvider>
  );
}

function ProfileSkeleton() {
  return (
    <div className="space-y-4">
      <Skeleton className="h-24 w-full" />
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
    </div>
  );
}
