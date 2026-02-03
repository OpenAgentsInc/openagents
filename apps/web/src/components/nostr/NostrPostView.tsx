import { useEffect, useMemo, useRef, useState } from 'react';
import { Link } from '@tanstack/react-router';
import { useSinglePost } from '@/hooks/useSinglePost';
import {
  usePostRepliesThread,
  type ThreadNode,
} from '@/hooks/usePostRepliesThread';
import { useBatchAuthors } from '@/hooks/useBatchAuthors';
import { useBatchPostVotes } from '@/hooks/useBatchPostVotes';
import { useBatchZaps } from '@/hooks/useBatchZaps';
import {
  getPostCommunity,
  formatRelativeTime,
  formatCount,
  formatSats,
  hasAILabel,
} from '@/lib/clawstr';
import { pubkeyToNpub } from '@/lib/npub';
import { Skeleton } from '@/components/ui/skeleton';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { AIBadge } from '@/components/nostr/AIBadge';
import { VoteScore } from '@/components/nostr/VoteScore';
import { ThreadedReplyList } from '@/components/nostr/ThreadedReply';
import { NostrReplyForm } from '@/components/nostr/NostrReplyForm';
import { prefetchProfile, prefetchCommunity } from '@/lib/nostrPrefetch';
import { posthogCapture } from '@/lib/posthog';

function collectPubkeysFromThread(nodes: ThreadNode[]): string[] {
  const keys = new Set<string>();
  function walk(n: ThreadNode) {
    keys.add(n.event.pubkey);
    n.children.forEach(walk);
  }
  nodes.forEach(walk);
  return [...keys];
}

function countThreadNodes(nodes: ThreadNode[]): number {
  let c = 0;
  function walk(n: ThreadNode[]) {
    for (const node of n) {
      c++;
      walk(node.children);
    }
  }
  walk(nodes);
  return c;
}

interface NostrPostViewProps {
  eventId: string;
  community?: string;
  showAll?: boolean;
}

function NostrPostViewInner({
  eventId,
  community: communityProp,
  showAll = false,
}: NostrPostViewProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  const lastViewRef = useRef<string | null>(null);

  const postQuery = useSinglePost(eventId);
  const threadQuery = usePostRepliesThread(eventId, showAll);
  const votesQuery = useBatchPostVotes([eventId]);
  const zapsQuery = useBatchZaps([eventId]);
  const post = postQuery.data ?? null;
  const threadNodes = threadQuery.data ?? [];
  const voteSummary =
    votesQuery.data?.get(eventId) ?? { score: 0, up: 0, down: 0 };
  const zapSummary = zapsQuery.data?.get(eventId) ?? {
    count: 0,
    totalSats: 0,
  };
  const zapCountLabel = formatCount(zapSummary.count);
  const zapSatsLabel = formatSats(zapSummary.totalSats);

  const pubkeys = useMemo(() => {
    const keys = new Set<string>();
    if (post) keys.add(post.pubkey);
    collectPubkeysFromThread(threadNodes).forEach((k) => keys.add(k));
    return [...keys];
  }, [post, threadNodes]);

  const replyCount = useMemo(
    () => countThreadNodes(threadNodes),
    [threadNodes],
  );

  const authorsQuery = useBatchAuthors(pubkeys);
  const authors = authorsQuery.data ?? new Map();

  const communityForEffect = post
    ? (communityProp ?? getPostCommunity(post) ?? null)
    : null;
  useEffect(() => {
    if (!post || lastViewRef.current === post.id) return;
    lastViewRef.current = post.id;
    posthogCapture('nostr_post_view', {
      event_id: post.id,
      community: communityForEffect,
      author_pubkey: post.pubkey,
      is_ai: hasAILabel(post),
      content_length: post.content?.length ?? 0,
    });
  }, [post, communityForEffect]);

  if (!mounted || postQuery.isLoading) {
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

  if (postQuery.isError) {
    return (
      <Card>
        <CardContent className="py-8">
          <div className="text-center space-y-2" role="alert">
            <p className="text-destructive font-medium">Could not load post.</p>
            <p className="text-muted-foreground text-sm">
              {postQuery.error instanceof Error
                ? postQuery.error.message
                : 'Unknown error'}
            </p>
            <Link
              to="/feed"
              className="text-primary hover:underline text-sm"
            >
              Back to feed
            </Link>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (!post) {
    return (
      <Card>
        <CardContent className="py-8">
          <p className="text-muted-foreground text-center">
            Post not found.{' '}
            <Link to="/feed" className="text-primary hover:underline">
              Back to feed
            </Link>
            .
          </p>
        </CardContent>
      </Card>
    );
  }

  const community = communityProp ?? getPostCommunity(post) ?? null;
  const authorName =
    authors.get(post.pubkey)?.name ?? post.pubkey.slice(0, 12) + '…';
  const lines = post.content.split('\n').filter((l) => l.trim());
  const firstLine = lines[0] ?? post.content;
  const title = firstLine;
  const rest = lines.slice(1).join('\n').trim();

  return (
    <div className="w-full space-y-0">
      <Link
        to="/feed"
        className="inline-flex items-center text-sm text-muted-foreground hover:text-primary mb-4"
      >
        ← Back to feed
      </Link>

      <article className="border-b border-border py-4 first:pt-0">
        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-2">
          <VoteScore
            summary={voteSummary}
            target={{ id: post.id, pubkey: post.pubkey }}
          />
          {community && (
            <Link
              to="/c/$community"
              params={{ community }}
              className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs hover:underline"
              onMouseEnter={() => void prefetchCommunity(community)}
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
          {(zapSummary.count > 0 || zapSummary.totalSats > 0) && (
            <>
              <span>·</span>
              <span
                title={`${zapSummary.count} zap(s), ${zapSummary.totalSats} sats`}
              >
                ⚡ {zapCountLabel}{' '}
                {zapSummary.totalSats > 0 &&
                  `· ${zapSatsLabel} sats`}
              </span>
            </>
          )}
        </div>
        <h1 className="text-xl font-semibold leading-snug mb-2">{title}</h1>
        {rest ? (
          <div className="whitespace-pre-wrap text-sm text-foreground/90">
            {rest}
          </div>
        ) : null}
      </article>

      <NostrReplyForm parentEvent={post} />

      {threadNodes.length > 0 && (
        <div className="mt-2">
          <h2 className="text-sm font-medium text-muted-foreground mb-2">
            {replyCount} reply{replyCount !== 1 ? 's' : ''}
          </h2>
          <ThreadedReplyList nodes={threadNodes} authors={authors} />
        </div>
      )}

      {threadQuery.isLoading && threadNodes.length === 0 && (
        <div className="border-t border-border pt-4">
          <Skeleton className="h-16 w-full" />
        </div>
      )}
    </div>
  );
}

export function NostrPostView(props: NostrPostViewProps) {
  return <NostrPostViewInner {...props} />;
}
