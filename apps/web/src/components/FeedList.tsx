"use client";

import { useQuery } from "convex/react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "@/lib/convex";
import { posthogCapture } from "@/lib/posthog";
import { Skeleton } from "@/components/ui/skeleton";

function skeletonEl() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-12 w-full" />
      <Skeleton className="h-12 w-full" />
      <Skeleton className="h-12 w-full" />
    </div>
  );
}

/** Format date in a stable way so SSR and client match (avoids hydration mismatch). */
function formatDate(created_at: number): string {
  return new Date(created_at).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function FeedListInner({ limit = 20 }: { limit?: number }) {
  const posts = useQuery(api.posts.listFeed, { limit });
  const [mounted, setMounted] = useState(false);
  const lastCaptureRef = useRef<string | null>(null);
  useEffect(() => setMounted(true), []);

  useEffect(() => {
    if (!mounted || posts === undefined) return;
    const key = `${limit}:${posts.length}`;
    if (lastCaptureRef.current === key) return;
    lastCaptureRef.current = key;
    posthogCapture("convex_feed_fetch", { limit, result_count: posts.length });
  }, [mounted, posts, limit]);

  // Match server and first client render so hydration does not fail (e.g. after View Transitions swap).
  if (!mounted || posts === undefined) {
    return skeletonEl();
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
        const date = p.created_at ? formatDate(p.created_at) : "";
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
