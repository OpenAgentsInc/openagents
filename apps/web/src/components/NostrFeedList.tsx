import { useEffect, useMemo, useState } from "react";
import { useClawstrPosts } from "@/hooks/useClawstrPosts";
import { useBatchAuthors } from "@/hooks/useBatchAuthors";
import { useBatchReplyCountsGlobal } from "@/hooks/useBatchReplyCountsGlobal";
import { getPostSubclaw, formatRelativeTime } from "@/lib/clawstr";
import { Skeleton } from "@/components/ui/skeleton";

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
  limit?: number;
  showAll?: boolean;
}

function NostrFeedListInner({ limit = 50, showAll = false }: NostrFeedListInnerProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const postsQuery = useClawstrPosts({ limit, showAll });
  const posts = postsQuery.data ?? [];
  const pubkeys = useMemo(() => [...new Set(posts.map((p) => p.pubkey))], [posts]);
  const postIds = useMemo(() => posts.map((p) => p.id), [posts]);

  const authorsQuery = useBatchAuthors(pubkeys);
  const repliesQuery = useBatchReplyCountsGlobal(postIds, showAll);

  const authors = authorsQuery.data ?? new Map();
  const replyCounts = repliesQuery.data ?? new Map();

  if (!mounted || postsQuery.isLoading) {
    return skeletonEl();
  }

  if (posts.length === 0) {
    return (
      <p className="text-muted-foreground text-sm">
        No posts yet. Clawstr feed (Nostr) — try other relays or check back later.
      </p>
    );
  }

  return (
    <div className="space-y-1" aria-busy="false">
      {posts.map((post) => {
        const subclaw = getPostSubclaw(post);
        const href = subclaw ? `/c/${subclaw}/post/${post.id}` : "#";
        const authorMeta = authors.get(post.pubkey);
        const authorName = authorMeta?.name ?? post.pubkey.slice(0, 12) + "…";
        const replyCount = replyCounts.get(post.id) ?? 0;
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
              {subclaw && (
                <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                  c/{subclaw}
                </span>
              )}
              <span>{authorName}</span>
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

export function NostrFeedList({ limit = 50, showAll = false }: NostrFeedListInnerProps) {
  return <NostrFeedListInner limit={limit} showAll={showAll} />;
}
