# CAD Week-1 Parallel-Jaw Gripper Plan

## Purpose

Define the minimum extension to our current CAD chat implementation so week-1 demo prompts can generate a **basic 2-jaw robotic gripper** that is:

- parametric
- 3D-printable-first
- variant-capable (4 variants)
- material-distinct per variant
- visually honest in pane/snapshot reporting

This plan is scoped to MVP boundaries in [`docs/MVP.md`](/Users/christopherdavid/code/openagents/docs/MVP.md) and crate ownership in [`docs/OWNERSHIP.md`](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md).

## Week-1 Demo Contract

Prompt target:

`Create a basic 2-jaw robotic gripper with a base plate, two parallel fingers, and mounting holes for a servo motor. Make it 3D-printable and parametric for easy scaling.`

Required visible outcomes:

1. Chat routes this as CAD work without manual tool forcing.
2. CAD model updates to a gripper geometry (not rack geometry).
3. User can ask for four variants and receive four deterministic gripper variants.
4. User can set four different materials and see those assignments reflected in CAD state.
5. Snapshot/state reporting does not claim “all four visible” unless 2x2 viewport is actually active.

Out of scope for week-1:

- tendon routing
- compliant joints
- underactuation physics
- motor wiring/electronics packs
- full humanoid hand kinematics

## References Reviewed

- `/Users/christopherdavid/code/AmazingHand`
- `/Users/christopherdavid/code/grablab`

Key reference takeaways used for scope:

- AmazingHand confirms 3D-printable servo-driven hand architecture and practical tolerance variability in printed parts.
- OpenHand (especially M2/T42 families) reinforces that a simple gripper-first progression is the right stepping stone before anthropomorphic hands.
- Week-1 should focus on rigid gripper geometry + parametric dimensions + mounting interfaces, not dexterous mechanisms.

## Extracted Reference Constraints (to encode in implementation)

### From AmazingHand (`/Users/christopherdavid/code/AmazingHand`)

- `AmazingHand_3DprintingTips.pdf`
  - rigid functional parts: PLA, 0.2 mm layers, high infill (typically 60-80%+ depending on part)
  - flexible parts: TPU/Filaflex class materials, avoid support when possible, print one-by-one to reduce failed bridges
- `AmazingHand_Assembly.pdf`
  - small hardware stack is common: M2 / M2.5 screws, drilled holes around 1.5 mm and 2 mm workflows, short rods/pins
  - printed-hole post-processing is expected; friction/fit is tuned after print
- `AmazingHand_Overview.pdf`
  - compact in-hand actuation is viable for social demo value
  - published headline specs: 5V supply, 400 g class hand, up to 1 kg payload target

### From OpenHand (`/Users/christopherdavid/code/grablab`)

- `openhand-hardware/model f3 (forces-for-free hand)/sldprt/params.txt`
  - explicit print-fit constants worth reusing as defaults:
    - `print fit = 0.15`
    - `print free = 0.35`
  - pin dimensions are around 3.0-3.18 mm class and should inform baseline mounting-hole presets
- `Model F3 Assembly Guide 1.0.pdf`
  - practical BOM shows repeated use of M2/M2.5 fasteners and 3 mm pin/bearing classes
  - tolerance note explicitly recommends printer-specific test fitting for bearing sleeves
- `openhand_node/src/openhand_node/hands.py` (`Model_T42`)
  - two-finger architecture with normalized close command (`close` default around 0.45; `max_close` around 0.75 normalized)
  - useful mapping for week-1 animation semantics: open/close travel should remain bounded and reversible

### Direct week-1 implications

- Provide explicit `print_fit_mm` and `print_clearance_mm` parameters with defaults near `0.15` / `0.35`.
- Default mount-hole and pin-friendly dimensions should target common M2/M2.5 + 3 mm hardware classes.
- Keep gripper kinematics in week-1 as bounded open/close stroke, not underactuated tendon physics.

## Current State (Implementation Reality)

### What already works

