# DSPy / RLM Python backend vs Effect reimplementation — decision audit

Date: 2026-06-28
Status: research / decision record (public-safe; no secrets, no deploy)

## Question

Should we run a **Python backend that directly uses the upstream DSPy and RLM
libraries** (and GEPA, the DSPy optimizer) checked out under
`projects/repos/`, instead of reimplementing those systems in our
Effect/TypeScript stack — or some hybrid?

## Headline recommendation

**Hybrid (option c), with a strict tier boundary.**

- **Adopt the real Python libraries for the OFFLINE optimization/compile tier.**
  Use upstream **GEPA** (`projects/repos/gepa`) and **DSPy** teleprompt
  optimizers (`projects/repos/dspy/dspy/teleprompt/`) as a separate,
  non-Worker batch service that produces *candidate* artifacts (optimized
  instructions, few-shot demos, signature variants). Reimplementing GEPA's
  Pareto-evolutionary search or DSPy's MIPRO/Bootstrap/SIMBA optimizers in
  Effect would be large, low-leverage duplication of a fast-moving,
  paper-backed, production-proven library — and DSPy already depends on
  `gepa[dspy]==0.1.1` (`projects/repos/dspy/pyproject.toml`), so the two are
  co-developed upstream.

- **Adopt the real Python RLM only as a leaf EXECUTOR** on the existing
  sandbox-capable tier (Pylon / our cloud / a container), never in Workers.
  RLM's whole paradigm is "the model writes Python and runs it in a REPL"
  (`projects/repos/rlm/README.md`), which is sandboxed-code-execution work that
  already has a home in our executor/dispatch model. Keep the
  *orchestration/market dispatch* (the FRLM-conductor / NIP-90 fanout
  direction) native in Effect.

- **Keep the ONLINE serving + governance path natively in Effect/TS.** The
  Blueprint signature-lookup selector
  (`packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`), the
  evidence-only action-submission boundary
  (`packages/probe/packages/runtime/src/blueprint/action-submission.ts`), and
  the registry/release-gate projection are latency-sensitive, run inside
  Cloudflare Workers, and *are* our governance moat. Do **not** port them to
  Python; Python cannot run in Workers and a network hop on the hot path would
  weaken both latency and the typed boundary.

The unifying rule: **Python is offline/leaf compute that produces untrusted
candidates and evidence; Effect is the online authority that selects, gates,
and admits them.** The evidence / receipt / Blueprint-signature governance
model holds identically whether or not a Python backend exists.

---

## 1. What the reference material actually provides

### DSPy — `projects/repos/dspy` (v`3.3.0b1`, "Development Status :: 3 - Alpha")

"Programming — not prompting — foundation models." Mature, broad API surface
(`projects/repos/dspy/dspy/`):

- **Signatures** (`signatures/signature.py`, `field.py`): typed input/output
  field contracts for an LM call — the conceptual cousin of our Blueprint
  Program Signatures.
- **Modules** (`predict/`): `Predict`, `ChainOfThought`, `ReAct`,
  `ProgramOfThought`, `CodeAct`, `BestOfN`, `Refine`, and — notably —
  `predict/rlm.py`, a first-party Recursive Language Model module.
- **Optimizers / teleprompters** (`teleprompt/__init__.py`): `MIPROv2`,
  `BootstrapFewShot`, `BootstrapFewShotWithRandomSearch`, `BootstrapFinetune`,
  `COPRO`, `SIMBA`, `GRPO`, `BetterTogether`, `Ensemble`, `KNNFewShot`,
  `InferRules`, and `GEPA` (`teleprompt/gepa/gepa.py`).
- **Evaluate / datasets / adapters / clients** (`evaluate/`, `datasets/`,
  `adapters/`, `clients/`): metric harness, data loaders, prompt adapters, and
  a `litellm`-based provider layer.
- **Runtime deps** (`pyproject.toml`): `litellm`, `openai`, `pydantic`,
  `diskcache`, `cloudpickle`, `gepa[dspy]==0.1.1`. Pure Python, `>=3.10`.

Maturity: labeled Alpha and versioned in beta, but very widely deployed and
actively released; API churns at the optimizer layer.

### GEPA — `projects/repos/gepa` (v`0.1.1`)

