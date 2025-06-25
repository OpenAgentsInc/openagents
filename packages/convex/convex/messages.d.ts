/**
 * Convex functions for chat message operations
 * @since 1.0.0
 */
/**
 * Create a new chat message
 */
export declare const create: import("convex/server").RegisteredMutation<"public", {
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}, Promise<import("convex/values").GenericId<"messages">>>;
/**
 * Get messages for a session
 */
export declare const listBySession: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    offset?: number;
    sessionId: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}[]>>;
/**
 * Get recent messages for a session
 */
export declare const getRecent: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    sessionId: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}[]>>;
/**
 * Get message by entry UUID
 */
export declare const getByUuid: import("convex/server").RegisteredQuery<"public", {
    entryUuid: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
} | null>>;
/**
 * Get messages by type
 */
export declare const listByType: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    sessionId: string;
    entryType: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}[]>>;
/**
 * Get tool usage messages
 */
export declare const listToolUsage: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    toolName?: string;
    sessionId: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}[]>>;
/**
 * Get messages by tool use ID (for linking tool_use and tool_result)
 */
export declare const listByToolUseId: import("convex/server").RegisteredQuery<"public", {
    toolUseId: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}[]>>;
/**
 * Update message content (for streaming updates)
 */
export declare const updateContent: import("convex/server").RegisteredMutation<"public", {
    content?: string;
    thinking?: string;
    entryUuid: string;
}, Promise<void>>;
/**
 * Update message fields (comprehensive update for fixing imported data)
 */
export declare const update: import("convex/server").RegisteredMutation<"public", {
    content?: string;
    thinking?: string;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    entryUuid: string;
}, Promise<void>>;
/**
 * Get session message statistics
 */
export declare const getSessionStats: import("convex/server").RegisteredQuery<"public", {
    sessionId: string;
}, Promise<{
    totalMessages: number;
    userMessages: number;
    assistantMessages: number;
    toolUses: number;
    totalTokens: number;
    totalCost: number;
    firstMessageAt: any;
    lastMessageAt: any;
    averageTokensPerMessage: number;
}>>;
/**
 * Search messages by content
 */
export declare const search: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    sessionId: string;
    searchTerm: string;
}, Promise<{
    _id: import("convex/values").GenericId<"messages">;
    _creationTime: number;
    content?: string;
    role?: string;
    thinking?: string;
    summary?: string;
    model?: string;
    token_usage?: {
        input_tokens: number;
        output_tokens: number;
        total_tokens: number;
    };
    cost?: number;
    turn_count?: number;
    tool_name?: string;
    tool_input?: any;
    tool_use_id?: string;
    tool_output?: string;
    tool_is_error?: boolean;
    timestamp: number;
    session_id: string;
    entry_uuid: string;
    entry_type: string;
}[]>>;
//# sourceMappingURL=messages.d.ts.map