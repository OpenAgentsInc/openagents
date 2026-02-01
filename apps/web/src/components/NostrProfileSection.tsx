import type { NostrEvent } from "@nostrify/nostrify";
import { useEffect, useMemo, useState } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { RelayConfigProvider, useRelayConfigContext } from "@/contexts/RelayConfigContext";
import { NostrProvider } from "@/components/NostrProvider";
import { useBatchAuthors } from "@/hooks/useBatchAuthors";
import { useAuthorPosts } from "@/hooks/useAuthorPosts";
import { useBatchReplyCountsGlobal } from "@/hooks/useBatchReplyCountsGlobal";
import { useBatchPostVotes } from "@/hooks/useBatchPostVotes";
import { getPostSubclaw, formatRelativeTime } from "@/lib/clawstr";
import { npubDecodeToHex, pubkeyToNpub } from "@/lib/npub";
import { Skeleton } from "@/components/ui/skeleton";
import { VoteScore } from "@/components/VoteScore";
import { AIBadge } from "@/components/AIBadge";
import { AIToggle } from "@/components/AIToggle";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { getQueryClient } from "@/lib/queryClient";

interface NostrProfileSectionProps {
  npub: string;
  limit?: number;
}

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
  const authorName = authorMeta?.name ?? profilePubkey.slice(0, 12) + "…";

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
        const subclaw = getPostSubclaw(post);
        const href = subclaw ? `/c/${subclaw}/post/${post.id}` : "#";
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
              <a
                href={`/u/${profileNpub}`}
                className="hover:text-primary hover:underline"
              >
                {authorName}
              </a>
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

function NostrProfileSectionInner({ npub, limit = 50 }: NostrProfileSectionProps) {
  const [showAll, setShowAll] = useState(false);

  const pubkey = useMemo(() => npubDecodeToHex(npub), [npub]);
  const profileNpub = useMemo(() => (pubkey ? pubkeyToNpub(pubkey) : ""), [pubkey]);

  const authorsQuery = useBatchAuthors(pubkey ? [pubkey] : []);
  const postsQuery = useAuthorPosts(pubkey, { showAll, limit });

  const authors = authorsQuery.data ?? new Map();
  const posts = postsQuery.data ?? [];
  const meta = pubkey ? authors.get(pubkey) : undefined;
  const displayName = meta?.name ?? (pubkey ? pubkey.slice(0, 12) + "…" : "");

  if (!pubkey) {
    return (
      <div className="rounded-lg border border-border bg-muted/30 p-6 text-center">
        <p className="text-muted-foreground">
          Invalid profile (npub). Check the URL and try again.
        </p>
        <a href="/feed" className="text-primary hover:underline mt-2 inline-block">
          Back to feed
        </a>
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
              {displayName.slice(0, 2).toUpperCase() || "?"}
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
          <AIToggle showAll={showAll} onChange={setShowAll} />
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

/**
 * Single island: QueryClient + NostrProvider + profile. Renders profile only after mount
 * so useNostr() is never called during SSR/prerender (Astro).
 */
export function NostrProfileSection(props: NostrProfileSectionProps) {
  const [queryClient] = useState(() => getQueryClient());
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? <NostrProfileSectionWithRelays {...props} /> : <ProfileSkeleton />}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}

function NostrProfileSectionWithRelays(props: NostrProfileSectionProps) {
  const { relayUrls } = useRelayConfigContext();
  return (
    <NostrProvider relayUrls={relayUrls}>
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
