declare const ClaudeCodeNotFoundError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "ClaudeCodeNotFoundError";
} & Readonly<A>;
/**
 * Error thrown when Claude Code CLI is not found or not executable
 * @since 1.0.0
 */
export declare class ClaudeCodeNotFoundError extends ClaudeCodeNotFoundError_base<{
    readonly message: string;
    readonly cause?: unknown;
}> {
}
declare const ClaudeCodeExecutionError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "ClaudeCodeExecutionError";
} & Readonly<A>;
/**
 * Error thrown when Claude Code command execution fails
 * @since 1.0.0
 */
export declare class ClaudeCodeExecutionError extends ClaudeCodeExecutionError_base<{
    readonly command: string;
    readonly exitCode: number;
    readonly stderr: string;
    readonly cause?: unknown;
}> {
}
declare const ClaudeCodeParseError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "ClaudeCodeParseError";
} & Readonly<A>;
/**
 * Error thrown when Claude Code output parsing fails
 * @since 1.0.0
 */
export declare class ClaudeCodeParseError extends ClaudeCodeParseError_base<{
    readonly output: string;
    readonly format: string;
    readonly cause?: unknown;
}> {
}
declare const ClaudeCodeSessionError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "ClaudeCodeSessionError";
} & Readonly<A>;
/**
 * Error thrown when Claude Code session is invalid or expired
 * @since 1.0.0
 */
export declare class ClaudeCodeSessionError extends ClaudeCodeSessionError_base<{
    readonly sessionId: string;
    readonly message: string;
}> {
}
declare const ClaudeCodeInitError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "ClaudeCodeInitError";
} & Readonly<A>;
/**
 * Error thrown when Claude Code initialization fails
 * @since 1.0.0
 */
export declare class ClaudeCodeInitError extends ClaudeCodeInitError_base<{
    readonly message: string;
    readonly cause?: unknown;
}> {
}
declare const ClaudeCodeError_base: new <A extends Record<string, any> = {}>(args: import("effect/Types").Equals<A, {}> extends true ? void : { readonly [P in keyof A as P extends "_tag" ? never : P]: A[P]; }) => import("effect/Cause").YieldableError & {
    readonly _tag: "ClaudeCodeError";
} & Readonly<A>;
/**
 * General Claude Code error
 * @since 1.0.0
 */
export declare class ClaudeCodeError extends ClaudeCodeError_base<{
    readonly message: string;
    readonly cause?: unknown;
}> {
}
export {};
//# sourceMappingURL=index.d.ts.map