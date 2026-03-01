# Autopilot CAD Plan (WGPUI-First, Demo-First)

Last updated: 2026-03-01

## 1) Objective

Add serious CAD capability to OpenAgents with a first demo that proves:

1. Real parametric CAD workflows (not random mesh generation)
2. Fast local interaction
3. Programmable model edits
4. AI-assisted engineering decisions
5. Native desktop UX through WGPUI

This plan starts in `crates/cad/` and integrates with `apps/autopilot-desktop`.

## 2) Definition of Success

Demo success is not replacement success.

- Demo success proves: deterministic kernel behavior, pro UX feel, structured AI edits, and live engineering overlays.
- Replacement success requires broader part-modeling workflows (Wave 2+ below).

### Drop-In Replacement Scope Ladder

- Wave 1 (Demo / Alpha):
  - Template-driven parametric solids
  - Fast edits + variant generation
  - STEP export
  - Pro UX baseline (camera, hotkeys, basic 3D mouse mapping)
- Wave 2 (Part Modeling MVP):
  - Sketcher + constraints
  - Extrude / cut / revolve
  - Fillet / chamfer / shell
  - STEP import + export
- Wave 3 (Pro Workflows):
  - Drawings
  - Assemblies/configurations
  - Section analysis and measurement suite
  - Materials/properties depth

## 3) Constraints From Current Repo

From `docs/MVP.md` and `docs/OWNERSHIP.md`:

- Product behavior stays in `apps/autopilot-desktop`.
- Reusable primitives stay in crates.
- `crates/wgpui*` remains product-agnostic.
- Prefer small, verifiable increments over platform expansion.

Implication for CAD:

- `crates/cad` owns model/feature/query/eval/analysis/export logic.
- `apps/autopilot-desktop` owns pane UX, orchestration, and Codex chat integration.
- `crates/wgpui` only gets generic rendering/input primitives needed by any app.

## 3.1) CAD Code Health Runbook

All CAD implementation work in this plan is gated by the canonical runbook:

- [`docs/cad/CAD_CODE_HEALTH.md`](/Users/christopherdavid/code/openagents/docs/cad/CAD_CODE_HEALTH.md)

Use that runbook for formatting, CAD clippy policy lanes, release gates, and strict production hardening checks.

## 4) What We Borrow From `vcad` (and What We Don’t)

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
- AI as structured editor of a feature graph, not blob generation.
- Headless/programmatic operation model for agents and tests.
- Unified pane UX where geometry + analysis + chat coexist.

Defer for later:

- Full physics-in-the-loop CAD.
- Full CRDT collaboration.
- Full manufacturing stack (CAM/PCB/advanced FEA/CFD).
- Browser-first WASM pipeline.

## 5) First Demo Definition (North Star)

Demo: "Design a wall-mount aluminum rack for 2 Mac Studio units"

Must show in ~20 seconds:

1. Prompt creates 4 parametric variants.
2. Variants are rotatable solids in a CAD pane.
3. Engineering overlay updates (weight, cost, deflection estimate, CoG).
4. AI command changes vent sizing / weight target and model updates immediately.
5. Manual dimension edit proves non-AI parametric control.
6. STEP export works.

STEP policy for this demo:

- Required: STEP export.
- Optional: STEP import.

Wave 2 requirement:

- STEP import is required for credibility as a replacement workflow.

## 6) Proposed Repo Shape

## `crates/cad` (new)

Single crate first; split later only when needed.

Planned modules:

- `document`: CAD document, units, metadata, revision IDs
- `params`: named parameters + expressions
- `feature_graph`: ordered feature nodes, dependencies, stable IDs
- `features`: primitive and parametric feature ops
- `eval`: rebuild pipeline from feature graph to solids/mesh-ready outputs
- `mesh`: tessellation output for viewport
- `query`: ray hit test, nearest edge/face/body queries
- `selection`: selection sets, filters, persistent selection state IDs
- `analysis`: volume/mass/cost/deflection/CoG heuristics
- `warnings`: validity checks and structured warning receipts
- `history`: undo/redo command stack and timeline entries
- `export`: STEP writer facade
- `compact`: compact CAD text format parser/serializer for AI/tool calls

## `apps/autopilot-desktop` integration

- Add `PaneKind::CadDemo` and a CAD pane renderer module.
- Add CAD pane state in `app_state.rs` and reducers/events.
- Add command-palette entry and singleton pane registration.
- Add chat-to-CAD intent bridge for Autopilot Chat.

## `crates/wgpui*` integration (generic only)

Phase-gated:

- Phase 1: existing scene primitives for placeholder projections.
- Phase 2: generic mesh draw primitive and renderer pipeline.
- Add reusable orbit/pan/zoom helper if generic enough.

No Autopilot-specific CAD business logic in `wgpui`.

## 7) Core Architectural Invariants

### Geometry Representation Strategy (Wave 1)

Options considered:

- A: Depend on/vend a constrained subset of VCAD kernel crates.
- B: Use OpenCascade bindings directly.
- C: Build a minimal in-house B-Rep subset for generator-only solids.
- D: Use CSG tree now and exact solid eval later.

