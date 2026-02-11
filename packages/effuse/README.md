# @openagentsinc/effuse

Effect-native UI framework: components, templates, and signature-driven UITree/patch runtime.

Use in browser or Node (e.g. autopilot web). No Tauri or desktop deps.

## API

- **Templates:** `html`, `rawHtml`, `escapeHtml`, `joinTemplates`
- **State:** `makeCell`, `StateCell`
- **Services:** `DomService`, `StateService` (tags + live implementations)
- **Components:** `mountComponent`, `Component`, `ComponentContext`
- **Layers:** `EffuseLive` (browser DOM + state)
- **EZ (hypermedia):** `mountEzRuntime`, `EzRegistryTag`, `makeEzRegistry`
- **UI (signature-driven):** `renderTree`, `applyPatch`, `createCatalog`, `createEmptyTree`, types for `UITree`, `UIElement`, `PatchOp`, etc.

## Usage (e.g. in apps/web)

```ts
import { Effect } from "effect"
import { html, mountComponent, EffuseLive } from "@openagentsinc/effuse"
import type { Component } from "@openagentsinc/effuse"
// or UI runtime:
import { renderTree, applyPatch, createCatalog } from "@openagentsinc/effuse/ui"
```

Run with a layer:

```ts
Effect.runPromise(
  yourProgram.pipe(Effect.provide(EffuseLive))
)
```

## Development Setup

Install deps, then patch TypeScript for Effect build-time diagnostics:

```bash
npm install
npm run effect:patch
```

The package tsconfig includes the `@effect/language-service` plugin for editor diagnostics.

## Logging

Runtime logging in Effect codepaths is routed through `Effect.log*` APIs
instead of ad-hoc `console.*` calls. This keeps logs composable with Effect
logger layers and consistent across browser/test runtimes.

## Tracing Span Names

Public router operations are instrumented with `Effect.fn` using deterministic
`effuse.router.<operation>` span names:

- `effuse.router.makeRouter`
- `effuse.router.start`
- `effuse.router.stop`
- `effuse.router.navigate`
- `effuse.router.prefetch`

## Test Style

Effuse tests use `@effect/vitest` APIs for Effect-native execution:

- Effect-aware tests use `it.live` via `tests/helpers/effectTest.ts` (`itLivePromise`).
- DOM layer provisioning is standardized with `withDom(...)` from the same helper.
