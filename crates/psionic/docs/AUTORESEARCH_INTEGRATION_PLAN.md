# Rust-Native Research Loop Plan For OpenAgents

> Status: updated 2026-03-09 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `crates/psionic/docs/ROADMAP.md`,
> `crates/psionic/docs/CONFORMANCE_AND_EVIDENCE_CONTRACT.md`,
> `docs/pylon/PYLON_SANDBOX_CONTRACT.md`,
> `crates/psionic/psionic-runtime/src/lib.rs`,
> `crates/psionic/psionic-provider/src/lib.rs`,
> `crates/psionic/psionic-sandbox/src/lib.rs`,
> `crates/openagents-provider-substrate/src/sandbox.rs`, and the local
> `~/code/autoresearch` tree as inspiration only.

## Why This Doc Exists

`autoresearch` is useful as a pattern, not as the architecture to copy.

The good idea is:

- fixed-budget experiments
- a measurable score
- keep/discard frontier advancement
- autonomous controller policy

The wrong part for OpenAgents is:

- Python as the runtime source of truth
- a mutable `train.py`
- training infrastructure that lives outside the Psionic stack

If OpenAgents builds this, it should be Rust-native and Psionic-native.

## Non-Negotiable Direction

The target system is:

- no `train.py`
- no Python trainer as the architectural core
- no external repo as the execution substrate
- no pretending open-ended research is just another `inference` job

The loop should be split into three layers:

1. `Autopilot` owns the research controller and frontier policy.
2. `Psionic` owns reusable training, evaluation, artifact, and runtime truth.
3. `sandbox_execution` owns bounded execution and receipts for each run step.

That keeps the responsibilities honest and aligned with `docs/OWNERSHIP.md`.

## Short Conclusion

OpenAgents should implement an autoresearch-style loop as a Rust system that:

- uses Autopilot as the loop controller
- uses new `crates/psionic/*` training/eval crates for the actual model work
- uses bounded `sandbox_execution` jobs to run compiled Rust runners under
  explicit profiles
- promotes winning artifacts back through `psionic-catalog`, `psionic-models`,
  `psionic-serve`, and `psionic-provider`

The controller is labor.
The bounded run steps are compute.
The promoted artifact is Psionic truth.

## What The Loop Should Actually Be

The research loop should look like this:

1. The controller chooses the next candidate.
2. The controller serializes a typed experiment spec.
3. A bounded Rust runner executes that spec for a fixed budget.
4. The runner emits metrics, logs, checkpoints, and artifact digests.
5. The controller decides keep, discard, or branch.
6. Winning artifacts are re-evaluated through the served Psionic path before
   promotion.

That is the same spirit as `autoresearch`, but the implementation is Rust all
the way down.

## Controller Versus Compute Boundary

This repo already has the right conceptual boundary.

`docs/pylon/PYLON_SANDBOX_CONTRACT.md` is explicit:

- `run this bounded thing in this declared sandbox` is Compute
- `figure out what to do and do it` is Labor

That means:

- choosing hypotheses
- deciding what source to mutate
- interpreting results
- selecting the next move

belong to the Autopilot controller, not to the compute-market execution unit.

The remote or local bounded run should only do declared work such as:

- compile this candidate
- train for 300 seconds
- evaluate on this held-out suite
- emit these declared artifacts

## Rust-Native Architecture

### App-owned controller

`apps/autopilot-desktop` should own:

- run creation
- frontier state
- keep/discard logic
- branch and workspace policy
- operator controls
- experiment summaries and UI

This is product behavior and should not live in reusable Psionic crates.

### New Psionic-owned research crates

If we want this loop to be real and Rust-native, we should extend Psionic with new
reusable crates rather than bolt a Python subsystem onto the side.

Recommended new crates:

- `psionic-train`
  - training graph execution
  - optimizer state
  - checkpoint I/O
  - fixed-budget trainer loop
- `psionic-eval`
  - benchmark suites
  - held-out evaluation
  - fixed scoring contracts
  - replay-safe result summaries
- `psionic-research`
  - experiment spec types
  - candidate mutation descriptors
  - run manifests
  - frontier and promotion records
