# OpenAgents CAD Docs

This directory is the canonical design and implementation reference for the `openagents-cad` system.
It documents the current MVP CAD stack, how CAD integrates into Autopilot Desktop, what guarantees we enforce
for trust and determinism, and how the team should evolve CAD without regressing professional usability.

The intent is explicit: this is not a toy "extrude demo." The CAD subsystem is designed as a structured,
deterministic, and testable modeling runtime that can be driven both manually (pane interactions and direct parameter edits)
and programmatically (intent dispatch and chat-assisted workflows). The docs here are written to keep that contract stable
as implementation continues.

The current architecture is scoped to MVP realities:
`crates/cad` owns CAD-domain primitives, evaluation, geometry/analysis metadata, and serialization contracts;
`apps/autopilot-desktop` owns pane behavior, orchestration, and user experience wiring.
The design center is "engineering confidence under iteration": when a model changes, the system should explain what changed,
why it changed, and whether that change remains valid within declared tolerances and safety policies.

## How To Use This Folder

Start with the plan and policy documents to understand system boundaries and delivery sequencing.
Then read the schema/contract docs to understand data flow and intent execution.
Then read the reliability/performance/runbook docs to understand what must pass before anything is considered production-safe.
Finally, use decisions and spike notes for historical context when evaluating architectural changes.

Recommended reading path:

1. [`PLAN.md`](./PLAN.md)
2. [`VCAD_PARITY_PLAN.md`](./VCAD_PARITY_PLAN.md)
3. [`VCAD_PARITY_BASELINE_MANIFESTS.md`](./VCAD_PARITY_BASELINE_MANIFESTS.md)
4. [`VCAD_CAPABILITY_CRAWLER.md`](./VCAD_CAPABILITY_CRAWLER.md)
5. [`OPENAGENTS_CAPABILITY_CRAWLER.md`](./OPENAGENTS_CAPABILITY_CRAWLER.md)
6. [`PARITY_GAP_MATRIX.md`](./PARITY_GAP_MATRIX.md)
7. [`PARITY_SCORECARD.md`](./PARITY_SCORECARD.md)
8. [`PARITY_FIXTURE_CORPUS.md`](./PARITY_FIXTURE_CORPUS.md)
9. [`PARITY_CHECK_ORCHESTRATION.md`](./PARITY_CHECK_ORCHESTRATION.md)
10. [`PARITY_CI_LANE.md`](./PARITY_CI_LANE.md)
11. [`PARITY_RISK_REGISTER.md`](./PARITY_RISK_REGISTER.md)
12. [`PARITY_DASHBOARD.md`](./PARITY_DASHBOARD.md)
13. [`PARITY_BASELINE_DASHBOARD.md`](./PARITY_BASELINE_DASHBOARD.md)
14. [`KERNEL_ADAPTER_V2.md`](./KERNEL_ADAPTER_V2.md)
15. [`KERNEL_MATH_PARITY.md`](./KERNEL_MATH_PARITY.md)
16. [`KERNEL_TOPOLOGY_PARITY.md`](./KERNEL_TOPOLOGY_PARITY.md)
17. [`KERNEL_GEOM_PARITY.md`](./KERNEL_GEOM_PARITY.md)
18. [`KERNEL_PRIMITIVES_PARITY.md`](./KERNEL_PRIMITIVES_PARITY.md)
19. [`KERNEL_TESSELLATE_PARITY.md`](./KERNEL_TESSELLATE_PARITY.md)
20. [`KERNEL_BOOLEANS_PARITY.md`](./KERNEL_BOOLEANS_PARITY.md)
21. [`KERNEL_BOOLEAN_DIAGNOSTICS_PARITY.md`](./KERNEL_BOOLEAN_DIAGNOSTICS_PARITY.md)
22. [`KERNEL_BOOLEAN_BREP_PARITY.md`](./KERNEL_BOOLEAN_BREP_PARITY.md)
23. [`KERNEL_NURBS_PARITY.md`](./KERNEL_NURBS_PARITY.md)
24. [`KERNEL_TEXT_PARITY.md`](./KERNEL_TEXT_PARITY.md)
25. [`KERNEL_FILLET_PARITY.md`](./KERNEL_FILLET_PARITY.md)
26. [`KERNEL_SHELL_PARITY.md`](./KERNEL_SHELL_PARITY.md)
27. [`KERNEL_STEP_PARITY.md`](./KERNEL_STEP_PARITY.md)
28. [`PRIMITIVE_CONTRACTS_PARITY.md`](./PRIMITIVE_CONTRACTS_PARITY.md)
29. [`TRANSFORM_PARITY.md`](./TRANSFORM_PARITY.md)
30. [`PATTERN_PARITY.md`](./PATTERN_PARITY.md)
31. [`SHELL_FEATURE_GRAPH_PARITY.md`](./SHELL_FEATURE_GRAPH_PARITY.md)
32. [`FILLET_FEATURE_GRAPH_PARITY.md`](./FILLET_FEATURE_GRAPH_PARITY.md)
33. [`CHAMFER_FEATURE_GRAPH_PARITY.md`](./CHAMFER_FEATURE_GRAPH_PARITY.md)
34. [`EXPANDED_FINISHING_PARITY.md`](./EXPANDED_FINISHING_PARITY.md)
35. [`SWEEP_PARITY.md`](./SWEEP_PARITY.md)
36. [`LOFT_PARITY.md`](./LOFT_PARITY.md)
37. [`TOPOLOGY_REPAIR_PARITY.md`](./TOPOLOGY_REPAIR_PARITY.md)
38. [`MATERIAL_ASSIGNMENT_PARITY.md`](./MATERIAL_ASSIGNMENT_PARITY.md)
39. [`VCAD_EVAL_RECEIPTS_PARITY.md`](./VCAD_EVAL_RECEIPTS_PARITY.md)
40. [`FEATURE_OP_HASH_PARITY.md`](./FEATURE_OP_HASH_PARITY.md)
41. [`MODELING_EDGE_CASE_PARITY.md`](./MODELING_EDGE_CASE_PARITY.md)
42. [`CORE_MODELING_CHECKPOINT_PARITY.md`](./CORE_MODELING_CHECKPOINT_PARITY.md)
43. [`SKETCH_ENTITY_SET_PARITY.md`](./SKETCH_ENTITY_SET_PARITY.md)
44. [`SKETCH_PLANE_PARITY.md`](./SKETCH_PLANE_PARITY.md)
45. [`KERNEL_PRECISION_PARITY.md`](./KERNEL_PRECISION_PARITY.md)
46. [`DEPENDENCY_POSTURE.md`](./DEPENDENCY_POSTURE.md)
47. [`UNITS_TOLERANCE_POLICY.md`](./UNITS_TOLERANCE_POLICY.md)
48. [`CAD_DOCUMENT_SCHEMA.md`](./CAD_DOCUMENT_SCHEMA.md)
49. [`CAD_INTENTS.md`](./CAD_INTENTS.md)
50. [`CAD_CODE_HEALTH.md`](./CAD_CODE_HEALTH.md)
51. [`CAD_DEMO_RELEASE_GATES.md`](./CAD_DEMO_RELEASE_GATES.md)

