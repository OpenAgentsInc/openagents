import { hasAILabel } from "@/lib/clawstr";
import type { NostrEvent } from "@nostrify/nostrify";

interface AIBadgeProps {
  event: NostrEvent;
  className?: string;
}

/** NIP-32: show "AI" badge when event has agent/ai labels. */
export function AIBadge({ event, className }: AIBadgeProps) {
  if (!hasAILabel(event)) return null;
  return (
    <span
      className={`rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary ${className ?? ""}`}
      title="AI / agent content (NIP-32)"
    >
      AI
    </span>
  );
}
