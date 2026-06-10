# Tassadar And Percepta Audit

Date: 2026-06-10

Status: full history audit of Tassadar across the `openagents` monorepo and
the `psionic` repo commit histories, including all Percepta-related
material, and how the lane relates to Psion, Psionic, Pylon, and Autopilot.

Disclosure posture: this audit follows the rules in psionic's
`docs/TASSADAR_PUBLIC_DISCLOSURE_FLOW.md` — public names only, claims
bounded to committed evidence, dependency markers preserved, refusal
language kept. The live Tassadar roadmap is private (the psionic repo's own
public bridge doc says so); this audit cites only what the two public repos
already record.

## What Tassadar Is

Tassadar is the **executor-capable bounded Psion profile** inside Psionic:
a transformer whose weights are compiled and/or trained so that it
**executes programs exactly inside its own inference loop** — every
instruction fetch and memory read is an attention operation, with no
external interpreter and no tool use. The transformer is the computer.

The naming rule (psionic `docs/PSION_EXECUTOR_PROGRAM.md`, the
`PSION-0001`/`#700` contract) keeps the lanes from being flattened:

- `Psion` is the umbrella learned-model family inside `psionic`.
- The generic compact-decoder `Psion` is a separate learned lane with its
  own route, refusal, serving, and training contracts (and is *not*
  exactness-claiming).
- `Tassadar` names the executor-capable bounded `Psion` profile and route
  family — currently the bounded article-transformer route and artifact
  family.

The name sits in the StarCraft Protoss family with Psionic, Psion, Pylon,
and Artanis.

## The Percepta Lineage

Percepta is the research origin. Their post "Can LLMs Be Computers?"
(percepta.ai) claims transformers can execute arbitrary programs via 2D
convex-hull ("parabolic") attention with O(log t) per-step decoding. The
workspace tracks the public independent validation,
`projects/repos/llm-as-computer` — a compiled transformer executor
implementing a 55-opcode stack-machine ISA modeled on WebAssembly's i32
subset, with opcode dispatch in the feed-forward layers and memory
addressing in attention heads (their benchmarks: 1.2M steps in 17ms on the
Mojo backend). Its companion essay "The Free Computer" makes the economic
argument: offloading exact computation into compiled weights makes the
execution effectively free relative to ordinary attention-driven token
generation.

The public OpenAgents framing is on the record in two transcripts:

- Episode 216 (`docs/transcripts/216.md`): "Psion is also going to be an
  executor model. We've more or less reproduced the Percepta paper...
  CPU compute added to the weights of models," alongside the Psion naming
  reveal and the pause-markets-to-focus-on-training decision.
- Episode 220 (`docs/transcripts/220.md`): "we've talked about this
  Percepta post. There's some pretty advanced shit that we can do that the
  other labs wouldn't be able to do" — Percepta-class execution named as a
  differentiator for the small-specialized-models thesis.

Psionic's committed Percepta record is
`docs/PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md` (`PSION-0705`/`#774`,
2026-03-30): one typed bounded closeout-status packet binding workload
truth, fast-path truth, and route-replacement truth. Retained truth at that
record:

- canonical model: `tassadar-article-transformer-trace-bound-trained-v0`
- canonical route: `tassadar.article_route.direct_hull_cache_runtime.v1`
- bounded closeout status: `green_bounded` (workload, fast-path, and
  route-replacement truths each `green`; the executor-style research branch
  explicitly `research_only`)
- retained `HullKVCache` fast path: ≥1.69× over the reference-linear
  baseline, with a ≤2.55× remaining gap versus direct CPU reference
- limitations kept explicit, starting with `arbitrary_c_or_wasm_not_claimed`

That is the honest shape of the "reproduced Percepta" claim: a bounded,
digest-pinned article-workload closeout — not arbitrary-program execution.

## History In The `openagents` Repo

Tassadar was **born in this repo**. In March 2026, psionic lived in-tree at
`crates/psionic/`, and the first Tassadar commits land here:

- `0363dff6c` (2026-03-16) "psionic: add Tassadar sequence dataset
  contracts" — `psionic-data/src/tassadar.rs` (597 lines) plus eval
  surfaces; the lane's first commit.
