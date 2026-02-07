/**
 * Effuse - Effect-Native UI Framework
 *
 * Public API exports
 */

// Template system
export { html, rawHtml, joinTemplates } from "./template/html.js"
export type { TemplateResult, TemplateValue } from "./template/types.js"
export { isTemplateResult } from "./template/types.js"
export { escapeHtml, escapeJsonForHtmlScript } from "./template/escape.js"
export { renderToString } from "./template/render.js"

// State management
export type { Eq, StateCell, StateCellView } from "./state/cell.js"
export { makeCell } from "./state/cell.js"

// Services
export {
  DomServiceTag,
  DomError,
  type DomService,
  type DomSwapMode,
} from "./services/dom.js"
export { StateServiceTag, type StateService } from "./services/state.js"

// Service implementations
export { DomServiceLive } from "./services/dom-live.js"
export { StateServiceLive } from "./services/state-live.js"

// Component system
export type { Component, ComponentContext } from "./component/types.js"
export { mountComponent, type MountedComponent } from "./component/mount.js"

// Layers
export { EffuseLive } from "./layers/live.js"

// Hypermedia actions (HTMX-inspired)
export type { EzAction, EzSwapMode } from "./ez/types.js"
export { EzRegistryTag, makeEzRegistry } from "./ez/registry.js"
export { mountEzRuntime, mountEzRuntimeWith } from "./ez/runtime.js"

// Signature-driven UI runtime
export * from "./ui/index.js"

// App route contract (server + client)
export type {
  CachePolicy,
  CookieMutation,
  DehydrateFragment,
  HydrationMode,
  NavigationSwapMode,
  ReceiptsFragment,
  RedirectStatus,
  Route,
  RouteContext,
  RouteHead,
  RouteId,
  RouteMatch,
  RouteOkHints,
} from "./app/route.js"
export { RouteOutcome } from "./app/route.js"
export type { RouteRun, RouteRunStage } from "./app/run.js"
export { runRoute } from "./app/run.js"

// Router (client-side navigation + loaders)
export * from "./router/index.js"

// Tool part rendering + bounded payload helpers
export * from "./toolParts/index.js"
