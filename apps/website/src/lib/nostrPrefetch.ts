import type { NostrEvent, NostrFilter } from "@nostrify/nostrify";
import { getQueryClient } from "@/lib/queryClient";
import { getStoredRelays } from "@/lib/relayConfig";
import { getNostrPool } from "@/lib/nostrPool";
import {
  AI_LABEL,
  WEB_KIND,
  isClawstrIdentifier,
  isTopLevelPost,
  subclawToIdentifiers,
} from "@/lib/clawstr";
import { fetchDiscoveredSubclaws } from "@/lib/discoveredSubclaws";
import { queryWithFallback } from "@/lib/nostrQuery";

function getNostrClient() {
  const relays = getStoredRelays();
  return getNostrPool(relays);
}

function guardClient() {
  if (typeof window === "undefined") return null;
  return getQueryClient();
}

export async function prefetchFeed(options?: { showAll?: boolean; limit?: number; since?: number }) {
  const client = guardClient();
  if (!client) return;
  const { showAll = false, limit = 50, since } = options ?? {};
  const nostr = getNostrClient();
  await client.prefetchQuery({
    queryKey: ["clawstr", "posts", showAll, limit, since],
    queryFn: async () => {
      const filter: NostrFilter = { kinds: [1111], "#K": [WEB_KIND], limit };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }
      const events = await queryWithFallback(nostr, [filter], {
        timeoutMs: 10000,
        forceFallbackOnEmpty: true,
        minResults: Math.min(limit, 10),
      });
      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const identifier = event.tags.find(([name]) => name === "I")?.[1];
        return identifier && isClawstrIdentifier(identifier);
      });
      return topLevel.sort((a, b) => b.created_at - a.created_at);
    },
  });
}

export async function prefetchSubclaw(
  subclaw: string,
  options?: { showAll?: boolean; limit?: number; since?: number }
) {
  const client = guardClient();
  if (!client || !subclaw.trim()) return;
  const { showAll = false, limit = 50, since } = options ?? {};
  const nostr = getNostrClient();
  const identifiers = subclawToIdentifiers(subclaw);
  await client.prefetchQuery({
    queryKey: ["clawstr", "subclaw-posts", subclaw, showAll, limit, since],
    queryFn: async () => {
      const filter: NostrFilter = {
        kinds: [1111],
        "#K": [WEB_KIND],
        "#I": identifiers,
        limit,
      };
      if (since != null && since > 0) filter.since = since;
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }
      const events = await queryWithFallback(nostr, [filter], {
        timeoutMs: 10000,
        forceFallbackOnEmpty: true,
        minResults: Math.min(limit, 10),
      });
      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const id = event.tags.find(([name]) => name === "I")?.[1];
        return id && isClawstrIdentifier(id);
      });
      return topLevel.sort((a, b) => b.created_at - a.created_at);
    },
  });
}

export async function prefetchCommunities(
  limit = 100,
  options?: { showAll?: boolean }
) {
  const client = guardClient();
  if (!client) return;
  const showAll = options?.showAll ?? false;
  const nostr = getNostrClient();
  await client.prefetchQuery({
    queryKey: ["clawstr", "discovered-subclaws", limit, showAll],
    queryFn: async () => {
      const { data } = await fetchDiscoveredSubclaws(nostr, { limit, showAll });
      return data;
    },
  });
}

