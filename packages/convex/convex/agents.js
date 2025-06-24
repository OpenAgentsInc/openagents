/**
 * Convex functions for agent profile operations
 * @since 1.0.0
 */
import { v } from "convex/values";
import { query, mutation } from "./_generated/server";
/**
 * Create or update an agent profile
 */
export const upsert = mutation({
    args: {
        pubkey: v.string(),
        agent_id: v.string(),
        name: v.optional(v.string()),
        status: v.string(),
        balance: v.optional(v.number()),
        metabolic_rate: v.optional(v.number()),
        capabilities: v.array(v.string()),
        last_activity: v.number(),
        profile_event_id: v.optional(v.string())
    },
    handler: async (ctx, args) => {
        const now = Date.now();
        // Check if agent already exists
        const existing = await ctx.db
            .query("agent_profiles")
            .filter(q => q.eq(q.field("pubkey"), args.pubkey))
            .first();
        if (existing) {
            // Update existing agent
            return await ctx.db.patch(existing._id, {
                ...args,
                updated_at: now
            });
        }
        else {
            // Create new agent
            return await ctx.db.insert("agent_profiles", {
                ...args,
                created_at: now,
                updated_at: now
            });
        }
    }
});
/**
 * List all active agents
 */
export const listActive = query({
    args: {},
    handler: async (ctx) => {
        return await ctx.db
            .query("agent_profiles")
            .withIndex("by_status", q => q.eq("status", "active"))
            .order("desc")
            .collect();
    }
});
/**
 * Get agent by pubkey
 */
export const getByPubkey = query({
    args: { pubkey: v.string() },
    handler: async (ctx, args) => {
        return await ctx.db
            .query("agent_profiles")
            .filter(q => q.eq(q.field("pubkey"), args.pubkey))
            .first();
    }
});
/**
 * Get agent by agent_id
 */
export const getByAgentId = query({
    args: { agent_id: v.string() },
    handler: async (ctx, args) => {
        return await ctx.db
            .query("agent_profiles")
            .withIndex("by_agent_id", q => q.eq("agent_id", args.agent_id))
            .first();
    }
});
/**
 * Update agent status
 */
export const updateStatus = mutation({
    args: {
        pubkey: v.string(),
        status: v.string()
    },
    handler: async (ctx, args) => {
        const agent = await ctx.db
            .query("agent_profiles")
            .filter(q => q.eq(q.field("pubkey"), args.pubkey))
            .first();
        if (!agent) {
            throw new Error(`Agent not found: ${args.pubkey}`);
        }
        return await ctx.db.patch(agent._id, {
            status: args.status,
            last_activity: Date.now(),
            updated_at: Date.now()
        });
    }
});
/**
 * Update agent balance
 */
export const updateBalance = mutation({
    args: {
        pubkey: v.string(),
        balance: v.number()
    },
    handler: async (ctx, args) => {
        const agent = await ctx.db
            .query("agent_profiles")
            .filter(q => q.eq(q.field("pubkey"), args.pubkey))
            .first();
        if (!agent) {
            throw new Error(`Agent not found: ${args.pubkey}`);
        }
        return await ctx.db.patch(agent._id, {
            balance: args.balance,
            last_activity: Date.now(),
            updated_at: Date.now()
        });
    }
});
/**
 * List agents by capability
 */
export const listByCapability = query({
    args: { capability: v.string() },
    handler: async (ctx, args) => {
        const agents = await ctx.db
            .query("agent_profiles")
            .withIndex("by_status", q => q.eq("status", "active"))
            .collect();
        // Filter agents that have the requested capability
        return agents.filter(agent => agent.capabilities.includes(args.capability));
    }
});
/**
 * Get agents with low balance (for hibernation)
 */
export const listLowBalance = query({
    args: { threshold: v.optional(v.number()) },
    handler: async (ctx, args) => {
        const threshold = args.threshold ?? 1000; // Default 1000 sats
        return await ctx.db
            .query("agent_profiles")
            .withIndex("by_balance")
            .filter(q => q.lte(q.field("balance"), threshold))
            .collect();
    }
});
/**
 * Record agent activity
 */
export const recordActivity = mutation({
    args: { pubkey: v.string() },
    handler: async (ctx, args) => {
        const agent = await ctx.db
            .query("agent_profiles")
            .filter(q => q.eq(q.field("pubkey"), args.pubkey))
            .first();
        if (!agent) {
            throw new Error(`Agent not found: ${args.pubkey}`);
        }
        return await ctx.db.patch(agent._id, {
            last_activity: Date.now(),
            updated_at: Date.now()
        });
    }
});
//# sourceMappingURL=agents.js.map