Wave 1 chosen direction:

- Choose A as the default implementation path for demo velocity and STEP credibility.
- Keep B as fallback if A fails predefined spike criteria.
- C and D are not selected for Wave 1 due to higher tolerance/robustness risk.
- Decision record: `docs/cad/decisions/0001-kernel-strategy.md`

Decision criteria to keep objective:

- Boolean reliability on rack geometry
- STEP export quality and checker compatibility
- Integration complexity with Rust desktop runtime
- Performance under demo budgets
- Licensing and maintenance burden

### Units, Tolerances, and Robustness Policy

- Canonical units: millimeters (`mm`) for model inputs and displayed dimensions.
- Internal tolerance baseline: `1e-6 m` equivalent (`1e-3 mm`) unless op-specific tighter bounds are required.
- Modeling policy: tolerant modeling with explicit tolerance-aware operations.
- Boolean rule: every boolean returns either valid solid or structured failure; silent corruption is disallowed.
- Failure receipt payload must include: operation type, operand semantic refs, tolerance used, and classification.
- policy document: `docs/cad/UNITS_TOLERANCE_POLICY.md`
- error model document: `docs/cad/CAD_ERROR_MODEL.md`
- payload contracts document: `docs/cad/CAD_CONTRACTS.md`
- core analysis behavior document: `docs/cad/CAD_ANALYSIS.md`
- cost heuristic model document: `docs/cad/CAD_COST_HEURISTIC.md`
- deflection heuristic model document: `docs/cad/CAD_DEFLECTION_HEURISTIC.md`
- engineering overlay panel document: `docs/cad/CAD_ENGINEERING_OVERLAY.md`
- semantic ref registry document: `docs/cad/CAD_SEMANTIC_REFS.md`
- app pane-state document: `docs/cad/CAD_PANE_STATE.md`
- input reducer scaffold document: `docs/cad/CAD_INPUT_SCAFFOLD.md`
- params store document: `docs/cad/CAD_PARAMS.md`
- feature ops document: `docs/cad/CAD_FEATURE_OPS.md`
- finishing operations document: `docs/cad/CAD_FINISHING_OPS.md`
- sketch constraint solver document: `docs/cad/CAD_SKETCH_CONSTRAINTS.md`
- sketch-to-feature operations document: `docs/cad/CAD_SKETCH_FEATURE_OPS.md`
- STEP import pipeline document: `docs/cad/CAD_STEP_IMPORT.md`
- rack template generator document: `docs/cad/CAD_RACK_TEMPLATE.md`
- CAD intent schema document: `docs/cad/CAD_INTENTS.md`
- CAD intent dispatch document: `docs/cad/CAD_INTENT_DISPATCH.md`
- CAD chat adapter document: `docs/cad/CAD_CHAT_ADAPTER.md`
- CAD chat session lifecycle document: `docs/cad/CAD_CHAT_SESSION_LIFECYCLE.md`

### Eval, Caching, and Concurrency

- `eval()` is pure and deterministic.
- Cache key shape: `(document_revision, feature_node_id, params_hash)`.
- Background rebuild is allowed; UI shows last-good mesh until next mesh is ready.
- Every rebuild emits a receipt with duration and cache hit-rate.

### Stable References MVP

- Feature outputs publish semantic refs (example: `rack_outer_face`, `mount_hole_pattern`, `vent_face_set`).
- Downstream features reference semantic names, not positional names like `Face12`.
- Generator-built geometry uses semantic naming immediately.
- General topological naming for arbitrary edits is deferred to Wave 2+.

### Model Validity and Failure Surfacing

Minimum validity checks for Wave 1:

- manifold check
- self-intersection check
- zero-thickness face check
- tiny sliver-face warning
- fillet/chamfer failure classification

Surface model health via:

- warnings panel in CAD pane
- viewport overlays for selected warnings
- structured warning events in activity feed

### Undo/Redo and Feature Timeline

- Maintain a deterministic undo/redo stack from typed CAD commands.
- Show a minimal visible feature timeline listing ordered feature nodes.
- Selecting a timeline entry reveals its parameter payload.
- Undo/redo must preserve stable IDs and semantic refs.

### Persistence and Deterministic File Format

Use `.apcad` as the CAD document format:

- versioned schema
- deterministic ordering for diffability
- stable IDs retained across saves
- analysis cache optional and non-authoritative
- storable in standard Autopilot workspace paths
- spec document: `docs/cad/APCAD_FORMAT.md`
- core schema document: `docs/cad/CAD_DOCUMENT_SCHEMA.md`

Golden diff expectation:

- meaningful document diffs across param/feature changes, not random ordering noise.

### Rendering Policy: CAD Visual Requirements

Gate-level rendering requirements:

- crisp edge overlay (feature edges and silhouette)
- stable face shading normals
- selection outline
- hidden-line style option for wireframe/section contexts

### Input Mapping Policy

3D mouse minimum profile:

- translate mode and rotate mode
- speed scalar control
- axis lock toggles

Hotkey baseline:

- view snaps
- sketch mode entry (Wave 2 prep)
- measure tool
- section tool
- export

Discoverability requirement:

- all major hotkey actions mirrored in command palette.

### Performance Budgets

- rebuild budget: <80ms for rack-sized model
- mesh generation budget: <30ms for rack-sized model
- selection/hit-test budget: <5ms/frame target
- interactive viewport target: >=55 FPS
- memory budget target: <800MB process RSS during demo flow

### Headless CAD Script Harness

Add a headless CAD script test format:

- apply sequence of `CadIntent` commands
- assert final geometry hash, analysis snapshot, and warning set
- run in CI and demo reliability tests

### STEP Validation Strategy

Checker for Wave 1:

- use OpenCascade-based validation utility in test harness.

Validation rules:

- exported STEP re-imports without fatal topology errors
- bounding box and volume match expected values within tolerance
- deterministic writer ordering preserved across repeated exports

### External Dependency License and Security Posture

If vendoring/adopting external kernel crates:

- document license and attribution in-repo
- define upstream sync/update cadence
- declare fork-vs-vendor posture explicitly
- run dependency security checks on imported code path
- policy document: `docs/cad/DEPENDENCY_POSTURE.md`

## 8) Build Order

## Phase 0: Contracts + Decisions + Skeleton

Deliverables:

- Create `crates/cad` and add to workspace.
- Execute kernel strategy decision issue with a short spike plan.
- Lock units/tolerance/robustness policy.
- Define core contracts (`CadDocument`, `FeatureNode`, `CadAnalysis`, `CadSelection`, `CadWarning`).
- Add `CadDemo` pane shell and command registration.
- Add deterministic JSON snapshot tests for document serialization.

Exit criteria:

- Kernel representation direction selected and documented.
- CAD pane opens with no overflow/clipping regressions.
- Empty document state renders and round-trips.

## Phase 1: Deterministic Parametric Core + Validity + History

Deliverables:

- Implement feature graph with stable IDs and rebuild order.
- Implement MVP feature set:
  - base solids (box/cylinder)
  - transforms
  - cutouts/holes
  - linear pattern
  - fillet marker API (visual/no-op acceptable initially)
- Implement named parameters and expression updates.
- Implement rebuild invalidation on parameter changes.
- Implement model validity checks and warning classifications.
- Implement undo/redo command stack and minimal timeline.

Exit criteria:

- Parameter updates rebuild deterministically.
- Validity checks emit deterministic warnings.
- Undo/redo is functional and timeline is visible.
- Target rebuild latency: <80ms for rack-sized model on dev machine.

## Phase 2: Viewport + Pro UX Foundation

Deliverables:

- Add mesh output from `crates/cad::eval`.
- Extend WGPUI with generic mesh primitive and renderer path.
- Add crisp edge and silhouette overlay.
- Add selection outline + stable normal shading behavior.
- Add hidden-line style mode for wireframe/section contexts.
- Add orbit/pan/zoom + view cube + ortho/perspective toggle.
- Add render modes: shaded, shaded+edges, wireframe.
- Add snapping toggles: grid/origin/endpoint/midpoint.
- Add hotkey map with small customizable defaults and command-palette parity.
- Add minimal 3D mouse input mapping.
- Add CAD-pane contextual menu framework.
- Add 4-up variant view support.

Exit criteria:

- >=55 FPS while orbiting a single variant.
- Mesh generation <30ms for rack-sized model.
- Selection/hit-test budget meets <5ms/frame target.
- No pane overflow or out-of-bounds rendering.
- Required UX parity primitives listed above are all functional.

## Phase 3: Selection + Inspect + Engineering Overlay

Deliverables:

- Hover highlight and click select for body/face/edge.
- `query` and `selection` integration into pane input/render loop.
- Properties panel updates on selection:
  - body: volume, surface area, mass, CoG, bounding box
  - face: area + normal
  - edge: length + type
- Basic section analysis and measure tools (MVP implementations).
- Warning panel + marker overlays in viewport.
- Engineering sidebar overlay with live updates:
  - material
  - mass
  - cost estimate
  - deflection estimate

Exit criteria:

- Selection is stable through routine parameter updates.
- Geometry updates, warnings, and analysis updates land in the same visible cycle.

## Phase 4: Rack Generator + Variant Engine

Deliverables:

- Build deterministic rack template generator:
  - two-bay geometry
  - wall mount holes
  - vent arrays
  - wall thickness and rib parameters
- Produce 4 candidate variants optimized for:
  - lowest weight
  - lowest estimated cost
  - highest stiffness
  - airflow bias

Exit criteria:

- 4 variants appear from one request in <2s.
- Variants are independently selectable and inspectable.

## Phase 5: AI Contract + Chat Bridge

Deliverables:

- Define strict `CadIntent` schema.
- Add translator from Autopilot chat response to `CadIntent`.
- Execute intents only through typed CAD command handlers.
- Add explicit fallback messaging when intent parsing fails.

