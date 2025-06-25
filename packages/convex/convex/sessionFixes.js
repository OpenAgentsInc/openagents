/**
 * Mutations to fix session and message data issues
 * @since Debug session
 */
import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
/**
 * Update session project information
 */
export const updateProjectInfo = mutation({
    args: {
        sessionId: v.string(),
        projectPath: v.optional(v.string()),
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
        const updates = {};
        if (args.projectPath !== undefined)
            updates.project_path = args.projectPath;
        if (args.projectName !== undefined)
            updates.project_name = args.projectName;
        return await ctx.db.patch(session._id, updates);
    }
});
/**
 * Fix all sessions with "unknown" project paths
 */
export const fixAllProjectNames = mutation({
    handler: async (ctx) => {
        const sessions = await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("project_path"), "unknown"))
            .collect();
        let updated = 0;
        for (const session of sessions) {
            // Generate a better project name from session ID
            const projectName = `Project ${session.id.substring(0, 8)}`;
            await ctx.db.patch(session._id, {
                project_name: projectName
            });
            updated++;
        }
        return { updated, total: sessions.length };
    }
});
/**
 * Update session message count based on actual messages
 */
export const fixMessageCounts = mutation({
    handler: async (ctx) => {
        const sessions = await ctx.db.query("sessions").collect();
        let fixed = 0;
        for (const session of sessions) {
            const actualMessages = await ctx.db
                .query("messages")
                .withIndex("by_session_id", q => q.eq("session_id", session.id))
                .collect();
            const actualCount = actualMessages.length;
            if (actualCount !== session.message_count) {
                await ctx.db.patch(session._id, {
                    message_count: actualCount
                });
                fixed++;
            }
        }
        return { fixed, total: sessions.length };
    }
});
/**
 * Parse JSON content in messages to plain text
 */
export const fixMessageContent = mutation({
    args: {
        sessionId: v.optional(v.string()),
        limit: v.optional(v.number())
    },
    handler: async (ctx, args) => {
        let messages = await ctx.db.query("messages").collect();
        if (args.sessionId) {
            messages = messages.filter(m => m.session_id === args.sessionId);
        }
        if (args.limit) {
            messages = messages.slice(0, args.limit);
        }
        let fixed = 0;
        for (const message of messages) {
            let needsUpdate = false;
            const updates = {};
            // Check if content looks like JSON
            if (message.content && message.content.startsWith('[') && message.content.endsWith(']')) {
                try {
                    const parsed = JSON.parse(message.content);
                    if (Array.isArray(parsed)) {
                        // Extract text from the content blocks
                        const textParts = [];
                        for (const block of parsed) {
                            if (block.type === 'text' && block.text) {
                                textParts.push(block.text);
                            }
                        }
                        if (textParts.length > 0) {
                            updates.content = textParts.join('\n');
                            needsUpdate = true;
                        }
                    }
                }
                catch (e) {
                    // Not valid JSON, leave as is
                }
            }
            if (needsUpdate) {
                await ctx.db.patch(message._id, updates);
                fixed++;
            }
        }
        return { fixed, total: messages.length };
    }
});
/**
 * Get diagnostic info for a session
 */
export const getSessionDiagnostics = query({
    args: { sessionId: v.string() },
    handler: async (ctx, args) => {
        const session = await ctx.db
            .query("sessions")
            .filter(q => q.eq(q.field("id"), args.sessionId))
            .first();
        if (!session) {
            return null;
        }
        const messages = await ctx.db
            .query("messages")
            .withIndex("by_session_id", q => q.eq("session_id", args.sessionId))
            .collect();
        const messageTypes = new Map();
        let emptyContent = 0;
        let jsonContent = 0;
        for (const msg of messages) {
            const count = messageTypes.get(msg.entry_type) || 0;
            messageTypes.set(msg.entry_type, count + 1);
            if (!msg.content || msg.content === "") {
                emptyContent++;
            }
            else if (msg.content.startsWith('[') && msg.content.endsWith(']')) {
                jsonContent++;
            }
        }
        return {
            session: {
                id: session.id,
                project_path: session.project_path,
                project_name: session.project_name,
                claimed_messages: session.message_count,
                actual_messages: messages.length
            },
            messages: {
                total: messages.length,
                empty_content: emptyContent,
                json_content: jsonContent,
                by_type: Object.fromEntries(messageTypes)
            }
        };
    }
});
//# sourceMappingURL=sessionFixes.js.map