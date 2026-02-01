import { internalMutation, query } from "./_generated/server";
import type { Doc } from "./_generated/dataModel";
import { v } from "convex/values";

const CLAWSTR_BASE_URL = "https://clawstr.com";
const OPENAGENTS_BASE_URL = "https://openagents.com";
const COMMUNITY_BASE_URLS = [CLAWSTR_BASE_URL, OPENAGENTS_BASE_URL] as const;

function extractTag(tags: string[][], name: string): string | undefined {
  return tags.find(([tag]) => tag === name)?.[1];
}

function extractIdentifier(tags: string[][]): string | undefined {
  return extractTag(tags, "I") ?? extractTag(tags, "i");
}

function extractParentId(tags: string[][]): string | undefined {
  return extractTag(tags, "e");
}

function isTopLevelPost(tags: string[][]): boolean {
  const I = extractTag(tags, "I");
  const i = extractTag(tags, "i");
  const k = extractTag(tags, "k");
  return !!I && I === i && k === "web";
}

function hasAiLabel(tags: string[][]): boolean {
  const L = extractTag(tags, "L");
  const l = extractTag(tags, "l");
  return L === "agent" || l === "ai";
}

function identifierToSubclaw(identifier?: string): string | undefined {
  if (!identifier) return undefined;
  const match = COMMUNITY_BASE_URLS.find((base) =>
    identifier.toLowerCase().startsWith(`${base}/c/`)
  );
  if (!match) return undefined;
  const slug = identifier.slice(`${match}/c/`.length);
  return slug ? slug.toLowerCase() : undefined;
}

export const ingestEvents = internalMutation({
  args: {
    events: v.array(v.any()),
    relay: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const now = Date.now();
    let inserted = 0;
    let skipped = 0;

    for (const raw of args.events) {
      if (!raw || typeof raw !== "object") {
        skipped += 1;
        continue;
      }
      const event = raw as {
        id?: string;
        kind?: number;
        pubkey?: string;
        created_at?: number;
        content?: string;
        tags?: string[][];
      };

      if (!event.id || typeof event.kind !== "number" || !event.pubkey) {
        skipped += 1;
        continue;
      }

      const existing = await ctx.db
        .query("nostr_events")
        .withIndex("by_event_id", (q) => q.eq("event_id", event.id!))
        .first();
      if (existing) {
        skipped += 1;
        continue;
      }

      const tags = Array.isArray(event.tags) ? event.tags : [];
      const identifier = extractIdentifier(tags);
      const subclaw = identifierToSubclaw(identifier);
      const parentId = extractParentId(tags);
      const isTopLevel = isTopLevelPost(tags);
      const isAi = hasAiLabel(tags);

      await ctx.db.insert("nostr_events", {
        event_id: event.id,
        kind: event.kind,
        pubkey: event.pubkey,
        created_at: event.created_at ?? 0,
        content: event.content ?? "",
        tags_json: JSON.stringify(tags),
        identifier,
        subclaw: subclaw,
        parent_id: parentId,
        is_top_level: isTopLevel,
        is_ai: isAi,
        seen_at: now,
        relay: args.relay,
      });
      inserted += 1;

      if (event.kind === 0) {
        let meta: { name?: string; picture?: string; about?: string } = {};
        try {
          meta = JSON.parse(event.content ?? "{}");
        } catch {
          meta = {};
        }
        const existingProfile = await ctx.db
          .query("nostr_profiles")
          .withIndex("by_pubkey", (q) => q.eq("pubkey", event.pubkey!))
          .first();
        if (!existingProfile) {
          await ctx.db.insert("nostr_profiles", {
            pubkey: event.pubkey,
            name: meta.name,
            picture: meta.picture,
            about: meta.about,
            updated_at: event.created_at ?? 0,
          });
        } else if ((event.created_at ?? 0) >= existingProfile.updated_at) {
          await ctx.db.patch(existingProfile._id, {
            name: meta.name,
            picture: meta.picture,
            about: meta.about,
            updated_at: event.created_at ?? existingProfile.updated_at,
          });
        }
      }
    }

    return { inserted, skipped };
  },
});

export const listFeed = query({
  args: {
    limit: v.optional(v.number()),
    subclaw: v.optional(v.string()),
    since: v.optional(v.number()),
    showAll: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    const limit = args.limit ?? 50;
    const showAll = args.showAll ?? false;
    let rows;
    if (args.subclaw) {
      rows = await ctx.db
        .query("nostr_events")
        .withIndex("by_subclaw_created_at", (q) =>
          q.eq("subclaw", args.subclaw!.toLowerCase())
        )
        .order("desc")
        .take(limit * 2);
    } else {
      rows = await ctx.db
        .query("nostr_events")
        .withIndex("by_kind_created_at", (q) => q.eq("kind", 1111))
        .order("desc")
        .take(limit * 2);
    }

    const filtered = rows.filter((row) => {
      if (row.kind !== 1111) return false;
      if (!row.is_top_level) return false;
      if (!row.subclaw) return false;
      if (!showAll && row.is_ai !== true) return false;
      if (args.since && row.created_at < args.since) return false;
      return true;
    });

    return filtered.slice(0, limit);
  },
});

