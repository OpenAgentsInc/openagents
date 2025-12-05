/**
 * Effuse Testing Errors
 *
 * Effect-native error types for test operations.
 */

import { Data } from "effect"

/**
 * Error type for test operations.
 *
 * Uses Effect's Data.TaggedError for proper error handling.
 */
export class TestError extends Data.TaggedError("TestError")<{
  readonly reason:
    | "element_not_found"
    | "timeout"
    | "assertion_failed"
    | "action_failed"
    | "mount_failed"
  readonly message: string
}> {}

/**
 * Options for waiting operations.
 */
export interface WaitOptions {
  /** Maximum time to wait in milliseconds (default: 5000) */
  readonly timeout?: number
  /** Polling interval in milliseconds (default: 50) */
  readonly interval?: number
}
