# Addendum — Pooled coding agents, PR-review-as-a-service, and the Autopilot/Verse loop

> Follow-on to [`2026-06-24-autonomous-qa-e2e-from-computer-use.md`](./2026-06-24-autonomous-qa-e2e-from-computer-use.md).
> Vision + architecture audit. **Not a product promise or public-claim copy**, and not a
> runtime change. It sketches what the shipped autonomous-QA + trace primitive becomes once
> it's the front-end for a **pool of paid coding agents** orchestrated by Khala, surfaced in
> the Autopilot desktop Verse, and pointed at **any pull request**.

## The thesis in one line

We already ship the unit: **an agent does real work with real tools → a verified, redacted,
shareable trace** (`/trace/{uuid}`). The opportunity is to make that unit the **product of a
marketplace**: *pay once for massive firepower, and OpenAgents fans the work out across a pool
of many paid coding-agent workers, orchestrated by Khala, watchable in the Verse, settled in
Bitcoin.* Autonomous **QA / PR review** is the killer first task because every team has
infinite PRs and finite review capacity.

```
Customer pays once  ──>  OpenAgents (the network)
                            │  Khala orchestrates + fans out
                            ▼
        ┌───────────┬───────────┬───────────┬─────────── ... (the pool)
     Autopilot    Autopilot   Autopilot   Autopilot     paid coding-agent workers
     worker A     worker B    worker C    worker D       (Pylon edge / hosted)
        │            │           │           │
        └── each: real tools → verified outcome → /trace/{uuid} → Bitcoin-settled
                            │
                            ▼
        Customer gets: a fan-out of verified traces (PR review, QA, audits, ...)
        Watchable live in the Autopilot desktop Verse (energy → Pylons → settlement beams)
```

This is the same machine as the Khala buildout — **OpenAgents deploys Autopilot agents,
powered by Khala** — with **autonomous QA / PR review** as the demand that justifies the pool.

## 1. Autopilot desktop ↔ trace ↔ Verse loop

When an Autopilot agent runs in **super-code mode** (the computer-use QA/coding flow), the
desktop app should make the work *visible and shareable*:

- **Step-through:** the agent drives the flow (browser/terminal/edits), and the desktop shows
  the live session (it already records video + steps via `qa-runner`).
