
# Psionic ARC Solver Spec
Status: draft for coding agents
Date: 2026-03-15
Audience: coding agents and human maintainers working in `crates/psionic/*`, `docs/*`, and kernel-facing eval infrastructure.

## 0. Why this exists

This spec defines a concrete solver program for ARC-AGI that fits the current Psionic and OpenAgents architecture.

It answers a narrower question than the full Psionic roadmap:

> what should the first serious Psionic-native ARC solver actually be?

This is **not** a general “make Psionic good at everything” plan.
It is a bounded solver spec for ARC-AGI-1 and ARC-AGI-2, with explicit seams that prepare the codebase for ARC-AGI-3.

The main research takeaway behind this spec is simple:

- ARC-AGI-2 is deliberately designed to expose gaps in **symbolic interpretation**, **compositional reasoning**, and **contextual rule application**, while also stressing **efficiency**.
- ARC Prize 2024 and 2025 both show that progress is coming from **hybrid systems**: induction/program synthesis, transduction/direct prediction, test-time training, evolutionary refinement, recursive small models, and MDL/compression-style adaptation.
- ARC Prize 2025 explicitly identifies the **refinement loop** as the central pattern.
- ARC-AGI-3 shifts the benchmark into **interactive reasoning**, where action efficiency, replay, planning, exploration, and memory become first-class.

Therefore the Psionic solver must be built around:

1. **object-centric representations**
2. **program-like hypotheses**
3. **task-local refinement**
4. **portfolio solving across multiple solver lanes**
5. **strict budgeted verification and replay**
6. **trace capture for learning and audit**

## 1. Architectural fit and boundaries

This spec must obey existing repo boundaries.

### 1.1 Psionic owns the solver runtime
Psionic owns:

- task parsing and canonicalization
- object extraction
- DSL / hypothesis IR
- search / refinement control
- local adaptation
- portfolio arbitration
- replayable traces
- training/eval clients for ARC-specific models
- local and clustered search execution
- acceptance artifacts and eval reports

### 1.2 Kernel does not own the hot loop
The OpenAgents kernel must **not** sit in the cognitive inner loop.

Kernel/Nexus may own:

- experiment registration
- run receipts
- accepted-outcome projections
- score publication
- benchmark evidence bundles
- public/internal stats
- later marketplace or verification projections

Kernel must **not** own:

- per-candidate search steps
- per-branch verifier logic
- hypothesis generation
- object extraction
- task-local adaptation state

### 1.3 Apps do not own solver semantics
`apps/*` may expose operators, dashboards, or CLI control, but solver truth lives in Psionic crates.

### 1.4 Framework rule
ARC semantics belong **above** tensor/compiler/runtime substrate, not inside `psionic-core`, `psionic-ir`, or `psionic-runtime`.

## 2. Solver thesis

The solver should be a **hybrid portfolio system**, not a single monolithic model.

The solver must combine four families of capability:

### 2.1 Induction / program synthesis
Infer an explicit latent transformation program from demonstration pairs.

Use this for:
- exact computations
- multi-step composition
- controllable reasoning
- hypothesis debugging
- high-confidence generalization from few examples

### 2.2 Transduction / direct output prediction
Predict output grids directly from task context and test input.

Use this for:
- perceptual or fuzzy concepts
- pattern completion
- weakly symbolic visual regularities
- proposals to seed or rescue induction

### 2.3 Recursive / iterative refinement
Repeatedly improve a candidate answer or program using a verifier-derived feedback signal.

Use this for:
- answer repair
- branch pruning
- iterative latent updates
- masked/diffusion-style denoising
- self-correction loops
- test-time training / weight-space adaptation

### 2.4 Compression / simplicity pressure
Prefer explanations that are short, stable, and falsifiable.

Use this for:
- candidate ranking
- search pruning
- distinguishing brittle pattern fits from actual rules
- MDL-style adaptation lanes

## 3. Design goals

The solver must satisfy all of the following.

### 3.1 Correctness
For a solved task, the final selected answer must exactly match the required output grid for all test inputs.

### 3.2 Efficiency
The solver must track and optimize:
- wall-clock time per task
- candidate count per task
- verifier calls per task
- train-pair executions per candidate
- adaptation steps per task
- total compute cost proxy

