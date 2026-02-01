import type { NostrEvent } from "@nostrify/nostrify";
import { convexClient } from "@/lib/convex";
import { api } from "../../convex/_generated/api";

type NostrRow = {
  event_id: string;
  kind: number;
  pubkey: string;
  created_at: number;
  content: string;
  tags_json: string;
};

function safeParseTags(tagsJson: string): string[][] {
  try {
    const parsed = JSON.parse(tagsJson) as unknown;
    if (Array.isArray(parsed)) {
      return parsed.filter((tag): tag is string[] => Array.isArray(tag));
    }
    return [];
  } catch {
    return [];
  }
}

function rowToEvent(row: NostrRow): NostrEvent {
  return {
    id: row.event_id,
    kind: row.kind,
    pubkey: row.pubkey,
    created_at: row.created_at,
    content: row.content,
    tags: safeParseTags(row.tags_json),
    sig: "",
  };
}

function isBrowser(): boolean {
  return typeof window !== "undefined";
}

export async function fetchConvexFeed(args: {
  limit?: number;
  subclaw?: string;
  since?: number;
  showAll?: boolean;
}): Promise<NostrEvent[]> {
  if (!isBrowser()) return [];
  try {
    const rows = await convexClient.query(api.nostr.listFeed, args);
    return rows.map(rowToEvent);
  } catch {
    return [];
  }
}

export async function fetchConvexSubclaws(limit = 200) {
  if (!isBrowser()) return [];
  try {
    return await convexClient.query(api.nostr.listSubclaws, { limit });
  } catch {
    return [];
  }
}

export async function fetchConvexAuthorPosts(args: {
  pubkey: string;
  limit?: number;
  since?: number;
  showAll?: boolean;
}): Promise<NostrEvent[]> {
  if (!isBrowser()) return [];
  try {
    const rows = await convexClient.query(api.nostr.listAuthorPosts, args);
    return rows.map(rowToEvent);
  } catch {
    return [];
  }
}

export async function fetchConvexPost(eventId: string): Promise<NostrEvent | null> {
  if (!isBrowser()) return null;
  try {
    const row = await convexClient.query(api.nostr.getPost, { event_id: eventId });
    return row ? rowToEvent(row) : null;
  } catch {
    return null;
  }
}

export async function fetchConvexReplies(eventId: string, showAll = false): Promise<NostrEvent[]> {
  if (!isBrowser()) return [];
  try {
    const rows = await convexClient.query(api.nostr.listReplies, {
      event_id: eventId,
      showAll,
    });
    return rows.map(rowToEvent);
  } catch {
    return [];
  }
}

export async function fetchConvexThread(eventId: string, showAll = false): Promise<NostrEvent[]> {
  if (!isBrowser()) return [];
  try {
    const rows = await convexClient.query(api.nostr.listThread, {
      root_id: eventId,
      showAll,
    });
    return rows.map(rowToEvent);
  } catch {
    return [];
  }
}

export async function fetchConvexProfiles(pubkeys: string[]) {
  if (!isBrowser()) return new Map<string, { name?: string; picture?: string; about?: string }>();
  try {
    const rows = await convexClient.query(api.nostr.getProfiles, { pubkeys });
    const map = new Map<string, { name?: string; picture?: string; about?: string }>();
    for (const row of rows) {
      const profile = row.profile as {
        name?: string;
        picture?: string;
        about?: string;
      };
      map.set(row.pubkey, {
        name: profile?.name,
        picture: profile?.picture,
        about: profile?.about,
      });
    }
    return map;
  } catch {
    return new Map();
  }
}

export async function fetchConvexEventsByParent(
  kind: number,
  parentIds: string[],
  limit = 200
) {
  if (!isBrowser() || parentIds.length === 0) return new Map<string, NostrEvent[]>();
  try {
    const rows = await convexClient.query(api.nostr.listEventsByParent, {
      kind,
      parentIds,
      limit,
    });
    const map = new Map<string, NostrEvent[]>();
    for (const parentId of parentIds) {
      const events = rows[parentId] ?? [];
      map.set(parentId, events.map(rowToEvent));
    }
    return map;
  } catch {
    return new Map();
  }
}

export async function fetchConvexReplyCounts(parentIds: string[], showAll = false) {
  if (!isBrowser() || parentIds.length === 0) return new Map<string, number>();
  try {
    const result = await convexClient.query(api.nostr.listReplyCounts, {
      parentIds,
      showAll,
    });
    const map = new Map<string, number>();
    for (const parentId of parentIds) {
      map.set(parentId, result[parentId] ?? 0);
    }
    return map;
  } catch {
    return new Map();
  }
}
