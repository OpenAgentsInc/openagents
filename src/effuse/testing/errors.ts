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
 * Error type for CDP operations.
 *
 * Covers browser launch, WebSocket, and protocol errors.
 */
export class CDPError extends Data.TaggedError("CDPError")<{
  readonly reason:
    | "browser_not_found"
    | "launch_failed"
    | "connection_failed"
    | "protocol_error"
    | "navigation_failed"
    | "page_closed"
  readonly message: string
  readonly code?: number
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
 * Options for browser launch.
 */
export interface BrowserOptions {
  /** Run headless (default: true) */
  readonly headless?: boolean
  /** Delay between actions in ms (default: 0) */
  readonly slowMo?: number
  /** Custom Chrome path */
  readonly chromePath?: string
  /** Viewport width (default: 1280) */
  readonly width?: number
  /** Viewport height (default: 720) */
  readonly height?: number
}
