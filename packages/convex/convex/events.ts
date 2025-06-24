/**
 * Convex functions for Nostr event operations
 * @since 1.0.0
 */

import { v } from "convex/values"
import { query, mutation } from "./_generated/server"

/**
 * Create a new Nostr event
 */
export const create = mutation({
  args: {
    id: v.string(),
    pubkey: v.string(),
    created_at: v.number(),
    kind: v.number(),
    tags: v.array(v.array(v.string())),
    content: v.string(),
    sig: v.string(),
    received_at: v.optional(v.number()),
    relay_url: v.optional(v.string())
  },
  handler: async (ctx, args) => {
    // Set received_at if not provided
    const received_at = args.received_at ?? Date.now()
    
    const eventId = await ctx.db.insert("events", {
      ...args,
      received_at
    })

    // Extract and store tags for efficient querying
    for (let i = 0; i < args.tags.length; i++) {
      const tag = args.tags[i]
      if (tag.length > 0) {
        await ctx.db.insert("event_tags", {
          event_id: args.id,
          tag_name: tag[0],
          tag_value: tag[1] || "",
          tag_index: i
        })
      }
    }

    return eventId
  }
})

/**
 * Query events with filters
 */
export const list = query({
  args: {
    pubkey: v.optional(v.string()),
    kind: v.optional(v.number()),
    since: v.optional(v.number()),
    until: v.optional(v.number()),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    let query = ctx.db.query("events")

    // Apply filters
    if (args.pubkey) {
      query = query.filter(q => q.eq(q.field("pubkey"), args.pubkey))
    }
    
    if (args.kind !== undefined) {
      query = query.filter(q => q.eq(q.field("kind"), args.kind))
    }

    if (args.since !== undefined) {
      query = query.filter(q => q.gte(q.field("created_at"), args.since!))
    }

    if (args.until !== undefined) {
      query = query.filter(q => q.lte(q.field("created_at"), args.until!))
    }

    // Order by created_at descending and apply limit
    const limit = args.limit ?? 100
    return await query
      .order("desc")
      .take(limit)
  }
})

/**
 * Get event by ID
 */
export const getById = query({
  args: { id: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("events")
      .filter(q => q.eq(q.field("id"), args.id))
      .first()
  }
})

/**
 * Get events by tag filter
 */
export const getByTag = query({
  args: {
    tagName: v.string(),
    tagValue: v.string(),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    // Find event tags matching the filter
    const tags = await ctx.db
      .query("event_tags")
      .withIndex("by_tag_name_value", q => 
        q.eq("tag_name", args.tagName).eq("tag_value", args.tagValue)
      )
      .take(args.limit ?? 100)

    // Get the corresponding events
    const eventIds = tags.map(tag => tag.event_id)
    const events = []
    
    for (const eventId of eventIds) {
      const event = await ctx.db
        .query("events")
        .filter(q => q.eq(q.field("id"), eventId))
        .first()
      
      if (event) {
        events.push(event)
      }
    }

    // Sort by created_at descending
    return events.sort((a, b) => b.created_at - a.created_at)
  }
})

/**
 * Get recent events for a pubkey
 */
export const getByPubkeyRecent = query({
  args: {
    pubkey: v.string(),
    limit: v.optional(v.number())
  },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("events")
      .withIndex("by_pubkey_created", q => q.eq("pubkey", args.pubkey))
      .order("desc")
      .take(args.limit ?? 20)
  }
})