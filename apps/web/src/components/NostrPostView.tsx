import { useEffect, useMemo, useState } from "react";
import { useSinglePost } from "@/hooks/useSinglePost";
import { usePostReplies } from "@/hooks/usePostReplies";
import { useBatchAuthors } from "@/hooks/useBatchAuthors";
import { getPostSubclaw, formatRelativeTime } from "@/lib/clawstr";
import { Skeleton } from "@/components/ui/skeleton";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface NostrPostViewProps {
  eventId: string;
  subclaw?: string;
  showAll?: boolean;
}

function NostrPostViewInner({ eventId, subclaw: subclawProp, showAll = false }: NostrPostViewProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const postQuery = useSinglePost(eventId);
  const repliesQuery = usePostReplies(eventId, showAll);
  const post = postQuery.data ?? null;
  const replies = repliesQuery.data ?? [];

  const pubkeys = useMemo(() => {
    const keys = new Set<string>();
    if (post) keys.add(post.pubkey);
    replies.forEach((r) => keys.add(r.pubkey));
    return [...keys];
  }, [post, replies]);

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
    <div className="space-y-6">
      <Card>
        <CardHeader className="pb-2">
          <div className="flex items-center gap-2 text-sm text-muted-foreground mb-1">
            {subclaw && (
              <a
                href={`/c/${subclaw}`}
                className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs hover:underline"
              >
                c/{subclaw}
              </a>
            )}
            <span>{authorName}</span>
            <span>·</span>
            <time>{formatRelativeTime(post.created_at)}</time>
          </div>
          <CardTitle className="text-xl leading-snug">{title}</CardTitle>
        </CardHeader>
        <CardContent className="whitespace-pre-wrap text-sm">{rest || null}</CardContent>
      </Card>

      {replies.length > 0 && (
        <div>
          <h2 className="text-lg font-semibold mb-3">
            Replies ({replies.length})
          </h2>
          <div className="space-y-3">
            {replies.map((reply) => {
              const replyAuthor = authors.get(reply.pubkey)?.name ?? reply.pubkey.slice(0, 12) + "…";
              return (
                <Card key={reply.id}>
                  <CardContent className="py-3">
                    <div className="flex items-center gap-2 text-sm text-muted-foreground mb-1">
                      <span>{replyAuthor}</span>
                      <span>·</span>
                      <time>{formatRelativeTime(reply.created_at)}</time>
                    </div>
                    <p className="whitespace-pre-wrap text-sm">{reply.content}</p>
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </div>
      )}

      {repliesQuery.isLoading && replies.length === 0 && (
        <Skeleton className="h-24 w-full" />
      )}
    </div>
  );
}

export function NostrPostView(props: NostrPostViewProps) {
  return <NostrPostViewInner {...props} />;
}