### 3.3 Replay
Every final answer must be reproducible from:
- task input
- solver version
- lane versions
- seeds
- budgets
- artifacts
- trace bundle

### 3.4 Solver diversity
The system must support at least three materially distinct solving lanes in v1:
- symbolic induction lane
- transductive lane
- recursive/MDL lane

### 3.5 Verifier-first operation
No lane may publish a final answer without passing through the common candidate verification pipeline.

### 3.6 Benchmark hygiene
The public evaluation split must never be used as iterative score feedback during development. Internal development must use:
- synthetic tasks
- train-derived validation
- author-created hidden holdout
- trace-based regression sets

### 3.7 Future compatibility
The solver must expose state and traces in a way that can later be reused for ARC-AGI-3 interactive agents.

## 4. Explicit non-goals

This spec does **not** aim to:

- solve ARC using only a general-purpose frontier API model
- turn Psionic into a Python-first ARC lab
- claim that one neural architecture is the final answer
- hide search behind opaque prompting
- move kernel authority logic into Psionic
- prioritize PyTorch-credible breadth over ARC-specific progress
- optimize first for cloud-scale model serving
- productize ARC market flows before the solver is real
- tune directly against hidden ARC Prize test sets

## 5. Core system requirements

## 5.1 Required solver behavior

The solver must:

1. ingest raw ARC JSON tasks
2. build canonical grid/object/task representations
3. generate candidate hypotheses through multiple lanes
4. score candidates against demonstration pairs with a common verifier
5. run iterative refinement under a strict budget
6. arbitrate across lanes
7. emit final answers plus trace bundle
8. expose acceptance artifacts and regression fixtures

## 5.2 Required task-local budget controls

Every solve attempt must bind a `TaskBudget` with at least:

- `max_wall_ms`
- `max_candidates`
- `max_verifier_evals`
- `max_train_pair_execs`
- `max_refinement_steps`
- `max_model_forward_calls`
- `max_ttt_updates`
- `max_memory_mb`

Typed refusal must occur when budget is exceeded.

## 5.3 Required output contract

Every solve attempt must produce a `SolveResult`:

- `status`: `solved | unsolved | budget_exhausted | refused | invalid`
- `task_id`
- `attempt_index`
- `selected_answer`
- `selected_lane`
- `confidence`
- `verification_summary`
- `budget_summary`
- `trace_digest`
- `trace_locator`
- `seed_bundle`
- `solver_manifest`

## 6. Proposed crate shape

Start with one new crate:

- `crates/psionic/psionic-arc`

Do **not** oversplit on day one. Keep the ARC semantics in one visible crate first, with modules:

- `arc_task`
- `grid`
- `canonicalize`
- `objects`
- `relations`
- `dsl`
- `search`
- `verify`
- `lanes`
- `arbiter`
- `trace`
- `budget`
- `synthetic`
- `eval`

If module weight becomes real, split later into:
- `psionic-arc-core`
- `psionic-arc-search`
- `psionic-arc-eval`

Only split after acceptance artifacts exist.

## 7. Domain model

## 7.1 Fundamental task types

```rust
pub struct ArcTask {
    pub task_id: String,
    pub benchmark: ArcBenchmark,      // ArcAgi1 | ArcAgi2 | InternalSynthetic | InternalHoldout
    pub train: Vec<ArcExamplePair>,
    pub test: Vec<ArcTestInput>,
}

pub struct ArcExamplePair {
    pub input: ArcGrid,
    pub output: ArcGrid,
}

pub struct ArcTestInput {
    pub index: usize,
    pub input: ArcGrid,
}
```

```rust
pub struct ArcGrid {
    pub height: u8,
    pub width: u8,
    pub cells: Vec<u8>,               // row-major, values 0..=9
}
```

## 7.2 Canonicalized task state

```rust
pub struct CanonicalTask {
    pub raw: ArcTask,
    pub normalized_train: Vec<CanonicalPair>,
    pub object_views: Vec<ObjectView>,
    pub relation_views: Vec<RelationGraph>,
    pub global_features: TaskFeatureBundle,
}
```

### Canonicalization must include
- optional color renaming normalization
- grid padding / bounds metadata
- connected-component extraction
- hole detection
- symmetry signatures
- bounding-box inventory
- repetition and alignment signatures
- train/test dimension summaries
- object correspondence candidates between train input and train output

