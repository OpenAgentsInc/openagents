import { Data } from "effect";
/**
 * Error thrown when Claude Code CLI is not found or not executable
 * @since 1.0.0
 */
export class ClaudeCodeNotFoundError extends Data.TaggedError("ClaudeCodeNotFoundError") {
}
/**
 * Error thrown when Claude Code command execution fails
 * @since 1.0.0
 */
export class ClaudeCodeExecutionError extends Data.TaggedError("ClaudeCodeExecutionError") {
}
/**
 * Error thrown when Claude Code output parsing fails
 * @since 1.0.0
 */
export class ClaudeCodeParseError extends Data.TaggedError("ClaudeCodeParseError") {
}
/**
 * Error thrown when Claude Code session is invalid or expired
 * @since 1.0.0
 */
export class ClaudeCodeSessionError extends Data.TaggedError("ClaudeCodeSessionError") {
}
/**
 * Error thrown when Claude Code initialization fails
 * @since 1.0.0
 */
export class ClaudeCodeInitError extends Data.TaggedError("ClaudeCodeInitError") {
}
/**
 * General Claude Code error
 * @since 1.0.0
 */
export class ClaudeCodeError extends Data.TaggedError("ClaudeCodeError") {
}
//# sourceMappingURL=index.js.map