- `psionic-data` or `psionic-datasets`
  - dataset manifests
  - tokenizer/dataset digests
  - split declarations
  - local blob-backed training corpora

These crates would be reusable engine substrate, so they fit `crates/psionic/*`
better than app crates do.

### Existing Psionic crates stay in the loop

The current Psionic crates already cover the downstream half:

- `psionic-catalog` for blob identity
- `psionic-models` for model metadata and artifact governance
- `psionic-serve` for actual served evaluation
- `psionic-provider` for capability and receipt truth
- `psionic-runtime` for cache, topology, and bounded execution evidence

The new research crates should feed into those crates, not bypass them.

## The Recommended Mutation Surface

The first version should not let the controller mutate arbitrary Rust files
across the whole repo.

The safer and more replayable path is:

### Phase A: typed spec mutation

The controller mutates typed experiment inputs such as:

- model family
- layer count
- hidden size
- head count
- optimizer family
- learning rate schedule
- batch sizing
- rope / context settings
- checkpoint cadence
- evaluation suite selection

This is the fastest path to a Rust-native loop that is still bounded and
receiptable.

### Phase B: bounded Rust lab crate mutation

If we want architectural search beyond config mutation, constrain it to a
dedicated subtree such as:

- `crates/psionic-lab/*`
- `crates/psionic-research-kernels/*`

Then the controller can propose source patches against that bounded lab area,
compile them, and run them under the same fixed-budget experiment contract.

Do not make "the whole repo is mutable research state" the default design.

## Bounded Execution Shape

The execution unit should be a compiled Rust runner, not a Python script.

Examples:

- `sandbox.posix.exec`
  - `cargo run -p psionic-research-runner -- --spec spec.json`
- `sandbox.container.exec`
  - run a pinned OCI image containing `psionic-research-runner`

The runner should accept a typed spec and emit typed outputs.

This fits the existing substrate in:

- [psionic-sandbox/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-sandbox/src/lib.rs)
- [sandbox.rs](/Users/christopherdavid/code/openagents/crates/openagents-provider-substrate/src/sandbox.rs)
- [psionic-runtime/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-runtime/src/lib.rs)
- [psionic-provider/src/lib.rs](/Users/christopherdavid/code/openagents/crates/psionic/psionic-provider/src/lib.rs)

Important consequence:

- the execution substrate does not choose the experiment
- it only executes the declared experiment

## Proposed Run Contract

Each experiment run should have a typed identity at least as strong as current
`sandbox_execution` evidence.

Minimum fields:

- `experiment_id`
- `base_artifact_digest`
- `dataset_manifest_digest`
- `tokenizer_digest`
- `candidate_mutation_digest`
- `runner_binary_digest`
- `sandbox_profile_digest`
- `budget_ms`
- `requested_backend`
- `requested_visible_devices`

Minimum outputs:

- `score`
- `score_name`
- `wall_time_ms`
- `peak_memory_bytes`
- `stdout_sha256`
- `stderr_sha256`
- `checkpoint_artifact_digests`
- `promoted_artifact_digest` when applicable
- `training_receipt_digest`
- `evaluation_receipt_digest`

This keeps the loop replay-safe and comparable.

## Promotion Path

The winner should not be trusted just because the training runner said so.

Promotion should require a second pass through the shipped Psionic surfaces:

1. artifact is registered through `psionic-catalog`
2. metadata and governance are attached in `psionic-models`
3. served identity is computed in `psionic-serve`
4. capability / supply policy / receipt truth is computed in `psionic-provider`
5. the promoted model is evaluated through the same served path the product
   would actually expose

That uses existing Psionic work correctly:

- `PSI-162` served-artifact identity
- `PSI-163` cache invalidation
- `PSI-164` provenance and license gating
- `PSI-171` through `PSI-175` capability and receipt evidence

## Local And Remote Modes

### Local-first mode

The first honest mode is local:

- Autopilot controller runs on the desktop
- the Rust runner executes in a local bounded sandbox
- artifacts remain local
- promotion targets the local Psionic runtime

