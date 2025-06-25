/**
 * Convex functions for agent profile operations
 * @since 1.0.0
 */
/**
 * Create or update an agent profile
 */
export declare const upsert: import("convex/server").RegisteredMutation<"public", {
    name?: string;
    balance?: number;
    metabolic_rate?: number;
    profile_event_id?: string;
    pubkey: string;
    agent_id: string;
    status: string;
    capabilities: string[];
    last_activity: number;
}, Promise<void | import("convex/values").GenericId<"agent_profiles">>>;
/**
 * List all active agents
 */
export declare const listActive: import("convex/server").RegisteredQuery<"public", {}, Promise<{
    _id: import("convex/values").GenericId<"agent_profiles">;
    _creationTime: number;
    name?: string;
    balance?: number;
    metabolic_rate?: number;
    profile_event_id?: string;
    pubkey: string;
    created_at: number;
    agent_id: string;
    status: string;
    capabilities: string[];
    last_activity: number;
    updated_at: number;
}[]>>;
/**
 * Get agent by pubkey
 */
export declare const getByPubkey: import("convex/server").RegisteredQuery<"public", {
    pubkey: string;
}, Promise<{
    _id: import("convex/values").GenericId<"agent_profiles">;
    _creationTime: number;
    name?: string;
    balance?: number;
    metabolic_rate?: number;
    profile_event_id?: string;
    pubkey: string;
    created_at: number;
    agent_id: string;
    status: string;
    capabilities: string[];
    last_activity: number;
    updated_at: number;
} | null>>;
/**
 * Get agent by agent_id
 */
export declare const getByAgentId: import("convex/server").RegisteredQuery<"public", {
    agent_id: string;
}, Promise<{
    _id: import("convex/values").GenericId<"agent_profiles">;
    _creationTime: number;
    name?: string;
    balance?: number;
    metabolic_rate?: number;
    profile_event_id?: string;
    pubkey: string;
    created_at: number;
    agent_id: string;
    status: string;
    capabilities: string[];
    last_activity: number;
    updated_at: number;
} | null>>;
/**
 * Update agent status
 */
export declare const updateStatus: import("convex/server").RegisteredMutation<"public", {
    pubkey: string;
    status: string;
}, Promise<void>>;
/**
 * Update agent balance
 */
export declare const updateBalance: import("convex/server").RegisteredMutation<"public", {
    pubkey: string;
    balance: number;
}, Promise<void>>;
/**
 * List agents by capability
 */
export declare const listByCapability: import("convex/server").RegisteredQuery<"public", {
    capability: string;
}, Promise<{
    _id: import("convex/values").GenericId<"agent_profiles">;
    _creationTime: number;
    name?: string;
    balance?: number;
    metabolic_rate?: number;
    profile_event_id?: string;
    pubkey: string;
    created_at: number;
    agent_id: string;
    status: string;
    capabilities: string[];
    last_activity: number;
    updated_at: number;
}[]>>;
/**
 * Get agents with low balance (for hibernation)
 */
export declare const listLowBalance: import("convex/server").RegisteredQuery<"public", {
    threshold?: number;
}, Promise<{
    _id: import("convex/values").GenericId<"agent_profiles">;
    _creationTime: number;
    name?: string;
    balance?: number;
    metabolic_rate?: number;
    profile_event_id?: string;
    pubkey: string;
    created_at: number;
    agent_id: string;
    status: string;
    capabilities: string[];
    last_activity: number;
    updated_at: number;
}[]>>;
/**
 * Record agent activity
 */
export declare const recordActivity: import("convex/server").RegisteredMutation<"public", {
    pubkey: string;
}, Promise<void>>;
//# sourceMappingURL=agents.d.ts.map