## 7.3 Object model

```rust
pub struct ArcObject {
    pub object_id: ObjectId,
    pub color_histogram: SmallVec<[u8; 10]>,
    pub bbox: Rect,
    pub mask: BitGrid,
    pub holes: u8,
    pub connectivity: ConnectivityKind,
    pub centroid: (f32, f32),
    pub shape_signature: ShapeSignature,
}
```

```rust
pub struct RelationGraph {
    pub objects: Vec<ArcObject>,
    pub edges: Vec<ObjectRelation>,
}
```

`ObjectRelation` must represent:
- adjacency
- overlap
- containment
- touch
- distance ordering
- row/column alignment
- symmetry pairing
- count relations
- color mapping
- source/target correspondence hypotheses

## 7.4 Hypothesis model

The solver must represent explicit hypotheses.

```rust
pub struct Hypothesis {
    pub hypothesis_id: HypothesisId,
    pub lane: SolverLaneId,
    pub program: Option<ArcProgram>,
    pub latent_answer: Option<ArcGrid>,
    pub latent_state: Option<OpaqueLatentState>,
    pub evidence: HypothesisEvidence,
    pub score: HypothesisScore,
    pub status: HypothesisStatus,
}
```

Where:
- symbolic lanes will usually fill `program`
- transductive lanes will usually fill `latent_answer`
- recursive lanes may use `latent_state` + `latent_answer`
- mixed lanes may use all three

## 8. ARC DSL / program IR

The solver must define an explicit ARC program IR.
This IR is the heart of the induction lane.

## 8.1 DSL principles

The IR must be:

- typed
- small
- inspectable
- serializable
- executable deterministically
- expressive enough for common ARC transformations
- stable under tracing and replay

It must **not** be Python as the primary IR.
Python may exist as an experimental export/debug backend, not as canonical solver truth.

## 8.2 Required IR capability families

### A. Selectors
- select all objects
- select by color
- select by size / count / holes / connectivity
- select by relation
- select extreme object(s): topmost, leftmost, largest, smallest, etc.
- select by contextual predicate

### B. Constructors
- empty grid
- copy input grid
- crop bbox / crop region
- pad / resize
- paint rectangle / line / mask
- paste object / transform object
- tile / repeat / stack / align

### C. Object transforms
- translate
- reflect
- rotate
- recolor
- scale by integer factor
- fill holes
- extract outline / interior
- merge / split
- order by relation and place

### D. Structural ops
- connected components
- flood fill
- count
- grouping / partition
- frame / border detection
- symmetry detection
- correspondence inference

### E. Control flow
- map over selected objects
- filter
- if/else
- reduce/fold
- ordered composition
- bounded loop over object list
- recurrence over ordered placements
- contextual gating

### F. Symbol binding
- define a task-local symbol meaning from demonstrations
- bind symbol/value lookup tables
- use symbol semantics in downstream transforms

This is required because ARC-AGI-2 explicitly stresses in-context symbol definition and contextual rule application.

## 8.3 IR shape

```rust
pub enum ArcExpr {
    Grid(GridExpr),
    Objects(ObjectExpr),
    Scalar(ScalarExpr),
    Bool(BoolExpr),
    Color(ColorExpr),
    ProgramCall { op: OpId, args: Vec<ArcExpr> },
    If { cond: Box<ArcExpr>, then_branch: Box<ArcExpr>, else_branch: Box<ArcExpr> },
    Let { name: Symbol, value: Box<ArcExpr>, body: Box<ArcExpr> },
}
```

```rust
pub struct ArcProgram {
    pub inputs: ArcProgramInputs,
    pub body: ArcExpr,
    pub metadata: ArcProgramMetadata,
}
```

## 8.4 Required interpreter properties

The interpreter must be:

- pure
- deterministic
- side-effect free
- total over supported inputs
- explicitly refusing on unsupported semantics
- traceable per node

No hidden fallback behavior is allowed.

## 9. Solver lanes

The system must support multiple lanes with a common interface.