- The first experimental ladder follows over days: executor transformer
  family (`32534efa0`), next-token training and eval (`749ab9be3`), neural
  linear decode benchmark (`9664719ed`), phase-7 reference run persistence
  (`3090ccf70`), run telemetry (`371528388`), a first-run postmortem
  (`0f4cb16f0`), neural hull decode (`5826f0f4b`), a 9x9 (Sudoku) scale
  plan (`039d6db3e`), phase 12 boundary curriculum (`c4f732b87`), phase 13
  trainable-surface ablation (`55bb4e1a7`), executor-attention comparison
  lane (`b559ba580`), phase 14 promotion tooling (`9fc9b98e7`), compiled
  executor bundle (`7fb1983bc`), Hungarian-matching executor bundle
  (`52eced170`), fine-grained progress logging (`5ac5697f3`), promotion v2
  bundle (`ae059cc2f`), and a candid sequence of training-limit evidence:
  attention boundary training improvements (`cc18e7de4`), step-index
  boundary blocker diagnosis (`e8e076ddd`), transition-conditioned boundary
  adapter (`c13cf35a6`), joint-adapter plateau (`50c8fdb86`), and adapter
  saturation evidence (`fa80adcf8`). The arc is visible in the commit
  titles alone: try to *train* exact execution, hit the plateau honestly,
  and pivot weight production toward trace-bound/compiled routes.