## System Overview (Longform)

OpenAgents CAD is a deterministic modeling pipeline with explicit separation between modeling authority and UI presentation.
The CAD crate owns document schema, feature graph semantics, parameter mutation handling, evaluation scheduling, mesh/tessellation handoff,
engineering heuristics, and integrity signals (warnings, validity classes, deterministic hashes, repeatable outputs).
The desktop app consumes these structured outputs and renders them in WGPUI panes, preserving interaction fluency while keeping CAD truth in Rust-domain state.

The modeling path is intentionally incremental and composable. A document carries versioned schema and stable identities.
Feature operations produce geometry snapshots and semantic references that downstream operations and inspection tooling can rely on.
Evaluation is structured as a pure-ish runtime path over document + parameters + feature graph, with cache-aware behavior and reproducible outputs.
This keeps AI-driven and human-driven edits equivalent at the CAD layer: both are just structured state transitions over the same graph.

The AI integration strategy is constrained by design. Chat-assisted workflows dispatch declared intents rather than free-form geometry mutations.
Intents map to typed operations, and operations map to deterministic state changes. This prevents "AI guessware" from silently rewriting model authority.
In practical terms, CAD remains sovereign data, while conversational tooling becomes an orchestration layer over existing contracts.

Engineering overlays are treated as inspectable heuristics, not certification. Cost, mass, deflection, and related values are surfaced with policy intent:
help the user make better decisions quickly without pretending these are signed-off analyses.
That same principle appears throughout the docs: explicit caveats, explicit warning classes, explicit release gates.
The system should fail loudly and legibly when assumptions are violated.