```rust
pub trait SolverLane {
    fn id(&self) -> SolverLaneId;
    fn propose(
        &self,
        task: &CanonicalTask,
        budget: &TaskBudget,
        ctx: &LaneContext,
    ) -> LaneProposalBatch;

    fn refine(
        &self,
        task: &CanonicalTask,
        budget: &TaskBudget,
        ctx: &LaneContext,
        prior: &[Hypothesis],
    ) -> LaneProposalBatch;
}
```

## 9.1 Lane A: Symbolic induction / program search

### Purpose
Search explicit programs over the ARC DSL.

### Required behavior
- generate candidate programs
- execute them on train pairs
- rank by fit + simplicity + stability
- repair near-miss programs
- carry full execution traces

### Search strategies allowed
- beam search
- best-first search
- MCTS
- evolutionary search
- DreamCoder-style abstraction library growth
- sketch completion
- branch-and-bound
- typed enumerative search

### Required repair operators
- replace sub-expression
- insert control-flow gate
- specialize selector
- swap transform primitive
- add correspondence binding
- add symbol table
- repair output sizing step

### Required ranking
Candidate program ranking must consider:
- exact train-pair fit
- residual error count
- program size / description length
- execution stability under canonical augmentations
- partial agreement across train examples
- structural plausibility
- runtime cost

## 9.2 Lane B: Transductive neural lane

### Purpose
Predict output grids directly.

### Candidate architectures
Allowed:
- 2D-aware transformer
- masked diffusion grid model
- seq2seq with 2D positional encoding
- latent denoising model
- other spatially aware neural decoders

### Required behavior
- consume multi-example task context
- propose one or more output grids
- support test-time adaptation
- support iterative self-refinement
- expose answer probability or calibrated score
- emit internal uncertainty

### Mandatory properties
- 2D-aware positional treatment
- exact output shape prediction path
- support for multiple train examples
- support for augmentation consistency scoring
- support for recursive refinement over previous guesses

## 9.3 Lane C: Recursive tiny-model lane

### Purpose
Run small task-local recursive solvers in the style of HRM/TRM-like systems, but with outer-loop refinement treated as first-class.

### Required behavior
- initialize from tiny pretrained checkpoint or from scratch
- maintain latent state and answer state separately
- apply bounded recursive improvement steps
- support halt/continue scoring
- allow optional test-time updates

### Why this lane exists
Official ARC work in 2025 showed that:
- outer-loop refinement is driving much of the gain in small recursive systems
- recursive tiny models can be surprisingly competitive
- extremely small parameter budgets can still matter on ARC

### Constraint
This lane is not allowed to become a magic box. It must expose:
- step count
- intermediate answer snapshots
- halt decisions
- validation summaries

## 9.4 Lane D: MDL / compression lane

### Purpose
Use compression or description-length minimization as a solver and scorer.

### Required behavior
- optimize a compact task-local model or code representation
- score candidates by compression of the demonstrations plus solution
- optionally run from random initialization
- allow no-pretraining mode

### Use cases
- rescue tasks where large pretrained priors are unhelpful
- bias toward minimal explanations
- provide independent signal for arbiter decisions
- expose alternative search dynamics

## 9.5 Lane E: Learned search-guide lane

### Purpose
Guide the symbolic induction lane rather than directly replace it.

### Required outputs
- branch prior over DSL expansions
- value estimate for partial programs
- repair suggestion distribution
- output-shape prior
- object correspondence prior

### This lane is mandatory in v2, optional in v1
The 2024 official ARC report explicitly identifies specialist deep learning models that guide branching decisions in discrete search as a promising next step. Build the interface now even if the model arrives later.

## 10. Common verifier and falsifier

The solver must have one common verifier.

```rust
pub trait CandidateVerifier {
    fn evaluate(
        &self,
        task: &CanonicalTask,
        hypothesis: &Hypothesis,
    ) -> VerificationReport;
}
```

## 10.1 Verifier duties

The verifier must:
- run candidate program or answer on all train pairs
- measure exact fit
- compute per-pair residuals
- detect invalid grids / invalid dimensions
- compute invariance/stability checks
- compute simplicity/compression features
- identify likely spurious fits

## 10.2 Required falsification checks

For each candidate, where budget allows, run:

1. **augmentation stability**
   - color permutation invariance where semantically valid
   - geometric canonicalization checks where valid
   - permutation of example ordering

