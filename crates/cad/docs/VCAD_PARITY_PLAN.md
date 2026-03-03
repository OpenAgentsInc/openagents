# VCAD Parity Plan (100% Target)

Last updated: 2026-03-02
Owner lane: `crates/cad` + `apps/autopilot-desktop`

## 1) Goal

Reach 100% capability parity with `vcad` by implementing and validating every currently implemented `vcad` capability in OpenAgents, with deterministic behavior and release-gated quality.

Pinned baseline for this plan:
- `vcad` commit: `1b59e7948efcdb848d8dba6848785d57aa310e81`
- `openagents` starting point: `04faa5227f077c419f1c5c52ddebbb7552838fd4`

## 2) What "100% parity" means

Parity scope is split into two required tiers:

1. Core CAD parity (must hit 100%):
- Everything in `vcad` docs currently marked shipped/partial for core CAD stack, including modeling, sketch/constraints, assembly, drafting, import/export, rendering, headless API, physics/URDF paths that are implemented in code.

2. Full workspace parity (must hit 100% for true repo parity):
- Additional `vcad` workspace lanes present in codebase: CRDT/collab, ECAD stack, slicer stack, embroidery stack, CAM/stocksim.

If either tier is incomplete, parity is not complete.

## 3) Source material reviewed

Primary references reviewed from `~/code/vcad`:
- `README.md`
- `docs/features/index.md`
- `docs/features/ROADMAP.md`
- `docs/features/sketch-mode.md`
- `docs/features/sketch-operations.md`
- `docs/features/boolean-operations.md`
- `docs/features/import-export.md`
- `docs/features/assembly-joints.md`
- `docs/features/drafting-2d.md`
- `docs/features/headless-api.md`
- `docs/features/ray-tracing.md`
- `docs/features/physics-simulation.md`
- `crates/*` workspace inventory

OpenAgents references reviewed:
- `crates/cad/docs/PLAN.md`
- `crates/cad/docs/decisions/0001-kernel-strategy.md`
- `crates/cad/docs/CAD_FEATURE_OPS.md`
- `crates/cad/docs/CAD_SKETCH_CONSTRAINTS.md`
- `crates/cad/docs/CAD_SKETCH_FEATURE_OPS.md`
- `crates/cad/docs/CAD_STEP_IMPORT.md`
- `crates/cad/docs/CAD_STEP_EXPORT.md`

## 4) Current gap snapshot

OpenAgents currently has strong demo-oriented CAD scaffolding and deterministic contracts, but not full `vcad` feature depth.

Major gaps to close:
- Kernel breadth: NURBS, full booleans robustness, full finishing ops, full sweep/loft fidelity.
- Sketch/constraint depth: full constraint set and iterative solving parity.
- Assembly/joints: missing full part-instance-joint model and FK parity.
- Drafting: missing full 2D drafting/GD&T stack.
- IO/headless: missing full CLI + MCP parity surface.
- Rendering: no direct BRep ray tracing parity.
- Physics/URDF: not at parity.
- Extended workspace lanes: CRDT, ECAD, slicer, embroidery, CAM/stocksim.

## 5) Execution rules

- Issues are strictly sequential. No skipping ahead.
- Every issue must ship with deterministic tests and parity fixtures.
- Every phase ends with a parity checkpoint report.
- No feature marked "parity complete" without vcad-side fixture comparison.

## 6) Sequential issue queue

### Phase A - Program baseline (`VCAD-PARITY-001` to `VCAD-PARITY-010`)

1. [x] VCAD-PARITY-001: Freeze parity baseline manifests for pinned `vcad` commit.
2. [x] VCAD-PARITY-002: Build `vcad` capability crawler (docs + crates + commands).
3. [x] VCAD-PARITY-003: Build OpenAgents capability crawler for CAD surfaces.
4. [x] VCAD-PARITY-004: Generate machine-readable gap matrix (`vcad` vs OpenAgents).
5. [x] VCAD-PARITY-005: Define parity scorecard and pass/fail thresholds.
6. [x] VCAD-PARITY-006: Create shared parity fixture corpus and seed data.
7. [x] VCAD-PARITY-007: Add `scripts/cad/parity_check.sh` orchestration command.
8. [x] VCAD-PARITY-008: Add CI lane for parity checks and artifact upload.
9. [x] VCAD-PARITY-009: Add parity risk register and blocker workflow.
10. [x] VCAD-PARITY-010: Publish baseline parity dashboard in repo docs.