Performance and pro-feel are both first-class. The docs codify budgets and gate checks for rebuild timing, reliability under scripted interaction,
and release-readiness criteria that reject "viewer-quality" UX. This includes selection/inspection behavior, deterministic variant behavior,
STEP validation pathways, and operational runbooks for lint, tests, and hardening lanes. The target outcome is confidence:
engineers should trust that CAD edits are reproducible, debuggable, and reversible.

## Full Table Of Contents

Everything currently in `crates/cad/docs/` is indexed below.

### Core Plan And Policy

- [`PLAN.md`](./PLAN.md)
- [`VCAD_PARITY_PLAN.md`](./VCAD_PARITY_PLAN.md)
- [`VCAD_PARITY_BASELINE_MANIFESTS.md`](./VCAD_PARITY_BASELINE_MANIFESTS.md)
- [`VCAD_CAPABILITY_CRAWLER.md`](./VCAD_CAPABILITY_CRAWLER.md)
- [`OPENAGENTS_CAPABILITY_CRAWLER.md`](./OPENAGENTS_CAPABILITY_CRAWLER.md)
- [`PARITY_GAP_MATRIX.md`](./PARITY_GAP_MATRIX.md)
- [`PARITY_SCORECARD.md`](./PARITY_SCORECARD.md)
- [`PARITY_FIXTURE_CORPUS.md`](./PARITY_FIXTURE_CORPUS.md)
- [`PARITY_CHECK_ORCHESTRATION.md`](./PARITY_CHECK_ORCHESTRATION.md)
- [`PARITY_CI_LANE.md`](./PARITY_CI_LANE.md)
- [`PARITY_RISK_REGISTER.md`](./PARITY_RISK_REGISTER.md)
- [`PARITY_DASHBOARD.md`](./PARITY_DASHBOARD.md)
- [`PARITY_BASELINE_DASHBOARD.md`](./PARITY_BASELINE_DASHBOARD.md)
- [`KERNEL_ADAPTER_V2.md`](./KERNEL_ADAPTER_V2.md)
- [`KERNEL_MATH_PARITY.md`](./KERNEL_MATH_PARITY.md)
- [`KERNEL_TOPOLOGY_PARITY.md`](./KERNEL_TOPOLOGY_PARITY.md)
- [`KERNEL_GEOM_PARITY.md`](./KERNEL_GEOM_PARITY.md)
- [`KERNEL_PRIMITIVES_PARITY.md`](./KERNEL_PRIMITIVES_PARITY.md)
- [`KERNEL_TESSELLATE_PARITY.md`](./KERNEL_TESSELLATE_PARITY.md)
- [`KERNEL_BOOLEANS_PARITY.md`](./KERNEL_BOOLEANS_PARITY.md)
- [`KERNEL_BOOLEAN_DIAGNOSTICS_PARITY.md`](./KERNEL_BOOLEAN_DIAGNOSTICS_PARITY.md)
- [`KERNEL_BOOLEAN_BREP_PARITY.md`](./KERNEL_BOOLEAN_BREP_PARITY.md)
- [`KERNEL_NURBS_PARITY.md`](./KERNEL_NURBS_PARITY.md)
- [`KERNEL_TEXT_PARITY.md`](./KERNEL_TEXT_PARITY.md)
- [`KERNEL_FILLET_PARITY.md`](./KERNEL_FILLET_PARITY.md)
- [`KERNEL_SHELL_PARITY.md`](./KERNEL_SHELL_PARITY.md)
- [`KERNEL_STEP_PARITY.md`](./KERNEL_STEP_PARITY.md)
- [`PRIMITIVE_CONTRACTS_PARITY.md`](./PRIMITIVE_CONTRACTS_PARITY.md)
- [`TRANSFORM_PARITY.md`](./TRANSFORM_PARITY.md)
- [`PATTERN_PARITY.md`](./PATTERN_PARITY.md)
- [`SHELL_FEATURE_GRAPH_PARITY.md`](./SHELL_FEATURE_GRAPH_PARITY.md)
- [`FILLET_FEATURE_GRAPH_PARITY.md`](./FILLET_FEATURE_GRAPH_PARITY.md)
- [`CHAMFER_FEATURE_GRAPH_PARITY.md`](./CHAMFER_FEATURE_GRAPH_PARITY.md)
- [`EXPANDED_FINISHING_PARITY.md`](./EXPANDED_FINISHING_PARITY.md)
- [`SWEEP_PARITY.md`](./SWEEP_PARITY.md)
- [`LOFT_PARITY.md`](./LOFT_PARITY.md)
- [`TOPOLOGY_REPAIR_PARITY.md`](./TOPOLOGY_REPAIR_PARITY.md)
- [`MATERIAL_ASSIGNMENT_PARITY.md`](./MATERIAL_ASSIGNMENT_PARITY.md)
- [`VCAD_EVAL_RECEIPTS_PARITY.md`](./VCAD_EVAL_RECEIPTS_PARITY.md)
- [`FEATURE_OP_HASH_PARITY.md`](./FEATURE_OP_HASH_PARITY.md)
- [`MODELING_EDGE_CASE_PARITY.md`](./MODELING_EDGE_CASE_PARITY.md)
- [`CORE_MODELING_CHECKPOINT_PARITY.md`](./CORE_MODELING_CHECKPOINT_PARITY.md)
- [`SKETCH_ENTITY_SET_PARITY.md`](./SKETCH_ENTITY_SET_PARITY.md)
- [`SKETCH_PLANE_PARITY.md`](./SKETCH_PLANE_PARITY.md)
- [`KERNEL_PRECISION_PARITY.md`](./KERNEL_PRECISION_PARITY.md)
- [`DEPENDENCY_POSTURE.md`](./DEPENDENCY_POSTURE.md)
- [`UNITS_TOLERANCE_POLICY.md`](./UNITS_TOLERANCE_POLICY.md)
- [`CAD_CODE_HEALTH.md`](./CAD_CODE_HEALTH.md)
- [`CAD_DEMO_RELEASE_GATES.md`](./CAD_DEMO_RELEASE_GATES.md)
- [`CAD_DEMO_RELIABILITY.md`](./CAD_DEMO_RELIABILITY.md)
- [`CAD_QUALITY_REVIEW.md`](./CAD_QUALITY_REVIEW.md)
- [`CAD_PERFORMANCE_BENCHMARKS.md`](./CAD_PERFORMANCE_BENCHMARKS.md)

