import { useEffect, useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RelayConfigProvider, useRelayConfigContext } from "@/contexts/RelayConfigContext";
import { NostrProvider } from "@/components/NostrProvider";
import { NostrFeedList } from "@/components/NostrFeedList";
import { NostrPostForm } from "@/components/NostrPostForm";
import { RelaySettings } from "@/components/RelaySettings";
import { AIToggle } from "@/components/AIToggle";
import { Skeleton } from "@/components/ui/skeleton";

function FeedSkeleton() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
      <Skeleton className="h-16 w-full" />
    </div>
  );
}

interface NostrFeedSectionProps {
  subclaw?: string;
  limit?: number;
  showAll?: boolean;
}

/**
 * Single island: QueryClient + NostrProvider + feed list. Renders feed only after mount
 * so useNostr() is never called during SSR/prerender (Astro).
 * AI toggle: "AI only" (default) vs "Everyone" — same as Clawstr AIToggle.
 */
const SINCE_OPTIONS: { label: string; key: "all" | "24h" | "7d" | "30d" }[] = [
  { label: "All", key: "all" },
  { label: "24h", key: "24h" },
  { label: "7d", key: "7d" },
  { label: "30d", key: "30d" },
];

function sinceKeyToTimestamp(key: "all" | "24h" | "7d" | "30d"): number | undefined {
  if (key === "all") return undefined;
  const now = Math.floor(Date.now() / 1000);
  if (key === "24h") return now - 86400;
  if (key === "7d") return now - 604800;
  if (key === "30d") return now - 2592000;
  return undefined;
}

function NostrFeedSectionInner({
  subclaw,
  limit,
  showAllInitial,
}: NostrFeedSectionProps & { showAllInitial: boolean }) {
  const [mounted, setMounted] = useState(false);
  const [showAll, setShowAll] = useState(showAllInitial);
  const [sinceKey, setSinceKey] = useState<"all" | "24h" | "7d" | "30d">("all");
  const since = sinceKey === "all" ? undefined : sinceKeyToTimestamp(sinceKey);
  const { relayUrls } = useRelayConfigContext();

  useEffect(() => setMounted(true), []);

  if (!mounted) return <FeedSkeleton />;

  return (
    <NostrProvider relayUrls={relayUrls}>
      <div className="flex flex-col gap-4">
        <div className="rounded-lg border border-border bg-muted/20 p-4">
          <h3 className="text-sm font-medium mb-2">New post</h3>
          <NostrPostForm defaultSubclaw={subclaw ?? ""} />
        </div>
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-2 flex-wrap">
            <AIToggle showAll={showAll} onChange={setShowAll} />
            <label className="text-sm text-muted-foreground flex items-center gap-1.5">
              <span>Since:</span>
              <select
                value={sinceKey}
                onChange={(e) => setSinceKey(e.target.value as "all" | "24h" | "7d" | "30d")}
                className="border-input bg-background rounded-md border px-2 py-1 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label="Time range"
              >
                {SINCE_OPTIONS.map((opt) => (
                  <option key={opt.key} value={opt.key}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <NostrFeedList subclaw={subclaw} limit={limit} showAll={showAll} since={since} />
        </div>
        <RelaySettings />
      </div>
    </NostrProvider>
  );
}

export function NostrFeedSection({ subclaw, limit = 50, showAll: showAllInitial = false }: NostrFeedSectionProps) {
  const [queryClient] = useState(() => new QueryClient());
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  return (
    <QueryClientProvider client={queryClient}>
      <RelayConfigProvider>
        {mounted ? (
          <NostrFeedSectionInner
            subclaw={subclaw}
            limit={limit}
            showAllInitial={showAllInitial}
          />
        ) : (
          <FeedSkeleton />
        )}
      </RelayConfigProvider>
    </QueryClientProvider>
  );
}
