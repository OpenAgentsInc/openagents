/**
 * DomService - Type-safe DOM operations
 */

import { Context, Effect } from "effect"
import type { TemplateResult } from "../template/types.js"

export type DomSwapMode =
  | "inner"
  | "outer"
  | "beforeend"
  | "afterbegin"
  | "delete"
  | "replace"

export class DomError {
  readonly _tag = "DomError"
  constructor(
    readonly message: string,
    readonly cause?: unknown
  ) {}
}

export interface DomService {
  /**
   * Query for a single element (throws if not found)
   */
  readonly query: (selector: string) => Effect.Effect<Element, DomError>

  /**
   * Query for a single element (returns Option)
   */
  readonly queryOption: (selector: string) => Effect.Effect<Element | null, DomError>

  /**
   * Query for multiple elements
   */
  readonly queryAll: (selector: string) => Effect.Effect<readonly Element[], DomError>

  /**
   * Render template to container (replaces innerHTML)
   */
  readonly render: (
    container: Element,
    content: TemplateResult
  ) => Effect.Effect<void, DomError>

  /**
   * Swap rendered content into a target element.
   */
  readonly swap: (
    target: Element,
    content: TemplateResult,
    mode?: DomSwapMode
  ) => Effect.Effect<void, DomError>

  /**
   * Set up event delegation
   */
  readonly delegate: (
    container: Element,
    selector: string,
    event: string,
    handler: (e: Event, target: Element) => void
  ) => Effect.Effect<void, DomError>
}

export const DomServiceTag = Context.GenericTag<DomService>("DomService")
