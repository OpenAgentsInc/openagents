# Apple FM on DSE (DSPy-in-Effect): a design analysis

Date: 2026-07-20
Status: speculation / design analysis. Not dispatch authority. Not a promise.
Not a product commitment. It reads the shipped Apple Foundation Models (Apple
FM) system and the historical DSPy-in-Effect ("DSE") work, then asks one
question: how could and should the OpenAgents Apple FM system use DSE, and where
must it stop.

Audience: human.

Companions:
- `docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md` (what DSE was).
- `docs/apple-fm/2026-07-19-apple-fm-swift-bridge-full-audit-and-openagents-desktop-plan.md`
  (what Apple FM is).
- `docs/apple-fm/2026-07-20-apple-fm-analyzer-boot-sequence-audit.md`
- `docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md`

---

## 1. Thesis in one line

DSE compiles brittle hand-written prompts into typed, evaluated, versioned
programs. The Apple FM system runs a small model on free on-device inference and
depends today on brittle hand-written prompts. The free inference removes the
cost that usually blocks DSE optimization. That match makes Apple FM the
strongest DSE fit in this repository.

---

## 2. What DSE is in this repository today (honest state)

DSE is **not present as active code** on `origin/main`. The DSPy-in-Effect audit
is direct on this point: the current tree has no `packages/dse`, no active
`ds-effect` package, no active `dsrs` package, no real GEPA optimizer in an
Effect runtime, and no real MIPROv2 optimizer in an Effect runtime. The audit
records the exact deletion: commit `5afa49cdbc1520e753cfce5260328078a9098068`
(2026-02-20) removed `packages/dse` (53 files, 9,678 lines) under a Rust-only
mandate. The removal was an architecture and language pivot, not a quality
rejection. No removal commit cites a DSE test failure.

DSE therefore exists here as **design and regression-test evidence**, recoverable
from Git history, plus a small set of surviving ideas in the current Blueprint
contracts. Any use of DSE for Apple FM is new implementation, not a restore.

### 2.1 The primitives DSE had (from the audit)

The terminal `@openagentsinc/dse` package was a working, DSPy-inspired Effect
system. Its primitives were:

- **Signatures.** `DseSignature<I, O>` carried a stable signature ID, an Effect
  Schema input contract, an Effect Schema output contract, structured Prompt IR
  (System, Instruction, FewShot, ToolPolicy, OutputFormat, Context blocks),
  default parameters, and parameter constraints. A language-neutral
  `SignatureContractExportV1` emitted JSON Schema for outside inspection.
- **Module / Predict.** `Predict.make(signature)` resolved the active compiled
  artifact or the signature defaults, rendered Prompt IR deterministically,
  called the model, decoded the result with Effect Schema, ran a bounded repair
  on a first decode failure, and wrote a success or failure receipt. The generic
  Module graph runner stayed conceptual; normal Effect composition supplied
  execution flow.
- **Optimizers / compiler.** The compiler produced immutable
  `openagents.dse.compiled_artifact` artifacts from
  `openagents.dse.compile_job` jobs. The real optimizer IDs were
  `instruction_grid.v1`, `fewshot_greedy_forward.v1`,
  `joint_instruction_grid_then_fewshot_greedy_forward.v1`, `knobs_grid.v1`, and
  `knobs_grid_refine.v1`. These were deterministic bounded searches with a
  default candidate cap of 128. The package did **not** contain MIPROv2, GEPA,
  COPRO, BootstrapFewShot, a Pareto optimizer, or a Bayesian scheduler. Honest
  naming matters here: DSE was a DSPy-inspired system, not a full DSPy port.
- **Metrics / evaluation.** Ordered hashable datasets, named train/development/
  holdout splits, seeded sampling, deterministic metrics, pinned judge metrics,
  reward bundles (format validity, task metric, tool failure, evidence quote
  quality, cost), evaluation caching, and evaluation reports.
- **Examples.** Labeled example sets, imported and stored by the application
  (Convex, in the deleted app). One example signature was
  `@openagents/autopilot/blueprint/SelectTool.v1`, a real hot path.
- **Runtime controls.** Runtime-enforced budgets (max elapsed time, max LM
  calls, max tool calls, max RLM iterations, max output characters), append-only
  `openagents.dse.predict_receipt` receipts, bounded RLM-lite traces, immutable
  active-pointer and rollback controls, canary selection, and holdout-gated
  promotion.

### 2.2 The corrections the audit demands of any successor