- **Autopilot Tassadar Lab** (2026-03-17 → 03-25): the desktop app grew a
  replay-first Tassadar lab pane (`1cafa430e`), live lab sessions
  (`2ee586a51`), finished controls and persistence (`eefb356e7`), a widened
  run explorer (`77174ad7c`), and deferred pane loading (`446ec5d3c`, PR
  #4008) — an operator UI for replaying and inspecting executor runs inside
  Autopilot. That UI lived in `apps/autopilot-desktop` (later
  `apps/deprecated/autopilot-deprecated`) and was removed with everything
  else in the Bun rebuild (`f5919c766`).

When psionic was extracted to its own repo, the Tassadar lane moved with
it. The current monorepo footprint is references only: transcripts 203
(first on-air mention, 2026-03-17 era), 216, and 220; and two doc trails —
the workspace benchmark-systems audit
(`apps/pylon/docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`)
lists "Tassadar article, Sudoku, plugin conformance, universality, and
compiled weight eval reports" among the benchmark systems, and the Apple FM
first-backend audit (`packages/probe/docs/2026-06-07-apple-fm-first-backend-audit.md`)
records that "the Tassadar Apple FM plugin-session pilot proved a useful
controller pattern" that Probe's Apple FM backend then reused, citing the
psionic plugin-session audit and
`psionic-apple-fm/src/tassadar_post_article_starter_plugin_tools.rs`.

## History And Current State In `psionic`

The psionic repo is the implementation home: ~231 Tassadar-titled commits
and 20+ `docs/TASSADAR_*` documents, plus the `PSION_EXECUTOR_*` family.
The landed structure, per `docs/ARCHITECTURE.md` ("Tassadar Executor-Class
Lane") and `docs/ROADMAP_TASSADAR_INDEX.md`:

**The phase ladder (1 → 7D, all landed with committed artifacts):**

1. CPU reference fixture + exact parity harness.
2. Digest-bound program artifacts + model/program compatibility contracts.
3. Typed environment bundle + package-driven exactness benchmark suite
   (CPU and reference-linear baselines).
4. Emitted trace artifacts, runtime-manifest lineage, proof-bundle
   integration for replay-stable executor evidence.
5. `HullCache` fast-path decode with exact CPU/reference-linear/hull-cache
   equivalence on the validated acyclic subset and **typed refusal** for
   workloads outside it.
6. Machine-legible runtime capability reports + decode-selection
   diagnostics (direct / fallback / refused).
7. A/B/C/D: a served `psionic.executor_trace` product in `psionic-serve`
   (typed contracts, pull-driven trace streaming, typed refusals, served
   evidence bundles) plus the specialized `psionic.article_executor_session`
   surface; the widened `core_i32_v2` Wasm profile with article-class
   benchmark coverage (`MicroWasmKernel`, `BranchHeavyKernel`,
   `MemoryHeavyKernel`, `LongLoopKernel`, `SudokuClass`,
   `HungarianMatching`) at exact parity; a frozen long-horizon trace
   ABI/versioning decision; and a machine-readable workload capability
   matrix that keeps runtime-exact, fallback-only, compiled-exact, bounded
   learned, and partial learned-long-horizon postures separate per
   workload family.

**The universality program:** TCM.v1 substrate model declaration
(`ac174ceb`), universal-machine proof targets (`b1a033f7`), a universality
witness suite (`04ea60bd`), a minimal universal substrate gate
(`694878e9`), a universality verdict *split* (`a189aae7`), a
Turing-completeness closeout audit (`92e97913`), and an
article-equivalence blocker matrix (`56d83ac2`) — the claim machinery that
keeps "this substrate is universal in principle" separate from "this
served route executes these committed workloads exactly."

**The Wasm lane:** a frozen core-Wasm lane with a declared semantic window,
committed closure gate, public acceptance gate, and operator runbook — with
the current closure and public-acceptance verdicts **suppressed**
(`served_publication_allowed = false`), plus a bounded scalar-`f32`
semantics matrix (canonical quiet-NaN normalization, ordered comparisons,
CPU-reference-only posture, explicit refusal on `f64`, NaN-payload
preservation, and non-CPU fast-math).

**The plugin system:** post-article plugin manifests, packet ABI, receipts,
world-mount contracts; an operator-curated starter-plugin catalog and
runtime with one registry deriving identity/schemas/refusals/replay-class/
capability posture; an authoring contract; the Apple FM plugin session
(the weighted-controller admission pattern Probe later reused); and a
hardening tranche — platform threat model, audit invariants, anti-drift
stability, control-plane proof, machine-closure bundle.

**Training:** `./TRAIN_TASSADAR` is the frozen default train lane producing
`tassadar-article-transformer-trace-bound-trained-v0` (bounded
article-weight production), with a trained-v1 replacement report and
ablation records. The serving posture remains
`tassadar.internal_compute.article_closeout.v1` — benchmarked, bounded
internal computation under named profiles with explicit refusal surfaces.

## How Tassadar Relates To Everything Else

- **Psion**: Tassadar is one profile of the Psion umbrella. The generic
  compact-decoder Psion (the `./TRAIN` pretraining lane, the Qwen work, the
  epic-3 training issues) is deliberately *not* exactness-claiming;
  Tassadar is where exactness lives, and `PSION_EXECUTOR_PROGRAM.md` is the
  wall between them.
- **Psionic**: the implementation home across `psionic-data` / `-models` /
  `-train` / `-eval` / `-serve` / `-provider` / `-runtime` / `-apple-fm`,
  with fixtures under `fixtures/tassadar/`.
- **Pylon**: the connection is the capability-envelope pattern.
  `psionic-provider` wraps the executor capability publication into a
  provider-facing `TassadarCapabilityEnvelope` and a workload
  capability-frontier receipt — the same shape Pylon's GEPA capability
  envelope follows, and the published route for a future Pylon to advertise
  executor-class capacity without overclaiming. The shared
  benchmark-systems audit already treats Tassadar eval reports as one lane
  of the same benchmark/evidence system Pylon and Probe use.
- **Probe**: consumed the Tassadar Apple FM plugin-session controller
  pattern for its Apple FM backend — the first concrete cross-lane reuse.
- **Autopilot**: hosted the (now-removed) Tassadar Lab replay UI; any
  future operator surface would be rebuilt on the current product stack.
- **alpha**: the live roadmap, tranche definitions, and terminal-contract
  language live privately; psionic's `docs/ROADMAP_TASSADAR.md` is the
  public bridge and is deliberately subordinate to it.
- **The network economy (forward-looking, bounded):** executor work is the
  **most verifiable workload class we have**. Tassadar runs are
  deterministic, digest-pinned, trace-replayable, and already emit proof
  bundles — which makes them ideal commit-and-challenge homework for the
  verification layer described in the CS336 continuation audit
  (`docs/2026-06-10-cs336-distributed-homework-continuation-audit.md`):
  validators replay traces exactly instead of needing Freivalds-style
  probabilistic checks. The "Free Computer" economics also slot into the
  compute-revenue story: exact compiled execution is a sellable compute
  product whose verification cost is near zero. None of this is claimed as
  live — it is the natural continuation seam.

## Registry And Disclosure Posture

There is **no product promise for Tassadar** in the public registry, and
that appears deliberate: the psionic disclosure flow gates any widening of
public claims, and the current Wasm-lane public-acceptance verdicts are
suppressed (`served_publication_allowed = false`). Episodes 216/220 made
bounded public statements ("more or less reproduced the Percepta paper",
"executor model"), and the psionic repo's committed evidence supports
exactly that bounded phrasing. If Tassadar ever becomes user-facing copy or
a registry promise, the path is: disclosure-flow review → a promise record
whose safeCopy mirrors the capability matrix's per-workload postures and
whose unsafeCopy forbids arbitrary-program and universal-claims copy — the
same scope-language discipline the Qwen fine-tune gate uses.

## What A Continuation Would Look Like (Not Filed)

1. Re-attach an operator replay/inspection surface (the old Tassadar Lab's
   job) on the current stack, reading served evidence bundles.
2. Executor-trace homework: dispatch bounded article-class workloads to
   Pylons through the epic-3 connector, verified by exact trace replay —
   the cheapest verification class in the whole homework program.
3. A `TassadarCapabilityEnvelope` consumer in Pylon's capability reporting,
   so executor-class capacity is advertised with the same no-overclaim
   posture as GEPA.
4. A registry promise via the disclosure flow, only when publication
   suppression lifts.

All four are sequenced behind the epic-3 connector (#4664/#4669) and the
disclosure flow's own gates.

## Evidence Reviewed

- `openagents` git history: `0363dff6c` (first Tassadar commit,
  `crates/psionic/psionic-data/src/tassadar.rs`), the March 2026
  experimental ladder (`32534efa0`, `749ab9be3`, `9664719ed`, `3090ccf70`,
  `371528388`, `0f4cb16f0`, `5826f0f4b`, `039d6db3e`, `c4f732b87`,
  `55bb4e1a7`, `b559ba580`, `9fc9b98e7`, `7fb1983bc`, `52eced170`,
  `ae059cc2f`, `cc18e7de4`, `e8e076ddd`, `c13cf35a6`, `50c8fdb86`,
  `fa80adcf8`), the Autopilot Tassadar Lab series (`1cafa430e`,
  `2ee586a51`, `eefb356e7`, `77174ad7c`, `446ec5d3c`/PR #4008),
  `f5919c766` (removal of the desktop lab with the rebuild)
- `openagents` tree: `docs/transcripts/203.md`, `216.md`, `220.md`;
  `apps/pylon/docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`;
  `packages/probe/docs/2026-06-07-apple-fm-first-backend-audit.md`
- `psionic` (231 Tassadar-titled commits; 20+ docs):
  `docs/PSION_EXECUTOR_PROGRAM.md`, `docs/PSION_PROGRAM_MAP.md`,
  `docs/ARCHITECTURE.md` (executor-class lane),
  `docs/ROADMAP_TASSADAR.md` (bridge), `docs/ROADMAP_TASSADAR_INDEX.md`,
  `docs/PSION_EXECUTOR_PERCEPTA_CLOSEOUT_STATUS.md`,
  `docs/TASSADAR_DEFAULT_TRAIN_LANE.md`,
  `docs/TASSADAR_STARTER_PLUGIN_RUNTIME.md`,
  `docs/TASSADAR_PUBLIC_DISCLOSURE_FLOW.md`, plus the universality/TCM.v1
  commit series (`ac174ceb`, `b1a033f7`, `04ea60bd`, `694878e9`,
  `a189aae7`, `92e97913`, `56d83ac2`) and the plugin/hardening series
  (`49a1d193` → `fcd3cd0a`)
- `projects/repos/llm-as-computer` (public Percepta validation: README,
  ISA, benchmark refs)
