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