Exit criteria:

- Chat can create a new CAD session and mutate active CAD session reliably.
- AI path does not bypass typed CAD command schema.

## Phase 6: Manual Proof + STEP Export

Deliverables:

- Clickable dimension labels in viewport overlay.
- Direct numeric dimension edits (example: `4mm -> 6mm`) with immediate rebuild.
- STEP export action with receipts and deterministic output ordering.

Exit criteria:

- Manual edits visibly update geometry and analysis.
- Exported STEP passes checker validation within configured tolerance.

## Phase 7: Demo Polish + Reliability

Deliverables:

- Remove noise, keep focused HUD style.
- Harden pane clipping and scroll behavior.
- Add CAD script harness for headless end-to-end intent tests.
- Add canned demo reset/state bootstrap.
- Add golden snapshot tests for:
  - rack baseline
  - each variant objective
  - one follow-up parameter edit path

Exit criteria:

- Full 20s demo script runs end-to-end without flicker/state loss/stalls.
- Performance and memory budgets are met.

## 9) CAD-Autopilot Contract (MVP)

`CadIntent` (from chat/codex adapter):

- `CreateRackSpec { units, material, airflow, mount_type }`
- `GenerateVariants { count, objective_set }`
- `SetObjective { objective }`
- `AdjustParameter { parameter, operation, value }`
- `SetMaterial { material_id }`
- `AddVentPattern { pattern, size, density }`
- `Select { selector }`
- `CompareVariants { variant_ids }`
- `Export { format, variant_id }`

`CadEvent` (to UI/activity feed):

- `DocumentCreated`
- `VariantGenerated`
- `SelectionChanged`
- `WarningRaised`
- `ParameterUpdated`
- `RebuildCompleted { duration_ms, cache_hit_rate }`
- `AnalysisUpdated`
- `ExportCompleted`
- `ExportFailed`

AI mutation rule:

- AI cannot invent new operations.
- AI must choose from schema-defined intents and document-declared parameters/features.

## 10) Milestone Acceptance Gates

Gate A (Kernel + Validity + History):

- Deterministic rebuilds
- Parameter editing
- Rebuild receipts available
- Units/tolerance policy implemented
- Model validity checks and warning classifications enabled
- Undo/redo stack and visible timeline available

Gate B (Viewport + Pro UX):

- Orbit/pan/zoom + view cube + ortho/perspective
- Hotkeys with minimal customization support + command palette parity
- Minimal 3D mouse support
- Contextual menu framework in CAD pane
- Shaded, shaded+edges, wireframe modes
- Crisp edges and silhouette overlay
- Selection outline and stable shading normals
- Hidden-line style mode for section/wireframe usage
- Snapping toggles (grid/origin/endpoint/midpoint)
- Selection + inspect panel
- No out-of-bounds pane rendering

Gate C (Generator):

- Rack generator and 4-variant engine deterministic and repeatable

Gate D (AI):

- Structured intents mutate CAD state predictably
- No free-text direct mutation path

Gate E (Engineering + Demo):

- Overlay metrics update live and reflect real tradeoffs
- Full 20s script reproducible in one run on desktop
- Rebuild/mesh/hit-test/memory budgets satisfied

## 11) Risks and Mitigations

Risk: mesh viewport path in WGPUI slips.
Mitigation:

- Temporary projected wireframe fallback is allowed only as a short-lived bridge.
- Fallback must ship with:
  - defined mesh primitive API
  - target date/PR to remove fallback

Risk: boolean/topology instability for complex edits.
Mitigation:

- Constrain early generator to robust feature subset.
- Validate with model validity checks and golden geometry snapshots.

Risk: ambiguous AI edits.
Mitigation:

- Strict `CadIntent` parsing and schema enforcement.
- Explicit rejection + clarification prompts.

Risk: scope drift into full CAD platform before demo lock.
Mitigation:

- Demo gate completion required before Wave 2 expansion.

## 12) CAD Quality Bar (Non-Negotiable)

- No right-click-only critical discovery paths.
- Primary dimensions use typed input, not sliders.
- Constraints and snaps must cover common cases reliably.
- Snapping is visible and toggleable.
- Labels and measurement context remain visible by default.
- Interactions should feel like CAD tooling, not generic 3D scene controls.

## 13) Non-Goals for This First Wave

- Full Fusion/Onshape parity
- Full FEA/CFD
- Full assembly/joint simulation stack
- Real-time collaboration
- CAD plugin marketplace

## 14) Immediate Next Implementation Tasks

1. Open and resolve kernel representation decision issue with a short spike.
2. Create `crates/cad` contracts + `.apcad` serialization scaffolding.
3. Implement deterministic rebuild path plus validity checks and warnings panel stub.
4. Implement undo/redo stack and minimal feature timeline.
5. Add mesh viewport path and Gate B UX requirements.
6. Land rack generator/variants before AI chat bridge.

## 15) GitHub Issue Backlog (Sequential)

This ordered issue list implements the plan end-to-end. Each issue should include phase label, owner, dependencies, and acceptance criteria.

