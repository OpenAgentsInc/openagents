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
 * Error type for webview-bun test operations.
 *
 * Covers webview launch, subprocess, and execution errors.
 */
export class WebviewTestError extends Data.TaggedError("WebviewTestError")<{
  readonly reason:
    | "webview_not_found"
    | "launch_failed"
    | "subprocess_failed"
    | "execution_failed"
    | "timeout"
    | "parse_failed"
  readonly message: string
  readonly exitCode?: number
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

/**
 * Options for webview test execution.
 */
export interface WebviewTestOptions {
  /** Show webview window for debugging (default: false) */
  readonly headed?: boolean
  /** Delay between actions in ms (default: 0) */
  readonly slowMo?: number
  /** Test timeout in ms (default: 30000) */
  readonly timeout?: number
}
