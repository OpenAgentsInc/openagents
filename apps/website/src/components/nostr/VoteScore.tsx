import { ChevronDown, ChevronUp } from "lucide-react";
import { useMemo, useState } from "react";
import { useNostr } from "@nostrify/react";
import { useQueryClient } from "@tanstack/react-query";
import type { PostVoteSummary } from "@/hooks/useBatchPostVotes";
import { publishReaction } from "@/lib/publishReaction";
import { hasNostrExtension } from "@/lib/publishKind1111";
import { posthogCapture } from "@/lib/posthog";

export function VoteScore({
  summary,
  className,
  target,
}: {
  summary: PostVoteSummary;
  className?: string;
  target?: { id: string; pubkey: string };
}) {
  const { nostr } = useNostr();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<null | "up" | "down">(null);
  const hasExtension = hasNostrExtension();
  const canVote = !!target && hasExtension;
  const title = useMemo(() => {
    if (!target) return `${summary.up} up, ${summary.down} down`;
    if (!hasExtension) return "Connect a Nostr extension to vote";
    return `${summary.up} up, ${summary.down} down`;
  }, [target, hasExtension, summary]);

  async function handleVote(direction: "up" | "down") {
    if (!target || !hasExtension) return;
    if (pending) return;
    setPending(direction);
    posthogCapture("nostr_vote_attempt", {
      direction,
      target_id: target.id,
      target_pubkey: target.pubkey,
    });
    try {
      const content = direction === "up" ? "+" : "-";
      await publishReaction(nostr, target, content);
      await queryClient.invalidateQueries({ queryKey: ["clawstr", "batch-post-votes"] });
      posthogCapture("nostr_vote_success", {
        direction,
        target_id: target.id,
        target_pubkey: target.pubkey,
      });
    } catch {
      posthogCapture("nostr_vote_error", {
        direction,
        target_id: target.id,
        target_pubkey: target.pubkey,
      });
      // ignore publish errors (extension rejection, relay failure)
    } finally {
      setPending(null);
    }
  }

  return (
    <span
      className={`inline-flex items-center gap-0.5 text-muted-foreground ${className ?? ""}`}
      title={title}
    >
      <button
        type="button"
        onClick={() => void handleVote("up")}
        disabled={!canVote || pending !== null}
        className="inline-flex items-center disabled:opacity-50"
        aria-label="Upvote"
      >
        <ChevronUp className="size-3.5 text-muted-foreground" aria-hidden />
      </button>
      <span className="min-w-[1.25rem] text-center text-xs font-medium tabular-nums">
        {summary.score}
      </span>
      <button
        type="button"
        onClick={() => void handleVote("down")}
        disabled={!canVote || pending !== null}
        className="inline-flex items-center disabled:opacity-50"
        aria-label="Downvote"
      >
        <ChevronDown className="size-3.5 text-muted-foreground" aria-hidden />
      </button>
    </span>
  );
}
