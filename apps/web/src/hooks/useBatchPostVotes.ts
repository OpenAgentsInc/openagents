import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { useNostr } from "@nostrify/react";
import { useQuery } from "@tanstack/react-query";

/** NIP-25: +1 for positive reaction (+ or 👍 etc), -1 for negative (- or 👎). */
function reactionToDelta(content: string): 1 | -1 | 0 {
  const t = content.trim();
  if (t === "+" || t === "👍" || t === "❤️" || t === "🤙" || t === "😀") return 1;
  if (t === "-" || t === "👎") return -1;
  return 0;
}

export interface PostVoteSummary {
  score: number;
  up: number;
  down: number;
}

/**
 * Batch-fetch NIP-25 reactions (kind 7) for the given post event IDs,
 * aggregate by post: one vote per (e, pubkey) — latest reaction wins.
 * Returns a map from eventId -> { score, up, down }.
 */
export function useBatchPostVotes(eventIds: string[]) {
  const { nostr } = useNostr();
  const stableIds = eventIds.length > 0 ? [...eventIds].sort() : [];
  const queryKeyHash = stableIds.length > 0 ? stableIds.join(",") : "empty";

  return useQuery<Map<string, PostVoteSummary>>({
    queryKey: ["clawstr", "batch-post-votes", queryKeyHash],
    queryFn: async ({ signal }) => {
      if (eventIds.length === 0) return new Map<string, PostVoteSummary>();

      const filter: NostrFilter = {
        kinds: [7],
        "#e": eventIds,
        limit: 5000,
      };

      const events = await nostr.query([filter], {
        signal: AbortSignal.any([signal, AbortSignal.timeout(5000)]),
      });

      // One vote per (target e, reactor pubkey): keep latest by created_at
      const byTargetAndAuthor = new Map<string, NostrEvent>();
      for (const ev of events) {
        const eTag = ev.tags.find(([name]) => name === "e");
        const targetId = eTag?.[1];
        if (!targetId || !eventIds.includes(targetId)) continue;
        const key = `${targetId}:${ev.pubkey}`;
        const existing = byTargetAndAuthor.get(key);
        if (!existing || ev.created_at > existing.created_at) {
          byTargetAndAuthor.set(key, ev);
        }
      }

      const summaryByPost = new Map<string, { up: number; down: number }>();
      for (const id of eventIds) {
        summaryByPost.set(id, { up: 0, down: 0 });
      }
      for (const ev of byTargetAndAuthor.values()) {
        const eTag = ev.tags.find(([name]) => name === "e");
        const targetId = eTag?.[1];
        if (!targetId || !summaryByPost.has(targetId)) continue;
        const delta = reactionToDelta(ev.content);
        const s = summaryByPost.get(targetId)!;
        if (delta === 1) s.up += 1;
        else if (delta === -1) s.down += 1;
      }

      const result = new Map<string, PostVoteSummary>();
      for (const [id, { up, down }] of summaryByPost) {
        result.set(id, { score: up - down, up, down });
      }
      return result;
    },
    enabled: eventIds.length > 0,
    staleTime: 60 * 1000,
  });
}
