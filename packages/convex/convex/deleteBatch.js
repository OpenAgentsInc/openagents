/**
 * Batch delete functions with smaller limits
 */
import { mutation } from "./_generated/server";
/**
 * Delete a batch of messages
 */
export const deleteMessageBatch = mutation({
    args: {},
    handler: async (ctx) => {
        const messages = await ctx.db.query("messages").take(50);
        for (const message of messages) {
            await ctx.db.delete(message._id);
        }
        return {
            deleted: messages.length,
            hasMore: messages.length === 50
        };
    }
});
/**
 * Delete a batch of sessions
 */
export const deleteSessionBatch = mutation({
    args: {},
    handler: async (ctx) => {
        const sessions = await ctx.db.query("sessions").take(50);
        for (const session of sessions) {
            await ctx.db.delete(session._id);
        }
        return {
            deleted: sessions.length,
            hasMore: sessions.length === 50
        };
    }
});
//# sourceMappingURL=deleteBatch.js.map