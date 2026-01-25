# Effuse Signature-Driven UI Implementation Plan

## Summary

Implement the full Effuse UI plan in `apps/autopilot-desktop` by adding a
signature-driven UI runtime (catalog + UITree + renderer + validation +
actions), wiring DSRS signature metadata into a registry + Tauri IPC, and
replacing the unified-stream UI with a minimal canvas-based UI that streams
updates from Autopilot signatures. The result is a deterministic, validated UI
surface that can be incrementally enhanced by AI-generated layouts while
retaining safe fallbacks.

## Goals

- Provide a dynamic UI runtime for Effuse based on a flat `UITree` model.
- Constrain AI-generated layouts to an approved catalog of Effuse primitives.
- Stream UI updates from DSRS signatures as JSON patches.
- Replace current UI with a minimal canvas view that renders the UI tree.
- Consolidate signature definitions to the dsrs crate, removing local duplicates.
- Expose signature metadata over Tauri IPC with generated TS types.
- Keep rendering deterministic, validated, and safe.

## Non-goals (for this iteration)

- Full design polish or component parity with the deprecated React UI.
- Complex canvas interactions (drag, zoom, layout engines) beyond basic layout.
- Multi-user collaboration or remote sync of UI state.

## Current State (implemented)

- Effuse now includes a UITree runtime (catalog, patches, visibility, actions).
- Autopilot Desktop renders the signature-driven canvas via `AutopilotCanvasComponent`.
- Adjutant plan pipeline uses dsrs signatures; local duplicates are removed.
- DSRS signatures are exposed via Tauri IPC and are available to the UI.

## Target Architecture (overview)

### 1) UI Tree Model

- `UITree` with `root` and `elements` (flat map of `UIElement`).
- `UIElement` = `{ key, type, props, children?, visible? }`.
- `DynamicValue` supports literals or `{ path: "/data/path" }`.
- `JsonPatch` updates UI tree (`add`, `replace`, `remove`, `set`).

### 2) Catalog + Prompt Assembly

- Effuse catalog defines component props (Effect Schema), actions, validation
  functions, and a generated prompt scaffold for AI.
- Catalog is the authoritative vocabulary (hard whitelist).

### 3) Runtime

- `DataModel` stored in a `StateCell`, with `getByPath`/`setByPath`.
- Visibility evaluation (boolean + auth + logic expressions).
- Validation per field with built-in and custom validators.
- Action execution maps UI `Action` objects to Effuse EZ actions.
- Renderer walks the tree, resolves visibility + dynamic values, and renders
  Effuse `TemplateResult` for each node.
- Streaming patches update the tree and trigger re-render.

### 4) Signature Registry + IPC

- `SignatureRegistry` in dsrs (or autopilot-desktop backend) with explicit list
  of signatures (constructor fn -> `MetaSignature`).
- Tauri commands:
  - `list_dsrs_signatures`
  - `get_dsrs_signature(name)`
- TS types generated via ts-rs and validated in frontend with Effect Schema.

### 5) Signature-Driven UI Generation

- Deterministic form rendering from signature metadata (type -> component map).
- AI layout via `UiComposerSignature`:
  - Inputs: signature metadata + catalog prompt + layout constraints.
  - Outputs: `UITree` or patch stream.
  - Validation rejects tree outside catalog; fallback to deterministic form.

### 6) Autopilot Desktop UI

- Replace unified-stream UI with a minimal canvas UI. (done)
- Canvas renders the current UITree (initially empty).
- UI input lets user set working directory + start Autopilot session.
- Autopilot emits UI patches via Tauri events; frontend applies them.

## Detailed Implementation Plan

### Phase 0: Repo prep + backups

1) Move current UI to a backup location (no deletion):
   - `apps/autopilot-desktop/src/components/unified-stream`
     -> `apps/autopilot-desktop/.reference/legacy-unified-stream` (done)
   - Preserve any other UI modules needed for comparison.
2) Update `apps/autopilot-desktop/src/main.ts` to mount the new root component.

### Phase 1: Effuse UI runtime (frontend)

Create a new Effuse UI runtime in `apps/autopilot-desktop/src/effuse/ui`:

- `types.ts`:
  - `UITree`, `UIElement`, `DynamicValue`, `VisibilityCondition`, `JsonPatch`.
- `data.ts`:
  - `getByPath`, `setByPath`, `resolveDynamicValue`.
- `catalog.ts`:
  - `createCatalog`, `validateTree`, `generateCatalogPrompt`.
  - Use Effect `Schema` instead of Zod.
- `visibility.ts`:
  - `evaluateVisibility`, logic expression evaluation.
- `validation.ts`:
  - `ValidationCheck`, built-in functions, `runValidation`.
- `actions.ts`:
  - Action definitions + `resolveAction` + `executeAction`.
- `patch.ts`:
  - `applyPatch` (per json-render) + helpers.
- `renderer.ts`:
  - walk `UITree`, render per component registry.
  - resolve dynamic values in props.

Add unit tests (vitest) for:
  - `applyPatch`
  - visibility + logic evaluation
  - data path resolve/set
  - validation functions

