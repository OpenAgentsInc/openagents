/**
 * Convex functions for chat session operations
 * @since 1.0.0
 */
import { v } from "convex/values";
import { query, mutation } from "./_generated/server";
/**
 * Create a new chat session
 */
export const create = mutation({
    args: {
        id: v.string(),
        user_id: v.string(),
        project_path: v.string(),
        project_name: v.optional(v.string()),
        status: v.string(),
        started_at: v.number(),
        last_activity: v.number(),
        message_count: v.number(),
        total_cost: v.number()
    },
    handler: async (ctx, args) => {
        return await ctx.db.insert("sessions", args);
    }
});
/**
 * Get sessions for a user
 */
export const listByUser = query({
    args: {
        userId: v.string(),
        limit: v.optional(v.number())
    },
    handler: async (ctx, args) => {
        return await ctx.db
            .query("sessions")
            .withIndex("by_user_id", q => q.eq("user_id", args.userId))
            .order("desc")
            .take(args.limit ?? 50);
    }
});
/**
 * Get session by ID
 */
export const getById = query({
    args: { sessionId: v.string() },
    handler: async (ctx, args) => {
        return await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("id"), args.sessionId))
            .first();
    }
});
/**
 * Update session activity timestamp
 */
export const updateActivity = mutation({
    args: {
        sessionId: v.string(),
        timestamp: v.optional(v.number())
    },
    handler: async (ctx, args) => {
        const session = await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("id"), args.sessionId))
            .first();
        if (!session) {
            throw new Error(`Session not found: ${args.sessionId}`);
        }
        return await ctx.db.patch(session._id, {
            last_activity: args.timestamp ?? Date.now()
        });
    }
});
/**
 * Update session message count and cost
 */
export const updateStats = mutation({
    args: {
        sessionId: v.string(),
        messageCount: v.number(),
        totalCost: v.number()
    },
    handler: async (ctx, args) => {
        const session = await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("id"), args.sessionId))
            .first();
        if (!session) {
            throw new Error(`Session not found: ${args.sessionId}`);
        }
        return await ctx.db.patch(session._id, {
            message_count: args.messageCount,
            total_cost: args.totalCost,
            last_activity: Date.now()
        });
    }
});
/**
 * Update session status
 */
export const updateStatus = mutation({
    args: {
        sessionId: v.string(),
        status: v.string()
    },
    handler: async (ctx, args) => {
        const session = await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("id"), args.sessionId))
            .first();
        if (!session) {
            throw new Error(`Session not found: ${args.sessionId}`);
        }
        return await ctx.db.patch(session._id, {
            status: args.status,
            last_activity: Date.now()
        });
    }
});
/**
 * Update session project information
 */
export const updateProject = mutation({
    args: {
        sessionId: v.string(),
        projectPath: v.string(),
        projectName: v.optional(v.string())
    },
    handler: async (ctx, args) => {
        const session = await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("id"), args.sessionId))
            .first();
        if (!session) {
            throw new Error(`Session not found: ${args.sessionId}`);
        }
        const updateData = {
            project_path: args.projectPath,
            last_activity: Date.now()
        };
        if (args.projectName !== undefined) {
            updateData.project_name = args.projectName;
        }
        return await ctx.db.patch(session._id, updateData);
    }
});
/**
 * List recent sessions across all users
 */
export const listRecent = query({
    args: { limit: v.optional(v.number()) },
    handler: async (ctx, args) => {
        return await ctx.db
            .query("sessions")
            .withIndex("by_last_activity")
            .order("desc")
            .take(args.limit ?? 20);
    }
});
/**
 * Get sessions by project path
 */
export const listByProject = query({
    args: {
        projectPath: v.string(),
        limit: v.optional(v.number())
    },
    handler: async (ctx, args) => {
        return await ctx.db
            .query("sessions")
            .withIndex("by_project_path", q => q.eq("project_path", args.projectPath))
            .order("desc")
            .take(args.limit ?? 20);
    }
});
/**
 * Get session statistics
 */
export const getStats = query({
    args: { userId: v.optional(v.string()) },
    handler: async (ctx, args) => {
        let query = ctx.db.query("sessions");
        if (args.userId) {
            query = query.filter(q => q.eq(q.field("user_id"), args.userId));
        }
        const sessions = await query.collect();
        const totalSessions = sessions.length;
        const activeSessions = sessions.filter(s => s.status === "active").length;
        const totalMessages = sessions.reduce((sum, s) => sum + s.message_count, 0);
        const totalCost = sessions.reduce((sum, s) => sum + s.total_cost, 0);
        return {
            totalSessions,
            activeSessions,
            totalMessages,
            totalCost,
            averageMessagesPerSession: totalSessions > 0 ? totalMessages / totalSessions : 0,
            averageCostPerSession: totalSessions > 0 ? totalCost / totalSessions : 0
        };
    }
});
//# sourceMappingURL=sessions.js.map