The audit lists limits a future DSE must fix. They constrain any Apple FM use:

- The compiled ID must cover the complete artifact, not only the parameter hash.
- A missing holdout must fail; train data must never silently become holdout.
- Optimizer names must match the actual algorithm.
- The evaluator must have an explicit independence rule.
- Each candidate must bind to an immutable dataset revision.
- Cost and budget evidence must remain part of admission.
- A successor must use the current Effect v4, Node 24, pnpm, and Vite Plus
  toolchain, not the deleted Bun / Effect 3.19 / Convex / Cloudflare stack.

---

## 3. Why the Apple FM system is a strong DSE fit

Three properties of the Apple FM system line up with what DSPy optimization
wants.

### 3.1 Free inference makes optimizer rollouts affordable

DSPy-style optimization is expensive because it runs the model many times: it
proposes instruction and few-shot candidates, evaluates each candidate over a
dataset, and keeps the best. The audit records that the deleted DSE production
loop hit a compile timeout after 600 seconds and needed provider timeouts, a
circuit breaker, and a fallback model to finish one narrow SelectTool compile.
That cost came from a metered cloud provider (OpenRouter, Kimi K2.5).

Apple FM removes that cost at the point of use. The free-inference analysis
states the property plainly: a free on-device model "can run on every keystroke,
every save, every test run… speculatively and redundantly, with nothing leaving
the machine." A DSE optimizer that evaluates candidates against the same free
on-device model can afford many rollouts at zero marginal token cost. This is the
exact regime DSPy optimization was built for, and it is the regime the deleted
DSE loop could not reach on a paid provider.

An honest caveat: free is not instant. The free-inference analysis notes that
"free inference costs wall-clock." So the Apple FM DSE cost model is
"many rollouts, zero token spend, bounded wall-clock" — expensive optimization
becomes a time budget, not a money budget.

### 3.2 The Apple FM system relies on brittle hand-written prompts today

The Apple FM system today carries several hand-tuned prompts. Each is exactly the
artifact DSE is meant to compile and evaluate instead of hand-write.

- **The honesty preamble.** `buildOpenAgentsAppleFmPrompt` in
  `apps/openagents-desktop/src/renderer/shell.ts` prepends a long hand-written
  "strict preamble" (lines 2247-2255). Its own comment records the cause: "the
  on-device model must not lie about actions or capabilities… This is the only
  honesty control we have over a small local model — keep it strict." The
  preamble forbids the model from claiming it dispatched agents, set reminders,
  ran commands, or edited files. It exists because the ~3B model was
  hallucinating actions.
- **The boot-analyzer prompt.** The BOOT SEQUENCE audit proposes an environment
  analyzer whose whole output quality depends on one hand-written prompt over a
  bounded environment slice, plus an optional bounded read loop.
- **The truncation preamble.** The same `buildOpenAgentsAppleFmPrompt` hand-packs
  a preamble plus the most recent turns into a 3,900-character budget, dropping
  the oldest turns first. Context selection under a hard cap is a tuning problem.

Hand-written prompts are unversioned, unevaluated, and adjusted by intuition. DSE
replaces each with a typed signature, a compiled artifact, a metric, and a
receipt. The honesty preamble is the clearest case: it is a single string a human
edits by feel, with no dataset and no measured before/after.

### 3.3 A small model gains more from optimized prompting than a frontier model

The free-inference analysis is explicit that Apple FM is "a small model" (~3B
class), "not a frontier coder," with "small context." A frontier model often
follows a loose instruction well; a small model is more sensitive to instruction
wording, few-shot choice, output format, and context packing. DSPy's premise —
that optimized prompting and decomposition raise a weaker model's reliability —
applies most strongly here. The gain DSE can extract is larger on Apple FM than
on Codex or Claude.

---

## 4. Concrete DSE applications to Apple FM

Each application is a signature and module, a metric, and an example source. Each
stays advisory, local, and non-authority, per Section 5.

### 4.1 Honesty / no-false-capability behavior (tie to the hallucination bug)

- **Signature.** `AppleFm/HonestChatReply.v1`: input is the bounded flattened
  conversation (the same notes `buildOpenAgentsAppleFmPrompt` receives); output
  is a schema-typed reply plus a typed `claimed_actions` list.
- **Module.** `Predict.make` renders the compiled instruction and few-shot set,
  calls Apple FM, and decodes the reply.
