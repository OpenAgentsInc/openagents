# Plan: Signature-Driven Dynamic UI for Effuse

## Goals

- Dynamically generate UI from DSPy/dsrs signatures.
- Constrain AI layouts to an approved set of Effuse UI primitives.
- Convert Autopilot Desktop to use dsrs crate signatures (remove local duplicates).
- Keep rendering deterministic, validated, and safe.

---

## Research Summary

### json-render (reference implementation)

Key patterns to adopt:

- **Catalog-first constraints**: `createCatalog` defines components, actions, and validation functions using Zod schemas. The catalog is the authoritative "vocabulary" for AI output.
- **Flat UITree**: `UITree` has a `root` + `elements` map; each `UIElement` includes `key`, `type`, `props`, `children`, and `visible`.
- **Dynamic values**: Props can be literal values or `{ path: "/data/path" }` to bind to data.
- **Visibility logic**: `visible` supports boolean, auth gates, and boolean logic expressions.
- **Action runtime**: `Action` objects resolve params from data paths and support confirm/onSuccess/onError.
- **Validation**: `ValidationConfig` runs checks with built-in + custom functions.
- **Streaming**: JSON patch lines are streamed and applied to a `UITree` for progressive updates.
- **Prompt scaffolding**: `generateCatalogPrompt` summarizes available components/actions/visibility/validation for the LLM.

### Effuse (Autopilot Desktop UI)

Effuse gives us:

- **TemplateResult + html** for safe HTML generation.
- **StateCell** for reactive state and re-render loops.
- **DomService.swap** for targeted DOM updates (no VDOM).
- **EZ runtime** (`data-ez` attributes) for hypermedia actions and event wiring.

Current Effuse usage now includes a signature-driven UITree canvas (Effuse UI runtime + catalog); the legacy unified stream is archived for reference.

### dsrs (Signature metadata)

- `MetaSignature` exposes `instruction`, `input_fields`, and `output_fields` as JSON.
- `dsrs-macros` can auto-populate `schema` for complex types via `schemars`.
- Signatures are **not auto-registered**, so a registry is needed for discovery.

Autopilot Desktop currently defines **local signatures** in
`apps/autopilot-desktop/src-tauri/src/agent/adjutant/*` instead of reusing dsrs
crate signatures.

---

## Proposed Architecture

### 1) Data Model (UI Tree + Dynamic Values)

Adopt json-render's structure (portable + LLM-friendly):

- `UITree`: `{ root: string, elements: Record<string, UIElement> }`
- `UIElement`: `{ key, type, props, children?, visible? }`
- `DynamicValue`: literal or `{ path: "/data/path" }`
- `JsonPatch`: streaming updates with `add/remove/replace/set` operations

### 2) Effuse Catalog (UI primitives)

Create an Effuse catalog modeled after json-render:

- `ComponentDefinition`:
  - `props`: Effect Schema (or JSON schema)
  - `has_children`: boolean
  - `description`: string
- `ActionDefinition`:
  - name + param schema
  - description for LLM prompt
- `ValidationFunction` list (built-in + custom)

Add `generateCatalogPrompt` for dsrs prompt assembly.

Initial primitive set should mirror current UI needs (layout, text, inputs,
buttons, alerts, list/table, code block, form controls).

### 3) Effuse UI Runtime

Implement a lightweight runtime that mirrors json-render behavior:

- **Data model**: `StateCell<DataModel>` with `getByPath`/`setByPath`.
- **Visibility**: boolean logic + auth gates.
- **Validation**: field-level checks, show errors, configurable triggers.
- **Actions**: integrate with EZ registry:
  - map `Action` to `data-ez` attributes
  - resolve params using dynamic values
  - support confirmation + success/error handlers
- **Renderer**: walk `UITree`, evaluate visibility, render components, and
  pass `children` HTML to registries.
- **Streaming**: apply JSON patch lines to tree state, re-render subtree
  via `DomService.swap`.

### 4) Signature-Driven UI Generation

Two routes, both supported:

1. **Deterministic form rendering**  
   Map signature fields to UI primitives based on type/schema:
   - `String` -> TextField
   - `bool` -> Toggle
   - enums -> Select
   - JSON schema -> structured editor or JSON editor

2. **AI layout (validated)**  
   Add a dsrs signature (e.g., `UiComposerSignature`) that takes:
   - signature metadata
   - component catalog prompt
   - layout constraints
   Outputs `UITree` or JSON patch stream.

Validation must reject any tree that uses components outside the catalog.
Fallback to deterministic form when invalid.

### 5) Signature Registry + Tauri IPC

Create a Rust registry (manual, explicit list of signatures):

- `SignatureRegistry` returns:
  - name
  - instruction
  - input/output fields (including schema)

Expose via Tauri command, with `ts-rs` DTOs:

- `list_dsrs_signatures`
- `get_dsrs_signature(name)`

Frontend uses generated types + Effect Schema for decoding.

### 6) Convert Autopilot Desktop to dsrs crate signatures

Unify signatures across the repo:

- Remove local signature structs in `apps/autopilot-desktop/src-tauri/src/agent/adjutant/*`.
- Use dsrs crate signatures or move those signatures into `crates/dsrs`.
- Align `PlanModePipeline` with dsrs/adjutant pipelines to avoid drift.
- Ensure all signatures expose schemas to support UI generation.

---

## Implementation Phases

1. **Foundation**
   - Add `UITree` + `UIElement` + `JsonPatch` types in Effuse.
   - Implement patch application and tests.

2. **Runtime + Renderer**
   - Data/visibility/validation/action services.
   - Effuse renderer that produces `TemplateResult`.

3. **Signature Catalog + IPC**
   - Rust registry + Tauri commands.
   - Generated TS types + Effect Schema for decoding.

4. **Deterministic Signature UI**
   - Signature-to-form mapping.
   - Show signature inputs/outputs in UI.

5. **AI Layout**
   - Add `UiComposerSignature` in dsrs.
   - Validate output; fallback when invalid.
   - Optional streaming via JSON patch.

6. **Autopilot Desktop dsrs conversion**
   - Replace local signature definitions with dsrs crate signatures.
   - Consolidate planning pipeline and LM wiring.

---

## Testing and Verification

- Unit tests for patch application and visibility logic.
- Integration tests for action execution and validation.
- Tauri IPC schema decode tests.
- Render tests for tree -> TemplateResult output.

---

## Open Questions / Risks

- **Schema coverage**: manual dsrs signatures lack `schema`; either migrate to
  `dsrs-macros` or add schema fields manually.
- **Performance**: re-rendering on every patch vs targeted swaps.
- **Security**: strict component whitelist + param validation.
- **Ownership**: where catalog and renderer live (Effuse core vs app-local).

---

## Sources Reviewed

json-render:
- `packages/core/src/types.ts`
- `packages/core/src/catalog.ts`
- `packages/core/src/visibility.ts`
- `packages/core/src/actions.ts`
- `packages/core/src/validation.ts`
- `packages/react/src/renderer.tsx`
- `packages/react/src/hooks.ts`
- `examples/dashboard/lib/catalog.ts`
- `examples/dashboard/lib/codegen/*`

openagents / Effuse / dsrs:
- `apps/autopilot-desktop/src/effuse/*`
- `apps/autopilot-desktop/docs/effuse/*`
- `apps/autopilot-desktop/src/components/*`
- `apps/autopilot-desktop/src-tauri/src/agent/adjutant/*`
- `crates/dsrs/src/signatures/*`
- `crates/dsrs-macros/src/lib.rs`