This fits the MVP much better than jumping straight to a network market.

### Remote scaling mode

Later, the same bounded Rust runner can scale over provider infrastructure.

But the remote job must stay compute-shaped:

- execute this declared experiment spec
- under this sandbox profile
- on this GPU class
- for this fixed budget
- with these expected output paths

Not:

- "go improve the model"
- "decide what to try next"
- "operate the branch until it gets better"

Those are controller responsibilities.

## Ownership Map

| Concern | Right owner | Notes |
| --- | --- | --- |
| Research policy, frontier advancement, workspace state | `apps/autopilot-desktop` | Product workflow |
| Typed experiment specs and run manifests | new `crates/psionic/*` research crate | Reusable substrate |
| Training loop, checkpointing, optimizer state | `psionic-train` plus `psionic-ir` | Reusable engine work |
| Held-out evaluation suites and score contracts | `psionic-eval` plus `psionic-environments` | Reusable truth |
| Bounded execution profile and receipts | `psionic-runtime` + `psionic-provider` + provider substrate | Already shaped for this |
| Artifact identity and promotion | `psionic-catalog` + `psionic-models` + `psionic-serve` | Existing Psionic path |
| Market advertisement of promoted artifacts | `psionic-provider` | Only after supply policy allows it |

## Main Gaps

### 1. Psionic is still product-inference-first today

The repo now has real inference, embeddings, serving, provider evidence, and
CUDA/NVIDIA execution, and it now also has an early Rust-native training stack.

What it still does not have is a complete research-loop product stack.

### 2. Autodiff and optimizer substrate now exist, but the research loop is still incomplete

To make the research loop real in Rust, Psionic needed training primitives, not
just serve-time kernels. Those primitives are now real:

- reusable reverse-mode autodiff and explicit detach or no-grad semantics in
  `psionic-ir`
- reusable optimizer families in `psionic-train`
- fixed-budget trainer scheduling and checkpoint-aware step execution
- model IO, dataset contracts, and held-out eval crates

What is still missing for the broader research loop is:

- broader autodiff or operator coverage across future backend extensions
- production multi-device or distributed trainer execution
- research-run orchestration, keep or discard policy, and promotion flow
- long-run hardening around checkpoint, eval, and artifact promotion

### 3. Adapter policy is still conservative

`PSI-139` explicitly refuses adapter-bearing Ollama manifests at the replacement
boundary.

So the first Rust-native research loop should not assume "LoRA everywhere" is
already a solved promotion path. The first honest output is a Psionic-native
artifact contract, not a hand-waved adapter import story.

### 4. In-process Psionic serving is not the trainer crash boundary

`PSI-160` made the serving isolation policy explicit: current Psionic local serving
is `in_process`.

That is fine for the current inference path. It is not the right failure domain
for long-running training or architecture search. The research runner should be
executed outside the serving process, under sandbox controls.

### 5. MVP market scope is still narrower than the research controller

The current MVP is about truthful paid compute and real sats, not about shipping
an overnight Rust research swarm as a public provider product.

So the controller loop should be local-first even if the run steps later become
remote-executable.

## Recommended Build Sequence

1. Define typed Rust experiment specs and result manifests.
2. Add a Rust research runner binary that executes one fixed-budget experiment.
3. Run that binary under the existing bounded sandbox substrate.
4. Add Autopilot frontier logic and keep/discard controls in the app layer.
5. Add Psionic-native checkpoint and promotion contracts.
6. Add served-path re-evaluation before any promoted artifact becomes "real."
7. Only later allow bounded remote execution of the same runner.
8. Only after that, decide whether bounded Rust source-patch mutation is worth
   enabling beyond typed spec mutation.

## Bottom Line

OpenAgents should not copy `autoresearch` as "a repo with `train.py` that an
agent edits."

OpenAgents should copy the loop shape and rebuild it in Rust:

- Autopilot controls the search
- Psionic owns training, evaluation, artifacts, and served truth
- sandbox execution runs bounded compiled Rust jobs
- the provider stack only ever sees declared compute work, not open-ended
  research labor