### Phase B - Kernel substrate parity (`VCAD-PARITY-011` to `VCAD-PARITY-025`)

11. [x] VCAD-PARITY-011: Finalize `cad::kernel` adapter v2 for engine pluggability.
12. [x] VCAD-PARITY-012: Integrate `vcad-kernel-math` parity types/adapters.
13. [x] VCAD-PARITY-013: Integrate `vcad-kernel-topo` parity topology model.
14. [x] VCAD-PARITY-014: Integrate `vcad-kernel-geom` parity surface types.
15. [x] VCAD-PARITY-015: Integrate `vcad-kernel-primitives` parity constructors.
16. [x] VCAD-PARITY-016: Integrate `vcad-kernel-tessellate` parity pipeline.
17. [x] VCAD-PARITY-017: Align tolerance/exact-predicate strategy to vcad behavior.
18. [x] VCAD-PARITY-018: Integrate `vcad-kernel-booleans` staged boolean pipeline.
19. [x] VCAD-PARITY-019: Map vcad boolean diagnostics into OpenAgents error model.
20. [x] VCAD-PARITY-020: Preserve BRep after booleans (remove mesh-only fallback in parity lane).
21. [x] VCAD-PARITY-021: Integrate `vcad-kernel-nurbs` parity support.
22. [x] VCAD-PARITY-022: Integrate `vcad-kernel-text` parity text geometry support.
23. [x] VCAD-PARITY-023: Integrate `vcad-kernel-fillet` parity path.
24. [x] VCAD-PARITY-024: Integrate `vcad-kernel-shell` parity path.
25. [x] VCAD-PARITY-025: Integrate `vcad-kernel-step` parity adapter path.

### Phase C - Core modeling ops parity (`VCAD-PARITY-026` to `VCAD-PARITY-040`)

26. [x] VCAD-PARITY-026: Primitive parity (cube/cylinder/sphere/cone contracts).
27. [x] VCAD-PARITY-027: Transform parity (translate/rotate/scale deterministic compose).
28. [x] VCAD-PARITY-028: Pattern parity (linear and circular pattern ops).
29. [x] VCAD-PARITY-029: Production shell op parity in feature graph.
30. [x] VCAD-PARITY-030: Production fillet parity for planar-safe paths.
31. [x] VCAD-PARITY-031: Production chamfer parity paths and diagnostics.
32. [x] VCAD-PARITY-032: Expand fillet/chamfer beyond planar-safe constraints.
33. [x] VCAD-PARITY-033: Sweep op parity (path + twist + scale controls).
34. [x] VCAD-PARITY-034: Loft op parity (multi-profile + closed options).
35. [x] VCAD-PARITY-035: Topology repair parity after boolean/finishing ops.
36. [x] VCAD-PARITY-036: Material assignment parity at part/feature level.
37. [x] VCAD-PARITY-037: `vcad-eval` parity behaviors in OpenAgents eval receipts.
38. [x] VCAD-PARITY-038: Feature-op hash parity fixtures vs vcad reference corpus.
39. [x] VCAD-PARITY-039: Modeling edge-case parity fixtures (coincident/tangent/seam).
40. [x] VCAD-PARITY-040: Core modeling parity checkpoint (must hit 100% for this phase).

### Phase D - Sketch + constraints parity (`VCAD-PARITY-041` to `VCAD-PARITY-055`)

41. [x] VCAD-PARITY-041: Expand sketch entity set (line/rect/circle/arc/spline).
42. [x] VCAD-PARITY-042: Sketch plane parity (XY/XZ/YZ + planar face selection).
43. [x] VCAD-PARITY-043: Full constraint enum parity (geometric + dimensional).
44. [x] VCAD-PARITY-044: Iterative LM solver parity (not one-pass MVP).
45. [x] VCAD-PARITY-045: Jacobian/residual pipeline parity and diagnostics.
46. [x] VCAD-PARITY-046: Under/fully/over-constrained status parity semantics.
47. [x] VCAD-PARITY-047: Extrude-from-sketch parity behaviors.
48. [x] VCAD-PARITY-048: Revolve-from-sketch parity (full + partial angles).
49. [x] VCAD-PARITY-049: Sweep-from-sketch parity.
50. [x] VCAD-PARITY-050: Loft-from-sketch parity.
51. [x] VCAD-PARITY-051: Sketch profile validity parity checks.
52. [x] VCAD-PARITY-052: Sketch interaction parity (shortcuts + editing flow).
53. [x] VCAD-PARITY-053: Sketch undo/redo parity with deterministic replay.
54. [x] VCAD-PARITY-054: Sketch fixture equivalence tests vs vcad corpus.
55. [x] VCAD-PARITY-055: Sketch/constraints parity checkpoint.