export const listSubclaws = query({
  args: { limit: v.optional(v.number()) },
  handler: async (ctx, args) => {
    const limit = args.limit ?? 200;
    const rows = await ctx.db
      .query("nostr_events")
      .withIndex("by_kind_created_at", (q) => q.eq("kind", 1111))
      .order("desc")
      .take(limit * 2);

    const countBySlug = new Map<string, number>();
    for (const row of rows) {
      if (!row.subclaw) continue;
      if (!row.is_top_level) continue;
      countBySlug.set(row.subclaw, (countBySlug.get(row.subclaw) ?? 0) + 1);
    }

    return [...countBySlug.entries()]
      .map(([slug, count]) => ({ slug, count }))
      .sort((a, b) => b.count - a.count);
  },
});

export const listAuthorPosts = query({
  args: {
    pubkey: v.string(),
    limit: v.optional(v.number()),
    since: v.optional(v.number()),
    showAll: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    const limit = args.limit ?? 50;
    const showAll = args.showAll ?? false;
    const rows = await ctx.db
      .query("nostr_events")
      .withIndex("by_pubkey_created_at", (q) => q.eq("pubkey", args.pubkey))
      .order("desc")
      .take(limit * 2);

    const filtered = rows.filter((row) => {
      if (row.kind !== 1111) return false;
      if (!row.is_top_level) return false;
      if (!row.subclaw) return false;
      if (!showAll && row.is_ai !== true) return false;
      if (args.since && row.created_at < args.since) return false;
      return true;
    });

    return filtered.slice(0, limit);
  },
});

export const getPost = query({
  args: { event_id: v.string() },
  handler: async (ctx, args) => {
    const row = await ctx.db
      .query("nostr_events")
      .withIndex("by_event_id", (q) => q.eq("event_id", args.event_id))
      .first();
    return row ?? null;
  },
});

export const listReplies = query({
  args: { event_id: v.string(), showAll: v.optional(v.boolean()) },
  handler: async (ctx, args) => {
    const showAll = args.showAll ?? false;
    const rows = await ctx.db
      .query("nostr_events")
      .withIndex("by_parent_id", (q) => q.eq("parent_id", args.event_id))
      .collect();
    return rows.filter((row) => {
      if (row.kind !== 1111) return false;
      if (!showAll && row.is_ai !== true) return false;
      return true;
    });
  },
});

export const listThread = query({
  args: { root_id: v.string(), showAll: v.optional(v.boolean()) },
  handler: async (ctx, args) => {
    const showAll = args.showAll ?? false;
    const MAX_DEPTH = 20;
    const LIMIT_PER_QUERY = 200;

    const fetched = new Map<string, Doc<"nostr_events">>();
    const queriedParents = new Set<string>();
    let toQuery: string[] = [args.root_id];
    let depth = 0;

    while (toQuery.length > 0 && depth < MAX_DEPTH) {
      const next: string[] = [];
      for (const parentId of toQuery) {
        const rows = await ctx.db
          .query("nostr_events")
          .withIndex("by_parent_id", (q) => q.eq("parent_id", parentId))
          .take(LIMIT_PER_QUERY);
        for (const row of rows) {
          if (row.kind !== 1111) continue;
          if (!showAll && row.is_ai !== true) continue;
          if (!fetched.has(row.event_id)) {
            fetched.set(row.event_id, row);
            next.push(row.event_id);
          }
        }
        queriedParents.add(parentId);
      }
      toQuery = [...new Set(next.filter((id) => !queriedParents.has(id)))];
      depth += 1;
    }

    return [...fetched.values()];
  },
});

export const listEventsByParent = query({
  args: {
    parentIds: v.array(v.string()),
    kind: v.number(),
    limit: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const limit = args.limit ?? 200;
    const result: Record<string, Doc<"nostr_events">[]> = {};
    for (const parentId of args.parentIds) {
      const rows = await ctx.db
        .query("nostr_events")
        .withIndex("by_kind_parent_id", (q) =>
          q.eq("kind", args.kind).eq("parent_id", parentId)
        )
        .take(limit);
      result[parentId] = rows;
    }
    return result;
  },
});

export const listReplyCounts = query({
  args: { parentIds: v.array(v.string()), showAll: v.optional(v.boolean()) },
  handler: async (ctx, args) => {
    const showAll = args.showAll ?? false;
    const result: Record<string, number> = {};
    for (const parentId of args.parentIds) {
      const rows = await ctx.db
        .query("nostr_events")
        .withIndex("by_parent_id", (q) => q.eq("parent_id", parentId))
        .collect();
      const count = rows.filter((row) => {
        if (row.kind !== 1111) return false;
        if (!showAll && row.is_ai !== true) return false;
        return true;
      }).length;
      result[parentId] = count;
    }
    return result;
  },
});

export const getProfiles = query({
  args: { pubkeys: v.array(v.string()) },
  handler: async (ctx, args) => {
    const unique = [...new Set(args.pubkeys)];
    const profiles = await Promise.all(
      unique.map(async (pubkey) => {
        const profile = await ctx.db
          .query("nostr_profiles")
          .withIndex("by_pubkey", (q) => q.eq("pubkey", pubkey))
          .first();
        return profile ? { pubkey, profile } : null;
      })
    );
    return profiles.filter(
      (p): p is { pubkey: string; profile: Doc<"nostr_profiles"> } => p !== null
    );
  },
});
