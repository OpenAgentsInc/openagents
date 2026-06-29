# MirrorCode → Khala Gym Integration Analysis

> Status: **internal architecture analysis, 2026-06-27.** Direction-setting, not
> public claim copy and not a product promise. It studies how to integrate the
> Epoch Research **MirrorCode** benchmark into the Khala Gym (#6309), maps it onto
> the shipped benchmark harness, and proposes a phased plan. It flips no promise
> state and ships no code itself. Public-safe; no secrets.

## Why MirrorCode matters to us

MirrorCode is the single most on-thesis external benchmark for what Khala is
actually for. Our whole pitch — Khala → Pylon → Codex delegation, the gym
flywheel, the "10x daily tokens" goal — is that an agent backed by Khala can do
**sustained, real software work** and have it **verified**, not just answer a
short prompt. MirrorCode measures exactly that: an agent is asked to
**reimplement a real tool/library from scratch** (sed, brotli, ruff, a SQL
engine, a TeX subset, …), in a sandbox, over a **long horizon** — the longest
single sample in Epoch's paper ran **19 days** — and is scored by running the
agent's implementation against a held-out test suite it never saw.

That gives us three things our current gym rungs do not:

1. **A rigorous, externally-credible frontier coding rung.** Today the gym
   ladder (#6309) and head-to-head (#6308) measure Khala vs Big Pickle / free /
   paid-frontier lanes on a coding head-to-head. MirrorCode is a published Epoch
   Research benchmark with a paper, a real-software task distribution, and
   anti-contamination canaries. A Khala number on it is far harder to dismiss
   than a number on our own synthetic matrix.

2. **A real measurement of sustained reproduction**, not single-turn quality.
   MirrorCode scoring rewards getting an *entire program's observable behavior*
   right (including hidden/withheld test cases) — the exact capability our
   delegation thesis claims and the exact thing short-prompt evals miss.

3. **A legitimate, enormous token sink.** A real MirrorCode run burns
   token budgets measured in the **billions per sample** (the paper used a 1B
   token limit for S/M targets and 10B for L). Driving even a small bucket
   through `openagents/khala` is a large, *honest, internal-tagged* contribution
   to the public Khala token counter — directly aligned with the roadmap's
   "maximize the public Khala token counter" goal, as long as it is tagged
   `demand_kind=internal`, `demand_source=gym_ladder` per #6298 and stays
   preemptible behind real external demand (#6318).

The catch — and it is a real one — is **cost and wall-clock**. MirrorCode is the
opposite of a cheap smoke test. The integration value is high; the integration
discipline (small buckets first, honest gating) is what makes it safe.

## 1. What MirrorCode is

Verified against the local read-only reference clone at
`projects/repos/MirrorCode` (`README.md`, `mc/task.py`, `mc/__init__.py`,
`scripts/run_mirrorcode.py`, `example.py`, `pyproject.toml`, `mc/scorer.py`,
`docs/security-posture.md`, and task dirs `mc/sed/`, `mc/qsv_select/`,
`mc/ruff/`).

- **Built on Inspect** (the AISI eval framework, `inspect-ai==0.3.217` pinned in
  `pyproject.toml`). The benchmark *is* an Inspect `@task` (`mc/task.py:
  mirrorcode(...)`) with an Inspect dataset, solver, and scorer. Running it is
  `inspect_ai.eval(tasks=mirrorcode(), model=..., sample_id=...)`
  (`example.py`). Model selection is a standard Inspect model id string, e.g.
  `anthropic/claude-opus-4-7`, `openai/gpt-5.5`, `google/gemini-3.1-pro-preview`
  (`scripts/run_mirrorcode.py` docstring).
- **Public harness + public tasks; private tasks excluded.** The repo "is
  intended to be sufficient to run the benchmark" (`README.md`). Public task
  definitions ship; the private task set used in the paper does not. This is the
  same reference/backfill posture we already apply to Harvey: we can run the
  public surface; we do not get the hidden eval set.
- **Long-horizon, real-software reimplementation.** Each *target* in
  `TargetProgram` (`mc/__init__.py`) is a real tool: `sed`, `brotli`, `ruff`,
  `tssql`, `giac`, `cal`, `numfmt`, `uuidparse`, `qsv_select`, `wren_cli`,
  `mailauth`, `bib2json`, `pkl`, `cprepro`, `tex`, `choose`, `gron`, `hexyl`,
  `jq_simple`, `nonogrid`, `gotree`, `bitwise`, plus trivial excluded ones
  (`false`, `dirname`, `cal_simple`, `rev`). The agent is given task
  instructions, optional docs, optional source, and a set of **visible** test
  cases; it must produce an implementation in a chosen language
  (`AgentImplementationLanguage`: Python, C, Go, Rust, OCaml, Ada). A sample is a
  `(target, language)` pair.
- **Size buckets (paper Table 3), wired in `scripts/run_mirrorcode.py`:**
  - **S** (9 targets: `qsv_select`, `jq_simple`, `gron`, `bitwise`, `hexyl`,
    `uuidparse`, `numfmt`, `cal`, `choose`) — 1B token limit.
  - **M** (10 targets: `giac`, `tex`, `gotree`, `mailauth`, `brotli`,
    `wren_cli`, `nonogrid`, `sed`, `tssql`, `bib2json`) — 1B token limit.
  - **L** (3 targets: `ruff`, `pkl`, `cprepro`) — 10B token limit.
  - Default language policy is "1L": Go for `ruff`/`cprepro`/`nonogrid`, Python
    otherwise; `--language all6` runs every language.
- **Containerized, cloud-recommended execution.** Each sample runs in **four
  Docker containers** per `docs/security-posture.md`: a `workspace` (agent +
  reference binary, execute-only `chmod 2711` setgid), `reference-scoring`,
  `agent-scoring-visible`, and `agent-scoring-hidden`. The agent's container runs
  `network_mode: none`; a compiled `restricted-shell` installs a seccomp-BPF
  filter blocking ptrace/`process_vm_readv` to stop the agent from extracting the
  reference binary's behavior. Prebuilt images are published to
  `ghcr.io/epoch-research/mirrorcode`; the tag scheme is
  `get_identifier_for_image` in `mc/task.py`. The README is explicit: long runs
  "are likely impractical on a personal machine. Using the cloud is
  recommended." Paper results used METR's **Hawk** runner.
- **Scoring = behavioral test-suite reproduction with anti-cheat.**
  `mc/scorer.py` builds the agent's code in a clean scoring container *without*
  the reference binary, then runs **visible** and **withheld** (hidden + ablated)
  test cases, computing pass rates per group (`visible`, `hidden`, `withheld`,
  plus `exit_zero`/`exit_nonzero`). Hidden cases are semantically paired duals of
  visible ones (`visible_duals` in e.g. `mc/sed/sed.py`) specifically to deter
  hardcoding. A `gated_submit` threshold (default 0.3 of post-ablation visible
  cases) gates submission; a `solved_99` metric and the restricted-shell /
  no-reference-binary scoring container defeat the "wrap the reference binary"
  cheat. This is a strong, replay-style verifier — conceptually adjacent to our
  Tassadar "verification by replay" framing.
- **Contamination guards.** Both the MirrorCode canary
  (`mirrorcode:2b6c69c2-…`) and the BIG-Bench canary appear throughout (README,
  task source headers). The benchmark explicitly asks that its data never enter
  training corpora.

## 2. Fit with the Khala Gym (#6309)

Our shipped gym/benchmark layer
(`apps/openagents.com/workers/api/src/inference/benchmark/` +
`.../inference/gym/`) is a typed, public-safe, honesty-gated **measurement and
publication** system:

- A typed **matrix** of lanes (`matrix.ts`): `khala` is the protagonist
  (`available`); opponents include `bigpickle`, `gemini-free`, `openai-gpt`,
  `claude` (currently `fixture_only`), plus `available` provider lanes
  (`vertex-anthropic`, `vertex-gemini`, `fireworks`, `glm-52`, `gpt-oss-*`).
- A pluggable **lane seam** (`lane-seam.ts`) with a deterministic fixture lane
  and an owner-gated **real lane**; the concrete Khala transport
  (`real-lane-transports.ts`, `makeKhalaPublicTransport`) already points at the
  public OpenAI-compatible endpoint `https://openagents.com/api/v1` with model
  `openagents/khala` — own-capacity, zero rate card, internal-attribution
  headers.
- A **runner** + **report** that record canonical telemetry and emit a
  public-safe, dereferenceable report with `decisionGrade: true/false`.
- The **gym ladder** (`gym/ladder.ts`): three rungs (Big Pickle → free → paid
  frontier), each `published` only when a decision-grade real measurement
  exists, else `awaiting_owner` showing the gate — **fixture/synthetic numbers
  are never published as a measurement.**
- The **head-to-head** publication layer (`head-to-head.ts`, #6308): Khala vs
  the developer-default comparator set on `solveRate` + `costPerAcceptedOutcome`.

**Where MirrorCode slots in.** MirrorCode is *not* another lane on the existing
matrix — it is a **new, heavier benchmark family** that produces the same kind of
report the gym already knows how to publish. The cleanest framing:

- Add a **Rung 4 / "frontier-coding" tier** to the gym ladder (or a parallel
  MirrorCode sub-ladder) whose measurement is "MirrorCode public-bucket
  pass-rate (and cost-per-solved-target) for `openagents/khala` vs the same
  paper-reference models." This measures **sustained real-software reproduction**
  — a capability none of the current rungs (single coding task head-to-head)
  capture.
- The axes line up: MirrorCode's per-group pass rate maps onto our
  `verified-rate`/`acceptedOutcomes`, and tokens-spent-per-solved-target maps
  onto our `cost-per-accepted-outcome`. A solved MirrorCode target *is* an
  accepted outcome with an unusually strong verifier behind it.
- **Token burn is the feature, not a side effect.** Because each sample spends
  ≥1B tokens, a single bucket run is a material, honest contribution to the
  public Khala token counter — provided every request carries the
  `internal` / `gym_ladder` (or a new `gym_mirrorcode`) demand tags (#6298) and
  yields to real external demand (#6318). Counter movement is never the proof;
  the exact `token_usage_events` rows are.

## 3. Integration design

The owned-runner principle from the workspace `CLAUDE.md` (applied to Harvey)
governs here: **the upstream Python/Inspect harness is reference/backfill; owned
execution and publication belong in our gym, with the model provider as a runtime
adapter.** Two viable shapes, in increasing ownership:

### Option A (recommended first): run upstream Inspect, ingest the report

Treat Epoch's harness as the executor and our gym as the publisher.

1. **Khala as an Inspect model adapter.** MirrorCode takes any Inspect model id.
   Khala is OpenAI-compatible at `https://openagents.com/api/v1/chat/completions`
   with model `openagents/khala` (already encoded in
   `makeKhalaPublicTransport`). Point Inspect's OpenAI provider at that base URL
   with an `OPENAGENTS_AGENT_TOKEN` bearer, model `openagents/khala`. No
   provider code is written; Khala is *just another model* to Inspect. (Caveat:
   MirrorCode's default agent uses `CompactionSummary` and long tool loops —
   `mc/task.py` notes provider-specific compaction quirks — so the Khala endpoint
   must tolerate long multi-turn tool-calling sessions; validate this in the
   smoke phase.)
2. **Run the PUBLIC buckets only, smallest first.** Use
   `scripts/run_mirrorcode.py --model <khala-as-openai> --only s --language 1l`
   to bound scope. Start with a **single S target** (e.g. `qsv_select` or
   `cal`), not the whole bucket.
3. **Containerized cloud execution.** Reuse our existing cloud capacity (the
   GLM-fleet / "our cloud" GCE pattern) to host the four-container-per-sample
   Docker workload and pull prebuilt `ghcr.io/epoch-research/mirrorcode` images,
   rather than building locally. The 19-day-sample caveat is real: **do not** run
   L (`ruff`/`pkl`/`cprepro`) or `all6` until S/M economics are proven. Set a
   hard token-limit + wall-clock cap well below the paper's 1B/10B for the smoke
   and first bucket runs.
4. **Score → ingest → publish.** Inspect emits per-sample pass rates. A thin
   owned ingester converts each completed MirrorCode sample into a
   `GymLeaderboardReportInput`-shaped report (the same input
   `buildGymLadderLeaderboard` / the head-to-head layer already consume), tagged
   `decisionGrade: true` only for real runs, and publishes it on the existing
   `/api/public/gym/leaderboard` projection (new rung) and/or the head-to-head
   surface. Fixture/illustrative runs stay `decisionGrade: false` and are never
   shown as a measurement — the gym already enforces this.

This gets a credible, externally-anchored Khala number fastest, with the least
new code, while keeping publication inside our honesty-gated layer.

### Option B (later): owned runner

Port the *orchestration* (bucket selection, container lifecycle, scoring
ingest, attribution tagging, preemption) into a Bun/Effect owned runner under
`inference/benchmark/`, calling Khala through the existing real-lane transport,
and treat Epoch's `mc/` task definitions + scoring containers as the vendored
reference task spec. This matches the Harvey "owned execution in our runner"
mandate and lets the MirrorCode rung run on the same recurring cadence
(`per_model_release`) as the rest of the ladder. **Do not vendor large chunks of
`mc/` by default** (workspace policy); reference it, and only port the minimal
task metadata + the OpenAI-adapter + ingest glue we own.

### Contamination handling (non-negotiable)

- **Never train Khala (or any model) on MirrorCode tasks, sources, docs, or test
  cases.** Respect both canary strings; keep the `projects/repos/MirrorCode`
  clone read-only and out of any training corpus, dataset build, or RAG index.
- Keep MirrorCode traffic tagged and segmented so it is auditable and obviously
  an eval, not product data.
- Publish only public-bucket results, clearly labeled "Epoch Research MirrorCode,
  public tasks only (private set excluded)."

## 4. Risks, cost, caveats

- **Cost / wall-clock is the dominant risk.** ≥1B tokens/sample (S/M), 10B (L),
  up to 19 days/sample in the paper, "multiple days and thousands of dollars" for
  a full bucket sweep (`scripts/run_mirrorcode.py` warning). Mitigation: single
  target → S bucket only; hard token + wall-clock caps far below paper limits for
  the first runs; never auto-run L or `all6`; gate spendful/long runs behind the
  owner-armed real seam the gym already requires.
- **Image registry + container infra.** Needs Docker + GHCR pull of
  `ghcr.io/epoch-research/mirrorcode` (or local builds, slower) on cloud
  capacity that can run four containers per sample with `network_mode: none` for
  the agent. This is heavier than the current HTTP-only Khala transport.
- **Khala-endpoint compatibility.** MirrorCode is a long, tool-heavy,
  compaction-using Inspect agent loop. The public `/api/v1` path must sustain
  long multi-turn tool-calling and large context without truncating task
  instructions (the upstream code explicitly works around native-compaction
  dropping the task prompt). Validate in the smoke phase before any bucket run.
- **Private tasks excluded.** We can only ever report the public-task subset; our
  numbers are not directly comparable to the paper's private-set headline. Always
  label accordingly.
- **Contamination.** Covered above; the failure mode is silently ingesting the
  tasks into a training/RAG pipeline. Keep it read-only and tagged.
- **Honesty gating.** A MirrorCode rung must obey the same rule as every other
  rung: `published` only with a `decisionGrade: true` real measurement; otherwise
  `awaiting_owner` with the gate shown. No fixture MirrorCode number is ever
  published as a measurement.

## 5. Phased plan (mapped to issues to file)

1. **Phase 0 — Smoke a single small target against Khala (no publication).**
   Point Inspect at `openagents/khala`, run one S target (e.g. `cal` or
   `qsv_select`) with a hard token + wall-clock cap, in our cloud, pulling the
   prebuilt image. Goal: prove the Khala endpoint survives the MirrorCode agent
   loop and the four-container scoring flow completes end-to-end, producing a
   pass-rate. Output: a private smoke report + go/no-go on endpoint
   compatibility.
2. **Phase 1 — Ingest adapter + `awaiting_owner` rung.** Build the thin
   report-ingest that maps a MirrorCode sample result onto
   `GymLeaderboardReportInput`, and register a MirrorCode rung in the gym ladder
   that shows `awaiting_owner` + the owner gate (paid-spend / long-run approval)
   until a real bucket run lands. Ships the shape without a fabricated number.
3. **Phase 2 — Owner-armed S-bucket run + first published rung.** With owner
   spend/wall-clock approval, run the public **S** bucket (1L languages) for
   `openagents/khala` and at least one paper-reference comparator, tagged
   `internal`/`gym_mirrorcode`, preemptible behind real demand. Publish the rung
   with `decisionGrade: true`. Reconcile token spend against exact
   `token_usage_events` rows; confirm the public counter moved by at least that
   sum.
4. **Phase 3 — M bucket + recurring cadence; L only if economics allow.** Add M;
   wire the MirrorCode rung into the existing recurring ladder cadence
   (`per_model_release`). Treat L (`ruff`/`pkl`/`cprepro`, 10B, multi-day) as a
   separate, explicitly owner-gated experiment — do not fold it into the routine
   cadence by default.

Issues to file (under the #6309 gym ladder master, cross-ref #6308 head-to-head,
#6303 GTM, #6298 attribution, #6318 external-wins): one tracking issue for
"MirrorCode as a frontier-coding gym rung" with the four phases as a checklist,
or four scoped issues if parallelism is wanted. Phase 0 is non-spendful and can
start immediately; Phases 2–3 are owner-gated on spend + wall-clock.

## Recommendation (next concrete step)

**Yes — file a new gym issue under #6309** for "MirrorCode (Epoch Research) as a
recurring frontier-coding rung in the Khala gym ladder," scoped to the public
task buckets, with the phased plan above and the honesty/attribution/contamination
constraints called out. Then **execute Phase 0 only**: a single S-target smoke of
`openagents/khala` through upstream Inspect on our cloud, with hard caps, to
validate endpoint compatibility before committing any spendful bucket run. Keep
the upstream harness as reference/backfill (Option A); defer the owned Bun/Effect
runner (Option B) until the S/M economics and endpoint behavior are proven.

## References

- Local read-only reference: `projects/repos/MirrorCode` (`README.md`,
  `mc/task.py`, `mc/__init__.py`, `mc/scorer.py`, `scripts/run_mirrorcode.py`,
  `example.py`, `pyproject.toml`, `docs/security-posture.md`, `mc/sed/`,
  `mc/qsv_select/`, `mc/ruff/`).
- Khala gym ladder: `apps/openagents.com/workers/api/src/inference/gym/ladder.ts`.
- Benchmark harness + Khala transport:
  `apps/openagents.com/workers/api/src/inference/benchmark/` (`matrix.ts`,
  `lane-seam.ts`, `real-lane-transports.ts`, `head-to-head.ts`, `runner.ts`,
  `report.ts`, `index.ts`).
- Issues: #6309 (gym benchmark ladder), #6308 (external head-to-head), #6303
  (GTM), #6253 (Terminal-Bench replicate-and-beat), #6298 (demand attribution),
  #6318 (real external requests always win).
- Upstream: Epoch Research `epoch-research/mirrorcode`; Inspect
  (`inspect.aisi.org.uk`); METR Hawk (`github.com/metr/hawk`).
</content>
</invoke>

## Addendum — MirrorCode paper insights (MirrorCode_8ae911f.pdf, Epoch AI et al.; read 2026-06-27)

- **Definition:** reimplement entire CLI programs **from behavior alone** — execute-only access to the reference binary + visible tests + docs, **no source, no internet**; output must match byte-exact on end-to-end tests, including ~**34% HIDDEN** held-out tests (cheat-proof). 25 targets (Unix utils, serialization/query, bioinformatics, interpreters, static analysis, crypto, compression) across 6 languages (Python/C/Rust/Go/OCaml/Ada). **22/25 public** (132 task instances); 3 private held out. Buckets **S=10, M=11, L=4**.
- **Scaffold (what our runner uses):** Inspect **ReAct** agent + `text_editor` + `evaluate_testcases` (scores vs visible tests, non-terminal) + `submit`, in a **Docker** sandbox (execute-only target binary + language toolchain). Run with `openagents/khala` as the Inspect model.
- **Token budgets = THE burn lever:** **1B tokens per S/M task, 10B per L task.** One S/M run ≈ our entire **4x daily target (~1.3B) in a single useful job**; on Khala's own fleet that is our serving cost, not frontier $$. (Reference: 10B ≈ $5,000 on a frontier model; `gotree` (16K-line Go) solved in 14h / $251 / 2000-2001 tests.)
- **Scoring:** 100%-tests-passing = solved (stringent); ≥99% = near-perfect.
- **Frontier baselines (paper-reference; model ids forward-dated/illustrative — LABEL as such):** Claude Opus 4.7 **56%** perfect / 77% ≥99% (only model to solve Large); GPT-5.5 **44%** / 57%; Gemini 3.1 Pro Preview **32%** / 44%. 17/25 had ≥1 perfect run; `ruff` hardest. Solve rates ~equal across languages (generalized skill, not syntax memorization).
- **Official leaderboard:** Epoch is launching one at **epoch.ai/MirrorCode** — frame ours as **"Khala on MirrorCode"** next to those baselines.
- **Common failure modes:** edge cases (~40%), missing requirements (~10%), brittle/narrow (~5%), premature submission.
- **Product framing ("MirrorCode-as-a-Service"):** "point Khala at a CLI tool → it reimplements it from behavior alone, verified by tests." Hero demo = Khala reproducing real software (gotree-style). The benchmark validates the product.
