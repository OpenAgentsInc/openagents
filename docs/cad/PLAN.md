# Autopilot CAD Plan (WGPUI-First, Demo-First)

Last updated: 2026-03-01

## 1) Objective

Add a serious CAD capability to OpenAgents with a first demo that proves:

1. Real parametric CAD workflows (not random mesh generation)
2. Fast local interaction
3. Programmable model edits
4. AI-assisted engineering decisions
5. Native desktop UX through WGPUI

This plan starts in `crates/cad/` and integrates with `apps/autopilot-desktop`.

## 2) Constraints From Current Repo

From `docs/MVP.md` and `docs/OWNERSHIP.md`:

- Keep product behavior in `apps/autopilot-desktop`.
- Keep reusable primitives in crates.
- `crates/wgpui*` remains product-agnostic.
- Favor small, verifiable increments over large speculative architecture.

Implication for CAD:

- `crates/cad` owns model/feature/analysis logic and deterministic transforms.
- `apps/autopilot-desktop` owns pane UX, orchestration, and Codex/chat integration.
- `crates/wgpui` only gets generic rendering/input primitives needed by any app.

## 3) What We Borrow From `vcad` (and What We Don’t)

Reviewed sources include:
- `/Users/christopherdavid/code/vcad/docs/features/zero-latency-parametric-editing.md`
- `/Users/christopherdavid/code/vcad/docs/features/compact-ir.md`
- `/Users/christopherdavid/code/vcad/docs/features/ai-co-designer.md`
- `/Users/christopherdavid/code/vcad/docs/features/unified-canvas.md`
- `/Users/christopherdavid/code/vcad/docs/features/headless-api.md`
- `/Users/christopherdavid/code/vcad/docs/features/ROADMAP.md`

Adopt now:

- Local-first parametric rebuild loop.
- Compact, token-efficient CAD command format for AI interaction.
- AI as structured editor of feature graph, not blob generator.
- Headless/programmatic operation model (good for agents and tests).
- Unified pane UX where design + AI + engineering metrics coexist.

Defer for later:

- Full physics-in-the-loop CAD.
- Full CRDT collaboration.
- Full manufacturing stack (CAM/PCB/advanced FEA/CFD).
- Browser-first WASM pipeline.

## 4) First Demo Definition (North Star)

Demo: **“Design a wall-mount aluminum rack for 2 Mac Studio units”**

Must show in ~20 seconds:

1. Prompt creates 4 parametric variants.
2. Variants are rotatable solids in a CAD pane.
3. Engineering overlay updates (weight, cost, deflection estimate, CoG).
4. AI command changes vent sizing / weight target and model updates immediately.
5. Manual dimension edit proves non-AI parametric control.
6. STEP export works.

## 5) Proposed Repo Shape

## `crates/cad` (new)

Single crate first; split later only when needed.

Planned modules:

- `document`: CAD document, units, metadata, revision IDs
- `params`: named parameters + expressions
- `feature_graph`: ordered feature nodes, dependencies, stable IDs
- `ops`: primitive ops + boolean-like composition API (MVP subset)
- `eval`: rebuild pipeline from feature graph -> solid representation
- `mesh`: tessellation output for viewport
- `analysis`: volume/mass/cost/deflection heuristics
- `export`: STEP writer facade (initially minimal primitive-compatible path)
- `compact`: compact CAD text format parser/serializer for AI/tool calls

## `apps/autopilot-desktop` integration

- Add `PaneKind::CadDemo` and a CAD pane renderer module.
- Add CAD pane state in `app_state.rs` and lane/event reducers.
- Add command-palette entry and singleton pane registration.
- Add chat-to-CAD intent bridge for Autopilot Chat.

## `crates/wgpui*` integration (generic only)

Phase-gated:

- Phase 1: use existing scene primitives for placeholder wireframe/projections.
- Phase 2: add generic mesh draw primitive and renderer pipeline.
- Add generic orbit/pan/zoom input helper if reusable.

No Autopilot-specific CAD business logic in `wgpui`.

## 6) Build Order

## Phase 0: Contracts + Skeleton (2-3 days)

Deliverables:

- Create `crates/cad` and add to workspace.
- Define core data contracts (`CadDocument`, `FeatureNode`, `CadAnalysis`).
- Add `CadDemo` pane shell and command registration.
- Add deterministic JSON snapshot tests for document serialization.

Exit criteria:

- CAD pane opens with no overflow/clipping regressions.
- Empty document state renders and round-trips through serialization.

## Phase 1: Deterministic Parametric Core (1 week)

Deliverables:

- Implement feature graph with stable IDs and rebuild order.
- Implement MVP feature ops:
  - base solids (box/cylinder)
  - transform
  - cutouts/holes
  - linear pattern
  - fillet marker API (can be visual/no-op initially)
- Implement named parameters and expression updates.
- Implement rebuild invalidation by changed parameter.

Exit criteria:

- Parameter updates rebuild same model deterministically.
- Rebuild time target: <80ms for rack-sized model on dev machine.

## Phase 2: Viewport Path on WGPUI (1 week)

Deliverables:

- Add mesh output from `crates/cad::eval`.
- Extend WGPUI renderer with generic mesh primitive OR keep temporary projected wireframe for first merge then add mesh pipeline immediately after.
- Add orbit/pan/zoom camera controls in pane.
- Add variant tiles (4-up view) for generated alternatives.

