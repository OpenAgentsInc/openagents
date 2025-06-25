/**
 * Batch delete functions with smaller limits
 */
/**
 * Delete a batch of messages
 */
export declare const deleteMessageBatch: import("convex/server").RegisteredMutation<"public", {}, Promise<{
    deleted: number;
    hasMore: boolean;
}>>;
/**
 * Delete a batch of sessions
 */
export declare const deleteSessionBatch: import("convex/server").RegisteredMutation<"public", {}, Promise<{
    deleted: number;
    hasMore: boolean;
}>>;
//# sourceMappingURL=deleteBatch.d.ts.map