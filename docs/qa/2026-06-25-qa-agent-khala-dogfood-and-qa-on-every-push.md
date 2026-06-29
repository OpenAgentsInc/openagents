# Autonomous QA — Khala dogfood lane #1, and "QA on every push"

Updated: 2026-06-25

> **Status:** audit + direction, honest-scope. This evaluates (a) routing
> `qa-runner` through **Khala** as the first dogfood move of the Khala inference
> GTM push, and (b) the owner's ask that **every push to our GitHub run the QA
> agent**, given this repo's hard **no-GitHub-Actions** invariant. It builds on
> the GTM push memo
> ([`../inference/2026-06-25-khala-inference-gtm-push.md`](../inference/2026-06-25-khala-inference-gtm-push.md),
> §2 Pillar 1 + §5 Sequence) and the shipped autonomous-QA epic (#6181). Where it
> cites a live system it says so; where it sets direction it labels it
> **direction**. It flips **no** promise state — the product-promise registry is
> owner-gated. Nothing here is public-claim copy or a product promise. Secrets are
> referenced by file/label only; no key value appears here.

---

## 0. TL;DR

- `qa-runner` is shipped and real today: it drives a **real browser**, emits a
  **video + a committed e2e test + an honest pass/fail verdict**, is OSS /
  local-first / **BYO-model**, and can publish a shareable redacted `/trace/{uuid}`.
  Khala is now the default backend, while flags/env preserve third-party and
  local BYO overrides. (§1)
- **Route `qa-runner` → Khala is shipped (#6237):** the BYO-model config now
  defaults to `openagents/khala`, base `https://openagents.com/api/v1`, with a
  free key from `POST /api/keys/free`; flags/env still override. This is dogfood
  lane #1: it is already running, it is a steady **token floor** on the North Star
  (tokens served per day), and it is a continuous **correctness signal** on Khala
  over the exact code/verification workload Khala is meant to be good at. QA
  dogfood traffic is tagged `internal` / `qa-runner` in served-token metadata.
  (§2)
- **"QA on every push" cannot be a GitHub Action** — `check:no-github-actions`
  fails the deploy gate if any `.github/workflows/*.yml` exists. The real options
  are: a bounded **git pre-push hook** stage, the **deploy pipeline**, or **our own
  GCE runner** (`oa-codex-control` + GCE). Recommendation: a **tiered combo** —
  a fast, bounded, scoped smoke on pre-push, plus the **full QA pass async on our
  GCE runner** triggered by the push, which publishes the green VERIFIED traces +
  videos. (§3)
- **Tier 1 pre-push QA smoke is shipped (#6245):** `.githooks/pre-push` now runs a
  warning-only `scripts/qa-pre-push-smoke.ts` stage after `check:deploy`. It is
  scoped to changed user-facing surfaces, hard-timeout-bounded, deterministic
  (`qa run --fake-model`), and yields on failure/incomplete instead of forcing
  `--no-verify`. (§3, §4)
- **Tier 2 async GCE trigger is shipped (#6238):** the same pre-push hook now
  launches warning-only `scripts/qa-async-gce-trigger.ts` after Tier 1. When the
  owner-gated control env is armed, it posts an
  `openagents.codex_placement_assignment.v1` assignment to
  `oa-codex-control`'s `/v1/placement/start` endpoint, pins `cloud-gcp`, and asks
  the GCE runner to run the Khala-backed full QA matrix, publish `/trace/{uuid}`
  + `/pro` evidence, and post the existing `pr-comment-run.ts` verdict on PRs.
  Missing config skips; failed launch warns; neither blocks a green deploy gate.
  (§3, §4)

---

## 1. What `qa-runner` is today

`@openagentsinc/qa-runner` (v0.1.0, `apps/qa-runner/`) is the OSS, local-first,
runtime-agnostic autonomous-QA runner — the substrate and headline demo for the
Khala autonomous-QA flow (epic #6174, rolled up into #6181). The public pitch
(`apps/qa-runner/QA-RUNNER.md`) still emphasizes the standalone/BYO posture:

> drives a **real browser** against any target, verifies a check the way a person
> would, and emits a **video + a committed e2e test + an honest pass/fail
> verdict**. … It's OSS, local-first, and **bring-your-own-model — no OpenAgents
> login required.** Khala is one optional backend, not a dependency.

### What a run produces (path evidence)

A run writes, into its `--out` artifact dir (`apps/qa-runner/QA-RUNNER.md`,
`README.md` §Artifacts):

- `result.json` — the verdict (`status: pass | fail`), plus an additive
  `verify` field (CONFIRMED / REFUTED / INCONCLUSIVE against declared
  commitments, #6192) and an additive `receipt` field.
- `session.mp4` (or `session.webm` if ffmpeg is unavailable — reported in
  `result.json.artifacts.videoFormat`).
- `trace.zip` (Playwright trace) + `NN-step.png` per-step screenshots.
- the distilled **`*.e2e.test.ts`** (default `generated/<slug>.e2e.test.ts`) — a
  re-runnable, black-box test with named user-readable steps and deterministic
  waits; point `TARGET_URL` at dev or prod to run it anywhere.
- `session-trace.json` (Khala runs) — the deterministic, public-safe
  `KhalaSessionTrace` (`openagents.khala.session_trace.v1`), the distiller's input.

The exit code is honest: `0` only on a clean pass **and** an admissible distilled
test; a real deviation yields a FAIL visible in the video — **never a fake green**.

### BYO-model, no login (the core run is provider-agnostic)

The core path runs on your machine, against your target, driven by the default
Khala endpoint or **any OpenAI-compatible model** you bring, with **no OpenAgents
account/login required** (`apps/qa-runner/README.md`; issue #6191 / Rhys req #5).
Model/base overrides keep the BYO contract intact, while the default dogfood lane
requires a free `oa_agent_…` key from `POST /api/keys/free`. Model
selection precedence (`src/byo-model.ts`):

```
model    : --model    > QA_MODEL    > OPENAI_MODEL    > openagents/khala
base-url : --base-url > QA_BASE_URL > OPENAI_BASE_URL > https://openagents.com/api/v1
api-key  : --api-key  > QA_API_KEY  > OPENAI_API_KEY  > (required, unless --allow-keyless)
```

The key **value is never printed** — only its source label is logged. The shipped
`qa` CLI is a self-contained `dist/qa.js` bundle (workspace deps inlined; only
`playwright` external), so a standalone install needs no workspace and no login.

### The harness quality bar (the "last 10%", #6193)

`apps/qa-runner/docs/harness-quality-bar.md` is the reviewer checklist that makes
the green trustworthy:

1. **Honest outcomes — no flaky-pass, ever.** Passing steps assert real outcomes
   about the target, not tautologies; a false assertion / thrown error / timed-out
   step / refusal / interrupt all yield `status: "fail"`; retries are opt-in,
   bounded, and **visible** (`detail.attempts > 1`), never a cover-up.
2. **Deterministic** — waits are conditions (`wait-for`), never sleeps; timeouts
   and retries are clock-injectable; sharding is completion-driven and
   order-stable.
3. **Artifacts always flush — even on crash / interrupt** (the load-bearing
   guarantee): `Effect.ensuring(..., flushResult)` + the browser surface's
   release block leave `trace.zip`, video, and a schema-valid `result.json` on
   disk even on throw or SIGINT; a partial run is an honest `fail`.
4. **Fast path — parallel beats serial** (`runShards`, asserted margin in
   `shard.test.ts`).
5. **Public-safe by construction** — `assertPublicSafeResult` walks `result.json`
   before write and rejects tokens/secrets/prompts/cookies/credentials; typed
   text of a `type` step is never recorded (selector + length only); read-only
   targets (prod by default, #6190) refuse mutating steps.

### Drivable over HTTP (the QA Control API, #6196)

The whole flow — submit → run → fetch artifacts + verdict + `/pro` link — is also
API-first via a **qa-runner HTTP daemon** (`bun run api` /
`src/daemon.ts --api`), because the runner drives a real Chrome which cannot run
in a Cloudflare Worker. Auth is a Khala agent bearer token; a deterministic
**mock path** runs with no Chrome/network/spend by default; real runs are
owner-gated by `QA_CONTROL_ARM_REAL=1`, with a per-run `QA_CONTROL_TOKEN_BUDGET`
cap. Full curl walkthrough: `apps/qa-runner/docs/control-api-quickstart.md`.

### Shareable traces (`/trace/{uuid}`)

When run through the OpenAgents-backed path, a session is published as a redacted,
shareable **trace** — the full timeline with the recording and screenshots served
inline. Env-armed and honest no-op if unset (`apps/qa-runner/RUNBOOK.md` §2):

```sh
export QA_TRACE_PUBLISH_URL="https://openagents.com/api/traces"
export QA_TRACE_PUBLISH_VISIBILITY="public"        # or unlisted / owner_only
export OPENAGENTS_AGENT_PENDING_TOKEN="…"           # from a ~/work/.secrets/*.env file
```

`publishRunDir(...)` (`src/publish-trace.ts`) converts run → ATIF → **redacts** →
POSTs → prints `https://openagents.com/trace/{uuid}`. Secrets/PII/paths are
redacted before upload and the ingest tripwire rejects real leaked values. Proven
live 2026-06-24: a real prod run published
`https://openagents.com/trace/db838bdc-3bc6-48a5-8715-a6669f6b10c5` (11 steps,
`openagents/khala`, public, with video).

### Current epic state (#6181)

The out-ship-Factory QA epic **#6181 is CLOSED** (`EPIC: Out-ship Factory — full
Khala autonomous-QA vision`). Shipped: computer-use tools + qa-runner + capture +
distiller→committed test + demo (#6174), refusal posture (#6179), `/pro` shell
(#6180), balance-gate exemption armed + proven (#6180). It records that the core
vision is **proven live** (Zeratul, zero balance, `openagents/khala`): "Khala
autonomously drives real dev tools on our own infra, distills the session into a
committed e2e test, and emits a video — at \$0 (operator-credit exemption)." Where
we already beat Factory: a **committed, re-runnable e2e test** (they emit only
videos + reports); runtime-agnostic + own-infra + no funding; real
anti-fabrication (the distilled test passes against prod). Remaining children
spanned #6182–#6196 (OpenRouter provider layer, chill-evals + `/pro`, gh-attach
PR-evidence loop, driver/OS breadth, verify verdict, harness hardening, QA control
API, etc.).

---

## 2. Route `qa-runner` → Khala (dogfood lane #1)

### Why this is the highest-value first move

The GTM push memo (`../inference/2026-06-25-khala-inference-gtm-push.md`, §2
Pillar 1) ranks autonomous QA **#1** in "how much traffic they can move":

> **Autonomous QA (`qa-runner` → Khala).** … does real browser work and leaves
> green VERIFIED traces. Route its agent inference through `openagents/khala`. QA
> runs continuously, so it is a steady token floor and a continuous correctness
> signal on Khala itself. **Highest-value first move:** it is already running, it
> is internal, and it stress-tests the exact code/verification workload Khala is
> meant to be good at.

And §5 Sequence puts it first: *"Internal dogfood demand (Pillar 1), starting now.
Route qa-runner → Khala, then Autopilot/Raynor, then the rest of our products."*

This serves three goals at once:

1. **North Star — tokens served per day.** The memo: *"We are now in the
   inference business. The one metric that matters is tokens served per day, and
   we want it exponential."* Continuous QA is a steady **token floor** that the
   counter never drops below.
2. **Correctness signal on Khala.** Every QA run is a real
   computer-use/code/verification workload, so a regression in Khala shows up as a
   drop in QA pass-rate or a REFUTED verify verdict — a continuous, honest health
   check on the model itself.
3. **Realistic traffic for the Gym.** The verified traces + acceptance verdicts
   are exactly the decision-grade traffic the Gym needs (see §GYM cross-ref and
   `docs/gym/`).

### The exact config

Mint a free key (no signup, no payment — `docs/faq/khala-inference-quickstart.md`):

```sh
curl -X POST https://openagents.com/api/keys/free
#   -> { "tier":"free", "model":"openagents/khala",
#        "credential": { "token":"oa_agent_…" },
#        "quota": { "maxRequestsPerDay":2000, "maxTokensPerDay":2500000, "window":"utc_day" } }
```

The BYO-model config now defaults to Khala (handout card values) when model/base
are omitted:

```sh
export QA_API_KEY="oa_agent_…"                        # the free key above; never printed

bun run --cwd apps/qa-runner qa run \
  --url https://openagents.com \
  --goal "open /login and confirm the sign-in form renders" \
  --out ./runs/khala-dogfood
```

`QA_MODEL` / `QA_BASE_URL` (or `--model` / `--base-url`) still override the
defaults, so third-party OpenAI-compatible endpoints remain BYO. Equivalent
explicit flags: `--model openagents/khala --base-url https://openagents.com/api/v1
--api-key "$OA_AGENT_TOKEN"`. The dedicated headline path is `demo:khala`
(`src/demo-khala.ts`), which resolves the endpoint from env + `~/work/.secrets/`
in order: `OPENAGENTS_API_KEY` → a discovered `OPENAGENTS_AGENT_TOKEN` in
`~/work/.secrets/*.env` → `PROBE_OPENAI_API_KEY` as a clearly-labeled
loop-proving fallback (`--no-fallback` forbids it). One model surface only:
`openagents/khala` (no `-code`/`-mini`/`-pro` variants).

To also publish the green VERIFIED trace, set the §1 publish env vars; a finished
`control.ts` / `pr-comment` run publishes automatically.

### Quota headroom

Free tier is now **2,500,000 tokens/day · 2,000 requests/day** per key (resets at
UTC midnight; env-tunable `FREE_TIER_MAX_*`). That is ample headroom for a
continuous internal QA cadence; if QA volume ever approaches the cap, route it
through an authenticated/metered key rather than the free tier so external
free-tier users keep their headroom.

### Honest caveat — internal vs external tagging

The GTM memo's honesty rule (§2, §6) applies directly: *"internal dogfood tokens
are real served tokens and may be reported as such, but we should be able to
**distinguish** internal vs external demand in our own analytics so we never imply
external traction we do not have."* QA dogfood tokens must be **tagged internal**
in the inference analytics (the owner-gated
`GET /api/admin/inference-analytics` already breaks down by
provider/model/route/day; add an internal-vs-external dimension or use a
dedicated key class for dogfood so the split is queryable). Tokens served is real;
external traction is a separate, smaller number.

---

## 3. "QA on every push" — the no-GitHub-Actions-safe design

### The hard constraint

`INVARIANTS.md` (root), §"No GitHub-Hosted CI / Cloud Actions":

> - Never add GitHub Actions workflows or any GitHub-hosted CI to this repository.
>   `.github/workflows/` must contain no workflow files …
> - CI, scheduled jobs, freshness re-runs, and any recurring automation run on
>   OpenAgents-owned infrastructure (our GCE / cloud runners and cron), not on
>   GitHub-hosted compute.
> - **Enforced** by `check:no-github-actions` (in `check:deploy`): it fails if any
>   `.github/workflows/*.yml` exists. PR-evidence / autonomous-QA on a PR is
>   **agent- or manually-triggered** … never a `pull_request`/`push`/`schedule`
>   workflow.

So "run QA on every push" **must not** be a GitHub Action. (Note: the `qa-runner`
README/quickstart contain a sample `.github/workflows/qa.yml` and reference a
`chill-eval.yml` — those are illustrative copy for **outside OSS users** of the
standalone package; they are not present in this repo, and `pr-comment-run.ts` is
explicitly designed to be **agent-triggered on owned infra**, not a workflow.)

### How pushes are gated today

A local **git pre-push hook** (`.githooks/pre-push`, enabled via
`scripts/enable-git-hooks.sh` → `git config core.hooksPath .githooks`) is how
direct pushes to `main` are gated without CI. It:

- gates **only** pushes whose remote ref is `main` (other branches push freely),
- fast-fails on merge-conflict markers in `workers/api/src` and `apps/web/src`,
- then runs `check:deploy` in `apps/openagents.com` and **blocks the push** if it
  fails.

`check:deploy` (`apps/openagents.com/package.json`) chains
`check:no-github-actions` + conflict-markers + effect-topology + agent-doc-links +
architecture + contract-drift + public-projection-freshness + typechecks + the
web/api test suites.

### The #6234 lesson (must-apply)

Today's deploy-gate fiasco: the desktop `verse-launch-smoke` inside `check:deploy`
repeatedly SIGKILL-137'd and forced `--no-verify` pushes. The fix
(`a462f448d1`, "take Verse desktop UI smoke off Worker deploy + pre-push gate")
did three things, which are now the **design rules for any push-time QA**:

1. **Off the critical path.** The heavy smoke was removed from `check:deploy`
   entirely — the gate no longer runs it inline.
2. **Hard-bounded.** The smoke is wrapped by `scripts/run-bounded.ts` with a
   default 480s wall-clock timeout (`OA_VERSE_SMOKE_TIMEOUT_MS`), child spawned in
   a detached process group, SIGTERM→SIGKILL escalation, exit 124 on timeout
   (fail-fast + loud).
3. **Run-only-when-changed.** A conditional lane
   (`run-if-desktop-changed.ts`) runs the desktop verify only when
   `apps/autopilot-desktop/**` (or feeding packages) changed vs `origin/main`
   (`OA_FORCE_DESKTOP_VERIFY=1` overrides).

**Rule for QA-on-push: bounded, scoped to changed surfaces, fail loud, never
silently block or kill the push/deploy. Do not recreate a flaky gate that forces
`--no-verify`.**

### Option comparison

| Option | Where it runs | Blocks the push? | Strengths | Risks |
|---|---|---|---|---|
| **(a) Pre-push hook stage** | local `.githooks/pre-push` | **yes** (synchronous) | already the gate; instant feedback; no infra | a real browser run is slow/flaky → exactly the #6234 failure mode if unbounded/unscoped; would force `--no-verify` |
| **(b) Deploy pipeline** (`check:deploy`) | local / deployer | **yes** (synchronous) | one gate to reason about | same flakiness risk; couples QA latency to every deploy; verse-smoke proved this is fragile |
| **(c) Our own GCE runner** (`oa-codex-control` + GCE) | owned infra, async | **no** (out-of-band) | unbounded compute; real browser at scale; matches the invariant's "owned infra" intent; publishes traces/videos | needs a push→trigger seam; verdict is post-hoc, not a blocking gate |

`oa-codex-control` + GCE already exists and is deployed
(`cloud/crates/oa-codex-control`, deploy `cloud/scripts/gcp-codex-control-deploy.sh`):
a Rust HTTP control daemon on a persistent GCE container that accepts async run
requests and spawns ephemeral GCE VMs per run, with a concurrency cap
(`OA_CODEX_QUEUE_MAX_CONCURRENCY`). This is the canonical "autonomous/unattended
execution on OUR cloud" pattern.

### Recommendation — a tiered combo

Define **"a push runs QA"** as a two-tier contract:

- **Tier 1 — fast bounded smoke, in the pre-push hook (synchronous, blocking, but
  cheap).** A scoped, hard-timeout smoke that only runs against **changed
  user-facing surfaces** (diff vs `origin/main`, mirroring
  `run-if-desktop-changed.ts`). Keep it to a handful of deterministic
  `scriptedBrain`/`--fake-model` checks or a single short real-browser check with
  a strict `run-bounded.ts` cap. It must **fail loud** and, like the verse fix,
  **must never become an unbounded SIGKILL gate** — if QA can't finish inside the
  budget it reports `incomplete` and yields to Tier 2 rather than blocking the
  push. If even this is too flaky to be a hard gate, it stays a **warning-only**
  pre-push notice (loud stderr) and the real verdict is Tier 2.
- **Tier 2 — full QA pass, async on the GCE runner (non-blocking, authoritative).**
  The push (or its resulting deploy) **triggers `oa-codex-control`**, which runs
  the full `qa-runner` matrix against the relevant target with Khala as the model
  backend, then **publishes the green VERIFIED traces + videos** to
  `/trace/{uuid}` and (on a PR) posts the verdict comment via the existing
  agent-triggered `pr-comment-run.ts`. This is where "every push runs QA" actually
  lives: real, unbounded compute on owned infra, post-hoc but authoritative, and
  it cannot wedge a developer's push.

Concretely, **where the green traces + videos land:**
`https://openagents.com/trace/{uuid}` (the redacted shareable trace, video served
inline) and the `/pro` run/eval pages; on a PR, also the agent-posted PR comment
with the comparison table + video + distilled-test ref + `/pro` link.

This keeps the blocking gate fast and deterministic (Tier 1), and moves the
heavy, flaky, real-browser work off the push critical path and onto owned infra
(Tier 2) — the exact shape the #6234 fix arrived at for verse-smoke.

**Shipped trigger seam (#6238).** `.githooks/pre-push` now runs
`scripts/qa-async-gce-trigger.ts` after Tier 1. The trigger is configured only by
owner-gated env:

```bash
export OA_QA_ASYNC_CONTROL_URL="https://<oa-codex-control>/"
export OA_QA_ASYNC_CONTROL_TOKEN="<bearer token>"
export OA_QA_ASYNC_PROVIDER_ACCOUNT_REF="provider-account_..."
export OA_QA_ASYNC_AUTH_GRANT_REF="codex-auth-grant_..."
# optional
export OA_QA_ASYNC_OWNER_REF="owner://openagents/internal-qa"
export OA_QA_ASYNC_TARGET_URL="https://openagents.com"
export OA_QA_ASYNC_PRO_BASE_URL="https://openagents.com"
export OA_QA_ASYNC_PR_NUMBER="<pr number>"
```

It posts a refs-only `openagents.codex_placement_assignment.v1` payload to
`/v1/placement/start` with `lane: "cloud-gcp"`, `wallet_authority: false`, and
`repository: "OpenAgentsInc/openagents@<pushed sha>"`. The goal instructs the GCE
runner to use `apps/qa-runner`'s Khala defaults, publish only public-safe green
VERIFIED traces/videos to `/trace/{uuid}` and `/pro`, and report red/refuted/
incomplete results loudly without mutating or blocking the pushed commit. Missing
env exits as `SKIPPED`; HTTP/control failure exits non-zero so the hook prints a
warning, but the push remains allowed after `check:deploy` is green.

---

## 4. Relationship to the deploy gate

QA **complements** `check:deploy`; it must not **become** it.

- `check:deploy` stays the fast, deterministic, blocking gate (typechecks,
  topology, contract-drift, conflict-markers, unit suites, `no-github-actions`).
  It is the thing that must be green for a push to land. QA's real-browser work
  does **not** belong inline here — that is precisely the mistake the verse-smoke
  fix corrected.
- The autonomous QA agent is the **out-of-band acceptance verifier**: it watches
  the deployed surface continuously, drives a real browser like a user, and emits
  honest pass/fail + a shareable trace. It answers "does the shipped thing
  actually work for a human," which unit tests and typechecks cannot.
- The bright line, restated from #6234: **QA reports loudly; QA never silently
  blocks or SIGKILLs a push or deploy.** A QA red is a loud, dereferenceable
  artifact (trace + video + REFUTED verdict), not a wedged gate that drives people
  to `--no-verify`. If we ever feel tempted to make QA a hard inline blocker,
  re-read the verse-smoke postmortem first.

---

## 5. Scope, risks, next steps

**Shipped vs direction.**

- **Shipped (live):** `qa-runner` real-browser run + video + committed e2e test +
  honest verdict; BYO-model incl. Khala default (#6237); verify verdict (#6192);
  harness quality bar (#6193); QA Control API (#6196); shareable redacted
  `/trace/{uuid}` publish (proven live 2026-06-24); the pre-push `check:deploy`
  gate; warning-only scoped/bounded Tier 1 QA smoke (#6245); warning-only
  push→`oa-codex-control` GCE trigger seam (#6238); `oa-codex-control` + GCE
  (deployed). Epic #6181 closed.
- **Direction (to build):** a Gym environment seeded from QA tasks + the
  real-browser verifier (§GYM cross-ref), plus production operator policy for
  where the owner-gated Tier 2 env is armed.

**Risks.**

- **Flakiness / isolation.** A real browser is inherently flakier than a unit
  test. Mitigations are already in the harness: condition-only waits (no sleeps),
  bounded visible retries, artifact-flush-on-crash/interrupt, per-scenario
  isolated backend + artifact dir. Tier 1 must additionally be hard-timeout-bounded
  and scoped; Tier 2 runs isolated per-VM on GCE.
- **Cost / quota.** Free tier headroom is 2.5M tok/day · 2k req/day per key —
  ample for now. Continuous QA at scale should move to a metered key so it does
  not consume the public free-tier allowance, and so its tokens are cleanly
  attributable as internal.
- **Secrets.** Khala/agent tokens come from env + `~/work/.secrets/*.env`; the key
  **value is never printed** (only its source label). Trace publish redacts
  secrets/PII/paths before upload and the ingest rejects leaked values. Never put
  a key in tracked files, commit messages, or normal terminal output.
- **No fake green.** An un-armed backend throws, a missing-chromium run fails
  honestly, an unparseable model action is a real failure, and a run that never
  reaches a verdict is `incomplete`/`fail` — preserve this in every tier.

**Next steps (direction).**

1. Build the **push → `oa-codex-control`** trigger seam (deploy-hook or a
   thin owned-infra listener) that launches the full Khala-backed QA matrix async
   and publishes traces/videos.
2. Seed a **Gym environment from QA** (QA tasks + the real-browser verifier) — see
   the cross-ref appended to
   [`../gym/2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md`](../gym/2026-06-25-gym-opencode-head-to-head-and-khala-flywheel.md).

---

## Path evidence index

- `apps/qa-runner/QA-RUNNER.md`, `README.md`, `RUNBOOK.md` — overview, artifacts,
  publish flow, demos.
- `apps/qa-runner/docs/harness-quality-bar.md` — the quality bar (#6193).
- `apps/qa-runner/docs/control-api-quickstart.md` — QA Control API (#6196).
- `apps/qa-runner/docs/container-backend.md` — container isolation backend.
- `apps/qa-runner/src/byo-model.ts`, `khala-config.ts`, `demo-khala.ts`,
  `publish-trace.ts`, `daemon.ts`, `pr-comment-run.ts` — model selection, Khala
  endpoint defaults, trace publish, daemon, agent-triggered PR evidence.
- `apps/qa-runner/package.json` — scripts (`qa`, `demo:khala`, `api`, `evals`, …).
- `INVARIANTS.md` (root) — No GitHub-Hosted CI / Cloud Actions; `check:no-github-actions`.
- `apps/openagents.com/package.json` — `check:deploy`, `check:no-github-actions`.
- `.githooks/pre-push`, `scripts/enable-git-hooks.sh`,
  `scripts/qa-pre-push-smoke.ts`, `scripts/qa-async-gce-trigger.ts` — the
  pre-push gate, Tier 1 QA smoke, and Tier 2 async GCE trigger.
- `apps/autopilot-desktop/scripts/run-bounded.ts`,
  `scripts/run-if-desktop-changed.ts`; commit `a462f448d1` — the #6234 fix.
- `cloud/crates/oa-codex-control/`, `cloud/scripts/gcp-codex-control-deploy.sh`;
  `docs/launch/2026-06-20-cloud-agent-fleet-audit.md` — the GCE runner.
- `docs/faq/khala-inference-quickstart.md` — free key, base URL, quota.
- `docs/inference/2026-06-25-khala-inference-gtm-push.md` — §2 Pillar 1, §5
  Sequence, North Star, internal-vs-external honesty.
- GitHub: epic #6181 (closed), children #6174/#6178–#6196.