- **Metric.** A deterministic penalty when the output claims an action the model
  cannot take (dispatched a subagent, set a reminder, ran a command, edited a
  file, remembered across chats). The Apple FM boundary already proves these
  claims are false by construction: the host runs one bounded, read-only turn
  with no tools, so any first-person action claim is a hallucination. The metric
  scores the frequency of such claims plus an answer-quality judge.
- **Examples.** Mine real hallucination cases: prompts that historically induced
  false action claims (the exact behavior the strict preamble was written
  against), plus honest-refusal exemplars. The metric can be seeded from the
  forbidden-claim list already enumerated in the preamble comment.
- **Payoff.** Replace the hand-written strict preamble with a compiled program
  whose instruction and few-shot set are optimized against a measured
  false-claim rate. This converts "keep it strict, by feel" into "measured versus
  the hand-written preamble." This is the strongest first slice.

### 4.2 The BOOT SEQUENCE environment analyzer

- **Signature.** `AppleFm/EnvironmentSummary.v1`: input is a bounded environment
  slice that main already holds (path frontier, lockfiles, `package.json`
  scripts, `CLAUDE.md`, git status); output is a small set of typed summary
  lines (stack, monorepo shape, green gate, git state).
- **Module.** A single bounded prompt first; later, the bounded read/explore loop
  the BOOT SEQUENCE audit describes (a model proposes one allowlisted read per
  round, main executes it, repeat for 3-5 rounds).
- **Metric.** Grounded accuracy against the **deterministic fingerprint**. Main
  can compute the true stack, package managers, and workspace count directly, so
  the metric penalizes any model summary that disagrees with the deterministic
  fact. The BOOT SEQUENCE audit already fixes this rule: "prefer the
  deterministic fingerprint for facts, use the model for phrasing." DSE turns
  that rule into a scored metric.
- **Examples.** Snapshots of real repositories (this one, and small fixtures)
  paired with their deterministic fingerprints as ground truth.
- **Payoff.** A compiled analyzer whose phrasing is optimized for accuracy over
  the deterministic ground truth, not tuned by hand.

### 4.3 The free triage / router (answer-locally versus delegate)

- **Signature.** `AppleFm/TriageRoute.v1`: input is the bounded request plus the
  available-lane readiness facts; output is a typed route (`answer_local` or
  `delegate`) and, when delegating, a tightened context selection.
- **Module.** A bounded local turn in front of the metered agents, as the
  free-inference analysis describes: decide small-enough-to-answer-locally versus
  delegate, and rank the context to send.
- **Metric.** Correct routing plus cost. Reward a correct `answer_local` when the
  local answer was accepted; reward a correct `delegate` when the task genuinely
  needed a frontier agent; penalize a wrong local answer (worse than delegating)
  and penalize an unnecessary delegation (a needless cloud call). Cost is a
  first-class term, matching DSE's existing cost reward bundle.
- **Examples.** Labeled tasks tagged by their correct destination, drawn from
  real session history where the eventual resolver (local or cloud) is known.