- **Upload the trace:** on completion, the session is emitted as **ATIF**, redacted, and
  published to `/trace/{uuid}` (shipped: `publish-trace.ts` + the ingest + R2 media hosting
  #6223). The desktop surfaces the resulting trace URL.
- **Verse animation:** tie this into the **existing Pylon/Verse visualization** (the shared
  `persistentScene` / `landingSquares` energy + Pylon scene, settlement beams, agent
  avatars). A running super-code agent = a Pylon lighting up; a verified outcome = a verify
  glow; a Bitcoin settlement = a beam; a published trace = a portal you can click into the
  `/trace/{uuid}` render. This is the same "show Pylon stuff" idea, now driven by **real QA
  work** instead of a demo loop.
- **One scene, many workers:** when work is fanned out (below), the Verse shows the
  **fan-out** — N Pylons working the same job in parallel (this is exactly the M7 Conductor
  "compose-across-the-map" view, reused for QA fan-out rather than the head-to-head).

Net: the Autopilot desktop becomes the **operator cockpit** for a live, watchable army of
coding agents, each leaving a clickable verified trace.

## 2. The pool + fan-out economics (the actual product)

**Pay once → massive firepower.** The customer buys OpenAgents QA/coding capacity (credits;
MPP card/crypto + Bitcoin/Spark rails are already armed). They do **not** pick a model or a
worker. Khala **fans the work out** across a pool of paid coding-agent workers and returns a
set of verified traces.

- **The pool** = Autopilot agents running on Pylon edge nodes + hosted capacity (the Khala M4
  "Pylon workers in the pool" lane, currently inert/parity-gated — this is its demand-side
  justification).
- **Khala orchestrates:** route/shard the task across eligible workers, dedupe, aggregate,
  and (for hard tasks) compose (the Conductor lane). The capability gate already ensures a
  worker can only be named if it's receipt-eligible.
- **Settlement:** each worker's verified outcome is **Bitcoin-settled** (the Tassadar/Spark
  settlement leg #6011/#6023). The customer pays OpenAgents once; OpenAgents pays the workers
  per verified outcome. Margin = the spread + the orchestration value.
- **Why customers pay:** they get *parallel firepower they can't staff* — "review all 40 open
  PRs tonight," "QA every release candidate," "audit this repo across 5 lenses" — each as a
  verified, watchable, re-runnable trace. The incentive to pay OpenAgents for *additional* QA
  is that the marginal review is cheap, parallel, and leaves proof.

## 3. PR review on *any* pull request

The flagship recurring task: **autonomous PR review / flow-monitoring**. We already have the
agent-triggered PR-evidence path (`pr-comment-run.ts`, now no-GHA / agent-triggered) and the
distiller (session → committed e2e test). Generalize it:

- For a PR, fan out review **lenses** across the pool (correctness, regressions via the
  computer-use QA flow against a preview, security, perf, test-coverage, "does it actually
  run") — each worker produces a verified trace; Khala aggregates into one review.
- Output per PR: a **thin status** + a link to the **rich review on our surface**
  (`/trace/{uuid}` + a comparison view), not a wall of text in the PR.
- "Army of coding agents → a whole lot of PR review": run it across a customer's whole PR
  backlog, or **offer it broadly** (review public PRs to seed demand — see §4).

## 4. The GitHub presence — what we should actually do

We want OpenAgents to **review PRs as a recognizable identity**, while honoring our
**no-GitHub-Actions invariant** (agent-triggered, owned-infra) and our **reduce-GitHub-
dependency** direction (heavy artifacts on *our* surface). Options:

| Option | What it is | Pros | Cons |
|---|---|---|---|
| **A. GitHub App** ("OpenAgents QA") | An installable App that posts PR reviews/checks as the app identity, via webhooks→our infra | Native, per-repo install, fine-grained perms, a real Check + review, recognizable | App review/approval, webhook surface = some GitHub coupling |
| **B. Bot user account** (e.g. `@openagents-qa`) | A machine account that comments via a PAT | Trivial to start; a clear "OpenAgents" identity on the PR | Less integrated (no Checks API as an app), PAT mgmt, rate limits, looks less official |
| **C. Our-surface-only** | Review lives at `/trace/{uuid}`; a human/agent drops a link | Zero GitHub coupling; fits reduce-dependency | No GitHub-native trigger or identity; weak distribution |

**Recommendation — A, thin-on-GitHub + heavy-on-ours (a hybrid that respects both invariants):**

- Ship a **GitHub App "OpenAgents QA"** as the *trigger + thin presence*: it receives the PR
  event (webhook → **our owned infra**, NOT a GitHub Action — consistent with the no-GHA
  invariant, since the webhook fans into Khala on our side), runs the fan-out review on the
  pool, and posts a **compact** PR comment/Check: the verdict + a **link to the full review +
  traces on `openagents.com`**. The video, the step-by-step trace, the distilled tests, and
  the comparison all live on **our** surface (R2 + `/trace`), so GitHub holds only a thin
  pointer — exactly the dependency posture we want.
- The App identity makes it recognizable + installable by any team ("add OpenAgents QA to
  your repo"), which is the distribution flywheel for **paying for more QA**.
- Keep a **bot account** only as the fallback commenter where an App isn't installed (e.g.
  seeding review on public PRs), clearly labeled, rate-limit-aware.
- Do **not** implement this as a GitHub Action (invariant) — the App's webhook handler runs on
  owned infra (Worker/Khala), and the agent/operator path (`pr-comment-run.ts`) remains the
  manual/agentic entry today.

So: **GitHub App for trigger + identity + a thin comment; OpenAgents.com for the review,
traces, video, and the paid pool.** GitHub is the doorway; the product is ours.

## 5. How it composes with what's shipped

- **Trace primitive** (`/trace/{uuid}`, ATIF, redaction, R2 media #6223) — the artifact each
  worker produces. ✅ shipped.
- **qa-runner** (computer-use drivers, distiller, publish, OSS on npm) — the worker runtime. ✅
- **Khala** — the orchestrator that fans out + composes + meters. ✅ serves; fan-out/compose
  is the M4/M6/M7 lanes (parked as a north-star, but the *QA fan-out* is a simpler, nearer use
  of the same pool than the benchmark head-to-head).
- **Pylon / Verse** — the worker substrate + the watchable visualization. (M4 inert; Verse
  viz partial.)
- **Payments** — MPP (card/crypto) in, Bitcoin/Spark out to workers. ✅ armed.
- **no-GHA invariant** — the review trigger is a GitHub *App webhook → owned infra*, never a
  workflow. ✅ consistent.

## 6. What to build next (sequenced, honest)

1. **Autopilot-desktop trace+Verse hook** (codeable now): super-code-mode run → publish trace
   → surface the URL → light the Pylon/verify-glow/settlement-beam in the existing scene.
2. **PR-review fan-out v0** (codeable now, single worker → many lenses): generalize
   `pr-comment-run.ts` to run N review lenses → aggregate → one thin PR comment linking the
   traces. Agent-triggered (no GHA).
3. **GitHub App "OpenAgents QA"** (trigger + identity): webhook → owned infra → fan-out →
   thin comment/Check + link to our surface. (Owner: App registration + perms.)
4. **Real pool fan-out** (the long pole): activate M4 Pylon-in-pool + the worker settlement
   leg so "fan out to many *paid* workers" is real, not single-machine. (Owner/compute-gated —
   same gate as the Khala buildout; QA fan-out is a cheaper first demand than the head-to-head.)
5. **Pricing + "pay once for firepower"** packaging on the existing credits/MPP rails.

**Net:** the autonomous-QA + trace unit we shipped is the atom; the product is *a paid pool of
coding agents, orchestrated by Khala, doing verified PR review at scale, watchable in the
Verse, with the heavy artifacts on our own surface and GitHub as a thin doorway.* The nearest
honest wins are the Autopilot-desktop trace/Verse hook and a PR-review fan-out v0; the GitHub
App is the distribution unlock; the paid multi-worker pool is the same compute/arming gate as
the rest of the Khala buildout.