Genetic-Pareto optimizer for *any* textual parameter (prompts, code, agent
architectures, configs) using LLM reflection over full execution traces plus
Pareto-aware evolutionary selection. Headline claims from
`projects/repos/gepa/README.md`: ~90x cheaper than a frontier baseline,
~35x fewer rollouts than GRPO (100–500 vs 5,000–25,000+ evals), 50+ production
uses. Core API is a single `gepa.optimize(seed_candidate, trainset, valset,
task_lm=..., reflection_lm=..., max_metric_calls=...)` call. Base package has
**zero hard deps**; the `full` extra pulls `litellm`, `datasets`, `mlflow`,
`wandb`. It is *the* DSPy optimizer (DSPy depends on `gepa[dspy]`).

### RLM — `projects/repos/rlm` (`rlms` v`0.1.3`, "Development Status :: 4 - Beta")

Recursive Language Models: a task-agnostic inference paradigm that replaces
`llm.completion(prompt, model)` with `rlm.completion(prompt, model)`. The
context is offloaded as a variable in a **REPL environment**; the LM writes
Python that programmatically examines/decomposes the context and issues
`llm_query(...)` / `llm_query_batched(...)` sub-LM calls
(`projects/repos/rlm/README.md`,
`projects/repos/dspy/dspy/predict/rlm.py`). API surface
(`projects/repos/rlm/rlm/`):