### Phase E - Assembly + joints parity (`VCAD-PARITY-056` to `VCAD-PARITY-066`)

56. [x] VCAD-PARITY-056: Add assembly schema (`PartDef`, `Instance`, `Joint`, ground ID).
57. [x] VCAD-PARITY-057: Part definition and instance parity behavior.
58. [x] VCAD-PARITY-058: Joint parity (Fixed/Revolute/Slider).
59. [x] VCAD-PARITY-059: Joint parity (Cylindrical/Ball).
60. [x] VCAD-PARITY-060: Joint limits/state parity semantics.
61. [x] VCAD-PARITY-061: Forward kinematics solver parity.
62. [x] VCAD-PARITY-062: Ground instance + deletion cleanup invariants.
63. [x] VCAD-PARITY-063: Assembly UI pane parity for selection/editing.
64. [x] VCAD-PARITY-064: Assembly serialization and replay parity.
65. [x] VCAD-PARITY-065: Assembly acceptance scenes parity fixtures.
66. [x] VCAD-PARITY-066: Assembly parity checkpoint.

### Phase F - 2D drafting parity (`VCAD-PARITY-067` to `VCAD-PARITY-078`)

67. [x] VCAD-PARITY-067: Drafting kernel scaffolding parity modules.
68. [x] VCAD-PARITY-068: Orthographic/isometric projection parity.
69. [x] VCAD-PARITY-069: Hidden-line removal parity.
70. [x] VCAD-PARITY-070: Dimension parity (linear/angular/radial/ordinate).
71. [x] VCAD-PARITY-071: GD&T annotation parity.
72. [x] VCAD-PARITY-072: Section view parity.
73. [x] VCAD-PARITY-073: Detail view parity.
74. [x] VCAD-PARITY-074: Drawing mode UI parity in desktop app.
75. [x] VCAD-PARITY-075: Drawing persistence parity in CAD document schema.
76. [x] VCAD-PARITY-076: DXF export parity.
77. [x] VCAD-PARITY-077: PDF export parity.
78. [x] VCAD-PARITY-078: Drafting parity checkpoint.

### Phase G - IO + headless + AI parity (`VCAD-PARITY-079` to `VCAD-PARITY-092`)

79. [x] VCAD-PARITY-079: STEP import parity expansion to vcad-supported entities.
80. [x] VCAD-PARITY-080: STEP export parity for post-boolean BRep results.
81. [x] VCAD-PARITY-081: STL import/export parity.
82. [x] VCAD-PARITY-082: GLB export parity.
83. [x] VCAD-PARITY-083: Build `openagents-cad-cli` command surface scaffold.
84. [x] VCAD-PARITY-084: CLI command parity (`export`, `import`, `info`).
85. [x] VCAD-PARITY-085: MCP CAD tools parity (`create`, `export`, `inspect`).
86. [x] VCAD-PARITY-086: Compact IR parser/serializer parity.
87. [x] VCAD-PARITY-087: Intent-based modeling parity execution path.
88. [x] VCAD-PARITY-088: Text-to-CAD adapter parity lane (prompt->model).
89. [x] VCAD-PARITY-089: Dataset generation and annotation parity tooling.
90. [x] VCAD-PARITY-090: Training/eval pipeline parity hooks (gated).
91. [x] VCAD-PARITY-091: Headless script harness parity with vcad workflows.
92. [x] VCAD-PARITY-092: IO/headless/AI parity checkpoint.

### Phase H - Rendering + GPU + raytrace parity (`VCAD-PARITY-093` to `VCAD-PARITY-104`)