### Phase 2: Effuse component catalog

Define initial Effuse primitives in
`apps/autopilot-desktop/src/components/ai-elements`:

- `canvas.ts`, `node.ts`, `edge.ts` adapted from deprecated React components.
- `text.ts`, `heading.ts`, `code-block.ts`, `list.ts`, `form.ts`, `button.ts`,
  `input.ts`, `select.ts`, `alert.ts` (minimal set to start).

Create a catalog in `apps/autopilot-desktop/src/components/catalog.ts`:

- Component props schemas (Effect Schema).
- Action definitions (e.g., `ui.submit`, `ui.start`, `ui.reset`).
- Validation functions (optional custom).

### Phase 3: Signature registry + IPC (backend + frontend)

Backend (Rust):

- Add a `SignatureRegistry` with explicit list of signatures.
  - Prefer in `crates/dsrs` for reuse.
- Create data structs:
  - `DsrsSignatureInfo { name, instruction, input_fields, output_fields }`
  - `DsrsSignatureList { signatures: Vec<DsrsSignatureInfo> }`
- Add Tauri commands in `crates/autopilot-desktop-backend`:
  - `list_dsrs_signatures`
  - `get_dsrs_signature`
- Update `contracts/ipc.rs` and regenerate TS types.

Frontend (TS):

- Add Effect Schemas for the new IPC types.
- Add a thin client module (see `apps/autopilot-desktop/src/ipc/unified.ts`).

### Phase 4: Convert local signatures to dsrs crate

- Move `TopicDecompositionSignature`, `ParallelExplorationSignature`,
  `PlanSynthesisSignature`, `ComplexityClassificationSignature`,
  `DeepPlanningSignature`, and `ResultValidationSignature` into
  `crates/dsrs/src/signatures/`.
- Ensure non-primitive types derive `schemars::JsonSchema` for schema emission.
- Update `crates/autopilot-desktop-backend` to import from dsrs crate.
- Remove local signature duplicates and clean up old modules.

### Phase 5: Signature-driven UI generation

- Implement deterministic form mapping:
  - Field type -> UI primitive mapping based on `type` and `schema`.
- Add `UiComposerSignature` in dsrs for AI layout.
- Add a validation gate:
  - Only apply UI from AI if it validates against catalog.
  - Fall back to deterministic layout if invalid.

### Phase 6: Autopilot UI wiring

Frontend:

- New root component (e.g., `AutopilotCanvasComponent`) that:
  - Reads `current directory` via Tauri.
  - Accepts workspace path input and start button.
  - Renders `UITree` via Effuse UI renderer.
  - Subscribes to Tauri UI events and applies patches.

Backend:

- Add UI event types to unified event stream (or a new `ui-event` stream):
  - `UiTreeReset`, `UiPatch`, `UiDataUpdate`.
- Emit UI events as the plan pipeline advances.
- Provide a “start autopilot” command that:
  - Sets working directory.
  - Runs DSRS signatures and emits UI patches.

### Phase 7: Docs + ADRs

- Add an ADR for:
  - `UITree` + patch contract and IPC surface.
  - Signature-driven UI generation contract.
- Update Effuse docs if needed to reflect the new runtime.

## Validation & Testing

- `bun run test` (vitest) for new Effuse UI runtime unit tests.
- `cargo test -p dsrs` for signature registry changes.
- `cargo check -p autopilot-desktop` for workspace sanity.
- Optional: `bun run typecheck` to validate generated TS types.

## Risks & Mitigations

- **Schema coverage**: complex types may lack schema output.
  - Mitigation: derive `JsonSchema` or manually supply schema fields.
- **Rendering performance**: full re-render on every patch.
  - Mitigation: cache nodes and add targeted swaps later.
- **Catalog drift**: AI emits invalid components.
  - Mitigation: strict validation + fallback to deterministic forms.
- **Contract churn**: UI tree schema changes could break IPC.
  - Mitigation: ADR + versioned schema if needed.

## File Map (expected touch points)

- `apps/autopilot-desktop/src/effuse/ui/*`
- `apps/autopilot-desktop/src/components/ai-elements/*`
- `apps/autopilot-desktop/src/components/catalog.ts`
- `apps/autopilot-desktop/src/main.ts`
- `crates/dsrs/src/signatures/*`
- `crates/autopilot-desktop-backend/src/agent/adjutant/*`
- `crates/autopilot-desktop-backend/src/contracts/ipc.rs`
- `apps/autopilot-desktop/src/contracts/tauri.ts`
- `apps/autopilot-desktop/src/gen/tauri-contracts.ts`
- `docs/adr/*` (new ADRs for UI tree + IPC contract)

## Delivery Checklist

- [x] UI tree runtime + tests in Effuse.
- [x] Effuse catalog + component registry.
- [x] Signature registry + Tauri IPC.
- [x] Local signature definitions migrated into dsrs crate.
- [x] UI streaming events emitted from backend.
- [x] Minimal canvas UI mounted in `main.ts`.
- [x] ADR(s) documenting the UI tree/IPC contract.
- [x] Tests/builds executed; no TODO placeholders in production paths.
