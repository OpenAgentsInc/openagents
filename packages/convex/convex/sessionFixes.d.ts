/**
 * Mutations to fix session and message data issues
 * @since Debug session
 */
import type { Mutation } from "convex/server";
/**
 * Update session project information
 */
export declare const updateProjectInfo: import("convex/server").RegisteredMutation<"public", {
    projectPath?: string;
    projectName?: string;
    sessionId: string;
}, Promise<void>>;
/**
 * Fix all sessions with "unknown" project paths
 */
export declare const fixAllProjectNames: Mutation<any, any>;
/**
 * Update session message count based on actual messages
 */
export declare const fixMessageCounts: Mutation<any, any>;
/**
 * Parse JSON content in messages to plain text
 */
export declare const fixMessageContent: import("convex/server").RegisteredMutation<"public", {
    sessionId?: string;
    limit?: number;
}, Promise<{
    fixed: number;
    total: number;
}>>;
/**
 * Get diagnostic info for a session
 */
export declare const getSessionDiagnostics: import("convex/server").RegisteredQuery<"public", {
    sessionId: string;
}, Promise<{
    session: {
        id: string;
        project_path: string;
        project_name: string | undefined;
        claimed_messages: number;
        actual_messages: number;
    };
    messages: {
        total: number;
        empty_content: number;
        json_content: number;
        by_type: {
            [k: string]: number;
        };
    };
} | null>>;
//# sourceMappingURL=sessionFixes.d.ts.map