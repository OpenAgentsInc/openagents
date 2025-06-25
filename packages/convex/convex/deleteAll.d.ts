/**
 * Convex mutations to delete all data
 */
/**
 * Delete all messages
 */
export declare const deleteAllMessages: import("convex/server").RegisteredMutation<"public", {}, Promise<{
    deleted: number;
}>>;
/**
 * Delete all sessions
 */
export declare const deleteAllSessions: import("convex/server").RegisteredMutation<"public", {}, Promise<{
    deleted: number;
}>>;
/**
 * Delete everything (messages first, then sessions) with pagination
 */
export declare const deleteEverything: import("convex/server").RegisteredMutation<"public", {}, Promise<{
    messagesDeleted: number;
    sessionsDeleted: number;
}>>;
//# sourceMappingURL=deleteAll.d.ts.map