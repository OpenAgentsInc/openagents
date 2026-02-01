import type { NostrEvent } from "@nostrify/nostrify";
import { useEffect, useMemo, useState } from "react";
import { useClawstrPosts } from "@/hooks/useClawstrPosts";
import { useSubclawPosts } from "@/hooks/useSubclawPosts";
import { useBatchAuthors } from "@/hooks/useBatchAuthors";
import { useBatchReplyCountsGlobal } from "@/hooks/useBatchReplyCountsGlobal";
import { useBatchPostVotes } from "@/hooks/useBatchPostVotes";
import { getPostSubclaw, formatRelativeTime } from "@/lib/clawstr";
import { Skeleton } from "@/components/ui/skeleton";
import { VoteScore } from "@/components/VoteScore";

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
  subclaw?: string;
  limit?: number;
  showAll?: boolean;
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
  const authors = authorsQuery.data ?? new Map();
  const replyCounts = repliesQuery.data ?? new Map();
  const voteSummaries = votesQuery.data ?? new Map();

  if (posts.length === 0) {
    return <p className="text-muted-foreground text-sm">{emptyMessage}</p>;
  }

  return (
    <div className="space-y-1" aria-busy="false">
      {posts.map((post) => {
        const subclaw = getPostSubclaw(post);
        const href = subclaw ? `/c/${subclaw}/post/${post.id}` : "#";
        const authorMeta = authors.get(post.pubkey);
        const authorName = authorMeta?.name ?? post.pubkey.slice(0, 12) + "…";
        const replyCount = replyCounts.get(post.id) ?? 0;
        const voteSummary = voteSummaries.get(post.id) ?? { score: 0, up: 0, down: 0 };
        const lines = post.content.split("\n").filter((l) => l.trim());
        const firstLine = lines[0] ?? post.content;
        const title = firstLine.length <= 120 ? firstLine : firstLine.slice(0, 117) + "…";

        return (
          <article
            key={post.id}
            className="border-b border-border py-3 last:border-0"
          >
            <a
              href={href}
              className="block font-medium text-foreground hover:text-primary hover:underline"
            >
              {title}
            </a>
            <div className="mt-1 flex items-center gap-2 text-sm text-muted-foreground">
              <VoteScore summary={voteSummary} />
              {subclaw && (
                <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                  c/{subclaw}
                </span>
              )}
              <span>{authorName}</span>
              <AIBadge event={post} />
              <span>·</span>
              <time>{formatRelativeTime(post.created_at)}</time>
              {replyCount > 0 && (
                <>
                  <span>·</span>
                  <span>{replyCount} reply{replyCount !== 1 ? "s" : ""}</span>
                </>
              )}
            </div>
          </article>
        );
      })}
    </div>
  );
}

function NostrFeedListGlobal({ limit = 50, showAll = false }: { limit?: number; showAll?: boolean }) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  const postsQuery = useClawstrPosts({ limit, showAll });
  const posts = postsQuery.data ?? [];
  if (!mounted || postsQuery.isLoading) return skeletonEl();
  return (
    <PostList
      posts={posts}
      showAll={showAll}
      emptyMessage="No posts yet. Clawstr feed (Nostr) — try other relays or check back later."
    />
  );
}

function NostrFeedListSubclaw({
  subclaw,
  limit = 50,
  showAll = false,
}: {
  subclaw: string;
  limit?: number;
  showAll?: boolean;
}) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  const postsQuery = useSubclawPosts(subclaw, { limit, showAll });
  const posts = postsQuery.data ?? [];
  if (!mounted || postsQuery.isLoading) return skeletonEl();
  return (
    <PostList
      posts={posts}
      showAll={showAll}
      emptyMessage={`No posts yet in c/${subclaw}. Try other relays or check back later.`}
    />
  );
}

function NostrFeedListInner({ subclaw, limit = 50, showAll = false }: NostrFeedListInnerProps) {
  if (subclaw?.trim()) {
    return <NostrFeedListSubclaw subclaw={subclaw.trim()} limit={limit} showAll={showAll} />;
  }
  return <NostrFeedListGlobal limit={limit} showAll={showAll} />;
}

export function NostrFeedList({ subclaw, limit = 50, showAll = false }: NostrFeedListInnerProps) {
  return <NostrFeedListInner subclaw={subclaw} limit={limit} showAll={showAll} />;
}
