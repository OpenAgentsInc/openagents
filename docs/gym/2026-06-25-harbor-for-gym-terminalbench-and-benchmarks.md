# Using Harbor for the Gym, TerminalBench, and other benchmarks

Updated: 2026-06-25

> **Status:** audit + direction, honest-scope. This evaluates whether and how
> the OpenAgents Gym should adopt **Harbor** (`harbor-framework/harbor`, a
> read-only reference clone at `projects/repos/harbor`) as the executor/verifier
> for the Gym's `terminal-bench` environment and other benchmarks, building on
> the fact that we already use Harbor's trajectory format (ATIF) for agent
> traces. It builds on the Gym spec
> ([`openagents-gym.md`](openagents-gym.md)) and the Episode 243 considerations
> ([`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md));
> it changes none of their claims. Nothing here is a product promise, a served
> public capability, or public-claim copy. Anything not already landed in the
> Phase 0 fixture Gym (#6163–#6167) is labeled **direction**, not a live claim.
> Harbor is a **reference repo**: the workspace external-references rule says
> study + integrate at a seam, do not vendor or fork wholesale. This audit
> respects that — Harbor would run as an out-of-process harness in
> Hydralisk/Psionic, not be copied into the Worker.

## 0. TL;DR — the four sharpest findings

1. **Harbor's TerminalBench is genuinely adoptable as the executor+verifier for
   the Gym's `terminal-bench` environment, and it directly answers the Gym
   spec's two open TerminalBench questions.** Harbor *is* the official harness
   for Terminal-Bench 2.0 (`README.md`, `docs/.../running-terminal-bench.mdx`);
   TerminalBench ships as a first-party **dataset** in `registry.json`
   (`terminal-bench@2.0`, 89 tasks; plus `terminal-bench-pro@1.0`, 200 tasks; and
   `terminal-bench-sample@2.0`, 10 tasks). You do not write an adapter to run it —
   you run `harbor run -d terminal-bench/terminal-bench-2 -a <agent>`. Crucially,
   Harbor shipped **separate verifier sandboxes** (`[verifier] environment_mode =
   "separate"`, `docs/content/news/separate-verifier-sandboxes.mdx`,
   2026-05-15) — the verifier runs in a *different container* from the agent with
   explicit artifact handoff. That is exactly the Gym spec's
   "verifier on a **distinct** device from the producer" requirement, already a
   built feature rather than something we have to invent.

2. **The boundary is clean and favorable: Harbor owns the parts we should NOT
   rebuild (task sets, container verifiers, the trajectory format, the adapter
   ecosystem), and nothing it owns competes with what stays ours** (the typed
   `GymExperiment` config, the Khala coordinator/policy, cost-per-accepted-outcome,
   the public-safe report, metering/settlement, promotion). Harbor produces a
   `VerifierResult` whose canonical field is a **float `reward`** read from
   `/logs/verifier/reward.txt` (`src/harbor/verifier/verifier.py`). That float is
   precisely the "executed verification verdict" the Gym needs to feed
   `cost-per-accepted-outcome` — Harbor computes the verdict; we keep the cost,
   the price, the receipt, and the promotion decision.

3. **The hard integration seam is the stack split, and it is the same split the
   Gym spec already anticipates.** Harbor is **Python 3.12+ / uv / Docker**
   (`pyproject.toml`, `requires-python = ">=3.12"`); our product/Worker is
   Bun/Effect/Foldkit and *cannot* run Docker containers. So Harbor must run
   where Python + Docker + GPUs already live — **Hydralisk/Psionic** — and the
   Worker invokes it through an artifact/job seam, never in-process. This matches
   the Gym spec's own note that "a Python Harbor harness would run [in
   Hydralisk/Psionic], not in the Worker." Harbor is usable two ways from a
   non-Harbor system: the **`harbor` CLI** (`harbor run … -o <jobs-dir>` →
   trial artifacts) and the **`harbor.job.Job` Python library** (`await
   job.run()` → `TrialResult` objects with `verifier_result.rewards` and
   trajectory token metadata). Either way the OpenAgents seam consumes
   *artifacts/results*, not Harbor internals.

4. **Adopting Harbor is the opposite of a detour for our trajectory ambitions —
   it consolidates them.** We already chose ATIF (Harbor's `rfcs/0001-trajectory-format.md`,
   v1.7) as the format for `/trace/{uuid}` and the data market
   (`docs/traces/README.md`). Harbor is the reference implementation of ATIF and
   its agents export ATIF natively (Terminus 2, plus exporters for Claude Code,
   Codex, OpenHands, Gemini CLI, mini-swe-agent). So a Gym run executed through
   Harbor *already emits the exact trajectory format* our trace surface and our
   Khala training loop (GEPA/TRINITY/Conductor in Psionic) want — including the
   RL-grade fields (`completion_token_ids`, `logprobs`, per-step `reward`) the RL
   workflow needs. The Gym→trace→training pipeline and the Gym→benchmark pipeline
   become *one* artifact format instead of two.

## 1. What Harbor is (architecture, with path evidence)

Harbor (`projects/repos/harbor`, v0.15.0 per `pyproject.toml`) is a **framework
for evaluating and optimizing agents and language models in sandboxed container
environments**, from the creators of Terminal-Bench. It is not a single thing;
it is four things stacked:

- **An agent-eval harness / CLI.** The headline use is "point an agent at a
  benchmark and score it": `harbor run --dataset terminal-bench@2.0 --agent
  claude-code --model anthropic/claude-opus-4-1 --n-concurrent 4` (`README.md`).
  The CLI is Typer-based (`src/harbor/cli/`), with subcommands `run`, `datasets`,
  `trials`, `tasks`, `traces`, `sweeps`, `adapters`, `check`, `analyze`,
  `publish`, `view`. Entry points are `harbor` / `hr` / `hb`
  (`pyproject.toml [project.scripts]`).

- **A Python library.** `harbor.job.Job` (`src/harbor/job.py`) is the
  programmatic core: build a `JobConfig` (`src/harbor/models/job/config.py`) of
  `tasks × agents × models × attempts`, `await job.run()`, and read back
  `TrialResult`s. This is the seam the RL workflow uses
  (`docs/.../training-workflows/rl.mdx`, below).

- **A trajectory/dataset system.** It defines and reference-implements **ATIF**
  (Agent Trajectory Interchange Format, `rfcs/0001-trajectory-format.md`, v1.7),
  with Pydantic models in `src/harbor/models/trajectories/`. It distributes
  benchmark task sets through a **registry** (`registry.json`, a 13 MB index)
  and the `harbor-datasets` Git/HuggingFace repos.

- **An adapter registry/ecosystem.** `adapters/` holds ~86 adapters that convert
  *external* benchmarks into Harbor's task format (SWE-Bench family, Aider
  Polyglot, GAIA, LiveCodeBench, BFCL, AIME, GPQA, lawbench/financeagent/medagentbench,
  and many more — see `AGENTS.md` "Adapters").

### The execution model (the part that matters for the Gym)

The data model is a strict hierarchy (`docs/content/docs/core-concepts.mdx`):

- **Task** — one unit of evaluation, a *directory*: `task.toml` (config:
  timeouts, cpu/mem/gpu, network mode, mcp servers), `instruction.md` (the
  natural-language goal handed to the agent), `environment/Dockerfile` (the
  container the agent works in), `tests/test.sh` (the verifier), optional
  `solution/solve.sh` (the oracle). See `examples/tasks/hello-world/`.
- **Dataset** — a collection of tasks; usually one benchmark
  (Terminal-Bench, SWE-Bench Verified). Indexed in `registry.json`; resolved by
  `org/name@version`.
- **Agent** — a program that completes a task, implementing `BaseAgent`
  (`src/harbor/agents/base.py`): `setup(env)` + `run(instruction, env, context)`.
  Built-in installed agents include `claude-code`, `codex`, `opencode`, `aider`,
  `goose`, `gemini-cli`, `openhands`, `qwen-coder`, `mini-swe-agent`, plus the
  internal `terminus`/`terminus-2` and the utility `oracle`/`nop` agents
  (`AGENTS.md`).
- **Environment** — the *runtime* the container runs on, implementing
  `BaseEnvironment` (`src/harbor/environments/base.py`): `docker` (local,
  default) plus cloud backends `daytona`, `modal`, `e2b`, `runloop`, `gke`,
  `openshift`, `novita`, `apple_container`, EC2. Selected with `--env`.
- **Trial** — one agent attempt at one task ("essentially a rollout that
  produces a reward"). **Job** — a collection of trials run in parallel.

The async trial loop (`AGENTS.md` "Async Operations"):

```python
await environment.start(force_build=False)
await agent.setup(environment)
await agent.run(instruction, environment, context)
result = await verifier.verify()          # -> VerifierResult(rewards={"reward": float})
await environment.stop(delete=True)
```

### How verification actually works (the verdict we consume)

`src/harbor/verifier/verifier.py` is small and unambiguous: it uploads `tests/`
into the environment, runs the test script as the verifier user, and reads a
reward back. The reward is a **float in `/logs/verifier/reward.txt`** (or a JSON
dict of named float metrics in `/logs/verifier/reward.json`). `test.sh`
conventionally writes `1` on pass / `0` on fail (`docs/.../datasets/adapters.mdx`).
Multiple float-valued metrics are supported (RL-compatible). The result type is
`VerifierResult(rewards=...)`. For LLM-as-a-judge verifiers, the judge model +
key are passed via `[verifier.env]`, and Harbor ships **RewardKit**
(`packages/rewardkit`, `docs/.../rewardkit/`) with built-in + judge criteria.

### Trajectories (ATIF) and RL

ATIF (`rfcs/0001-trajectory-format.md`) is a JSON trajectory: root
`{schema_version, session_id, trajectory_id, agent{name,version,model_name,
tool_definitions}, steps[], final_metrics, subagent_trajectories}`, where each
`StepObject` carries `source: system|user|agent`, `message`,
`reasoning_content`, `tool_calls[]`, `observation{results[]}`, and `metrics`
(prompt/completion/cached tokens, `cost_usd`, and the RL fields
`completion_token_ids` + `logprobs`). Terminus 2 emits ATIF natively
(`src/harbor/agents/terminus_2/`). The RL workflow
(`docs/.../training-workflows/rl.mdx`) shows the canonical loop: build a `Job`
over `TaskConfig`s, `await job.run()`, then for each `trial_result` read
`verifier_result.rewards["reward"]` plus `agent_result.metadata["token_ids" /
"mask_ids"]` to produce `Rollout(reward, token_ids, mask_ids)` for a framework
like SkyRL.

## 2. How we already use Harbor today (with refs)

We do **not** currently run Harbor as a benchmark executor. Our current use is
narrower and one-directional: **Harbor's ATIF is the trajectory format we
adopted for agent traces.**

- `docs/traces/README.md` (the agent-traces spec, 2026-06-24) selects ATIF as
  the format for shareable `/trace/{uuid}` sessions and the trace data market,
  citing the vendored reference `projects/repos/harbor/rfcs/0001-trajectory-format.md`
  (ATIF-v1.7) and Harbor's docs. It calls out ATIF's existing tooling — "a
  validator, a viewer, and exporters for OpenHands, Claude Code, Codex, Gemini
  CLI, mini-swe-agent" — as a reason to adopt it. The QA-runner is being wired to
  emit an ATIF trajectory; the broader `/trace/{uuid}` surface is spec, not
  built. The data-market direction (#6206/#6219/#6220/#6221) is "upload traces →
  revshare → trains Khala," with ATIF (or raw Claude Code/Codex → ATIF
  converters) as the on-the-wire shape.
- Harbor/ATIF and Terminal-Bench also show up as **research/source material**:
  the TMAX work (`docs/research/tmax/`) studies `TMAX-15K-Harbor` as a
  Tassadar coding-environment source and trains terminal/coding agents on
  Terminal-Bench/SWE-Bench; StudyBench (`docs/research/machine-studying/`)
  explicitly positions itself *next to* "Terminal-Bench, Harbor, Probe
  closeouts, Psionic," not as a replacement.
- We have a **public-ref-only retained TerminalBench fixture package** in the
  Pylon runtime: `apps/pylon/packages/runtime/src/benchmark/fixtures.ts`
  (documented in `apps/pylon/docs/probe-port/probe-retained-terminal-bench-fixtures.md`).
  It records TerminalBench task ids, failure-family enums, Blueprint signature
  refs, tool-menu constraints, and public-safe verifier/scorer refs for GEPA
  Stage 0/1 — **deliberately with no task prompts, no solutions, no hidden
  verifier content, and no private Harbor traces**. This is the existing
  public-safety boundary the Gym must preserve, and it is the set the Gym spec's
  open question points at as the seed fixtures.

Net: we have already standardized on **Harbor's trajectory format** and we treat
**Harbor/Terminal-Bench as authoritative reference**, but we have not yet run the
Harbor **harness** to *execute* a benchmark. This audit is about closing that gap
for the Gym.

## 3. TerminalBench via Harbor — can it be the Gym's `terminal-bench` executor+verifier?

**Yes, and it is the cleanest path available.** The Gym spec already lists
`terminal-bench` as a Phase-1 environment and leaves two open questions: (a) which
retained fixtures seed it, and (b) where the executor runs so the verifier is on a
**distinct device** from the producer. Harbor answers both.

### 3.1 Mapping Harbor's contract onto the typed `GymEnvironment`

The Gym's `GymEnvironment` is "task set + verifier + acceptance contract +
default shapes." Harbor supplies the first two-and-a-half directly:

| `GymEnvironment` field | Harbor supplies it as | Path evidence |
|---|---|---|
| **task set** | a Harbor *dataset* — `terminal-bench@2.0` (89 tasks), or a curated subset by task id | `registry.json`; `harbor run -d terminal-bench/terminal-bench-2` |
| **verifier** | the per-task `tests/test.sh` → float reward in `/logs/verifier/reward.txt`, executed by `Verifier.verify()` → `VerifierResult` | `src/harbor/verifier/verifier.py`; `docs/.../datasets/adapters.mdx` |
| **acceptance contract** | the reward threshold (`reward == 1.0` = task passed) — the *executed* verdict, not a self-grade | the Gym's "verified work must execute the artifact" rule maps onto Harbor's container test exit |
| **default shapes (ISL/OSL/concurrency)** | *partially* — Harbor controls concurrency (`-n`), attempts (`-k`), timeouts, cpu/mem/gpu; sequence-shape tagging (`realistic|synthetic`) stays a **Gym-side** annotation on top | `JobConfig`, `OrchestratorConfig(n_concurrent_trials=…)`; `task.toml [agent]/[environment]` |
| **public-safety of task content** | **a Gym responsibility** — Harbor task content includes prompts + solutions; the Gym's report projection must never leak them (reuse the retained-fixture boundary) | `apps/pylon/.../probe-retained-terminal-bench-fixtures.md` |

So the typed `GymEnvironment` for `terminal-bench` becomes a thin descriptor:
`{ harborDataset: "terminal-bench@2.0", taskIdSubset?: [...], acceptanceRewardThreshold: 1.0,
verifierMode: "separate", env: "docker"|"daytona"|… }`. Selection stays
typed/semantic per the workspace rule — `terminal-bench` is a typed enum value,
not a string match.

### 3.2 The distinct-device verifier requirement — already a Harbor feature

The Gym spec wants the verifier on a **distinct device from the producer** (so a
policy can't reach its own grader — the TMAX reward-hacking lesson). Harbor
shipped exactly this on 2026-05-15
(`docs/content/news/separate-verifier-sandboxes.mdx`): set in `task.toml`

```toml
artifacts = ["/tmp/answer.json"]
[verifier]
environment_mode = "separate"
[verifier.environment]
network_mode = "no-network"
cpus = 2
memory_mb = 4096
```

and the verifier runs in a **separate container** with only the explicitly
declared artifacts copied across. The news post lists the exact motivations we
care about: "an additional security boundary between the agent and the
verification process," different resource configs, and pre-baked verifier
dependencies. A worked example is `examples/tasks/separate-verifier-environment/task.toml`
(verifier in its own `no-network` environment). This is process/container
isolation, not literal device isolation, but it is the right primitive: combine
`environment_mode = "separate"` with running **agent on a Pylon/worker node** and
**the Harbor verifier job on a Psionic/Hydralisk node** to get producer ≠ verifier
at the device level too. Recommendation: make `verifierMode: "separate"` the Gym
default for `terminal-bench` and any env where reward-hacking is a risk.

### 3.3 Where it runs (Hydralisk / Psionic / Pylon)

Harbor needs **Python 3.12+, uv, and Docker** (and GPUs for GPU tasks). Our
Worker has none of these. The placement that fits the existing stack:

- **Hydralisk / Psionic host** runs the `harbor` CLI or `harbor.job.Job` — it
  already has Python + NVIDIA + Docker (it hosts the GPT-OSS 20B/120B lanes per
  the Episode 243 doc). This is the Harbor harness home.
- **The agent under test** is the policy: either a built-in Harbor agent
  (`opencode`, `claude-code`, `codex`) pointed at a Khala/competitor endpoint via
  `--model` + `--ak base_url=…`, or (later) a custom Harbor agent wrapping the
  Khala coordinator. Run the agent's container on Pylon/a worker; run the
  verifier container `separate` on the Psionic/Hydralisk side.
- **The Worker** (`apps/openagents.com`) owns the typed `GymExperiment`, the
  quote/balance gate, and report rendering. It dispatches a Harbor job to the
  Hydralisk harness and ingests the resulting **artifacts** (per-trial
  `result.json` with `verifier_result.rewards`, ATIF trajectories, token/cost
  metrics), then compiles them into `openagents.khala.telemetry.v1` records and
  `buildBenchmarkReport`. No Harbor code runs in the Worker.

### 3.4 Onto the matrix → runner → report path

The Gym compiles to `expandMatrix → runner.ts → buildBenchmarkReport`. With
Harbor as the `terminal-bench` executor, the `runner.ts` step for that env is
"dispatch a Harbor job + collect artifacts" instead of a direct provider call.
The mapping is one-to-one:

- a Gym **matrix cell** (env × lane × policy × shape) → a Harbor **trial config**
  (task × agent × model × attempt);
- Harbor **`n_concurrent`/`n_attempts`** → the Gym's concurrency dial and
  samples-per-cell (the spec's ≥5 rule);
- Harbor **`VerifierResult.reward`** → the Gym's `test_passed` / scalar reward
  field in the telemetry record → `cost-per-accepted-outcome` (Harbor gives the
  verdict; the Gym multiplies by the real per-lane `cost_amount` basis from
  `2026-06-25-khala-cost-model-and-analytics.md`);
- ATIF `final_metrics.total_*_tokens` / `cost_usd` → cross-check against the
  served-tokens recorder (use the provider `usage` block as source of truth, per
  the honesty rule; ATIF cost is a snapshot, not authority).

The public-safety tripwire (`checkReportPublicSafety`) and the
`decisionGrade`/`not_measured` discipline are unchanged — they apply to the
*report*, regardless of who executed the trial.

## 4. Other Harbor benchmarks worth adopting for the Gym ladder

The Gym ladder (BigPickle → other free → paid frontier) is about *who* runs;
Harbor benchmarks are about *what task*. They compose: each rung runs against one
or more Harbor datasets. Strongest candidates, by fit:

- **`terminal-bench@2.0`** — the flagship; terminal/coding tasks, separate
  verifier, oracle solutions for sanity. The first Harbor env to wire (§3).
- **SWE-Bench family** (`adapters/swebench`, `swebenchpro`,
  `swebench_multilingual`, `swesmith`) — the canonical coding-fix benchmark; pairs
  naturally with the OpenCode coding head-to-head since the verifier is a real
  repo test suite (executed, not graded by the model). High signal for "Khala as
  a coding agent."
- **`aider_polyglot`** (`adapters/aider_polyglot`, also a first-party dataset) —
  multi-language code-edit tasks; small, fast, good for cheap-rung sweeps where
  the whole game is verified-rate + cost.
- **`livecodebench`, `bigcodebench_hard`, `ds1000`, `humanevalfix`** — code-gen
  rungs that exercise different difficulty bands without a full repo container.
- **`gaia`, `bfcl`** — agent/tool-use benchmarks; relevant once the Gym measures
  tool-call-completion (the Episode 243 first-class metric) beyond a single
  client surface.
- Domain benches (`lawbench`, `financeagent`, `medagentbench`) — **future**, only
  if a vertical demands it; several need custom agents and LLM-judge verifiers.

**How they coexist with the OpenCode client-surface environment.** These are
*different axes of the same Gym*, not competitors:

- The **OpenCode head-to-head** is a *client-surface* environment — it measures
  the whole coding-agent experience through the actual tool a developer uses
  (does the model drive OpenCode's tools to a verified result?). It is the Gym's
  first real environment and the GTM wedge (Episode 243).
- **Harbor benchmarks** are *task-set* environments — fixed, parity-validated
  task suites with container verifiers and a leaderboard lineage.

A clean two-tier design: the OpenCode environment is the *flagship demo + GTM
surface*; Harbor datasets are the *rigorous, citable, leaderboard-comparable*
environments behind it. Both produce ATIF trajectories and a verified-rate +
cost-per-accepted-outcome, so they share the report schema. Notably, Harbor has a
built-in **`opencode` agent** (`AGENTS.md` agent list) — so an OpenCode-style run
*can* itself be expressed as a Harbor job if we want one execution path; but the
Gym's bespoke OpenCode client runner (provisioning `opencode.json`, extracting
tool-call-completion) stays the right tool for the client-surface axis because it
measures things Harbor's task verifier doesn't.

## 5. The boundary — what Harbor owns vs what stays ours

Reference-repo discipline (workspace rule): **integrate at a seam, do not vendor
or fork wholesale.** The boundary is naturally clean because Harbor stops exactly
where our value begins.

**Harbor owns (adopt, don't rebuild):**
- Benchmark **task sets** + their container environments + the registry/versioning
  (`registry.json`, `harbor-datasets`).
- The **container verifier** execution (`tests/test.sh` → reward; separate-sandbox
  isolation; RewardKit judge criteria).
- The **trajectory format** (ATIF) and its reference Pydantic implementation +
  validator + exporters.
- The **adapter ecosystem** for pulling in new external benchmarks.
- The **execution backends** (docker/daytona/modal/…) and parallel orchestration.

**OpenAgents owns (never hand to Harbor):**
- The typed **`GymExperiment` config** and `compileGymExperiment` (Effect Schema;
  the human-authored superset of the matrix config).
- The **Khala policy** under test — coordinator candidate (`ModelRouter` →
  TRINITY → Conductor), provider fan-out + modes, tool set, plugin/module
  composition. Harbor sees a *model endpoint / agent*; it does not see Khala's
  routing.
- **cost-per-accepted-outcome** and the real per-lane cost basis + analytics
  (`2026-06-25-khala-cost-model-and-analytics.md`). Harbor reports a reward;
  pricing/economics are ours.
- The **public-safe report** + `checkReportPublicSafety` +
  `decisionGrade`/`not_measured` honesty rules + any leaderboard projection.
- **Metering / settlement / revenue split** (the balance gate, `MeteringHook`,
  RL-1/RL-2/RL-3). Harbor holds no money and issues no quotes.
- **Promotion** — a Gym-winning coordinator becomes live only via an
  approval-gated `runtime_promotion`. Harbor never promotes anything.
- The `openagents.khala.telemetry.v1` schema — we **map** Harbor results into it;
  we do not adopt a parallel metric vocabulary (the spec's "schema reuse, never
  fork" rule).

The seam is therefore: **Worker → (job spec) → Harbor harness on Hydralisk/Psionic
→ (artifacts: rewards + ATIF + token/cost) → Worker maps to telemetry → report.**
No Harbor types cross into the Worker; only data does.

## 6. Trajectory → training loop (Gym ↔ Khala flywheel)

This is where adopting Harbor pays off twice. The Gym spec's Phase 3 is
"Gym reports feed GEPA candidate feedback + TRINITY/Conductor training in
Psionic." Harbor makes the trajectory leg of that loop free:

- **Harbor → Khala training.** A Harbor trial already produces (a) a float
  reward (the executed verdict), and (b) an ATIF trajectory with per-step
  `metrics.completion_token_ids` + `logprobs` and `final_metrics` — the exact
  fields the RL workflow (`docs/.../training-workflows/rl.mdx`) turns into
  `Rollout(reward, token_ids, mask_ids)`. Psionic's GEPA candidate feedback
  (`psionic.probe_gepa_candidate_manifest.v1`), TRINITY (sep-CMA-ES), and
  Conductor (GRPO) consume reward + trajectory. ATIF's `is_copied_context` /
  `llm_call_count = 0` rules even tell SFT pipelines which steps to filter — that
  hygiene is built into the format.
- **Gym runs → Harbor-format trajectories.** Conversely, every Gym run (even the
  OpenCode client-surface env, even fixture runs) should **emit ATIF**, so a Gym
  run is simultaneously: a benchmark result, a shareable `/trace/{uuid}`, and a
  training sample. We already chose ATIF for traces (§2); making the Gym emit ATIF
  too means one format end-to-end. Where a Gym env runs *through* Harbor, the
  trajectory is ATIF for free; where it runs through our own runner (OpenCode),
  the runner should export ATIF using the in-repo public-safe ATIF subset the
  traces spec calls for.
- **The loop closes** exactly as the spec draws it: Gym report (reward +
  cost/outcome) → GEPA/TRINITY/Conductor in Psionic → shadow candidate → back
  into the Gym head-to-head → approval-gated `runtime_promotion`. Harbor sits on
  the *eval+trajectory* edge of that loop; Psionic owns training; the Gym owns
  the experiment surface and the promotion gate.

**Direction, not shipped:** none of this training wiring is a live seam today.
The QA-runner ATIF emitter is being built; the Gym↔Khala dog-food wiring is the
"next lane to build" per Episode 243.

## 7. Integration considerations & risks

- **Python/uv/Docker vs Bun/Effect (the central risk).** Harbor cannot live in
  the Worker. It must run on a Python+Docker host (Hydralisk/Psionic). The
  integration is an **out-of-process job seam** (dispatch + artifact ingest), not
  a library import. This is more moving parts (a harness host, job dispatch,
  artifact transport) but it is the *correct* boundary and matches where our
  Python already lives. Risk is operational (keeping the harness host healthy),
  not architectural.
- **Invocation seam choice.** Two options, both viable:
  (a) **CLI** — `harbor run … -o <jobs-dir>` then read `jobs/<job>/…/result.json`
  + trajectory files; simplest, language-agnostic, decision-grade-friendly
  (real artifacts on disk). (b) **Library** — a thin Python service wrapping
  `harbor.job.Job` exposing a typed HTTP/RPC the Worker calls; better for tight
  RL loops (token interception). **Recommendation:** start with the **CLI +
  artifact** seam (lowest coupling, easiest to keep Harbor un-forked); graduate
  to the library service only for the RL rollout loop where token_ids/logprobs
  matter.
- **Cost / spend gating.** A real Harbor run spends real money (provider tokens
  for the agent; cloud-sandbox costs if `--env daytona`). This must ride the
  existing gates: fixture/oracle runs are free and `decisionGrade:false`; real
  runs require `seam:'real'` + the owner-gated `preflightRealBenchmarkSweep` +
  balance gate. The `oracle` agent is a *free* correctness check (no model spend)
  — use it in CI to validate a `terminal-bench` env wiring without burning tokens.
- **Public-safety of task content + reports.** Harbor task directories contain
  prompts **and** solutions; Terminal-Bench tasks carry canary strings. The Gym
  must (a) never surface raw task prompts/solutions/hidden verifier content in any
  public report (reuse the retained-fixture boundary), and (b) keep reports to the
  public-safe projection (token counts, durations, neutral classifiers, verdict,
  cost — never prompt/completion/account/raw price). ATIF trajectories we publish
  must go through the public-safe subset, not raw.
- **Versioning / reproducibility.** Pin the Harbor version (currently 0.15.0),
  the dataset tag (`terminal-bench@2.0`), the agent version, and the dated model
  id — Harbor's own parity discipline (`docs/.../datasets/adapters.mdx`, Step 5)
  is a good template for "decision-grade" runs.
- **Honest scope.** Today we use Harbor's *format* only. Running Harbor as a
  benchmark executor is **direction**; the harness host, the job seam, the
  `terminal-bench` `GymEnvironment`, and the training wiring are all unbuilt. The
  Phase 0 fixture Gym (#6163–#6166) and `/gym/oss` (#6167) remain the only landed
  pieces.

## 8. Concrete next steps (keyed to the Gym Phase-1 roadmap)

1. **Stand up a Harbor harness on Hydralisk/Psionic** — `uv tool install harbor`,
   Docker available, validate with the free path: `harbor run -d
   terminal-bench/terminal-bench-2 -a oracle` (oracle = no model spend) and
   `harbor run -t hello-world/hello-world`. Confirm `environment_mode =
   "separate"` works on that host. (Operational; no Worker change.)
2. **Define the typed `terminal-bench` `GymEnvironment`** — `{ harborDataset,
   taskIdSubset?, acceptanceRewardThreshold: 1.0, verifierMode: "separate", env }`,
   selected via the typed env enum. Seed `taskIdSubset` from the existing retained
   fixtures (`apps/pylon/.../probe-retained-terminal-bench-fixtures.md`) so the
   first env stays inside the established public-safe boundary.
3. **Add the job-dispatch + artifact-ingest seam** — `runner.ts` learns to
   dispatch a Harbor job to the Hydralisk harness and ingest
   `result.json`/ATIF, mapping `verifier_result.rewards["reward"]` → the
   `test_passed`/scalar-reward field of `openagents.khala.telemetry.v1`, and
   cross-checking tokens against the served-tokens recorder. CLI+artifact seam
   first.
4. **Wire cost-per-accepted-outcome** for the env using the real per-lane
   `cost_amount` basis (the cost-model doc) × Harbor's verdict; render null
   cost-per-outcome for zero-accepted groups (no fake-cheap results).
5. **CI smoke (free, no spend)** — a `terminal-bench` env wiring test that runs
   the `oracle` agent on the retained subset through the harness, asserts reward
   1.0, asserts the report is `decisionGrade:false`, and asserts the public-safety
   tripwire strips task content. Mirrors the spec's "fixture run is deterministic"
   test.
6. **Emit ATIF from Gym runs** — adopt the in-repo public-safe ATIF subset the
   traces spec asks for; Harbor-executed envs get ATIF for free, the OpenCode
   runner exports it. One trajectory format for benchmark + trace + training.
7. **(Phase 2/3, owner-armed)** — a real `terminal-bench` sweep behind
   `preflightRealBenchmarkSweep` for a first decision-grade Khala-vs-competitor
   number on a citable benchmark; feed the resulting ATIF + rewards into the
   Psionic training loop as the trajectory leg of the flywheel.

## 9. Honest-scope boundaries

- **Direction vs shipped:** only the Phase 0 fixture Gym (#6163–#6166) and the
  owner-gated `/gym/oss` (#6167) are landed. Everything about running Harbor as a
  benchmark executor is direction. We use only Harbor's *format* (ATIF) today.
- **Reference-repo discipline:** Harbor stays a read-only reference; we integrate
  at the job/artifact seam and do not vendor or fork it into our repos.
- **No published numbers** without an owner-armed real seam over realistic
  traffic; oracle/fixture runs are `decisionGrade:false`.
- **No fabricated numbers:** `not_measured` ≠ `0`; an unavailable lane/dataset is
  an honest skipped run; a zero-accepted-outcome group is a `null`
  cost-per-outcome finding.
- **Typed/semantic selection only:** env/lane/tool/coordinator are typed enums or
  semantic lookups; the Harbor dataset id is a typed field, never string-matched
  intent routing.
- **Public-safety:** never surface raw Harbor task prompts, solutions, hidden
  verifier content, canary strings, or raw ATIF; reuse `checkReportPublicSafety`
  and the retained-fixture boundary. **Schema reuse, never a parallel vocabulary.**
- The product-promise registry governs public claims; this doc widens nothing.
