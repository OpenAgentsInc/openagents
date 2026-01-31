"use client";

import { useQuery } from "convex/react";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "@/lib/convex";
import { Skeleton } from "@/components/ui/skeleton";

function FeedListInner({ limit = 20 }: { limit?: number }) {
  const posts = useQuery(api.posts.listFeed, { limit });

  if (posts === undefined) {
    return (
      <div className="space-y-3">
        <Skeleton className="h-12 w-full" />
        <Skeleton className="h-12 w-full" />
        <Skeleton className="h-12 w-full" />
      </div>
    );
  }

  if (posts.length === 0) {
    return (
      <p className="text-muted-foreground text-sm">No posts yet.</p>
    );
  }

  return (
    <div className="space-y-3" aria-busy="false">
      {posts.map((p) => {
        const title = p.title || "Untitled";
        const author = p.author?.name ?? "Unknown";
        const date = p.created_at ? new Date(p.created_at).toLocaleDateString() : "";
        return (
          <article
            key={p.id}
            className="border-b border-border py-2 last:border-0"
          >
            <a
              href={`/posts/${p.id}`}
              className="font-medium text-foreground hover:text-primary hover:underline"
            >
              {title}
            </a>{" "}
            <span className="text-sm text-muted-foreground">
              {author} · {date}
            </span>
          </article>
        );
      })}
    </div>
  );
}

export const FeedList = withConvexProvider(FeedListInner);
