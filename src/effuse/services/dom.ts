/**
 * Effuse DOM Service
 *
 * Type-safe DOM operations with Effect error handling.
 */

import { Context, Effect } from "effect"
import type { TemplateResult } from "../template/types.js"

/**
 * Error types for DOM operations
 */
export class DomError extends Error {
  readonly _tag = "DomError"

  constructor(
    readonly reason: "element_not_found" | "render_failed" | "invalid_selector",
    message: string
  ) {
    super(message)
    this.name = "DomError"
  }
}

/**
 * Service interface for DOM operations.
 *
 * Provides type-safe element queries, rendering, and event handling.
 */
export interface DomService {
  /**
   * Query for an element by selector.
   * Fails with DomError if not found.
   */
  readonly query: <T extends Element = Element>(
    selector: string
  ) => Effect.Effect<T, DomError>

  /**
   * Query for an element, returning null if not found.
   * Never fails.
   */
  readonly queryOption: <T extends Element = Element>(
    selector: string
  ) => Effect.Effect<T | null, never>

  /**
   * Query for an element by ID.
   * Convenience wrapper around query.
   */
  readonly queryId: <T extends Element = Element>(
    id: string
  ) => Effect.Effect<T, DomError>

  /**
   * Render a TemplateResult to an element.
   * Replaces the element's innerHTML.
   */
  readonly render: (
    element: Element,
    content: TemplateResult
  ) => Effect.Effect<void, DomError>

  /**
   * Add an event listener to an element.
   * Returns an Effect that produces a cleanup function.
   */
  readonly listen: <K extends keyof HTMLElementEventMap>(
    element: Element,
    event: K,
    handler: (e: HTMLElementEventMap[K]) => void,
    options?: AddEventListenerOptions
  ) => Effect.Effect<() => void, never>

  /**
   * Add a delegated event listener to a container.
   * Events matching the selector bubble up to the handler.
   */
  readonly delegate: <K extends keyof HTMLElementEventMap>(
    container: Element,
    selector: string,
    event: K,
    handler: (e: HTMLElementEventMap[K], target: Element) => void
  ) => Effect.Effect<() => void, never>

  /**
   * Create a DocumentFragment from a TemplateResult.
   */
  readonly createFragment: (
    content: TemplateResult
  ) => Effect.Effect<DocumentFragment, DomError>
}

/**
 * Effect Context.Tag for DomService
 */
export class DomServiceTag extends Context.Tag("effuse/DomService")<
  DomServiceTag,
  DomService
>() {}