Exit criteria:

- Smooth interaction target: >=55 FPS while orbiting one variant.
- No pane overflow or draw outside bounds.

## Phase 3: Engineering Overlay (4-5 days)

Deliverables:

- Implement analysis heuristics in `crates/cad::analysis`:
  - volume
  - mass (material density table)
  - cost estimate (material + machining heuristic)
  - simple deflection estimate using beam approximation
  - center of gravity from volume decomposition
- Add sidebar overlay in CAD pane with live metrics.

Exit criteria:

- Any geometry or parameter update refreshes overlay in same cycle.
- Metric deltas are visible between variants.

## Phase 4: AI Contract + Chat Bridge (1 week)

Deliverables:

- Define `CadIntent` command schema (structured, not free text direct mutation).
- Build intent translator from Autopilot chat response -> `CadIntent`.
- Support intent classes:
  - `CreateRackSpec`
  - `GenerateVariants`
  - `AdjustParameter`
  - `OptimizeForWeight`
  - `ExportStep`
- Add clear error/fallback messaging when intent parse fails.

Exit criteria:

- Sending a request in Autopilot chat creates a new CAD session and document.
- Follow-up request mutates same active CAD session predictably.

## Phase 5: Mac Studio Rack Generator (4-5 days)

Deliverables:

- Hardcode/parameterize rack template generator:
  - two-bay dimensions
  - wall mount holes
  - vent arrays
  - wall thickness and rib options
- Variant engine produces 4 candidates optimized for:
  - lowest weight
  - lowest estimated cost
  - highest stiffness
  - airflow-biased

Exit criteria:

- 4 variants appear from one prompt in under 2s.
- User can rotate/select each and see independent metrics.

## Phase 6: Manual Proof + Export (3-4 days)

Deliverables:

- Clickable dimension labels in viewport overlay.
- Direct numeric edit (example 4mm -> 6mm) with immediate rebuild.
- STEP export action with deterministic file output + success/failure receipts.

Exit criteria:

- Manual edit visibly updates geometry and analysis.
- Exported STEP exists and can be re-imported by a basic checker.

## Phase 7: Demo Polish + Reliability (3-4 days)

Deliverables:

- Reduce UI noise, keep cinematic HUD style.
- Ensure pane clipping/scroll behavior is robust.
- Add canned demo command sequence and data reset.
- Add golden snapshot tests for:
  - rack baseline
  - each variant class
  - one follow-up parameter edit

Exit criteria:

- Full 20s script runs without stalls/flicker/state loss.

## 7) CAD-Autopilot Contract (MVP)

`CadIntent` (from chat/codex adapter):

- `CreateRackSpec { units, material, airflow, mount_type }`
- `GenerateVariants { count, objective_set }`
- `AdjustParameter { parameter, operation, value }`
- `CompareVariants { variant_ids }`
- `Export { format, variant_id }`

`CadEvent` (to UI/activity feed):

- `DocumentCreated`
- `VariantGenerated`
- `ParameterUpdated`
- `RebuildCompleted { duration_ms }`
- `AnalysisUpdated`
- `ExportCompleted` / `ExportFailed`

This keeps AI in a constrained mutation lane and prevents unstructured state corruption.

## 8) Suggested Data Model (MVP)

```text
CadSession
  session_id
  active_document_id
  variants: [CadDocument]
  selected_variant_id

CadDocument
  id
  revision
  units = mm
  material
  parameters: map<string, scalar>
  features: [FeatureNode]
  mesh_cache
  analysis_cache

FeatureNode
  id
  name
  op
  depends_on: [FeatureNodeId]
```

Stable IDs are required so AI and manual editing can target the same entities across rebuilds.

## 9) Milestone Acceptance Gates

Gate A (Kernel): deterministic rebuild + parameter editing.

Gate B (Viewport): interactive navigation + bounded rendering in pane.

Gate C (AI): structured intents modify model and produce predictable events.

Gate D (Engineering): overlay metrics update live and reflect tradeoffs.

Gate E (Demo): full script reproducible in one run on desktop.

## 10) Risks and Mitigations

Risk: 3D rendering in current WGPUI path may take longer than expected.
Mitigation: ship projected wireframe first, then mesh pipeline immediately after, without changing CAD data contracts.

Risk: boolean/topology robustness for non-trivial rack edits.
Mitigation: constrain generator ops to robust subset first and validate with snapshot/golden models.

Risk: AI emits ambiguous edit requests.
Mitigation: enforce `CadIntent` parser with strict schema and explicit clarification prompts.

Risk: scope explosion into full CAD platform.
Mitigation: lock scope to single hero workflow until demo passes.

## 11) Non-Goals for This First Wave

- Full general-purpose CAD parity with Fusion/Onshape
- Full FEA/CFD pipeline
- Full assembly/joint simulation
- Cloud collaboration and shared editing
- Marketplace of CAD plugins

## 12) Immediate Next Implementation Tasks

1. Create `crates/cad` with core contracts and tests.
2. Register `CadDemo` pane and render a bounded placeholder.
3. Implement document + feature graph + parametric rebuild MVP.
4. Add viewport rendering path through WGPUI.
5. Land rack generator + analysis overlay before AI bridge.