### Data Model, Formats, And Contracts

- [`APCAD_FORMAT.md`](./APCAD_FORMAT.md)
- [`CAD_DOCUMENT_SCHEMA.md`](./CAD_DOCUMENT_SCHEMA.md)
- [`CAD_CONTRACTS.md`](./CAD_CONTRACTS.md)
- [`CAD_ERROR_MODEL.md`](./CAD_ERROR_MODEL.md)
- [`CAD_EVENTS.md`](./CAD_EVENTS.md)
- [`CAD_PARAMS.md`](./CAD_PARAMS.md)
- [`CAD_HISTORY.md`](./CAD_HISTORY.md)
- [`CAD_TIMELINE.md`](./CAD_TIMELINE.md)
- [`CAD_WARNINGS_PANEL.md`](./CAD_WARNINGS_PANEL.md)
- [`CAD_VALIDITY.md`](./CAD_VALIDITY.md)
- [`CAD_SEMANTIC_REFS.md`](./CAD_SEMANTIC_REFS.md)

### Modeling And Geometry Pipeline

- [`CAD_FEATURE_OPS.md`](./CAD_FEATURE_OPS.md)
- [`CAD_FINISHING_OPS.md`](./CAD_FINISHING_OPS.md)
- [`CAD_SKETCH_CONSTRAINTS.md`](./CAD_SKETCH_CONSTRAINTS.md)
- [`CAD_SKETCH_FEATURE_OPS.md`](./CAD_SKETCH_FEATURE_OPS.md)
- [`CAD_DIMENSIONS.md`](./CAD_DIMENSIONS.md)
- [`CAD_EVAL_SCHEDULER.md`](./CAD_EVAL_SCHEDULER.md)
- [`CAD_REBUILD_WORKER.md`](./CAD_REBUILD_WORKER.md)
- [`CAD_MESH.md`](./CAD_MESH.md)
- [`CAD_TESSELLATION.md`](./CAD_TESSELLATION.md)