- **clients/** — `anthropic`, `openai`, `gemini`, `azure_openai`, `portkey`.
- **environments/** — `local` (in-process `exec`), `ipython`, `docker`,
  `modal`, `prime`, `daytona`, `e2b`. Non-isolated `local` is host-process code
  execution; production-safe use requires a cloud/container sandbox.
- **core/** — `rlm.py`, `lm_handler.py`, `types.py`; plus a `training/`
  (verifiers/prime-rl) environment and a trajectory `visualizer/`.

Maturity: Beta, single-lab (MIT OASYS) maintained, newer than DSPy/GEPA. The
defining property for us: **it is inherently a Python-`exec`/REPL system that
needs sandbox isolation.** That is executor-tier work, categorically not a
Workers fit.

---

## 2. What WE already have / plan in Effect/TS

- **Blueprint Signature Lookup Service** —
  `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`. A typed
  Effect selector that, from a *safe registry projection*, chooses program
  signatures, program types, module versions, tool scopes, release gates, and
  the evidence/receipt requirement refs for a request — enforcing risk
  ceilings, allowed surfaces, `safeProjection`, and
  `actionSubmissionRequiredForDirectEffects: true`. This is our governed
  answer to "which program/tools may this request select," and it is the
  semantic-routing-compliant analogue of a DSPy signature catalog.
- **Evidence-only Action Submission boundary** —
  `packages/probe/packages/runtime/src/blueprint/action-submission.ts`. Any
  externally-effecting action (`create_pull_request`, `deploy`, `send_email`,
  `post_public_claim`, `spend_money`, `legal_sensitive_commitment`,
  `mutate_source_backed_business_fact`, …) is forced through a *proposal-only*
  record: `directExecution: false`, `programRunAuthorityBoundary:
  "evidence_only"`, `approvalRequired: true`, `contentRedacted: true`,
  `modelConfidenceBypassDisabled: true`. This is the moat that makes any
  upstream optimizer or RLM output safe to ingest.
- **GEPA bounded scheduled runner** —
  `docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md` (proof
  `workers/api/src/artanis-gepa-scheduled-runner-proof.ts`). Today "GEPA" in
  our stack is a *bounded status-projection loop* on a Cloudflare minute cron;
  it explicitly **denies** assignment dispatch, model training, provider
  mutation, runtime promotion, settlement, and wallet-spend authority. It is
  not a real optimization/compile path — which is exactly the gap a Python GEPA
  backend would fill, offline.
- **RLM / FRLM-conductor direction** — issue #6654; the historical crate lives
  at `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/frlm/`
  (`src/conductor.rs`). The `FrlmConductor` already models recursive
  decomposition as **sub-query scheduling/fanout over NIP-90 with a local
  executor fallback** (`SubQuerySubmitter`, `LocalExecutor`, budget/policy
  enforcement, trace emission). That orchestration is market/dispatch logic;
  the *leaf* "execute code over a context fragment" is precisely what upstream
  RLM does.
- **Runtime substrate** — Khala/Artanis run on **Cloudflare Workers + Effect +
  Effect Schema + Bun**, with D1/R2/Queues/Durable Objects (root `CLAUDE.md`,
  `openagents/CLAUDE.md`). The autonomous loop and signature-governed action
  flow are described in `docs/artanis/2026-06-06-autonomous-loop-contract.md`.
  (Note: there is no
  `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`
  in the repo at the time of writing; the live governance surfaces are the
  signature-lookup and action-submission modules cited above.)

---

## 3. Workspace constraints (the honest part)

- **Python does not run in Cloudflare Workers.** Our online product surface is
  Workers/Effect/Bun. Any DSPy/RLM/GEPA Python code is therefore a **separate
  service** reached over the network — it can never be an in-process call from
  the hot path.
- **Where could the Python service live?** Three honest options:
  1. **A container / our cloud** (Cloudflare Containers, or the OpenAgents GCE
     box that is "our cloud" for unattended execution). Best fit for *offline*
     GEPA/DSPy compile jobs: long-running, batch, no Worker-latency budget,
     reached via Queues/cron, results landed in R2/D1.
  2. **Psionic** — the ML execution substrate. Conceptually the right owner for
     batch optimization/training, though Psionic is Rust-first and would
     host/shell a Python process or container rather than embed it.
  3. **Pylon-side** — Pylons already run sandboxed, Python-capable code
     (Codex; local/docker REPLs). RLM's `local`/`docker`/`modal`/`e2b`
     environment model maps **directly** onto the Pylon executor + assignment
     flow, and keeps untrusted code execution on the existing sandbox tier
     instead of anywhere near Workers.
- **Latency/runtime fit, by tier:** Workers serve at ms latency (online
  governance/selection); GEPA/DSPy compile runs minutes-to-hours (offline);
  RLM runs seconds-to-minutes of sandboxed execution (leaf). A hybrid aligns
  each system to the tier it actually fits.
- **Governance is invariant.** Whatever runs in Python, the
  evidence/receipt/Blueprint-signature/release-gate model must hold. Python
  outputs (optimized prompts, RLM trajectories) enter our system **only** as
  redacted, evidence-bearing *candidates* through the existing
  action-submission / release-gate boundary — never as direct mutation,
  provider, settlement, or wallet authority. This matches what the bounded GEPA
  runner already denies.

---

## 4. The three options weighed

### (a) Full Python DSPy/RLM backend, called from Effect surfaces

- **Pros:** Maximum reuse; we inherit DSPy modules, every optimizer, and RLM
  for free, tracking upstream.
- **Cons:** If "called from Effect surfaces" means on the online path, it puts
  a Python network hop in the Worker hot path (latency + new failure mode +
  weakens the typed boundary), and it tempts moving selection/governance into
  Python where it does not belong. Pushing the *governed serving* path into
  Python would duplicate hardened Effect invariants and break Workers fit.
- **Verdict:** Reject as a blanket strategy. Correct only for the
  offline/leaf slices, not the online authority.

### (b) Keep reimplementing the needed slices natively in Effect/TS

- **Pros:** One language on the product surface; everything inside the typed
  Effect boundary; no cross-runtime ops.
- **Cons:** Reimplementing GEPA's Pareto-reflective search and DSPy's
  MIPRO/Bootstrap/SIMBA optimizers in Effect is a large, ongoing
  reimplementation of an actively-evolving research library — high duplication
  cost, high drift risk, low differentiation. Reimplementing RLM means also
  rebuilding a sandboxed multi-environment Python REPL, which is squarely
  executor-tier work we should not reinvent.
- **Verdict:** Right for the **online serving/governance** path (already true).
  Wrong for the **optimizer** and **RLM-executor** slices.

### (c) Hybrid — Python for offline optimize/compile + RLM-executor; Effect for online serving/governance

- **Pros:** Each ecosystem used where it is strongest; no Python on the Worker
  hot path; no Effect reimplementation of fast-moving optimizers; governance
  boundary unchanged; clean staged adoption.
- **Cons:** Operating a second (Python) runtime + an artifact handoff
  contract; must keep that handoff strictly evidence-only.
- **Verdict:** **Recommended.**

---

## 5. Integration boundary (recommended shape)

- **Offline optimize/compile (GEPA + DSPy teleprompt):** runs as a
  scheduled/queued job on a container or Psionic-hosted Python service against
  **public-safe eval sets**. Output = compiled candidate artifacts (optimized
  instructions, few-shot demos, signature variants) written to **R2**, indexed
  in **D1**. Workers never call it synchronously; they enqueue work and read
  results. Promotion of a candidate into a live Blueprint **module version**
  goes through the existing `signature-lookup` selectability rules + release
  gates — a GEPA win is a *candidate*, not an auto-promotion. This directly
  upgrades today's bounded status-projection runner
  (`docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`) from
  "projection only" to "real offline optimization that still cannot self-promote."
- **RLM leaf executor:** invoked through the existing **Khala → Pylon →
  assignment** dispatch (the same path Codex turns use today, per
  `openagents/CLAUDE.md`), on the sandboxed Pylon/cloud tier. It returns
  evidence + **redacted** traces (mirroring the existing ATIF redaction +
  `token_usage_events` accounting), with no raw prompts/secrets/local paths in
  public projections. The **FRLM conductor / NIP-90 fanout** stays native in
  Effect; RLM is only the leaf "examine-and-recurse over a context fragment."
- **Online serving/governance:** unchanged in Effect — `signature-lookup.ts`,
  `action-submission.ts`, registry/release-gate projection. No Python here.

## 6. Optimizer story: GEPA in Python vs ours

Use **upstream GEPA in Python**. It is the DSPy-native optimizer, single-call
(`gepa.optimize(...)`), trace-reflective, Pareto-aware, and demonstrably far
cheaper than RL-style tuning (`projects/repos/gepa/README.md`). Our role is not
to reimplement the search; it is to (1) supply the metric + eval set, (2)
capture the compiled artifact as evidence, and (3) gate promotion through the
Blueprint release-gate path. The existing Effect "GEPA runner" should remain
the **governance/projection wrapper**, now backed by a real Python optimize job
rather than a status-only loop.

## 7. Maintenance / duplication cost

- Reimplementing GEPA + DSPy optimizers in Effect: **high** ongoing cost
  chasing an Alpha-status, frequently-released upstream — low leverage.
- Porting the serving/governance path to Python: **high** cost duplicating
  hardened Effect invariants and **breaking** the Workers runtime fit.
- Hybrid cost: operating one extra Python runtime + a narrow,
  evidence-only artifact/assignment handoff. This is the **lowest total**
  duplication because neither side reimplements the other.

## 8. Governance (must hold in every option)

The evidence-only action-submission boundary
(`action-submission.ts`) and the safe-projection signature lookup
(`signature-lookup.ts`) are the invariant. Concretely, regardless of backend:

- Python outputs are **untrusted candidates**; they acquire authority only via
  release gates + action-submission approval, never by direct execution.
- No Python service gets dispatch, provider-mutation, settlement,
  model-promotion, or wallet-spend authority (exactly the denials already listed
  in `docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`).
- RLM/optimizer traces are redacted before any public projection; token
  accounting stays exact, mirroring the Khala/Codex ingest contract.
- Routing to a program/optimizer remains via the typed selector, not ad-hoc
  string matching (root `CLAUDE.md` semantic-routing rule).

## 9. Staged recommendation

- **Stage 0 (now):** Keep the online serving/governance path in Effect. Keep
  the GEPA scheduled runner as a bounded status projection. No Python in the
  product runtime yet.
- **Stage 1 (offline GEPA/DSPy compile):** Stand up a Python optimization
  service (container or Psionic-hosted) running real GEPA + DSPy compile against
  public-safe eval sets; emit candidate artifacts to R2/D1; wire them as
  Blueprint module-version **candidates** behind existing release gates. No
  online dependency; the Effect runner becomes the governance wrapper over real
  results.
- **Stage 2 (RLM leaf executor):** Adopt upstream `rlms` (and/or DSPy's RLM
  module) as a sandboxed leaf executor on the Pylon/cloud tier, invoked through
  the existing Khala→Pylon assignment path with evidence + redacted traces.
  Keep FRLM-conductor/NIP-90 dispatch native in Effect.
- **Stage 3 (promotion):** Promote GEPA-optimized signatures into production
  via the existing release-gate + action-submission flow once eval gains are
  proven and reproducible.

**Do not:** run Python in Workers; move selection/governance into Python; or
let any Python output bypass the evidence/receipt boundary.

---

### Key paths referenced

- `projects/repos/dspy/` (README, `pyproject.toml`, `dspy/teleprompt/__init__.py`,
  `dspy/predict/rlm.py`, `dspy/signatures/`, `dspy/teleprompt/gepa/`)
- `projects/repos/gepa/` (README, `pyproject.toml`)
- `projects/repos/rlm/` (README, `pyproject.toml`, `rlm/clients/`, `rlm/environments/`, `rlm/core/`)
- `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`
- `packages/probe/packages/runtime/src/blueprint/action-submission.ts`
- `docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`
- `docs/artanis/2026-06-06-autonomous-loop-contract.md`
- `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/frlm/src/conductor.rs` (issue #6654 direction)
