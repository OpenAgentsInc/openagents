/**
 * Hypermedia action types (HTMX-inspired).
 */

import type { Effect } from "effect"
import type { DomService, DomSwapMode } from "../services/dom.js"
import type { TemplateResult } from "../template/types.js"

export type EzSwapMode = DomSwapMode

export type EzAction = (args: {
  readonly event: Event
  readonly el: Element
  readonly params: Record<string, string>
  readonly dom: DomService
}) => Effect.Effect<TemplateResult | void, unknown>