1. Title: Create `crates/cad` crate and workspace wiring
   Description: Add new `crates/cad` crate, register in workspace `Cargo.toml`, and ensure clean build. DoD: workspace builds without introducing app-layer dependencies into crate internals.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

2. Title: Kernel strategy decision (A/B/C/D) with spike plan
   Description: Open decision issue with options A-D, decision criteria (robustness, STEP quality, perf, integration, license), and run a short spike. DoD: signed decision record with fallback trigger conditions.
   Details: Include a weighted scorecard across robustness, STEP fidelity, integration cost, runtime performance, and maintenance risk. The spike must run the same rack test corpus across candidate kernels and attach artifacts: boolean failure log, STEP outputs, import/checker results, and peak memory. Issue must conclude with explicit chosen path, fallback path, and kill criteria for switching.

3. Title: Implement kernel adapter boundary in `crates/cad`
   Description: Create `cad::kernel` adapter traits so Wave 1 engine choice is isolated from higher layers. DoD: primitives/eval call engine through adapter only.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

4. Title: Document external dependency license and security posture
   Description: Record chosen engine license, attribution, update cadence, fork-vs-vendor policy, and security scanning requirements. DoD: doc committed and linked from CAD plan.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

5. Title: Lock units/tolerances/robustness policy in code and docs
   Description: Implement mm canonical units, tolerance constants, and tolerant-modeling defaults. DoD: policy constants referenced by eval/boolean paths and documented.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

6. Title: Define `.apcad` file format and deterministic ordering rules
   Description: Specify versioned schema, stable ordering, stable IDs, and optional analysis cache. DoD: format spec + deterministic serialization tests.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

7. Title: Define `CadDocument` core schema and serialization
   Description: Implement versioned document schema with units, revision, metadata, and deterministic serde round-trip tests. DoD: goldens for empty/minimal docs.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

8. Title: Define `FeatureNode`/`FeatureGraph` contracts with stable IDs
   Description: Add typed feature graph structs with explicit dependency edges and stable node IDs for reproducible rebuilds. DoD: deterministic topo-order tests.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

9. Title: Add CAD domain error model (`CadError`) and result conventions
   Description: Define crate-wide error taxonomy for parse/eval/query/export failures with actionable, non-panicking messages. DoD: errors map to structured UI events.
   Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

10. Title: Add `CadAnalysis`, `CadSelection`, and `CadWarning` contracts
    Description: Introduce typed analysis snapshots, selection state, and warning receipts used by pane/UI/event system. DoD: schema tests for each payload.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

11. Title: Add `CadDemo` pane kind and command palette registration
    Description: Register pane in `app_state`, `pane_registry`, and command palette as singleton. DoD: pane opens/focuses predictably via palette command.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

12. Title: Add bounded `CadDemo` placeholder renderer
    Description: Implement initial CAD pane rendering with strict clipping and no overflow regressions. DoD: pane system tests pass for bounds/clipping.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

13. Title: Add CAD pane state container in `app_state`
    Description: Add `CadDemoPaneState` with document/session/variant placeholders and deterministic defaults. DoD: state bootstraps and survives app refresh.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

14. Title: Add CAD reducer/action scaffolding in input layer
    Description: Add lane-neutral action handlers for CAD pane commands and state transitions. DoD: no-op command loop and state transitions tested.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

15. Title: Add serialization snapshot tests for empty/minimal CAD docs
    Description: Commit stable snapshot fixtures for empty docs and single-feature docs to lock schema determinism. DoD: CI snapshot tests green.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

16. Title: Implement parameter store (`params`) with typed scalar units
    Description: Add named parameter map, validation, and unit-aware scalar handling for mm-centric workflows. DoD: invalid unit/value cases rejected.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

17. Title: Implement expression evaluation for parameter dependencies
    Description: Support deterministic expressions (arithmetic + parameter refs) with cycle detection. DoD: expression graph tests include cycle and divide-by-zero failure cases.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

18. Title: Implement feature op: primitive box
    Description: Add box feature op with parameter binding and deterministic IDs. DoD: golden geometry hash for representative boxes.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

19. Title: Implement feature op: primitive cylinder
    Description: Add cylinder feature op with radius/height parameterization. DoD: deterministic geometry and tolerance edge-case tests.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

20. Title: Implement feature op: transform
    Description: Add translation/rotation/scale feature with robust validation. DoD: transform composition tests and deterministic output ordering.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

21. Title: Implement feature op: cut/hole
    Description: Add subtraction-style cutout feature for rack holes/vents. DoD: valid-result-or-structured-failure behavior tested.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

22. Title: Implement feature op: linear pattern
    Description: Add deterministic repeated-feature generation for vents/holes. DoD: pattern index stability across rebuilds.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

23. Title: Implement feature marker: fillet placeholder
    Description: Add fillet/chamfer marker contract (visual/no-op acceptable) to preserve graph compatibility. DoD: placeholder survives save/load/rebuild.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