2. **counterexample pressure**
   - synthesize alternative train-example views from inferred program
   - check for contradictions between derived latent rules and observed data

3. **holdout-on-train**
   - fit on subset of train pairs and validate on held pair(s) when tasks have >=3 demonstrations

4. **minimality pressure**
   - shorter consistent explanations outrank longer brittle ones

5. **cross-lane agreement**
   - if induction and transduction agree exactly, raise confidence
   - if they disagree but one is more stable/simple, arbitrate explicitly

## 10.3 Verification report

```rust
pub struct VerificationReport {
    pub exact_fit: bool,
    pub pair_reports: Vec<PairVerification>,
    pub simplicity: f32,
    pub stability: f32,
    pub residual_score: f32,
    pub confidence: f32,
    pub failure_modes: Vec<VerificationFailure>,
}
```

## 11. Portfolio arbiter

ARC evidence strongly suggests that different method families solve different task types. Therefore v1 must not pick one family and throw the others away.

```rust
pub trait PortfolioArbiter {
    fn select(
        &self,
        task: &CanonicalTask,
        candidates: &[ScoredCandidate],
        budget: &TaskBudget,
    ) -> ArbiterDecision;
}
```

## 11.1 Arbiter inputs
The arbiter must consider:
- exact verifier fit
- simplicity
- stability
- lane reliability priors
- compute spent so far
- diversity of candidate explanations
- historical calibration of each lane on internal holdout slices

## 11.2 Arbiter outputs
- final selected answer
- ranked fallback answers
- lane votes
- disagreement summary
- reasoned confidence
- whether a second-attempt candidate should differ materially from the first

## 11.3 Attempt policy
ARC evaluation allows multiple attempts. The system must not waste them on near-duplicates.

Attempt 2 must only be emitted if:
- it is materially distinct from attempt 1, and
- it survived verifier/falsifier checks sufficiently to justify the extra attempt.

## 12. Search and refinement control loop

## 12.1 Top-level algorithm

```text
1. Parse task JSON
2. Canonicalize grids and build object/relation views
3. Compute task signatures and cheap priors
4. Launch proposal batches from all enabled lanes
5. Verify all candidates on train pairs
6. Rank and prune
7. Run refinement cycles:
   a. symbolic repair
   b. transductive self-refinement
   c. recursive latent updates
   d. MDL optimization
   e. learned branch-guided search
8. Re-verify
9. Run arbiter
10. Emit attempt 1
11. Optionally emit materially distinct attempt 2
12. Persist full trace bundle
```

## 12.2 Required refinement loop semantics

A refinement loop must contain:
- `proposal`
- `verification`
- `feedback`
- `mutation/update`
- `re-evaluation`

This loop must be a first-class typed trace, not an accidental pattern hidden in model prompts.

## 12.3 Branch management
Search control must support:
- branch ids
- parent links
- mutation provenance
- pruning reason
- verifier summary
- budget attribution

## 13. Synthetic data and trace learning

The winning ARC systems increasingly rely on synthetic data, task augmentation, and learning from search traces.

## 13.1 Synthetic data program
The solver program must include a synthetic task generator pipeline with at least three sources:

1. **programmatic synthetic tasks**
   - generated from ARC DSL programs
2. **augmentation-derived tasks**
   - transformed versions of train tasks where allowed
3. **trace-derived tasks**
   - hard negatives, near-miss corrections, and repaired hypotheses turned into training examples

## 13.2 Required synthetic task metadata
Every synthetic task must record:
- generator family
- seed
- source program or transformation
- difficulty heuristics
- concept tags
- whether it is train-only or eval-only

## 13.3 Trace learning
All search/refinement attempts must be optionally convertible into supervised or preference-style data for:
- branch prior training
- repair operator training
- lane calibration
- candidate reranking
- recursive solver training
- verifier training

## 13.4 No direct benchmark leakage
Trace learning must never incorporate public evaluation ground truth into train-time improvements unless running an explicit frozen final evaluation report.

## 14. Evaluation program

## 14.1 Required evaluation splits

Maintain four internal evaluation classes:

1. `arc1_train_dev`
2. `arc2_train_dev`
3. `internal_hidden_holdout`
4. `synthetic_regression`