93. [x] VCAD-PARITY-093: Viewport camera/gizmo parity baseline.
94. [x] VCAD-PARITY-094: Render mode parity (standard/wire/hidden-line variants).
95. [x] VCAD-PARITY-095: GPU acceleration lane parity in rendering backend.
96. [x] VCAD-PARITY-096: Mesh upload + processing parity contracts.
97. [x] VCAD-PARITY-097: Direct BRep raytrace module scaffolding.
98. [x] VCAD-PARITY-098: Analytic ray intersections parity set.
99. [x] VCAD-PARITY-099: Trimmed-surface ray hit parity handling.
100. [x] VCAD-PARITY-100: BVH build/traverse parity for raytrace.
101. [x] VCAD-PARITY-101: Raytrace quality mode parity (`draft/standard/high`).
102. [x] VCAD-PARITY-102: Raytrace face-pick parity.
103. [ ] VCAD-PARITY-103: Raytrace UI toggle + fallback parity.
104. [ ] VCAD-PARITY-104: Rendering/raytrace parity checkpoint.

### Phase I - Physics + URDF parity (`VCAD-PARITY-105` to `VCAD-PARITY-114`)

105. [ ] VCAD-PARITY-105: Physics crate integration parity (`rapier` lane).
106. [ ] VCAD-PARITY-106: Collision shape generation parity (primitive/convex/trimesh).
107. [ ] VCAD-PARITY-107: Convex decomposition parity lane.
108. [ ] VCAD-PARITY-108: Joint-to-physics mapping parity.
109. [ ] VCAD-PARITY-109: Simulation `step/reset` API parity.
110. [ ] VCAD-PARITY-110: Simulation UI controls parity (play/pause/step/record).
111. [ ] VCAD-PARITY-111: Gym-style API parity (`obs/action/reward/done`).
112. [ ] VCAD-PARITY-112: MCP simulation tools parity.
113. [ ] VCAD-PARITY-113: URDF import parity.
114. [ ] VCAD-PARITY-114: URDF export parity + fixtures.

### Phase J - Full workspace parity lanes (`VCAD-PARITY-115` to `VCAD-PARITY-130`)

115. [ ] VCAD-PARITY-115: CRDT lane parity architecture (`vcad-crdt` equivalent).
116. [ ] VCAD-PARITY-116: Collaborative document sync parity implementation.
117. [ ] VCAD-PARITY-117: Presence/cursor/selection collaboration parity.
118. [ ] VCAD-PARITY-118: Branch/fork workflow parity.
119. [ ] VCAD-PARITY-119: ECAD symbols lane parity.
120. [ ] VCAD-PARITY-120: ECAD schematic lane parity.
121. [ ] VCAD-PARITY-121: ECAD PCB lane parity.
122. [ ] VCAD-PARITY-122: ECAD export lane parity.
123. [ ] VCAD-PARITY-123: ECAD simulation lane parity.
124. [ ] VCAD-PARITY-124: Slicer core lane parity.
125. [ ] VCAD-PARITY-125: Slicer G-code lane parity.
126. [ ] VCAD-PARITY-126: Slicer Bambu lane parity.
127. [ ] VCAD-PARITY-127: Slicer WASM lane parity.
128. [ ] VCAD-PARITY-128: Embroidery core lane parity.
129. [ ] VCAD-PARITY-129: Embroidery DST/PES lane parity.
130. [ ] VCAD-PARITY-130: CAM/stocksim lane parity.

### Phase K - Hardening + parity signoff (`VCAD-PARITY-131` to `VCAD-PARITY-136`)

131. [ ] VCAD-PARITY-131: License/compliance parity audit across all adopted lanes.
132. [ ] VCAD-PARITY-132: Security posture parity (dependency + build script review).
133. [ ] VCAD-PARITY-133: Deterministic replay parity across all capability fixtures.
134. [ ] VCAD-PARITY-134: Performance parity scorecard reaches target thresholds.
135. [ ] VCAD-PARITY-135: Release gates and runbooks updated for parity maintenance.
136. [ ] VCAD-PARITY-136: Final parity certification report (`100%` both tiers).

## 7) Definition of done for the full plan

The plan is complete only when:
- All 136 sequential issues are closed.
- Core CAD parity score is 100%.
- Full workspace parity score is 100%.
- Determinism, performance, and release-gate checks are green.
- Final parity certification is published and reproducible from CI artifacts.
