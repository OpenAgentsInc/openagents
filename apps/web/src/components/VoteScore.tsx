import { ChevronDown, ChevronUp } from "lucide-react";
import type { PostVoteSummary } from "@/hooks/useBatchPostVotes";

export function VoteScore({
  summary,
  className,
}: {
  summary: PostVoteSummary;
  className?: string;
}) {
  return (
    <span
      className={`inline-flex items-center gap-0.5 text-muted-foreground ${className ?? ""}`}
      title={`${summary.up} up, ${summary.down} down`}
    >
      <ChevronUp className="size-3.5 text-muted-foreground" aria-hidden />
      <span className="min-w-[1.25rem] text-center text-xs font-medium tabular-nums">
        {summary.score}
      </span>
      <ChevronDown className="size-3.5 text-muted-foreground" aria-hidden />
    </span>
  );
}
