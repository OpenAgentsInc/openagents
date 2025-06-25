/**
 * Convex functions for chat session operations
 * @since 1.0.0
 */
/**
 * Create a new chat session
 */
export declare const create: import("convex/server").RegisteredMutation<"public", {
    project_name?: string;
    id: string;
    status: string;
    last_activity: number;
    message_count: number;
    user_id: string;
    project_path: string;
    started_at: number;
    total_cost: number;
}, Promise<import("convex/values").GenericId<"sessions">>>;
/**
 * Get sessions for a user
 */
export declare const listByUser: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    userId: string;
}, Promise<{
    _id: import("convex/values").GenericId<"sessions">;
    _creationTime: number;
    project_name?: string;
    id: string;
    status: string;
    last_activity: number;
    message_count: number;
    user_id: string;
    project_path: string;
    started_at: number;
    total_cost: number;
}[]>>;
/**
 * Get session by ID
 */
export declare const getById: import("convex/server").RegisteredQuery<"public", {
    sessionId: string;
}, Promise<{
    _id: import("convex/values").GenericId<"sessions">;
    _creationTime: number;
    project_name?: string;
    id: string;
    status: string;
    last_activity: number;
    message_count: number;
    user_id: string;
    project_path: string;
    started_at: number;
    total_cost: number;
} | null>>;
/**
 * Update session activity timestamp
 */
export declare const updateActivity: import("convex/server").RegisteredMutation<"public", {
    timestamp?: number;
    sessionId: string;
}, Promise<void>>;
/**
 * Update session message count and cost
 */
export declare const updateStats: import("convex/server").RegisteredMutation<"public", {
    sessionId: string;
    messageCount: number;
    totalCost: number;
}, Promise<void>>;
/**
 * Update session status
 */
export declare const updateStatus: import("convex/server").RegisteredMutation<"public", {
    status: string;
    sessionId: string;
}, Promise<void>>;
/**
 * Update session project information
 */
export declare const updateProject: import("convex/server").RegisteredMutation<"public", {
    projectName?: string;
    sessionId: string;
    projectPath: string;
}, Promise<void>>;
/**
 * List recent sessions across all users
 */
export declare const listRecent: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
}, Promise<{
    _id: import("convex/values").GenericId<"sessions">;
    _creationTime: number;
    project_name?: string;
    id: string;
    status: string;
    last_activity: number;
    message_count: number;
    user_id: string;
    project_path: string;
    started_at: number;
    total_cost: number;
}[]>>;
/**
 * Get sessions by project path
 */
export declare const listByProject: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    projectPath: string;
}, Promise<{
    _id: import("convex/values").GenericId<"sessions">;
    _creationTime: number;
    project_name?: string;
    id: string;
    status: string;
    last_activity: number;
    message_count: number;
    user_id: string;
    project_path: string;
    started_at: number;
    total_cost: number;
}[]>>;
/**
 * Get session statistics
 */
export declare const getStats: import("convex/server").RegisteredQuery<"public", {
    userId?: string;
}, Promise<{
    totalSessions: number;
    activeSessions: number;
    totalMessages: number;
    totalCost: number;
    averageMessagesPerSession: number;
    averageCostPerSession: number;
}>>;
//# sourceMappingURL=sessions.d.ts.map