24. Title: Implement deterministic rebuild scheduler in `eval`
    Description: Evaluate feature graph in stable topological order and return reproducible geometry outputs. DoD: repeated rebuild hash equality tests.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

25. Title: Implement parameter-change invalidation logic
    Description: Recompute only affected downstream features after parameter edits. DoD: dependency pruning tests with unchanged upstream hashes.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

26. Title: Implement eval cache keys and cache store
    Description: Add cache keyed by `(document_revision, feature_node_id, params_hash)` with hit/miss accounting. DoD: cache correctness + eviction tests.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

27. Title: Add rebuild receipts (`duration_ms`, cache stats)
    Description: Emit structured rebuild receipts on each eval cycle for UI telemetry and debugging. DoD: receipts flow to pane/event stream.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

28. Title: Add background rebuild worker with last-good mesh strategy
    Description: Run rebuilds off UI thread; render last-good mesh until commit. DoD: stress test confirms no UI freeze or transient null mesh.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

29. Title: Implement model validity checks core
   Description: Add manifold/self-intersection/zero-thickness/sliver checks and fillet failure classification. DoD: check suite returns deterministic warnings.
   Details: Define stable warning codes (`CAD-WARN-*`) with severity (`info`, `warning`, `critical`) and semantic references to implicated entities/features. Require deterministic warning ordering and include fixtures that intentionally trigger each warning class. Add structured receipts so warning rows can deep-link to geometry in the pane.

30. Title: Add warnings panel and viewport warning markers
   Description: Surface validity warnings in CAD pane and overlay problem locations. DoD: clicking warning focuses implicated geometry.
   Details: Add warning filters by severity/code and persist panel state per session. Marker rendering must support hover state, click-to-focus, and fallback when exact geometry reference is unavailable (feature-level focus). Include UX tests proving no overlay overflow and no stale markers after rebuild.

31. Title: Implement undo/redo command stack in `crates/cad::history`
   Description: Capture typed CAD commands with deterministic reversible state transitions. DoD: multi-step undo/redo preserves IDs/semantic refs.
   Details: Define command granularity rules (single param edit = single history step, grouped gestures = coalesced step) and max stack policy. Include replay tests that run command sequences forward/backward and assert identical geometry hashes, warnings, and analysis snapshots. History must be session-scoped and reset-safe.

32. Title: Add minimal feature timeline UI bound to command history
   Description: Render ordered feature timeline and parameter inspector for selected node. DoD: selecting a timeline row highlights corresponding feature.
   Details: Timeline rows must show feature name, op type, status badge (ok/warn/fail), and last edit provenance (manual vs AI). Include keyboard navigation and auto-scroll to active feature on selection. Add regression tests for clipping and extremely long timelines in small pane sizes.

33. Title: Add mesh representation contract in `crates/cad::mesh`
    Description: Define renderer-facing mesh payload (vertices/indices/normals/material slots/edges). DoD: binary-compatible payload tests.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

34. Title: Add tessellation path from eval geometry to mesh output
    Description: Convert deterministic eval outputs into pane-renderable mesh payload. DoD: tessellation goldens for rack primitives.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

35. Title: Add generic mesh draw primitive contract in WGPUI core/render
    Description: Extend WGPUI with product-agnostic mesh scene primitive. DoD: primitive can be used by non-CAD demo surface.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

36. Title: Add mesh render pass and shader pipeline in WGPUI render
    Description: Implement minimal lit CAD mesh rendering pipeline compatible with existing renderer layers. DoD: render pass tests and fallback behavior documented.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

37. Title: Integrate CAD mesh rendering into `CadDemo` pane
    Description: Hook pane mesh output to WGPUI mesh primitive with bounded viewport. DoD: pane render remains clipped and layer-stable.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

38. Title: Add crisp edge/silhouette overlay pipeline
    Description: Render feature edges and silhouettes for CAD readability. DoD: edge visibility stable across zoom and projection modes.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

39. Title: Add selection outline and stable shading normal handling
    Description: Ensure selected entities get clear outline and shading remains stable frame-to-frame. DoD: no flicker in rotating/selected scenes.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

40. Title: Add hidden-line style mode for wireframe and section contexts
    Description: Provide hidden-line rendering style to improve CAD legibility. DoD: mode toggle behaves consistently with section tool.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

41. Title: Add orbit/pan/zoom camera controls in CAD pane
    Description: Implement mouse navigation controls with predictable sensitivity and reset behavior. DoD: camera state is deterministic and persisted per session.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

42. Title: Add view cube and standard view snaps
    Description: Add orientation widget and top/front/right/isometric snaps. DoD: snaps align with orthographic expectations.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

43. Title: Add orthographic/perspective toggle
    Description: Support per-pane projection mode switching. DoD: projection state persists in pane session state.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

44. Title: Add render modes (shaded, shaded+edges, wireframe)
    Description: Implement render mode toggles and visual parity checks. DoD: each mode has snapshot baselines.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

45. Title: Add snapping toggles (grid/origin/endpoint/midpoint)
    Description: Implement snap-mode state model and interaction hooks for selection/measure. DoD: snap indicator state visible in UI.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

