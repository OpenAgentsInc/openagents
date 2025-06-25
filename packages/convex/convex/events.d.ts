/**
 * Convex functions for Nostr event operations
 * @since 1.0.0
 */
/**
 * Create a new Nostr event
 */
export declare const create: import("convex/server").RegisteredMutation<"public", {
    received_at?: number;
    relay_url?: string;
    id: string;
    pubkey: string;
    created_at: number;
    tags: string[][];
    content: string;
    sig: string;
    kind: number;
}, Promise<import("convex/values").GenericId<"events">>>;
/**
 * Query events with filters
 */
export declare const list: import("convex/server").RegisteredQuery<"public", {
    pubkey?: string;
    kind?: number;
    since?: number;
    until?: number;
    limit?: number;
}, Promise<{
    _id: import("convex/values").GenericId<"events">;
    _creationTime: number;
    relay_url?: string;
    id: string;
    pubkey: string;
    created_at: number;
    tags: string[][];
    content: string;
    sig: string;
    received_at: number;
    kind: number;
}[]>>;
/**
 * Get event by ID
 */
export declare const getById: import("convex/server").RegisteredQuery<"public", {
    id: string;
}, Promise<{
    _id: import("convex/values").GenericId<"events">;
    _creationTime: number;
    relay_url?: string;
    id: string;
    pubkey: string;
    created_at: number;
    tags: string[][];
    content: string;
    sig: string;
    received_at: number;
    kind: number;
} | null>>;
/**
 * Get events by tag filter
 */
export declare const getByTag: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    tagName: string;
    tagValue: string;
}, Promise<{
    _id: import("convex/values").GenericId<"events">;
    _creationTime: number;
    relay_url?: string;
    id: string;
    pubkey: string;
    created_at: number;
    tags: string[][];
    content: string;
    sig: string;
    received_at: number;
    kind: number;
}[]>>;
/**
 * Get recent events for a pubkey
 */
export declare const getByPubkeyRecent: import("convex/server").RegisteredQuery<"public", {
    limit?: number;
    pubkey: string;
}, Promise<{
    _id: import("convex/values").GenericId<"events">;
    _creationTime: number;
    relay_url?: string;
    id: string;
    pubkey: string;
    created_at: number;
    tags: string[][];
    content: string;
    sig: string;
    received_at: number;
    kind: number;
}[]>>;
//# sourceMappingURL=events.d.ts.map