// Local shim to avoid requiring a published `@openagentsinc/effuse` package at build time.
//
// This repo often consumes internal packages via `file:` deps (npm) or direct source imports (bun).
// Using a relative import keeps `effuse-flow` usable from both paths without extra install steps.
export { html, rawHtml } from "../../effuse/src/index.js"
export type { TemplateResult } from "../../effuse/src/index.js"