46. Title: Add CAD hotkey map and minimal customization
    Description: Define default hotkeys and allow remap for baseline action set. DoD: remapped keys persist and conflict checks prevent invalid binds.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

47. Title: Add command palette parity for CAD hotkey actions
    Description: Ensure all major CAD actions are discoverable via command palette entries. DoD: parity matrix test covers hotkey/palette command equivalence.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

48. Title: Add minimal 3D mouse input mapping
    Description: Add translate/rotate modes, speed scalar, and axis locks with fallback when device absent. DoD: device mapping profile is configurable.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

49. Title: Add CAD contextual menu framework
    Description: Implement pane-local context menu shell for selection and edit actions. DoD: context menu works for body/face/edge selections.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

50. Title: Add 4-up variant viewport layout
    Description: Support side-by-side variant presentation and per-viewport focus/select interactions. DoD: each tile retains independent camera and selection state.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

51. Title: Implement query module ray hit tests
    Description: Add body/face/edge hit testing utilities with stable hit payloads and tolerance-aware picking. DoD: hit precision tests across zoom levels.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

52. Title: Implement selection module with persistent selection sets
    Description: Add primary/secondary selection model, filters, and resilient selection IDs. DoD: selections survive rebuild where semantic refs remain valid.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

53. Title: Add hover highlight and click selection interactions
    Description: Wire query + selection into input loop for immediate feedback. DoD: hover/click latency meets frame budget target.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

54. Title: Add inspect panel for body properties
    Description: Show volume, area, mass, CoG, and bounding box for selected body. DoD: values match analysis engine outputs.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

55. Title: Add inspect panel for face/edge properties
    Description: Show face area + normal and edge length + type for sub-entities. DoD: inspect outputs are stable across camera moves.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

56. Title: Add measure tool MVP
    Description: Implement distance/angle measurements with on-screen labels and snapping support. DoD: measurement values deterministic under tolerance policy.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

57. Title: Add section analysis tool MVP
    Description: Add clipping-plane section view with controls and hidden-line compatibility. DoD: section mode interoperates with selection and inspect.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

58. Title: Implement material density table and material assignment
    Description: Add material presets and deterministic density mapping for analysis calculations. DoD: changing material updates mass/cost paths.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

59. Title: Implement analysis engine: volume/mass/CoG
    Description: Compute and cache core physical properties. DoD: analysis outputs reproducible and tolerance-bounded.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

60. Title: Implement analysis engine: cost heuristic
    Description: Add first-pass CNC/material cost estimator driven by geometry/material complexity. DoD: estimator exposes assumptions in metadata.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

61. Title: Implement analysis engine: deflection heuristic
    Description: Add beam-style deflection approximation for rack use case. DoD: heuristic includes documented limits and confidence label.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

62. Title: Add live engineering overlay panel
    Description: Render material/weight/cost/deflection metadata and update with geometry changes. DoD: update appears in same cycle as rebuild commit.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

63. Title: Implement stable semantic reference registry
    Description: Generate semantic refs like `rack_outer_face`/`mount_hole_pattern` for downstream ops and AI edits. DoD: refs persisted in `.apcad` and survive routine rebuild.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

64. Title: Implement deterministic Mac Studio rack template generator
    Description: Build base two-bay rack model generator with explicit parameter schema and semantic refs. DoD: baseline model matches scripted demo dimensions.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

65. Title: Add wall-mount feature set to rack generator
    Description: Add mount hole pattern and mounting geometry using semantic refs. DoD: mount features can be toggled/edited without ID churn.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

66. Title: Add vent pattern generator features
    Description: Add configurable vent arrays, sizing, and density controls. DoD: vent parameters drive predictable airflow-oriented variants.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

67. Title: Add rib/thickness optimization hooks to generator
    Description: Add tunable structure parameters required by weight/stiffness tradeoffs. DoD: variant objectives can manipulate these hooks directly.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

68. Title: Implement variant objective engine (4 objective presets)
    Description: Generate four deterministic variants: lowest weight, lowest cost, highest stiffness, airflow-biased. DoD: variant generation is reproducible with seeded objective solver.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

69. Title: Add per-variant selection and inspect synchronization
    Description: Ensure each variant supports independent selection, camera, warnings, and analysis display. DoD: switching selected variant does not leak prior state.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

70. Title: Define strict `CadIntent` JSON schema and validators
    Description: Add typed intent schema and validator rules; reject unknown/invented operations. DoD: schema validation errors are machine-readable.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

71. Title: Implement `CadIntent` execution dispatcher
    Description: Route validated intents to typed CAD commands only; block free-text state mutation. DoD: dispatcher coverage tests for all intent types.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

72. Title: Implement chat-to-`CadIntent` translation adapter
    Description: Convert Autopilot chat outputs into schema-validated intents with explicit parse failures and recovery prompts. DoD: adapter tests include malformed/ambiguous chat outputs.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