- CAD pane can be opened/focused and manipulated by chat tool calls.
- User can cycle variants, material presets, projection mode, and camera views.
- CAD rebuild pipeline is deterministic and checkpointed.
- CAD chat tooling already has retry/failure contracts and e2e harness support.

### Gaps to close for week-1

1. **Intent model is rack-oriented**
   - `crates/cad/src/intent.rs`
   - `crates/cad/src/chat_adapter.rs`
   - `crates/cad/src/dispatch.rs`
2. **Feature graph builder is rack-specific**
   - `apps/autopilot-desktop/src/input/reducers/cad.rs` (`build_demo_feature_graph`)
3. **Tessellation handlers are rack-specific**
   - `crates/cad/src/tessellation.rs`
4. **CAD turn classifier misses robot/gripper nouns**
   - `apps/autopilot-desktop/src/input/cad_turn_classifier.rs`
5. **Variant visibility can be misreported**
   - legacy path renders 2x2; basic path is single viewport
   - snapshot payload currently lacks explicit viewport layout/visible-variant truth fields
   - `apps/autopilot-desktop/src/panes/cad.rs`
   - `apps/autopilot-desktop/src/input/tool_bridge.rs`

## Architecture Approach

Keep ownership clean:

- `crates/cad`: typed intent/schema, validation, dispatch semantics, tessellation op handlers.
- `apps/autopilot-desktop`: chat routing, pane snapshot truth, reducer graph generation, demo UX flow.

Do not import historical backroom code. Build the week-1 path directly in this repo.

## Detailed Work Plan

## Phase 1: Intent + Classification (Chat understands gripper requests)

### 1.1 Extend CAD turn classifier vocabulary

File:

- `apps/autopilot-desktop/src/input/cad_turn_classifier.rs`

Changes:

- Add nouns/signals: `gripper`, `robotic hand`, `robot hand`, `finger`, `jaw`, `servo mount`, `3d printable`.
- Keep deterministic keyword-pair classifier behavior.

Acceptance:

- Prompt with `create ... gripper ... servo ... 3D-printable` classifies as CAD.
- Existing non-CAD classification stays unchanged.

### 1.2 Add week-1 gripper intent contract

Files:

- `crates/cad/src/intent.rs`
- `crates/cad/src/chat_adapter.rs`
- `crates/cad/src/intent_execution.rs` (if execution-plan mapping needs updates)

Changes:

- Add `CreateParallelJawGripperSpec` intent (typed JSON).
- Add translation rule for natural language gripper prompt to this intent.
- Add strict field validation with bounded ranges (examples):
  - `jaw_open_mm`
  - `finger_length_mm`
  - `finger_thickness_mm`
  - `base_width_mm`
  - `base_depth_mm`
  - `base_thickness_mm`
  - `servo_mount_hole_diameter_mm`
  - `print_clearance_mm`

Acceptance:

- Valid JSON and NL prompt both map to typed gripper intent.
- Invalid dimensions return deterministic validation errors (no panic, no silent fallback).

## Phase 2: Dispatch State + Parameter Surface

### 2.1 Add explicit design profile in dispatch state

File:

- `crates/cad/src/dispatch.rs`

Changes:

- Add `design_profile` (e.g., `rack` | `parallel_jaw_gripper`) to `CadDispatchState`.
- On `CreateParallelJawGripperSpec`, set profile + seed canonical parameter map.
- Preserve backward compatibility for rack flow.

Acceptance:

- Dispatch receipt clearly indicates gripper intent applied.
- Revision increments and state mutation remain deterministic.

### 2.2 Add gripper-aware dimensions to CAD pane state

File:

- `apps/autopilot-desktop/src/app_state_domains.rs`

Changes:

- Add week-1 gripper dimensions to editable dimension set.
- Keep defaults realistic for printability and demo scale.
- Maintain existing dimension-edit + rebuild behavior.

Acceptance:

- Dimension edits for gripper parameters are reflected in next rebuild.

## Phase 3: Feature Graph + Tessellation (Actual gripper geometry)

### 3.1 Branch feature graph generation by profile

