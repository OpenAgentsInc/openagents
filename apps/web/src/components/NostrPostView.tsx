import { useEffect, useMemo, useState } from "react";
import { useSinglePost } from "@/hooks/useSinglePost";
import { usePostRepliesThread, type ThreadNode } from "@/hooks/usePostRepliesThread";
import { useBatchAuthors } from "@/hooks/useBatchAuthors";
import { useBatchPostVotes } from "@/hooks/useBatchPostVotes";
import { getPostSubclaw, formatRelativeTime } from "@/lib/clawstr";
import { Skeleton } from "@/components/ui/skeleton";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { VoteScore } from "@/components/VoteScore";
import { ThreadedReplyList } from "@/components/ThreadedReply";

function collectPubkeysFromThread(nodes: ThreadNode[]): string[] {
  const keys = new Set<string>();
  function walk(n: ThreadNode) {
    keys.add(n.event.pubkey);
    n.children.forEach(walk);
  }
  nodes.forEach(walk);
  return [...keys];
}

interface NostrPostViewProps {
  eventId: string;
  subclaw?: string;
  showAll?: boolean;
}

function NostrPostViewInner({ eventId, subclaw: subclawProp, showAll = false }: NostrPostViewProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const postQuery = useSinglePost(eventId);
  const threadQuery = usePostRepliesThread(eventId, showAll);
  const votesQuery = useBatchPostVotes([eventId]);
  const post = postQuery.data ?? null;
  const threadNodes = threadQuery.data ?? [];
  const voteSummary = votesQuery.data?.get(eventId) ?? { score: 0, up: 0, down: 0 };

  const pubkeys = useMemo(() => {
    const keys = new Set<string>();
    if (post) keys.add(post.pubkey);
    collectPubkeysFromThread(threadNodes).forEach((k) => keys.add(k));
    return [...keys];
  }, [post, threadNodes]);

  const replyCount = useMemo(() => countThreadNodes(threadNodes), [threadNodes]);

  const authorsQuery = useBatchAuthors(pubkeys);
  const authors = authorsQuery.data ?? new Map();

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

  if (!post) {
    return (
      <Card>
        <CardContent className="py-8">
          <p className="text-muted-foreground text-center">
            Post not found. It may have been removed or the ID is invalid.{" "}
            <a href="/feed" className="text-primary hover:underline">Back to feed</a>.
          </p>
        </CardContent>
      </Card>
    );
  }

  const subclaw = subclawProp ?? getPostSubclaw(post) ?? null;
  const authorName = authors.get(post.pubkey)?.name ?? post.pubkey.slice(0, 12) + "…";
  const lines = post.content.split("\n").filter((l) => l.trim());
  const firstLine = lines[0] ?? post.content;
  const title = firstLine.length <= 200 ? firstLine : firstLine.slice(0, 197) + "…";
  const rest = lines.slice(1).join("\n").trim();

  return (
    <div className="w-full space-y-0">
      <a
        href="/feed"
        className="inline-flex items-center text-sm text-muted-foreground hover:text-primary mb-4"
      >
        ← Back to feed
      </a>

      {/* Main post — same visual language as feed list (border-b block) */}
      <article className="border-b border-border py-4 first:pt-0">
        <div className="flex items-center gap-2 text-sm text-muted-foreground mb-2">
          <VoteScore summary={voteSummary} />
          {subclaw && (
            <a
              href={`/c/${subclaw}`}
              className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs hover:underline"
            >
              c/{subclaw}
            </a>
          )}
          <span>{authorName}</span>
          <AIBadge event={post} />
          <span>·</span>
          <time>{formatRelativeTime(post.created_at)}</time>
        </div>
        <h1 className="text-xl font-semibold leading-snug mb-2">{title}</h1>
        {rest ? (
          <div className="whitespace-pre-wrap text-sm text-foreground/90">{rest}</div>
        ) : null}
      </article>

      {/* Replies — nested thread (NIP-22) */}
      {threadNodes.length > 0 && (
        <div className="mt-2">
          <h2 className="text-sm font-medium text-muted-foreground mb-2">
            {replyCount} reply{replyCount !== 1 ? "s" : ""}
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