73. Title: Add CAD session lifecycle integration with Autopilot chat
    Description: Create new CAD sessions and bind follow-up intents to active session deterministically. DoD: no thread/session flicker or accidental session switching.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

74. Title: Add `CadEvent` emission into activity feed
    Description: Emit document/variant/selection/warning/rebuild/analysis/export events into existing activity lane. DoD: replay-safe dedupe behavior validated.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

75. Title: Add dimension labels and direct numeric editing
    Description: Allow click-to-edit dimensions (typed input only) with immediate rebuild. DoD: typed edits create undoable history entries and update overlays.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

76. Title: Implement STEP export writer (demo scope)
    Description: Export deterministic solid-only STEP (no assembly/PMI/colors). DoD: repeated exports of same doc are byte-stable except timestamp fields if any.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

77. Title: Add STEP checker utility using OpenCascade in CI
   Description: Build test utility that re-imports exported STEP and validates topology sanity. DoD: checker integrated into automated tests for CAD export path.
   Details: Checker output must include machine-readable diagnostics for invalid solids, missing shells, and non-manifold entities. CI should run checker on baseline and variant fixtures and upload checker logs as artifacts on failure. Document local command usage so developers can reproduce failures before pushing.

78. Title: Add STEP round-trip tolerance assertions (bbox/volume)
   Description: Compare exported-then-checked geometry properties against source with tolerance thresholds. DoD: tolerance failures produce actionable diff output.
   Details: Define per-metric thresholds (bbox axis deltas, volume delta) and a consistent report format showing expected vs actual values. Failures must include source doc revision, export settings, and checker metadata for triage. Include deterministic fixtures that sit near tolerance boundaries.

79. Title: Add CAD pane UX polish pass (noise reduction + readability)
    Description: Refine visual hierarchy, labels, and overlays while preserving engineering density. DoD: review checklist for CAD quality bar completed.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

80. Title: Harden pane clipping/scroll/overflow invariants for CAD
    Description: Ensure no CAD subview, overlay, timeline, or warning panel can overflow pane bounds. DoD: regression tests cover prior overflow classes.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

81. Title: Add CAD demo reset/bootstrap action
    Description: Provide one-click deterministic reset to baseline demo state for repeatable recordings. DoD: reset command idempotent across repeated use.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

82. Title: Add golden geometry snapshots for rack baseline and variants
    Description: Record and assert snapshot outputs for baseline and each objective variant. DoD: snapshot diff tooling highlights semantic geometry differences.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

83. Title: Add golden interaction test for follow-up parameter edit
    Description: Test prompt -> select -> typed edit -> rebuild -> warnings/analysis update path. DoD: scripted interaction produces deterministic receipts.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

84. Title: Add headless CAD script test harness (`CadIntent` sequences)
   Description: Add script runner that applies intent sequences and asserts geometry hash, analysis, and warnings. DoD: harness used by CI and demo reliability suite.
   Details: Define script file format with seeded randomness, explicit expected receipts, and optional timing assertions. Harness must support success and failure-path scripts (intent rejection, boolean failure, warning escalation). Add at least one canonical demo script consumed by both CI and release-gate reliability tests.

85. Title: Add performance benchmark suite for rebuild/mesh/hit-test/memory
    Description: Track rebuild latency, mesh generation time, hit-test time, FPS, and memory budget compliance. DoD: benchmark thresholds mapped to Gate A/B/E.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

86. Title: Add reliability test for full 20-second scripted demo
    Description: Automate scripted demo interactions and assert no stalls, flicker, state loss, or budget regressions. DoD: deterministic pass/fail criteria encoded.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

87. Title: Add release gate checklist for CAD demo milestone
    Description: Encode Gate A-E checks and block milestone release until all criteria are green. DoD: checklist linked from release process docs.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

88. Title: Wave 2 kickoff: sketch plane and sketch entity model
    Description: Start Wave 2 with sketch data model (lines/arcs/constraints anchors) and deterministic serialization. DoD: `.apcad` supports sketch entities with stable IDs.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

89. Title: Wave 2 kickoff: constraint solver MVP integration
    Description: Integrate basic constraints (coincident, horizontal/vertical, tangent, dimension) with solver diagnostics. DoD: common sketch scenarios solve deterministically.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

90. Title: Wave 2 kickoff: extrude/cut/revolve from sketch profiles
    Description: Convert constrained profiles into feature graph operations. DoD: sketch-derived features participate in history, warnings, and undo/redo.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

91. Title: Wave 2 kickoff: fillet/chamfer/shell production implementation
    Description: Replace placeholder operations with production-capable operations for part-modeling MVP. DoD: operations include failure classification and fallback messaging.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.

92. Title: Wave 2 kickoff: STEP import pipeline
    Description: Add deterministic STEP import into `CadDocument` for replacement-credibility workflows. DoD: imported solids map to stable IDs and survive save/reload.
    Details: Include explicit in-scope/out-of-scope boundaries, dependency links to prior issue numbers, test plan updates (unit/integration/snapshot/benchmark as applicable), and required observability/docs updates for reviewer verification.