Never use the official public evaluation split as online tuning feedback in the same iteration loop.

## 14.2 Required reported metrics

For every solver run, report:
- pass@1
- pass@2
- cost/task proxy
- wall time/task
- candidates/task
- verifier calls/task
- refinement steps/task
- lane win share
- exact-fit-before-refinement share
- exact-fit-after-refinement share
- symbolic / transductive / recursive / MDL contribution share

## 14.3 Required task-slice metrics

Tag and report performance by:
- symbolic interpretation
- compositional reasoning
- contextual rule application
- object-heavy
- symmetry-heavy
- counting-heavy
- simulation / sequential
- output-shape-hard
- sparse vs dense grid
- number of train pairs

## 14.4 Acceptance artifact rule

No solver milestone is complete without at least one of:
- capability matrix
- regression fixture set
- replay bundle set
- evaluation report
- trace corpus summary

## 15. ARC-AGI-3 preparation seam

The v1 solver targets ARC-AGI-1/2 static tasks, but must prepare for ARC-AGI-3.

## 15.1 What to preserve now
The system must already expose:
- state snapshots
- action-like traces
- replay logs
- budget/accounting
- memory state hooks
- branch/hypothesis history
- verifier feedback loops

## 15.2 What not to do yet
Do not prematurely turn the static ARC solver into a generic game agent.

The correct seam is:
- reuse perception/objectization
- reuse hypothesis/verification/refinement infrastructure
- replace final-output selection with action selection later

## 16. Integration with existing Psionic subsystems

## 16.1 `psionic-data`
Use for:
- task storage
- synthetic dataset manifests
- train/holdout split manifests
- tokenizer metadata for neural lanes if needed

## 16.2 `psionic-train`
Use for:
- small recursive model training
- transductive lane training
- learned branch prior/value models
- test-time adaptation kernels
- trace-derived supervision pipelines

Do not force full train-system breadth before ARC-specific lanes exist.

## 16.3 `psionic-eval`
Use for:
- eval-run orchestration
- aggregate reports
- regression packs
- holdout verification
- lane calibration summaries

## 16.4 `psionic-sandbox`
Use for:
- isolated symbolic search execution
- clusterable branch evaluation workers
- deterministic solver batch jobs
- trace replay

## 16.5 `psionic-runtime`
Use for:
- receipts and execution evidence
- not for ARC semantics themselves

## 17. Milestones

## M0: parser + canonical objects
Deliver:
- task parser
- grid canonicalizer
- connected component extraction
- relation graph
- replay fixtures

Acceptance:
- 100% parse success on ARC-AGI-1 and ARC-AGI-2 public tasks
- deterministic object extraction fixtures for 100 sampled tasks

## M1: DSL + interpreter
Deliver:
- typed ARC DSL
- deterministic interpreter
- basic selectors/transforms/control flow
- execution traces

Acceptance:
- hand-authored programs solving 50+ known tasks
- interpreter replay fixtures green

## M2: symbolic search lane
Deliver:
- enumerative/beam search
- simple repair operators
- common verifier integration

Acceptance:
- first non-trivial solved set on internal dev
- trace bundles for all solved tasks

## M3: transductive lane
Deliver:
- 2D-aware neural lane
- output-shape head
- augmentation stability scoring
- recursive answer refinement

Acceptance:
- measurable lift over symbolic-only on fuzzy/perceptual slices

## M4: arbiter + portfolio
Deliver:
- common candidate pool
- arbiter
- attempt policy
- per-slice lane analytics

Acceptance:
- portfolio beats best single lane on internal hidden holdout

## M5: recursive tiny-model lane
Deliver:
- task-local recursive model
- halt/continue
- intermediate answer trace
- optional test-time updates

Acceptance:
- competitive performance on a defined slice with small parameter budget

## M6: learned search-guide lane
Deliver:
- branch prior
- partial-program value model
- repair suggestion model

Acceptance:
- search efficiency improvement at fixed accuracy or accuracy lift at fixed budget

## M7: MDL lane
Deliver:
- compression-based scorer or solver
- no-pretraining option
- simplicity-aware reranking

Acceptance:
- solves a distinct subset of tasks not already solved by main lanes