- **Payoff.** The router is the pattern OpenAgents already built and deleted (the
  November 2025 `FMOrchestrator` / "concurrent delegations to Codex/Claude Code"
  path). DSE makes the routing decision a measured, compiled policy rather than a
  hand-tuned threshold. It is also the lane that grows the on-device share of the
  workload (episode 194's chronicle), so its metric directly measures the product
  goal.

### 4.4 Prompt truncation / context selection as an optimizable module

- **Signature.** `AppleFm/ContextPack.v1`: input is the full conversation plus
  the hard character budget; output is the selected, ordered context.
- **Module.** Today `buildOpenAgentsAppleFmPrompt` uses a fixed rule (keep newest
  turns, drop oldest first, hard-truncate). Model this as a compiled selection
  policy over the budget.
- **Metric.** Downstream answer quality under the fixed budget: does the selected
  context produce a better reply than the naive newest-first rule? The budget cap
  is the constraint; the metric is answer quality within it.
- **Examples.** Conversations longer than the budget, paired with a
  quality-judged target answer.
- **Payoff.** Context packing under a hard cap is a tuning problem DSE can
  optimize instead of a human choosing a heuristic.

### 4.5 Structured outputs via `@Generable` as DSE typed signatures

- **Growth path, not shippable today.** The shipped desktop bridge (v0.1.1) does
  plain-text completion only; the desktop plan and free-inference analysis both
  record that `@Generable` structured generation, tool callbacks, and streaming
  exist in the **mature** psionic bridge in Git history but not in the shipped
  subset. So this application is a growth path.
- **Signature.** When the structured bridge lands, every DSE signature's Effect
  Schema output maps to a `@Generable` / `GeneratedContent` schema-guided decode,
  which the psionic bridge already mapped Rust-side to `schemars::JsonSchema`.
- **Payoff.** DSE's existing "decode with Effect Schema, bounded repair on
  failure" flow becomes native structured decode rather than plain-text parsing.
  This raises decode reliability on a small model, where free-form output is
  least reliable. The DSE typed-signature contract and the `@Generable` schema
  are the same idea expressed once.

---

## 5. Hard constraints DSE must respect here

Any DSE use must inherit the Apple FM trust fence, not weaken it. These
constraints come directly from the shipped contract and the two Apple FM
analyses.

1. **Advisory only.** The free-inference analysis names the fence: a local model
   "lives entirely in the private advisory plane… never sets acceptance, never
   mints evidence, never decides delivery state." A DSE-compiled Apple FM program
   is still advisory. Its output never becomes authority.
2. **Main-owned boundary.** `apps/openagents-desktop/src/apple-fm-contract.ts` is
   the only renderer-visible surface. The renderer never learns the bridge path,
   loopback URL, tokens, workspace paths, tool arguments, file contents, or a raw
   transcript. A DSE program runs behind that same boundary. It adds no new
   renderer surface beyond bounded, schema-validated projections.
3. **Local-only, no upload.** Nothing leaves the machine. This is the strategic
   inverse of Cursor's remote-embedding upload. A DSE optimizer that runs against
   Apple FM must also run locally: the candidate search, the evaluation rollouts,
   and the dataset all stay on-device or in the repository. No repo bytes, no
   prompts, and no summaries go to a network endpoint.
4. **Non-determinism never becomes authority.** The BOOT SEQUENCE audit is
   explicit: "Anything a test or gate depends on must come from the deterministic
   environment facts, not from the model's wording." A DSE metric may score model
   prose, but no gate consumes the prose. The deterministic fingerprint, the host
   oracle, and the exact-preimage Git checks remain the sole authorities.
5. **Honest metrics and honest usage truth.** The Apple FM contract types
   `usageTruth` as `exact | estimated | unknown`, and the shipped bridge reports
   `estimated` (character-count usage). A DSE evaluation over Apple FM must record
   the same honest usage truth and must not synthesize an exact public claim. The
   DSE audit's correction — a missing holdout must fail, train must not become
   holdout, cost evidence must remain in admission — applies unchanged.

These constraints reconcile cleanly with DSE's own governance model. DSE already
separated candidate generation from admission and release, already enforced
runtime budgets, and already refused implicit self-promotion. The Apple FM fence
is the same posture at the product boundary.

---

## 6. Where optimization runs and where the compiled artifact lives

DSE's own architecture already splits offline compilation from online serving.
The DSPy-in-Effect audit's recommended split is: Effect owns signatures, Prompt
IR, policy identity, budgets, receipts, evidence, canaries, and release gates; an
offline optimizer owns expensive candidate search; the optimizer output stays
untrusted until an independent evaluation admits it. Apple FM fits that split
well.

- **Offline compile.** The DSE optimizer runs offline — during development or in
  a bounded local job — against the free on-device Apple FM model. It searches
  instruction and few-shot candidates, evaluates each over the dataset, and emits
  one immutable compiled artifact (a compiled instruction and few-shot set for
  the signature). Free inference is what makes running the optimizer affordable:
  the many rollouts cost wall-clock, not tokens.
- **On-device serve.** At runtime the desktop app serves the **checked-in
  compiled artifact**. The compiled prompt and few-shot set are a repository
  artifact, not a runtime dependency on an optimizer server. This matters
  doubly here: the whole Apple FM value is local-only and offline-capable, so a
  serve-time dependency on a remote optimizer would break the thesis. The
  compiled artifact ships in the app, the same way the strict preamble ships as a
  string today — but versioned, evaluated, and receipted.
- **Cost model.** For a paid model, the optimizer's rollout count is the binding
  cost, which is why the deleted DSE loop hit provider timeouts on one narrow
  compile. For Apple FM, the rollout count is free at the token level, so the
  optimizer can afford a wider search; the binding cost becomes local wall-clock
  and the honest requirement that the holdout evaluation stay independent.

---

## 7. Risks and open questions

- **Model capability ceiling.** A ~3B model gives a shallower read than a
  frontier model. DSE optimization raises reliability; it does not lift the
  ceiling. Keep claims modest and measure before widening scope.
- **Free is wall-clock, not zero.** A wide offline search still costs time.
  Bound the candidate cap (DSE defaulted to 128) and the wall-clock budget.
- **Holdout independence on one machine.** The DSE audit's sharpest correction is
  that a missing holdout must fail and train must not become holdout. A local,
  single-machine optimizer makes it easy to reuse data. The independence rule
  must be mechanical, not a convention.
- **Non-determinism of the optimizer itself.** The compiled artifact must be
  reproducible enough to review. Bind each candidate to an immutable dataset
  revision and cover the complete artifact in its ID, per the audit corrections.
- **Coverage.** Apple FM exists only on Apple Silicon with macOS 26. A DSE-
  compiled Apple FM program is a pure enhancement where present and must silently
  do nothing elsewhere. An Ollama or other-runtime fallback is a later, separate
  decision.
- **Structured generation is not shipped.** Application 4.5 depends on the
  `@Generable` bridge growth path. Until it lands, DSE signatures over Apple FM
  decode plain text with a bounded repair, exactly as the deleted DSE did.
- **DSE is new code here.** There is no `packages/dse` to restore. A successor
  must be built on the current Effect v4 / Node 24 / pnpm / Vite Plus toolchain.
  This is a build, not a cherry-pick.

---

## 8. Recommended first slice

Ship the smallest honest version first, behind the existing Apple FM gate:

1. **DSE-compile the honesty behavior (Section 4.1).** Define
   `AppleFm/HonestChatReply.v1` as an Effect Schema signature. Write the
   false-claim metric from the forbidden-action list the strict preamble already
   enumerates. Assemble a small dataset from real hallucination cases and honest-
   refusal exemplars. Run a bounded offline instruction-and-few-shot search
   against the free on-device model. Emit one immutable compiled artifact.
2. **Measure versus the hand-written preamble.** Compare the compiled program's
   false-claim rate and answer quality against the current strict preamble on a
   held-out set. Keep the honest usage truth (`estimated`). Report the delta.
3. **Only if it wins, adopt the compiled artifact** in
   `buildOpenAgentsAppleFmPrompt`, replacing the hand-written string with the
   checked-in compiled instruction and few-shot set. Keep the deterministic
   boundary (no tools, one bounded read-only turn) unchanged.

This proves the core claim — that DSE compiles a brittle hand-written prompt into
a measured, versioned artifact — on the exact prompt whose comment already admits
it is tuned by feel, at zero token cost, with no new authority and no new
renderer surface.

---

## 9. References

DSE (what it was and its corrections):
- `docs/dspy/2026-07-20-dspy-in-effect-git-history-audit.md`.
- Terminal DSE package deletion: commit
  `5afa49cdbc1520e753cfce5260328078a9098068` (2026-02-20).
- Serving-app deletion: commit `388473626c439a804568a461cbbbd21078f99492`.
- First DSE specification: commit
  `beebeb672e72ebf56b750526757bf7f24382e1e2` (docs/autopilot/ds-effect.md).
- Effect-native scaffold: commit
  `544aafa4b79dd6bc409146ab01ca2201305cbed3` (packages/dse, docs/autopilot/dse.md).
- Recorded production compile/promotion: commit
  `cbca11a03758c930d8aea4f4528d860bca088940`.
- Later hybrid audit (offline Python optimizer, online Effect gates): commit
  `b76fa980233e5853f1de862ee191900b4556ad73`.

Apple FM (what it is and its trust fence):
- `docs/apple-fm/2026-07-19-apple-fm-swift-bridge-full-audit-and-openagents-desktop-plan.md`.
- `docs/apple-fm/2026-07-20-apple-fm-analyzer-boot-sequence-audit.md`.
- `docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md`.

Code (the boundary and the hand-written prompts):
- `apps/openagents-desktop/src/renderer/shell.ts` — `buildOpenAgentsAppleFmPrompt`
  and the strict honesty preamble (lines ~2238-2269), the OpenAgents-authority
  submission branch (lines ~3004-3011).
- `apps/openagents-desktop/src/apple-fm-contract.ts` — the bounded, main-owned
  renderer IPC contract and honest `usageTruth` vocabulary.
- `apps/openagents-desktop/src/apple-fm-host.ts` — the main-owned supervisor and
  the single bounded read-only `runTurn`.
</content>
</invoke>
