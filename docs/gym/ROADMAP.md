# Khala Gym + QA + Dogfood→Benchmark Flywheel — One Unified Roadmap

Updated: 2026-06-25

> **Status:** planning roadmap, honest-scope. This is the single, ordered build
> plan that unifies everything in `docs/gym/` (the Gym spec, the OpenCode
> head-to-head / Khala flywheel, the Harbor-on-Hydralisk benchmark backend), the
> autonomous-QA work in [`../qa/`](../qa/), and the relevant pillars of the Khala
> GTM push ([`../inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md)).
> It is organized as **epics → proposed GitHub issues (title + body)** so the work
> can be filed and handed to coding agents in order.
>
> These issue bodies are planning material. Actual bug filing still goes through the
> strict-bug form; product claims still go through the product-promise registry
> (owner-gated) and the Forum-first report flow. Nothing here is public-claim copy
> or a product promise. Anything not already shipped is labeled **direction**.

## North Star

**Tokens served per day — and bend the curve exponentially, honestly.** Every epic
below is justified by how it moves that one number (read off
`/api/public/khala-tokens-served` + `/history`, shown on `/stats` and `/khala`),
*and* keeps the number real (internal dogfood tokens tagged as such; no vanity
inflation; published benchmark numbers only from owner-armed real seams).

The three levers, in gating order (they overlap; this is what blocks what):

1. **Dogfood (Pillar 1)** — route everything we run through Khala. The only lever
   we fully control; moves the counter before a single external user arrives.
   Ships first, blocks on nobody. (Epics A, B.)
2. **Ecosystem tools (Pillar 2)** — be a one-config-line drop-in in the coding/agent
   tools developers already use. First external demand. (Epic C.)
3. **Benchmarking (Pillar 3)** — make it good and *prove* it: the Gym + Harbor +
   the head-to-head ladder. Quality is what keeps tools pointed at us. (Epics D, E.)
4. **Measurement & honesty (cross-cutting)** — the analytics, cost basis, and
   counter that make all of the above legible and honest. (Epic F.)

The paid three-way-split economics are owner-gated and sequence **behind proof** —
they do not block dogfood or ecosystem landings.

## Status legend

`shipped` (live + verified) · `in-progress` · `direction` (specced, not built).

## What is already shipped (the floor this builds on)

- Khala: free, live, OpenAI-compatible `POST /api/v1/chat/completions`, single model
  `openagents/khala`; self-serve free key (`POST /api/keys/free`); free quota **2.5M
  tok / 2,000 req per UTC day** (env-tunable, raised on the cost model, #6232).
- Public **tokens-served counter**, now **realtime server-push over WebSocket**,
  monotonic + exact (#6231 + the authoritative-total fix).
- **Served-tokens recorder** writes one ledger row per completion with per-lane
  **`cost_amount`**; owner-gated **`GET /api/admin/inference-analytics`** (tokens +
  cost by provider/model/route/day). Cost model: real lane = Fireworks DeepSeek V4
  Flash, ~$0.24/Mtok blended (#6232).
- **OpenCode→Khala tool-calling** fixed across the Hydralisk + Fireworks adapters
  (content arrays + tool-call deltas preserved); ten concurrent OpenCode→Khala
  sessions ran. Runbook: `../inference/2026-06-25-opencode-khala-runbook-and-audit.md`.
- **Gym Phase 0**: public fixture `/gym` + typed `GymExperiment` + fixture run/report
  (#6163–#6166); owner-gated `/gym/oss` GPT-OSS latency playground (#6167).
- **Autonomous QA** (`apps/qa-runner`): real-browser, video + committed e2e test +
  verdict, BYO-model, `/trace/{uuid}` publish; epic #6181 closed.
- **Harbor on Hydralisk**: Hydralisk has already run Harbor for Terminal-Bench 2.0
  (89 tasks, 60 solved) with a committed evidence receipt + a
  `hydralisk-terminal-bench-summary` console script.
- Deploy gate hardened: the desktop verse-smoke is off the Worker deploy / pre-push
  critical path, bounded + scoped (#6234) — the pattern every new gate must follow.

---

## EPIC A — Internal dogfood: route everything we run through Khala

> Pillar 1. The fastest, most honest counter lever. Each child makes one internal
> system default to `openagents/khala` (premium/`claude` lanes still route to the
> balance+premium gate, never the free lane).
>
> **Not yet filed (deferred by owner):** A1–A3 below have no GitHub issue yet — file
> them when the dogfood wiring is ready to start. Epics B–F are filed (#6237–#6252).

### A1. Autopilot / Raynor default their agent inference to Khala
**Type:** task · **Lever:** dogfood · **Status:** direction
**Why:** Autopilot is the gateway's designated anchor buyer — every coding session is
captive first-party demand; Raynor's forum/progress posting is steady reasoning
traffic. Coding is the wedge, so this is high-value dogfood + a correctness signal.
**Scope:** point Autopilot's coding-session model + Raynor's reasoning calls at
`openagents/khala` via the existing model-config seam; keep premium lanes on the
balance+premium gate. Tag the traffic source so analytics can split it.
**Acceptance:** an Autopilot coding session and a Raynor post both move
`/api/public/khala-tokens-served`; analytics attribute the tokens to the internal
source; no premium call silently hits the free lane.
**Refs:** GTM §2.2; `2026-06-19-inference-gateway-business.md` §2.

### A2. Product seams default to Khala (Sites, forum agent, onboarding, Artanis, Concierge)
**Type:** task · **Lever:** dogfood · **Status:** direction
**Why:** "one internal seam, many internal callers, all counted" — every product that
calls a model directly is uncounted demand we already pay for elsewhere.
**Scope:** find each direct model call in the product surfaces and route it through
the Khala lane (free where model fit allows; premium via the gate). One shared
client, not N ad-hoc integrations.
**Acceptance:** each converted surface shows tokens on the counter under an internal
source tag; a test asserts no surface calls a third-party model directly bypassing
the gateway.
**Refs:** GTM §2.3.

### A3. Verse 3D visualization drives NPC/scene/narration inference through Khala
**Type:** task · **Lever:** dogfood · **Status:** direction
**Why:** the Verse already renders each Khala request as energy fanned to Pylons;
driving its own inference through Khala makes the visualization literally show its
own traffic and adds tokens.
**Scope:** route Verse NPC/scene/narration model calls to `openagents/khala`; wire the
realtime counter/feed the Verse already mirrors.
**Acceptance:** opening the Verse generates Khala traffic visible on the counter +
the in-world fan-out.
**Refs:** GTM §2.5; `../khala/khala-in-the-world.md`.

---

## EPIC B — Autonomous QA: Khala dogfood lane #1 + QA on every push

> Pillar 1's highest-value first move, and the owner's explicit ask. Full audit:
> [`../qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md`](../qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md).

### B1. qa-runner: default its model backend to Khala  ([#6237](https://github.com/OpenAgentsInc/openagents/issues/6237))
**Type:** task · **Lever:** dogfood · **Status:** shipped (#6237)
**Why:** QA runs continuously doing real browser work with verified verdicts — a
steady token floor on the North Star *and* a continuous correctness signal on Khala
over the exact code/verification workload it must be good at. Dogfood lane #1.
**Scope:** set `qa-runner`'s BYO-model config to `openagents/khala`, base
`https://openagents.com/api/v1`, key from `POST /api/keys/free` (`QA_MODEL` /
`QA_BASE_URL` / `QA_API_KEY`); keep BYO-model override intact (Khala is the default,
not a hard dependency). Tag QA traffic as internal.
**Acceptance:** a `qa run` moves the counter; the verdict + video + committed e2e
test are produced as today; analytics attribute the tokens to QA (internal).
**Refs:** QA audit §2; GTM §5.1.

**Shipped 2026-06-25:** the BYO `qa` CLI defaults to `openagents/khala` at
`https://openagents.com/api/v1`, preserves flag/env overrides, keeps
`--fake-model` no-network, and sends public-safe `internal` / `qa-runner`
attribution headers that the served-token recorder stores in ledger metadata.

### B2. QA on every push — Tier 1: bounded, scoped pre-push smoke (no GitHub Actions)  ([#6245](https://github.com/OpenAgentsInc/openagents/issues/6245))
**Type:** task · **Lever:** dogfood/quality · **Status:** shipped (#6245)
**Why:** the owner wants every push to run QA; the repo has a hard **no-GitHub-Actions**
invariant, so it must live in the local pre-push hook / our own infra — and must not
become the next verse-smoke (#6234).
**Scope:** add a QA stage to `.githooks/pre-push` that runs **only against changed
user-facing surfaces** (mirror `run-if-desktop-changed.ts`), **hard-timeout-bounded**
(`run-bounded.ts`), deterministic/`--fake-model` or a single short real-browser
check. Start **warning-only**; promote to blocking only once provably non-flaky. It
must yield (report `incomplete`) rather than block/SIGKILL if it can't finish.
**Acceptance:** pushing a change to a user-facing surface runs the scoped smoke
within the timeout and prints a clear verdict; pushing unrelated changes skips it; it
never forces `--no-verify`.
**Refs:** QA audit §3 (Tier 1), §4; #6234 lesson.

**Shipped 2026-06-25:** `.githooks/pre-push` runs
`scripts/qa-pre-push-smoke.ts` after `check:deploy`. The smoke scopes itself to
changed user-facing surfaces, runs deterministic `qa run --fake-model` under
`OA_QA_PRE_PUSH_TIMEOUT_MS` (default 60s), skips unrelated pushes, and remains
warning-only so a failed or incomplete QA run never forces `--no-verify`.

### B3. QA on every push — Tier 2: full async QA pass on our GCE runner  ([#6238](https://github.com/OpenAgentsInc/openagents/issues/6238))
**Type:** epic · **Lever:** dogfood/quality · **Status:** shipped (#6238)
**Why:** the authoritative, non-blocking, owned-infra home for the full matrix — what
the no-Actions invariant intends ("autonomous/unattended execution on OUR GCE").
**Scope:** trigger a full `qa-runner` matrix (model backend = Khala) on push/deploy
via `oa-codex-control` + GCE; publish green VERIFIED traces + videos to
`/trace/{uuid}` + `/pro`; on PRs, post the agent comment via `pr-comment-run.ts`.
Non-blocking and loud.
**Acceptance:** a push triggers an async GCE QA run whose traces/videos land at
`/trace/{uuid}` + `/pro`; tokens show on the counter; a failing check is reported
loudly without blocking the push/deploy.
**Refs:** QA audit §3 (Tier 2); "our cloud = OpenAgents GCE".

**Shipped 2026-06-25:** `.githooks/pre-push` now launches warning-only
`scripts/qa-async-gce-trigger.ts` after the Tier 1 smoke. The trigger posts an
`openagents.codex_placement_assignment.v1` assignment to `oa-codex-control`'s
`/v1/placement/start` surface, pins the lane to `cloud-gcp`, asks the GCE runner
to run the Khala-backed full QA matrix, publish `/trace/{uuid}` and `/pro`
evidence, and post the `pr-comment-run.ts` verdict when a PR number is supplied.
The hook remains non-blocking: missing owner-gated env skips, control failure
prints a warning, and pushes still proceed after `check:deploy` is green.

---

## EPIC C — Ecosystem tool landings (Pillar 2): one-config-line drop-ins

> First external demand. Each child = a verified, published "point your tool at us"
> recipe with a test checklist. Ordered by coding-traffic leverage.

### C1. Publish the OpenCode → Khala recipe (first external landing)  ([#6239](https://github.com/OpenAgentsInc/openagents/issues/6239))
**Type:** task · **Lever:** ecosystem · **Status:** shipped 2026-06-25
**Why:** OpenCode is the cleanest first landing — config-driven OpenAI-compatible
provider, coding wedge, exercises tool-calling. Tool-calling is already fixed.
**Scope:** finalize + publish the exact `opencode.json` recipe (base
`https://openagents.com/api/v1`, model `openagents/khala`, free key); resolve the
model-key selector double-segment question (keep `openagents/khala` vs add a shorter
server-accepted key) and document the chosen one — no ambiguous instructions.
**Acceptance:** a fresh user follows the published recipe, runs an OpenCode coding
task end-to-end (tool-calling + streaming), and sees their tokens on the counter; the
402/quota path is a legible error, not a crash.
**Refs:** GTM §3 "First target: OpenCode"; the runbook + `../opencode/`.

**Shipped:** `../opencode/opencode-khala-recipe.md` is now the canonical recipe.
The selector decision is model key `khala` with `api.id: "openagents/khala"`,
which displays `openagents/khala` in OpenCode and sends the same public model id
upstream. The support docs now use the real free-key response field
`credential.token` and the current free tier, 2,000 requests/day plus 2,500,000
tokens/day per key.

### C2. Land the next tools: Aider → Cline/Continue → Vercel AI SDK → LiteLLM/LangChain  ([#6240](https://github.com/OpenAgentsInc/openagents/issues/6240))
**Type:** epic · **Lever:** ecosystem · **Status:** shipped 2026-06-25
**Why:** breadth of one-config-line adoption across the coding/agent ecosystem; the
Vercel AI SDK recipe is high-leverage (substrate under many tools).
**Scope:** one verified recipe + test checklist per tool, in priority order; do our
own current research on which to integrate (don't trust stale training data). Track
per-tool token attribution.
**Acceptance:** each landed tool has a published recipe and shows attributable tokens
on the counter via per-tool analytics.
**Refs:** GTM §3 "Next tools after OpenCode".

**Shipped:** `../opencode/khala-ecosystem-tool-recipes.md` publishes current
recipes for Aider, Cline, Continue, AI SDK, LiteLLM, and LangChain. The recipe
set records upstream research sources, uses the current Khala free-key shape,
documents which clients can set `x-openagents-*` attribution headers today, and
uses fresh per-tool keys plus public counter deltas for clients that cannot set
headers. Owner-gated per-tool rollups remain the F1 analytics issue (#6252).

---

## EPIC D — The Gym: environments, policy matrix, paid runs, training loop, leaderboard

> Pillar 3. The lab where we train Khala and the benchmark-as-a-service product.
> Spec: [`openagents-gym.md`](openagents-gym.md); Episode 243 deltas:
> [`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md).

### D1. Phase 1 — competitor lanes + the OpenCode client-surface environment  ([#6246](https://github.com/OpenAgentsInc/openagents/issues/6246))
**Type:** epic · **Lever:** benchmarking · **Status:** shipped 2026-06-25
**Why:** the first real head-to-head: compare *model endpoints through a real coding
agent*, not just supply lanes. BigPickle (OpenCode's default free model) is rung 1.
**Scope:** add typed `BenchmarkLane` values for competitor endpoints (`bigpickle`,
`gemini-free`, `openai-gpt`, `claude`) + the own/open lanes (`gpt-oss-20b`,
`gpt-oss-120b`, `glm-52`) with honest `LANE_AVAILABILITY`; add an **OpenCode client
runner** that provisions `opencode.json`, runs a fixed task, and extracts
tokens (from the provider `usage`, never estimated), wall-clock,
**tool-call-completion**, and the independent verifier verdict; reuse the existing
matrix→runner→report + `checkReportPublicSafety`.
**Acceptance:** a fixture run compares Khala vs BigPickle on one OpenCode coding task,
scored on cost-per-accepted-outcome + verified-rate + tool-call-completion, with a
`decisionGrade:false` labeled report.
**Refs:** flywheel doc §3, §9; the OpenCode-via-Khala memo in `../opencode/`.

**Shipped:** `workers/api/src/inference/benchmark` now includes the typed OpenCode
endpoint lanes (`khala`, `bigpickle`, `gemini-free`, `openai-gpt`, `claude`) and
own/open lanes (`gpt-oss-20b`, `gpt-oss-120b`, `glm-52`). `fixture_only` availability
lets the deterministic fixture compare Khala vs BigPickle without pretending a
real/billable executor exists. `opencode-client-runner.ts` provisions public-safe
`opencode.json`, rejects missing provider `usage` instead of estimating tokens,
records wall-clock/tool-call success/verifier verdict, and feeds the existing
matrix→runner→report path. `OPENCODE_HEAD_TO_HEAD_GYM_EXPERIMENT` produces a
public-safe `decisionGrade:false` report over one OpenCode coding task.

### D2. Phase 1 — register the first environments (Terminal-Bench, khala-code, long-context, M8)  ([#6241](https://github.com/OpenAgentsInc/openagents/issues/6241))
**Type:** task · **Lever:** benchmarking · **Status:** shipped 2026-06-25
**Why:** an env without its verifier+acceptance contract is not runnable; these are the
first task sets the ladder runs on. Terminal-Bench rides Harbor (Epic E).
**Scope:** typed `GymEnvironment` registry entries (task set + verifier + acceptance
contract + default realistic shapes), selection typed/semantic only.
**Acceptance:** each env runs through the fixture seam with its grader bound; a run
cannot start without the env's verifier.
**Refs:** gym spec §3, §10; flywheel doc §9.

**Shipped:** `workers/api/src/inference/gym/experiment.ts` now has a typed
`GYM_ENVIRONMENT_REGISTRY` with task-set, verifier, acceptance-contract, default
shape, and default tool bindings for `terminal-bench`, `khala-code`,
`long-context-codebase-qa`, and `m8-head-to-head`, alongside the existing bundled
decision suite and OpenCode head-to-head. `compileGymExperiment` resolves the
environment through that registry, carries the grader binding in
`policySelection.environment`, and refuses unregistered/graderless environments.
Fixture experiments for all four Phase-1 environments run through the existing
matrix→fixture-seam→report path with `decisionGrade:false`.

### D3. Phase 2 — paid runs (owner-armed real seam → report receipt)  ([#6247](https://github.com/OpenAgentsInc/openagents/issues/6247))
**Type:** epic · **Lever:** benchmarking/revenue · **Status:** shipped 2026-06-25
**Why:** decision-grade numbers + benchmark-as-a-service revenue.
**Scope:** quote (`compileGymExperiment` + `LANE_AVAILABILITY` + samples) → balance
gate (`402`) → `preflightRealBenchmarkSweep` (budget cap, billable cap, realistic-
traffic evidence, approval ref) → real seam → receipt-first `MeteringHook` →
public-safe report receipt; cost-per-accepted-outcome consumes the real per-lane
`cost_amount` (Epic F). Splits ride the revenue-loop spine.
**Acceptance:** a funded account pays to run a real billable Khala-vs-BigPickle sweep
over realistic traffic and gets a `decisionGrade:true` report receipt; an un-armed
env cannot issue a billable request.
**Refs:** gym spec §6, §8 Phase 2; the cost-model doc.

**Shipped:** `workers/api/src/inference/gym/paid-run.ts` now prepares paid Gym
runs without spending during compile: it quotes executable matrix cells with
`priceRequest`, returns an explicit `payment_required`/`402` balance gate for
unfunded accounts, requires `preflightRealBenchmarkSweep` for budget caps,
billable-sample caps, realistic traffic evidence, and owner approval, then arms
the real lane seam only for startable plans. Owner-armed runs can declare a
narrow real executor for otherwise `fixture_only` lanes such as BigPickle, so the
global lane registry stays honest while the paid Khala-vs-BigPickle acceptance
path can run. The prepared run emits `MeteringContext`s for the existing
`MeteringHook` and builds a public-safe report receipt with `decisionGrade:true`
only after the real preflight passes. Tests inject the real executor, so CI covers
the funded acceptance path without live provider spend.

### D4. Phase 3 — Gym → training loop, and the Gym runs on Khala (the flywheel)  ([#6248](https://github.com/OpenAgentsInc/openagents/issues/6248))
**Type:** epic · **Lever:** benchmarking/dogfood · **Status:** direction
**Why:** the tightest flywheel — Gym reports are the eval+reward artifacts that train
the coordinator, and the Gym's own runner/eval inference is itself dogfood traffic.
**Scope:** feed Gym reports (executed verdict + cost-per-accepted-outcome) to GEPA /
TRINITY / Conductor training in Psionic; winners return as shadow candidates and
re-enter the head-to-head; promotion is an approval-gated `runtime_promotion`. Route
the Gym's own client-runner/eval inference through `openagents/khala` (counter moves).
**Acceptance:** a candidate trained on Gym-produced reward beats the heuristic in
shadow on cost-per-accepted-outcome, then is promoted via approval; Gym runs add
attributable internal tokens to the counter.
**Refs:** gym spec §5, §8 Phase 3; flywheel doc §6.

### D5. Phase 4 — public-safe leaderboard + (gated) plugin/module composition split  ([#6249](https://github.com/OpenAgentsInc/openagents/issues/6249))
**Type:** task · **Lever:** benchmarking · **Status:** direction
**Why:** a recurring, citable quality bar; eventually a per-trace author split.
**Scope:** public-safe leaderboard projection over `decisionGrade:true` reports only
(keep fixture/synthetic runs out of any ranked surface); FUTURE/gated per-trace
revenue split to composed-module authors, boundary intact (no public marketplace).
**Acceptance:** a leaderboard ranks only decision-grade reports with public-safe
fields; the author split is modeled on evidence behind owner arming.
**Refs:** gym spec §8 Phase 4, §10.

---

## EPIC E — Harbor on Hydralisk: the TerminalBench + benchmark backend

> How the Gym actually executes Terminal-Bench and other benchmarks. Full audit:
> [`2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`](2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md).
> Boundary: Harbor owns task sets/verifiers/trajectories; we own the typed config,
> Khala policy, cost-per-accepted-outcome, reports, metering, promotion. No Harbor
> code in the Worker.

### E1. Formalize the Worker/Gym → Hydralisk → Harbor dispatch seam  ([#6250](https://github.com/OpenAgentsInc/openagents/issues/6250))
**Type:** epic · **Lever:** benchmarking · **Status:** direction (Hydralisk has already run Harbor manually)
**Why:** Harbor is Python 3.12/uv/Docker and already provisioned on Hydralisk (our
own GPU infra) — formalize the out-of-process dispatch so the Gym can trigger runs.
**Scope:** Worker/Gym dispatches a job → Hydralisk runs `harbor run -d
terminal-bench/terminal-bench-2 --agent <agent> --model openagents/khala` (CLI +
artifacts first; library service later for RL token interception) → ingest the
sanitized result + trajectory (ATIF / `/trace/{uuid}`) back. Reuse the existing
`hydralisk-terminal-bench-summary` summarizer (schema
`hydralisk.evals.terminal_bench.summary.v1`) for the public-safe receipt.
**Acceptance:** the Gym triggers a Harbor Terminal-Bench run on Hydralisk against
`openagents/khala` and ingests a public-safe summary; no Harbor import in the Worker.
**Refs:** harbor doc §3 (Where Harbor runs — Hydralisk), §7, §8.

### E2. Distinct-device verifier via Harbor `environment_mode = "separate"`  ([#6251](https://github.com/OpenAgentsInc/openagents/issues/6251))
**Type:** task · **Lever:** benchmarking · **Status:** direction
**Why:** the Gym spec requires the verifier on a **distinct device** from the producer;
Harbor ships this as a feature.
**Scope:** run the agent container and the `no-network` verifier container on distinct
hosts/VMs (agent on a Pylon/Hydralisk lane, verifier on Psionic/another VM) using
`[verifier] environment_mode = "separate"` + explicit artifact handoff.
**Acceptance:** a Terminal-Bench run records the verifier executing on a different
device than the agent, with the reward read from the verifier's artifact.
**Refs:** harbor doc §3.4; gym spec §10.

### E3. Map Harbor reward → Gym cost-per-accepted-outcome; ingest Harbor trajectories for training  ([#6242](https://github.com/OpenAgentsInc/openagents/issues/6242))
**Type:** task · **Lever:** benchmarking/training · **Status:** direction
**Why:** Harbor's float reward IS the executed verdict the Gym multiplies by the real
per-lane cost basis; Harbor trajectories feed Khala training.
**Scope:** map Harbor's `reward.txt` → the Gym report's accepted-outcome; pipe
Harbor/ATIF trajectories into the Psionic training loop (Epic D4); guard against GPU
contention with live Khala serving lanes when scheduling runs.
**Acceptance:** a Harbor run produces a Gym report with cost-per-accepted-outcome from
the real cost basis, and a training-ready trajectory artifact.
**Refs:** harbor doc §3, §6; cost-model doc.

---

## EPIC F — Measurement & honesty (cross-cutting)

> Make the North Star legible and the claims honest. Several pieces shipped today;
> these are the gaps.

### F1. Internal-vs-external demand tagging across the counter + analytics  ([#6252](https://github.com/OpenAgentsInc/openagents/issues/6252))
**Type:** task · **Lever:** measurement · **Status:** direction
**Why:** dogfood tokens are real served tokens, but we must never imply external
traction we don't have. Every dogfood epic (A, B, D4) depends on this to report honestly.
**Scope:** tag each served completion with a demand source (internal-dogfood vs
external + a per-tool/per-system label); expose the split in
`GET /api/admin/inference-analytics`; keep the public counter total honest while the
breakdown stays owner-gated.
**Acceptance:** the admin analytics shows internal-vs-external + per-tool/per-system
token splits; no public surface implies external demand from internal tokens.
**Refs:** GTM §2 honesty note, §6; cost-model + analytics doc.

### F2. Per-day history + per-tool adoption surfaced for the North Star  ([#6243](https://github.com/OpenAgentsInc/openagents/issues/6243))
**Type:** task · **Lever:** measurement · **Status:** direction
**Why:** "we want the per-day history curve to bend upward and stay up" — the curve and
its per-tool decomposition are how we steer.
**Scope:** surface the tokens-served `/history` per-day curve + per-tool adoption (from
F1 tags) on `/stats` (+ owner views); keep `not_measured` ≠ `0`.
**Acceptance:** `/stats` shows the per-day curve; the admin view shows per-tool
adoption over time.
**Refs:** GTM §6.

### F3. Throughput/concurrency as a first-class Gym measurement (promote `/gym/oss` patterns)  ([#6244](https://github.com/OpenAgentsInc/openagents/issues/6244))
**Type:** task · **Lever:** benchmarking/measurement · **Status:** direction
**Why:** "we're in the inference business, we can't ship slow APIs" — tok/s and the
concurrency ceiling are product metrics (smoke tests reached ~9.5k tok/s; ten
concurrent OpenCode sessions ran; GLM-52 got a MTP2 speculative-decoding speed win).
**Scope:** generalize the `/gym/oss` 1→2→4→8 ramp + telemetry reconciliation into a
typed throughput/concurrency environment usable per lane (record spec-decoding
acceptance), feeding real latency/concurrency curves to the cost model.
**Acceptance:** a repeatable Gym run reports per-lane TTFT/TPS/ITL + the concurrency
point where latency/quota degrades, with `not_measured` distinct from `0`.
**Refs:** gym spec §3 (`/gym/oss`); flywheel doc §8; the GLM-52 REAP MTP2 win.

---

## Suggested filing order (one pass)

1. **F1 (#6252)** (demand tagging) — unblocks honest reporting for everything dogfood.
2. **B1 (#6237)** + **A1/A2** (qa-runner, Autopilot/Raynor, products → Khala) — move the
   counter now.
3. **C1 (#6239)** (publish the OpenCode recipe) — first external demand.
4. **B2 → B3** (QA on every push: pre-push smoke, then GCE async).
5. **D1 + D2 + E1** (Gym Phase 1 + Harbor/Terminal-Bench backend) — the head-to-head.
6. **E2/E3, D4, F3** (distinct-device verifier, training loop, throughput) — quality
   + flywheel.
7. **D3, C2, D5, A3, F2** (paid runs [owner-gated], more tools, leaderboard, Verse,
   history surfacing) — broaden behind proof.

> Cross-refs: Gym spec [`openagents-gym.md`](openagents-gym.md) · Episode 243 deltas
> [`2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md)
> · Harbor [`2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`](2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md)
> · QA [`../qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md`](../qa/2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md)
> · GTM [`../inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md).