File:

- `apps/autopilot-desktop/src/input/reducers/cad.rs`

Changes:

- Split current rack-only `build_demo_feature_graph` into:
  - `build_rack_feature_graph(...)`
  - `build_parallel_jaw_gripper_feature_graph(...)`
- Gripper graph minimal nodes:
  - base plate
  - left finger
  - right finger
  - servo mount hole pattern
  - optional edge softening placeholder

Acceptance:

- Gripper prompts produce graph nodes with gripper-specific operation keys/params.
- Rack prompts still produce rack graph unchanged.

### 3.2 Add tessellation handlers for gripper operation keys

File:

- `crates/cad/src/tessellation.rs`

Changes:

- Add deterministic mesh construction handlers for gripper ops.
- Keep deterministic hash stability guarantees.
- Ensure bounds/triangle counts are non-zero and plausible.

Acceptance:

- Rebuild completes with visible gripper geometry.
- Same input => same rebuild hash/mesh hash.

## Phase 4: Variants + Materials for Week-1 Story

### 4.1 Variant strategy for 4 deterministic gripper variants

Files:

- `apps/autopilot-desktop/src/input/reducers/cad.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`
- optional: `crates/cad/src/dispatch.rs` (if variant objective metadata is persisted there)

Changes:

- Define 4 gripper variants with clear parameter deltas:
  - `variant.baseline`
  - `variant.wide-jaw`
  - `variant.long-reach`
  - `variant.stiff-finger`
- Keep deterministic variant-id ordering and mapping.

Acceptance:

- `GenerateVariants count=4` yields four stable gripper variants.

### 4.2 Per-variant material assignment (not global-only)

Files:

- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/input/reducers/cad.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs` (checkpoint payload)

Changes:

- Add `variant_materials` map in pane state.
- `SetMaterial` applies to active variant unless explicitly overridden.
- Checkpoint payload reports per-variant materials.

Acceptance:

- Four variants can hold four distinct material IDs at once.
- Snapshot confirms material mapping deterministically.

## Phase 5: Snapshot Truth + Viewport Honesty

### 5.1 Report actual viewport layout in snapshot payload

Files:

- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/panes/cad.rs`

Changes:

- Add explicit fields in `pane_snapshot_details.cad`:
  - `viewport_layout`: `single` | `quad`
  - `visible_variant_ids`: list of currently visible variants
  - `all_variants_visible`: bool
- Compute from real render mode (`OPENAGENTS_CAD_USE_LEGACY_PANE` + active layout state).

Acceptance:

- Snapshot can no longer imply 2x2 visibility when pane is cycling a single viewport.

### 5.2 Optional but recommended: explicit 2x2 toggle in non-legacy pane

Files:

- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/panes/cad.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`

Changes:

- Add a stateful viewport layout toggle action in basic pane path.
- Keep single-view default to avoid UX churn.

Acceptance:

- Agent can intentionally request 2x2 and snapshot reflects it.

## Phase 6: Tests and Demo Gates

## 6.1 Unit tests

- `cad_turn_classifier` positive coverage for gripper prompts.
- `intent` parse/validate tests for `CreateParallelJawGripperSpec`.
- `dispatch` tests for profile + parameter mutation.
- `tessellation` deterministic gripper mesh tests.

## 6.2 Reducer integration tests

File:

- `apps/autopilot-desktop/src/input/reducers/cad.rs` tests

Add:

- gripper prompt -> intent -> rebuild -> receipt assertions
- variant/material mapping assertions

## 6.3 Tool bridge snapshot tests

File:

- `apps/autopilot-desktop/src/input/tool_bridge.rs` tests

Add:

- `viewport_layout` and `visible_variant_ids` contract tests for single vs quad.

## 6.4 E2E harness extension

Files:

- `apps/autopilot-desktop/tests/scripts/*`
- `apps/autopilot-desktop/tests/goldens/*`

Add new script/golden:

- week-1 gripper build flow with 4 variants + 4 materials + snapshot validation.

## Complete GitHub Issue Backlog (GitHub-ready)

This is the full issue list required to implement this plan end-to-end.

## OA-CAD-W1-00: Epic - Week-1 Parallel-Jaw Gripper CAD Demo

Description:
Track all week-1 scope for chat-driven gripper generation, variants/materials, and viewport-truth snapshots.

Scope:

- Create parent epic with linked child issues `OA-CAD-W1-01` through `OA-CAD-W1-19`.
- Track demo acceptance checklist and dependency order.

Acceptance Criteria:

1. All child issues linked with status.
2. Epic includes week-1 definition of done from this document.

Validation:

- GitHub board/issue links complete.

Dependencies:

- none

## OA-CAD-W1-01: Encode Reference-Derived Design Defaults and Bounds

Description:
Translate extracted AmazingHand/OpenHand constraints into explicit CAD defaults and bounds for week-1 gripper parameters.

Scope:

- Add a reference-backed defaults table in this doc and wire equivalent constants in code.
- Include `print_fit_mm`, `print_clearance_mm`, hardware-size defaults, and bounded open/close travel assumptions.

Acceptance Criteria:

1. Defaults are explicit and deterministic.
2. Bounds reject non-physical values.

Validation:

- `cargo test -p openagents-cad`
- `cargo test -p autopilot-desktop`

Dependencies:

- `OA-CAD-W1-00`

## OA-CAD-W1-02: Expand CAD Turn Classifier for Gripper/Robot-Hand Prompts

Description:
Ensure chat prompts about robotic grippers/hands are classified as CAD turns without requiring explicit tool-call markers.

Scope:

- Update `apps/autopilot-desktop/src/input/cad_turn_classifier.rs`.
- Add terms: `gripper`, `robotic hand`, `jaw`, `servo mount`, `3d printable`, `parametric`.
- Preserve existing rack classification behavior.

Acceptance Criteria:

1. Week-1 gripper prompt classifies as CAD.
2. Existing non-CAD prompts remain non-CAD.

Validation:

- `cargo test -p autopilot-desktop cad_turn_classifier -- --nocapture`

Dependencies:

- `OA-CAD-W1-00`

## OA-CAD-W1-03: Add `CreateParallelJawGripperSpec` CadIntent + JSON Schema

Description:
Introduce a first-class typed intent for week-1 gripper creation.

Scope:

- Update `crates/cad/src/intent.rs`.
- Add intent struct fields:
  - `jaw_open_mm`
  - `finger_length_mm`
  - `finger_thickness_mm`
  - `base_width_mm`
  - `base_depth_mm`
  - `base_thickness_mm`
  - `servo_mount_hole_diameter_mm`
  - `print_fit_mm`
  - `print_clearance_mm`
- Add validation ranges and schema entries.

Acceptance Criteria:

1. Valid payload parses and validates.
2. Invalid ranges return deterministic `CAD-INTENT-*` validation errors.

Validation:

- `cargo test -p openagents-cad intent`

Dependencies:

- `OA-CAD-W1-01`

## OA-CAD-W1-04: Add NL-to-Intent Translation for Week-1 Gripper Prompt Family

Description:
Map natural-language gripper prompts to the new typed intent with deterministic heuristics.

Scope:

- Update `crates/cad/src/chat_adapter.rs`.
- Add phrase extraction for gripper/jaw/finger/servo-mount/printability cues.
- Preserve existing rack path and fallback behavior.

Acceptance Criteria:

1. Canonical week-1 prompt resolves to `CreateParallelJawGripperSpec`.
2. Ambiguous prompt returns deterministic clarification path.

Validation:

- `cargo test -p openagents-cad chat_adapter`

Dependencies:

- `OA-CAD-W1-03`

## OA-CAD-W1-05: Add `design_profile` to Dispatch State and Receipts

Description:
Make dispatch profile-aware so rack and gripper paths can coexist safely.

Scope:

- Update `crates/cad/src/dispatch.rs`.
- Add profile enum and persistence in `CadDispatchState`.
- On gripper intent dispatch, set profile and seed canonical parameter map.

Acceptance Criteria:

1. Dispatch receipts reflect profile-aware state changes.
2. Rack behavior remains backward-compatible.

Validation:

- `cargo test -p openagents-cad dispatch`

Dependencies:

- `OA-CAD-W1-03`

## OA-CAD-W1-06: Add Gripper Dimension Surface to CAD Pane State

Description:
Expose editable gripper dimensions in desktop CAD state and keep dimension-edit rebuild loop intact.

Scope:

- Update `apps/autopilot-desktop/src/app_state_domains.rs`.
- Add gripper dimension entries with defaults/bounds from `OA-CAD-W1-01`.
- Ensure profile switch selects correct dimension subset.

Acceptance Criteria:

1. Gripper dimensions can be edited and persisted through rebuild cycles.
2. Rack dimensions still work unchanged.

Validation:

- `cargo test -p autopilot-desktop app_state_domains -- --nocapture`

Dependencies:

- `OA-CAD-W1-05`

## OA-CAD-W1-07: Split Reducer Feature Graph Builder by Profile

Description:
Replace rack-only graph generation with profile-based graph builders.

Scope:

- Update `apps/autopilot-desktop/src/input/reducers/cad.rs`.
- Extract:
  - `build_rack_feature_graph(...)`
  - `build_parallel_jaw_gripper_feature_graph(...)`
- Gripper graph must include base plate, two fingers, and mount holes.

Acceptance Criteria:

1. Gripper intent produces gripper graph nodes/params.
2. Rack prompt still produces existing rack graph.

Validation:

- `cargo test -p autopilot-desktop input::reducers::cad -- --nocapture`

Dependencies:

- `OA-CAD-W1-05`
- `OA-CAD-W1-06`

## OA-CAD-W1-08: Add Deterministic Tessellation Ops for Gripper Features

Description:
Implement mesh handlers for gripper operation keys so rebuild outputs actual gripper geometry.

Scope:

- Update `crates/cad/src/tessellation.rs`.
- Add deterministic handlers for gripper base/finger/mount-hole features.
- Keep stable hash behavior and non-empty mesh invariants.

Acceptance Criteria:

1. Rebuild succeeds for gripper graph.
2. Same input produces identical rebuild and mesh hashes.

Validation:

- `cargo test -p openagents-cad tessellation`

Dependencies:

- `OA-CAD-W1-07`

## OA-CAD-W1-09: Implement 4 Deterministic Gripper Variants

Description:
Support `GenerateVariants count=4` with clear week-1 gripper variant semantics.

Scope:

- Update reducer/state variant logic for gripper profile.
- Variant IDs:
  - `variant.baseline`
  - `variant.wide-jaw`
  - `variant.long-reach`
  - `variant.stiff-finger`
- Define deterministic parameter deltas per variant.

Acceptance Criteria:

1. Four variants generated in stable order.
2. Variant deltas visibly impact geometry.

Validation:

- `cargo test -p autopilot-desktop input::reducers::cad -- --nocapture`

Dependencies:

- `OA-CAD-W1-07`
- `OA-CAD-W1-08`

## OA-CAD-W1-10: Per-Variant Material Persistence and SetMaterial Semantics

Description:
Allow four variants to hold four different materials simultaneously.

Scope:

- Add `variant_materials` map to CAD pane state.
- Make `SetMaterial` target active variant by default.
- Update analysis/material read paths to consult per-variant mapping.

Acceptance Criteria:

1. Changing material on variant A does not overwrite variant B.
2. Snapshot/checkpoint exposes per-variant material mapping.

Validation:

- `cargo test -p autopilot-desktop input::reducers::cad -- --nocapture`
- `cargo test -p autopilot-desktop input::tool_bridge -- --nocapture`

Dependencies:

- `OA-CAD-W1-09`

## OA-CAD-W1-11: Add Printability Warnings for Gripper Profile

Description:
Add week-1 printability checks informed by reference tolerances.

Scope:

- Update warning-generation path in reducer validity snapshot helpers.
- Add warnings for:
  - wall/finger thickness below bound
  - hole-edge margin too small
  - print clearance below configured fit threshold

Acceptance Criteria:

1. Out-of-bounds configs generate deterministic warnings.
2. Valid baseline config remains warning-light.

Validation:

- `cargo test -p autopilot-desktop input::reducers::cad -- --nocapture`

Dependencies:

- `OA-CAD-W1-01`
- `OA-CAD-W1-07`

## OA-CAD-W1-12: Extend CAD Checkpoint Contract with Profile and Variant Materials

Description:
Expose enough state in tool responses for reliable agent reasoning.

Scope:

- Update `apps/autopilot-desktop/src/input/tool_bridge.rs` checkpoint payload.
- Add:
  - design profile
  - variant->material map
  - gripper parameter summary

Acceptance Criteria:

1. Tool response includes new fields for both `openagents.cad.intent` and `openagents.cad.action`.
2. Existing consumers remain compatible.

Validation:

- `cargo test -p autopilot-desktop input::tool_bridge -- --nocapture`

Dependencies:

- `OA-CAD-W1-10`

## OA-CAD-W1-13: Fix Snapshot Visibility Truth (Single vs Quad)

Description:
Prevent mismatch where agent thinks all variants are visible when UI is single-view cycling.

Scope:

- Update `pane_snapshot_details` and CAD pane snapshot path.
- Add fields:
  - `viewport_layout`
  - `visible_variant_ids`
  - `all_variants_visible`

Acceptance Criteria:

1. Single-view mode reports one visible variant.
2. Quad mode reports all visible variants.

Validation:

- `cargo test -p autopilot-desktop input::tool_bridge -- --nocapture`

Dependencies:

- `OA-CAD-W1-12`

## OA-CAD-W1-14: Add Explicit Quad-Viewport Toggle to Basic CAD Pane

Description:
Make 2x2 multi-variant display explicit and agent-addressable in non-legacy mode.

Scope:

- Update:
  - `apps/autopilot-desktop/src/app_state_domains.rs`
  - `apps/autopilot-desktop/src/panes/cad.rs`
  - `apps/autopilot-desktop/src/pane_system.rs`
  - `apps/autopilot-desktop/src/input/tool_bridge.rs`
- Add new CAD action key for layout toggle.

Acceptance Criteria:

1. User/agent can switch between single and quad layouts.
2. Snapshot truth fields mirror active layout.

Validation:

- `cargo test -p autopilot-desktop panes::cad -- --nocapture`
- `cargo test -p autopilot-desktop input::tool_bridge -- --nocapture`

Dependencies:

- `OA-CAD-W1-13`

## OA-CAD-W1-15: Update `autopilot-cad-builder` Skill for Week-1 Gripper Flows

Description:
Ensure skill guidance nudges Codex toward the new intent schema and truthful snapshot checks.

Scope:

- Update `skills/autopilot-cad-builder/SKILL.md`.
- Add gripper-first intent examples and variant/material workflow.
- Require snapshot verification before claiming 2x2 display.

Acceptance Criteria:

1. Skill examples include canonical week-1 prompt and strict `intent_json` form.
2. Skill explicitly references layout-truth fields.

Validation:

- Manual skill-read sanity check.

Dependencies:

- `OA-CAD-W1-12`
- `OA-CAD-W1-13`

## OA-CAD-W1-16: Cad Crate Test Expansion (Intent/Dispatch/Tessellation)

Description:
Add unit tests to lock deterministic behavior for new gripper intent and geometry handlers.

Scope:

- Add tests in:
  - `crates/cad/src/intent.rs`
  - `crates/cad/src/chat_adapter.rs`
  - `crates/cad/src/dispatch.rs`
  - `crates/cad/src/tessellation.rs`

Acceptance Criteria:

1. New intent paths fully covered.
2. Gripper tessellation deterministic tests pass.

Validation:

- `cargo test -p openagents-cad`

Dependencies:

- `OA-CAD-W1-03`
- `OA-CAD-W1-04`
- `OA-CAD-W1-05`
- `OA-CAD-W1-08`

## OA-CAD-W1-17: Desktop Reducer + Tool-Bridge Test Expansion

Description:
Cover profile branching, variants/material maps, and snapshot truth in desktop tests.

Scope:

- Add tests in:
  - `apps/autopilot-desktop/src/input/reducers/cad.rs`
  - `apps/autopilot-desktop/src/input/tool_bridge.rs`
  - `apps/autopilot-desktop/src/input/cad_turn_classifier.rs`

Acceptance Criteria:

1. Week-1 prompt path is covered end-to-end at reducer/tool-call level.
2. Snapshot truth fields tested for single and quad.

Validation:

- `cargo test -p autopilot-desktop input::reducers::cad -- --nocapture`
- `cargo test -p autopilot-desktop input::tool_bridge -- --nocapture`

Dependencies:

- `OA-CAD-W1-02`
- `OA-CAD-W1-09`
- `OA-CAD-W1-10`
- `OA-CAD-W1-13`

## OA-CAD-W1-18: Add Week-1 Gripper E2E Harness Script + Goldens

Description:
Add deterministic script/golden coverage for the entire week-1 gripper flow.

Scope:

- Add new script under `apps/autopilot-desktop/tests/scripts/`.
- Add corresponding golden under `apps/autopilot-desktop/tests/goldens/`.
- Cover:
  - gripper prompt
  - 4 variants
  - 4 materials
  - snapshot truth check

Acceptance Criteria:

1. Harness passes deterministically in CI and local runs.
2. Golden diff is stable after non-behavioral reruns.

Validation:

- `cargo test -p autopilot-desktop cad_chat_build_e2e_harness -- --nocapture`

Dependencies:

- `OA-CAD-W1-17`

## OA-CAD-W1-19: Demo Runbook + Release Gate Updates for Week-1 Gripper

Description:
Document operator/demo steps and ensure release checks include week-1 gripper path.

Scope:

- Update docs under `docs/codex/` and any CAD release checklist references.
- Add explicit Friday-demo script:
  - prompt
  - variant generation
  - material assignment
  - camera/projection switch
  - snapshot truth verification

Acceptance Criteria:

1. Operator can run demo from runbook without tribal knowledge.
2. Release checklist references new tests and expected artifacts.

Validation:

- Manual dry run + checklist completion.

Dependencies:

- `OA-CAD-W1-18`

## Delivery Sequence (Recommended)

1. `OA-CAD-W1-01` to `OA-CAD-W1-06` (constraints -> classifier/intent/dispatch/state)
2. `OA-CAD-W1-07` to `OA-CAD-W1-11` (graph/tessellation/variants/material/warnings)
3. `OA-CAD-W1-12` to `OA-CAD-W1-15` (checkpoint/snapshot/layout/skill)
4. `OA-CAD-W1-16` to `OA-CAD-W1-18` (test hardening + deterministic e2e)
5. `OA-CAD-W1-19` (demo runbook + release gate)

## Risks and Mitigations

1. Risk: breaking current rack demo behavior.
   - Mitigation: profile-branching with rack defaults preserved; rack tests remain green.
2. Risk: hash instability from new tessellation ops.
   - Mitigation: deterministic param normalization and fixed op ordering tests.
3. Risk: AI/user mismatch on what is visible.
   - Mitigation: explicit snapshot truth fields and optional explicit 2x2 action.
4. Risk: scope creep into underactuation/mechatronics.
   - Mitigation: hard week-1 non-goal list and acceptance gates.

## Definition of Done (Week-1)

- A natural-language week-1 gripper prompt builds a gripper model in CAD pane.
- The model is parameter-editable and rebuilds deterministically.
- Four deterministic variants can be produced.
- Four distinct materials can be assigned and persisted by variant.
- Snapshot payload accurately reports visibility/layout (no false “all four visible” claims).
- Added tests pass for new intent, reducer, tool snapshot contract, and e2e flow.