### Analysis And Engineering Overlay

- [`CAD_ANALYSIS.md`](./CAD_ANALYSIS.md)
- [`CAD_COST_HEURISTIC.md`](./CAD_COST_HEURISTIC.md)
- [`CAD_DEFLECTION_HEURISTIC.md`](./CAD_DEFLECTION_HEURISTIC.md)
- [`CAD_ENGINEERING_OVERLAY.md`](./CAD_ENGINEERING_OVERLAY.md)
- [`CAD_RACK_TEMPLATE.md`](./CAD_RACK_TEMPLATE.md)

### AI/Chat And Intent Orchestration

- [`CAD_INTENTS.md`](./CAD_INTENTS.md)
- [`CAD_INTENT_DISPATCH.md`](./CAD_INTENT_DISPATCH.md)
- [`CAD_CHAT_ADAPTER.md`](./CAD_CHAT_ADAPTER.md)
- [`CAD_CHAT_SESSION_LIFECYCLE.md`](./CAD_CHAT_SESSION_LIFECYCLE.md)

### App Integration And UX Behavior

- [`CAD_BOOTSTRAP.md`](./CAD_BOOTSTRAP.md)
- [`CAD_INPUT_SCAFFOLD.md`](./CAD_INPUT_SCAFFOLD.md)
- [`CAD_PANE_STATE.md`](./CAD_PANE_STATE.md)
- [`CAD_OVERFLOW_INVARIANTS.md`](./CAD_OVERFLOW_INVARIANTS.md)
- [`CAD_SCRIPT_HARNESS.md`](./CAD_SCRIPT_HARNESS.md)

### Import/Export And Checker Tooling

- [`CAD_STEP_EXPORT.md`](./CAD_STEP_EXPORT.md)
- [`CAD_STEP_IMPORT.md`](./CAD_STEP_IMPORT.md)
- [`CAD_STEP_CHECKER.md`](./CAD_STEP_CHECKER.md)

### Architecture Decisions

- [`decisions/0001-kernel-strategy.md`](./decisions/0001-kernel-strategy.md)

### Kernel Strategy Spike (Narrative + Evidence)

- [`spikes/2026-03-01-kernel-strategy/README.md`](./spikes/2026-03-01-kernel-strategy/README.md)
- [`spikes/2026-03-01-kernel-strategy/boolean_failure_log.md`](./spikes/2026-03-01-kernel-strategy/boolean_failure_log.md)
- [`spikes/2026-03-01-kernel-strategy/checker_results.md`](./spikes/2026-03-01-kernel-strategy/checker_results.md)
- [`spikes/2026-03-01-kernel-strategy/memory_results.md`](./spikes/2026-03-01-kernel-strategy/memory_results.md)
- [`spikes/2026-03-01-kernel-strategy/step_outputs.md`](./spikes/2026-03-01-kernel-strategy/step_outputs.md)
- [`spikes/2026-03-01-kernel-strategy/artifacts/opencascade_check.log`](./spikes/2026-03-01-kernel-strategy/artifacts/opencascade_check.log)
- [`spikes/2026-03-01-kernel-strategy/artifacts/vcad_cli_help.log`](./spikes/2026-03-01-kernel-strategy/artifacts/vcad_cli_help.log)
- [`spikes/2026-03-01-kernel-strategy/artifacts/vcad_kernel_check.log`](./spikes/2026-03-01-kernel-strategy/artifacts/vcad_kernel_check.log)

## Architectural Throughline

The docs in this folder intentionally connect product experience to technical invariants.
The "cool demo" layer only works when the reliability layer is strict: deterministic state contracts, explicit failure handling,
no silent geometry corruption, and repeatable command/test lanes.
That is why planning docs, runbooks, validity rules, and release gates are treated as first-class implementation artifacts rather than afterthoughts.

This is also why CAD data stays structured and portable. The `.apcad` envelope, semantic references, warnings model,
and intent contracts are all designed so a model remains usable without any specific assistant runtime.
AI can accelerate editing and orchestration, but CAD authority is encoded in documents, feature graphs, and typed operations.
That keeps the system debuggable, testable, and maintainable across future UI and agent surface changes.

## Maintenance Rule

When adding, moving, or removing CAD docs in this folder, update this README in the same PR so the table of contents remains complete.