## M8: ARC-AGI-3 seam
Deliver:
- replayable state/action trace schema
- memory hooks
- interactive eval adapter skeleton

Acceptance:
- static solver traces can be transformed into action-style trace format without schema redesign

## 18. Initial coding-agent backlog

### Phase 1: foundations
1. Create `psionic-arc` crate
2. Add `ArcGrid`, `ArcTask`, JSON parser
3. Add canonicalization and deterministic hash
4. Add connected components, holes, bbox, symmetry signatures
5. Add relation graph
6. Add fixture pack for parser/object extraction

### Phase 2: DSL
7. Define typed DSL enums and serializer
8. Implement pure interpreter
9. Add traceable execution nodes
10. Add typed refusal taxonomy

### Phase 3: symbolic lane
11. Implement baseline enumerative search
12. Implement beam search over DSL expansions
13. Add repair operators
14. Add common verifier
15. Add simplicity/stability scoring

### Phase 4: portfolio scaffolding
16. Define lane interface
17. Define candidate pool
18. Define arbiter interface
19. Add trace bundle writer
20. Add budget controller

### Phase 5: neural/transductive lane
21. Add 2D-aware model input formatter
22. Add output-shape predictor
23. Add recursive answer refinement loop
24. Add augmentation-consistency scorer
25. Add training/eval hooks through `psionic-train`

### Phase 6: recursive and MDL lanes
26. Add recursive tiny model runner
27. Add halt/continue scoring
28. Add MDL scorer / task-local compression lane
29. Add cross-lane agreement features

### Phase 7: search-guide learning
30. Add search trace schema
31. Add branch prior training dataset generator
32. Add value model training path
33. Integrate branch/value guidance into symbolic search

### Phase 8: evaluation
34. Add internal hidden holdout support
35. Add slice tags and per-slice reporting
36. Add pass@1/pass@2/cost metrics
37. Add regression dashboard artifacts

## 19. Acceptance matrices

## 19.1 Must be green before claiming “real ARC solver”
- parser/object extraction fixtures
- deterministic DSL execution
- common verifier
- symbolic lane solves non-trivial set
- at least one neural or recursive lane
- portfolio beats any single lane on hidden internal holdout
- full trace and replay bundle generation
- benchmark hygiene documented and enforced

## 19.2 Must be green before claiming “toward Chollet-style solver”
- task-local refinement loop is first-class
- induction and transduction both present
- explicit symbolic binding/contextual rule handling exists
- exact budget accounting exists
- simplicity/falsification pressure exists
- search-guide learning or recursive refinement exists

## 20. The single most important rule

Do not let broad framework work masquerade as ARC progress.

Only the following count as direct ARC progress:
- object extraction quality
- DSL expressivity and interpreter truth
- search efficiency
- refinement quality
- verifier/falsifier strength
- portfolio arbitration
- hidden-holdout accuracy at fixed budget
- replayable trace quality

Everything else is support work.

## 21. Bottom line

The first serious Psionic ARC solver should be:

- **object-centric**
- **program-centric**
- **refinement-centric**
- **portfolio-based**
- **budgeted**
- **replayable**

That means:
- a typed ARC DSL,
- explicit object and relation extraction,
- a symbolic induction lane,
- a transductive lane,
- a recursive small-model lane,
- an MDL/compression lane,
- one shared verifier/falsifier,
- and one portfolio arbiter.

That is the smallest honest architecture that matches what official ARC research is pointing toward while still fitting Psionic’s crate boundaries and OpenAgents’ authority model.

## 22. Research basis used for this spec

This spec is grounded in the following sources:

- François Chollet, *On the Measure of Intelligence* (2019)
- ARC Prize 2024 Technical Report
- ARC Prize 2025 Technical Report
- ARC-AGI-2 Technical Report
- ARC-AGI-2 official benchmark page
- ARC Prize leaderboard and competition pages
- ARC-AGI-3 docs and preview writeups
- Li et al., *Combining Induction and Transduction for Abstract Reasoning*
- Pourcel et al., *Self-Improving Language Models for Evolutionary Program Synthesis*
- Jolicoeur-Martineau, *Less is More: Recursive Reasoning with Tiny Networks*
- Liao and Gu, *ARC-AGI Without Pretraining*
- official NVARC and ARChitects solution summaries
