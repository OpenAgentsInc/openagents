import { Data } from "effect"

/**
 * Error thrown when Claude Code CLI is not found or not executable
 * @since 1.0.0
 */
export class ClaudeCodeNotFoundError extends Data.TaggedError("ClaudeCodeNotFoundError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

/**
 * Error thrown when Claude Code command execution fails
 * @since 1.0.0
 */
export class ClaudeCodeExecutionError extends Data.TaggedError("ClaudeCodeExecutionError")<{
  readonly command: string
  readonly exitCode: number
  readonly stderr: string
  readonly cause?: unknown
}> {}

/**
 * Error thrown when Claude Code output parsing fails
 * @since 1.0.0
 */
export class ClaudeCodeParseError extends Data.TaggedError("ClaudeCodeParseError")<{
  readonly output: string
  readonly format: string
  readonly cause?: unknown
}> {}

/**
 * Error thrown when Claude Code session is invalid or expired
 * @since 1.0.0
 */
export class ClaudeCodeSessionError extends Data.TaggedError("ClaudeCodeSessionError")<{
  readonly sessionId: string
  readonly message: string
}> {}

/**
 * Error thrown when Claude Code initialization fails
 * @since 1.0.0
 */
export class ClaudeCodeInitError extends Data.TaggedError("ClaudeCodeInitError")<{
  readonly message: string
  readonly cause?: unknown
}> {}

/**
 * General Claude Code error
 * @since 1.0.0
 */
export class ClaudeCodeError extends Data.TaggedError("ClaudeCodeError")<{
  readonly message: string
  readonly cause?: unknown
}> {}