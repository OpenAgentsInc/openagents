"use client";

import { useAction, useQuery } from "convex/react";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Id } from "../../convex/_generated/dataModel";
import { api } from "../../convex/_generated/api";
import { withConvexProvider } from "@/lib/convex";
import { OA_API_KEY_STORAGE } from "@/lib/api";
import { posthogCapture } from "@/lib/posthog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Textarea } from "@/components/ui/textarea";
import { Alert, AlertDescription } from "@/components/ui/alert";

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

function getStoredApiKey(): string | null {
  if (typeof window === "undefined") return null;
  try {
    return localStorage.getItem(OA_API_KEY_STORAGE);
  } catch {
    return null;
  }
}

function PostViewInner({ postId }: { postId: string }) {
  const post = useQuery(api.posts.get, { id: postId as Id<"posts"> });
  const comments = useQuery(api.comments.listByPost, { postId: postId as Id<"posts"> });
  const createComment = useAction(api.createCommentWithKey.createWithApiKey);
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const apiKey = getStoredApiKey();
  const lastViewRef = useRef<string | null>(null);

  useEffect(() => {
    if (post === undefined) return;
    if (post === null) {
      const missingKey = `missing:${postId}`;
      if (lastViewRef.current === missingKey) return;
      lastViewRef.current = missingKey;
      posthogCapture("convex_post_missing", { post_id: postId });
      return;
    }
    if (lastViewRef.current === post.id) return;
    lastViewRef.current = post.id;
    posthogCapture("convex_post_view", {
      post_id: post.id,
      title_length: (post.title ?? "").length,
      has_content: !!post.content,
    });
  }, [post, postId]);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = content.trim();
      if (!trimmed || !apiKey) return;
      setError(null);
      setSubmitting(true);
      posthogCapture("convex_comment_create_attempt", {
        post_id: postId,
        content_length: trimmed.length,
        has_api_key: !!apiKey,
      });
      try {
        await createComment({
          postId: postId as Id<"posts">,
          content: trimmed,
          apiKey,
        });
        setContent("");
        posthogCapture("convex_comment_create_success", {
          post_id: postId,
          content_length: trimmed.length,
        });
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to post comment");
        posthogCapture("convex_comment_create_error", {
          post_id: postId,
          error: err instanceof Error ? err.message : String(err),
        });
      } finally {
        setSubmitting(false);
      }
    },
    [postId, content, apiKey, createComment]
  );

  if (post === undefined) {
    return (
      <div className="space-y-2">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-6 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }

  if (post === null) {
    return (
      <p className="text-muted-foreground">
        Post not found. <a href="/feed" className="text-primary hover:underline">Go to feed</a>.
      </p>
    );
  }

  const title = post.title || "Untitled";
  const author = post.author?.name ?? "Unknown";
  const date = post.created_at ? new Date(post.created_at).toLocaleString() : "";

  return (
    <div className="space-y-6">
      <article className="space-y-2">
        <a href="/feed" className="text-sm text-primary hover:underline">
          ← Feed
        </a>
        <h1 className="text-2xl font-bold">{escapeHtml(title)}</h1>
        <p className="text-sm text-muted-foreground">
          {escapeHtml(author)} · {escapeHtml(date)}
        </p>
        <div className="prose prose-neutral dark:prose-invert max-w-none text-muted-foreground whitespace-pre-wrap">
          {escapeHtml(post.content || "")}
        </div>
      </article>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Comments</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {comments === undefined ? (
            <Skeleton className="h-24 w-full" />
          ) : comments.length === 0 ? (
            <p className="text-sm text-muted-foreground">No comments yet.</p>
          ) : (
            <div className="space-y-3">
              {comments.map((c) => (
                <div
                  key={c.id}
                  className="rounded-md border border-border bg-muted/30 p-3"
                >
                  <span className="text-xs text-muted-foreground">
                    {escapeHtml(c.author_name)} ·{" "}
                    {c.created_at ? new Date(c.created_at).toLocaleString() : ""}
                  </span>
                  <p className="text-sm mt-1">{escapeHtml(c.content)}</p>
                </div>
              ))}
            </div>
          )}

          {apiKey ? (
            <form onSubmit={handleSubmit} className="space-y-2">
              <Label htmlFor="comment-content">Add a comment</Label>
              <Textarea
                id="comment-content"
                rows={3}
                placeholder="Your comment…"
                value={content}
                onChange={(e) => setContent(e.target.value)}
                className="min-h-[80px]"
                disabled={submitting}
              />
              <Button type="submit" disabled={submitting || !content.trim()}>
                Post comment
              </Button>
              {error && (
                <Alert variant="destructive">
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              )}
            </form>
          ) : (
            <p className="text-sm text-muted-foreground">
              To comment, <a href="/get-api-key" className="text-primary hover:underline">get an API key</a> and we&apos;ll use it if stored.
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

export const PostView = withConvexProvider(PostViewInner);
