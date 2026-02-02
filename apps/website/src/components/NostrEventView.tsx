import { useEffect, useMemo, useRef, useState } from "react";
import { useSingleEvent } from "@/hooks/useSingleEvent";
import { isTopLevelPost, formatRelativeTime } from "@/lib/clawstr";
import { pubkeyToNpub } from "@/lib/npub";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { AIToggle } from "@/components/AIToggle";
import { NostrPostView } from "@/components/NostrPostView";
import { posthogCapture } from "@/lib/posthog";

interface NostrEventViewProps {
  eventId: string;
}

function stringifyContent(content: string): { label: string; value: string } {
  if (!content) return { label: "Content", value: "" };
  try {
    const parsed = JSON.parse(content);
    if (parsed && typeof parsed === "object") {
      return { label: "Content (JSON)", value: JSON.stringify(parsed, null, 2) };
    }
  } catch {
    // fallthrough
  }
  return { label: "Content", value: content };
}

export function NostrEventView({ eventId }: NostrEventViewProps) {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);
  const lastViewRef = useRef<string | null>(null);

  const [showAll, setShowAll] = useState(false);
  const eventQuery = useSingleEvent(eventId);
  const event = eventQuery.data ?? null;

  useEffect(() => {
    if (!event) return;
    if (lastViewRef.current === event.id) return;
    lastViewRef.current = event.id;
    posthogCapture("nostr_event_view", {
      event_id: event.id,
      kind: event.kind,
      pubkey: event.pubkey,
    });
  }, [event]);

  if (!mounted || eventQuery.isLoading) {
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

  if (eventQuery.isError) {
    return (
      <Card>
        <CardContent className="py-8">
          <div className="text-center space-y-2" role="alert">
            <p className="text-destructive font-medium">Could not load event.</p>
            <p className="text-muted-foreground text-sm">
              {eventQuery.error instanceof Error ? eventQuery.error.message : "Unknown error"}
            </p>
            <button
              type="button"
              onClick={() => eventQuery.refetch()}
              className="text-primary hover:underline text-sm"
            >
              Retry
            </button>
          </div>
        </CardContent>
      </Card>
    );
  }

  if (!event) {
    return (
      <Card>
        <CardContent className="py-8">
          <p className="text-muted-foreground text-center">
            Event not found. It may have been removed or the ID is invalid.{" "}
            <a href="/feed" className="text-primary hover:underline">Back to feed</a>.
          </p>
        </CardContent>
      </Card>
    );
  }

  if (event.kind === 1111 && isTopLevelPost(event)) {
    return (
      <div className="flex flex-col gap-3">
        <AIToggle showAll={showAll} onChange={setShowAll} source="event" />
        <NostrPostView eventId={eventId} showAll={showAll} />
      </div>
    );
  }

  const createdAt = event.created_at ? new Date(event.created_at * 1000) : null;
  const content = stringifyContent(event.content ?? "");
  const tags = useMemo(() => JSON.stringify(event.tags ?? [], null, 2), [event.tags]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-xl">Nostr event</CardTitle>
        <div className="text-sm text-muted-foreground flex flex-wrap items-center gap-2">
          <span>kind {event.kind}</span>
          <span>·</span>
          <a
            href={`/u/${pubkeyToNpub(event.pubkey)}`}
            className="hover:text-primary hover:underline"
          >
            {event.pubkey.slice(0, 12)}…
          </a>
          {createdAt && (
            <>
              <span>·</span>
              <time title={createdAt.toISOString()}>{formatRelativeTime(event.created_at)}</time>
            </>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div>
          <div className="text-xs text-muted-foreground mb-1">Event ID</div>
          <code className="text-xs break-all">{event.id}</code>
        </div>
        <div>
          <div className="text-xs text-muted-foreground mb-1">{content.label}</div>
          <pre className="whitespace-pre-wrap text-sm rounded-md border border-border bg-muted/30 p-3">
            {content.value || "(empty)"}
          </pre>
        </div>
        <div>
          <div className="text-xs text-muted-foreground mb-1">Tags</div>
          <pre className="whitespace-pre-wrap text-xs rounded-md border border-border bg-muted/30 p-3">
            {tags}
          </pre>
        </div>
      </CardContent>
    </Card>
  );
}