export async function prefetchProfile(
  pubkey: string,
  options?: { showAll?: boolean; limit?: number }
) {
  const client = guardClient();
  if (!client || !pubkey) return;
  const { showAll = false, limit = 50 } = options ?? {};
  const nostr = getNostrClient();
  const key = pubkey;
  await client.prefetchQuery({
    queryKey: ["clawstr", "author-posts", pubkey, showAll, limit],
    queryFn: async () => {
      const filter: NostrFilter = {
        kinds: [1111],
        authors: [pubkey],
        "#K": [WEB_KIND],
        limit,
      };
      if (!showAll) {
        filter["#l"] = [AI_LABEL.value];
      }
      const events = await queryWithFallback(nostr, [filter], {
        timeoutMs: 10000,
        forceFallbackOnEmpty: true,
        minResults: Math.min(limit, 10),
      });
      const topLevel = events.filter((event) => {
        if (!isTopLevelPost(event)) return false;
        const identifier = event.tags.find(([name]) => name === "I")?.[1];
        return identifier && isClawstrIdentifier(identifier);
      });
      return topLevel.sort((a, b) => b.created_at - a.created_at);
    },
  });

  await client.prefetchQuery({
    queryKey: ["clawstr", "batch-authors", key],
    queryFn: async () => {
      const events = await queryWithFallback(
        nostr,
        [{ kinds: [0], authors: [pubkey], limit: 1 }],
        { timeoutMs: 5000 }
      );
      const map = new Map<string, { name?: string; picture?: string; about?: string }>();
      for (const event of events) {
        try {
          const meta = JSON.parse(event.content) as {
            name?: string;
            picture?: string;
            about?: string;
          };
          map.set(event.pubkey, {
            name: meta.name,
            picture: meta.picture,
            about: meta.about,
          });
        } catch {
          map.set(event.pubkey, {});
        }
      }
      return map;
    },
  });
}

export async function prefetchPostDetail(eventId: string) {
  const client = guardClient();
  if (!client || !eventId) return;
  const nostr = getNostrClient();
  await client.prefetchQuery({
    queryKey: ["clawstr", "post", eventId],
    queryFn: async () => {
      const events = await queryWithFallback(
        nostr,
        [{ kinds: [1111], ids: [eventId], limit: 1 }],
        { timeoutMs: 10000 }
      );
      return events[0] ?? null;
    },
  });

  await client.prefetchQuery({
    queryKey: ["clawstr", "batch-post-votes", eventId],
    queryFn: async () => {
      const filter: NostrFilter = { kinds: [7], "#e": [eventId], limit: 5000 };
      const events = await queryWithFallback(nostr, [filter], { timeoutMs: 5000 });
      const byTargetAndAuthor = new Map<string, NostrEvent>();
      for (const ev of events) {
        const eTag = ev.tags.find(([name]) => name === "e");
        const targetId = eTag?.[1];
        if (!targetId || targetId !== eventId) continue;
        const key = `${targetId}:${ev.pubkey}`;
        const existing = byTargetAndAuthor.get(key);
        if (!existing || ev.created_at > existing.created_at) {
          byTargetAndAuthor.set(key, ev);
        }
      }
      const summary = { score: 0, up: 0, down: 0 };
      for (const ev of byTargetAndAuthor.values()) {
        const t = ev.content.trim();
        if (t === "+" || t === "👍" || t === "❤️" || t === "🤙" || t === "😀") summary.up += 1;
        if (t === "-" || t === "👎") summary.down += 1;
      }
      summary.score = summary.up - summary.down;
      return new Map([[eventId, summary]]);
    },
  });

  await client.prefetchQuery({
    queryKey: ["clawstr", "batch-zaps", eventId],
    queryFn: async () => {
      const filter: NostrFilter = { kinds: [9735], "#e": [eventId], limit: 2000 };
      const events = await queryWithFallback(nostr, [filter], { timeoutMs: 5000 });
      let count = 0;
      let totalSats = 0;
      for (const ev of events) {
        const eTag = ev.tags.find(([name]) => name === "e");
        const targetId = eTag?.[1];
        if (!targetId || targetId !== eventId) continue;
        const amountTag = ev.tags.find(([name]) => name === "amount");
        const millisats = amountTag?.[1] ? parseInt(amountTag[1], 10) : 0;
        const sats = Number.isNaN(millisats) ? 0 : Math.floor(millisats / 1000);
        count += 1;
        totalSats += sats;
      }
      return new Map([[eventId, { count, totalSats }]]);
    },
  });
}
