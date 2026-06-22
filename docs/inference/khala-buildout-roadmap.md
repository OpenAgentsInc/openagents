# Khala Buildout Roadmap — to the Head-to-Head

*Roadmap — 2026-06-22. The single sequenced buildout that takes Khala from the
existing gateway skeleton to a learned, verified, Bitcoin-settled inference model
that you **consume through Autopilot** and **watch serve in the Verse** — ending
in a concrete north-star: running our own version of the Fugu-Ultra-vs-frontier
head-to-head.*

Consolidates the per-area roadmaps: [`khala.md`](khala.md) (spec/§11),
[`khala-in-the-world.md`](khala-in-the-world.md) (visualization),
`docs/sakana/psionic-coordinator-roadmap.md` (coordinator primitives),
`docs/sakana/tassadar-run-integration.md` (verification classes),
`docs/research/tmax/synthesis.md` (recipe/stability), and the revenue-loop spine
(EPIC #5457).

## North-star goal (the end of the flow)

Run our own version of the head-to-head that's circulating publicly — **Sakana
Fugu Ultra vs. a frontier model**, same prompt, measured side by side:

> Prompt: *"build a really high quality single html file crossy road game with
> three.js"*
>
> **Sakana Fugu Ultra** (reported): ~89k tokens, ~$7.32, 22 min. Issues: inverted
> turn direction, wonky camera, no SFX, not identical to Crossy Road.
> **Claude Opus 4.8 (Ultracode)** (reported): ~940k tokens, ~$37.85, 79 min.
> Issues: stuck twice in a retry loop, wrong character position after restart,
> hard from the start (Fugu correctly ramped difficulty).
> Reported verdict: Opus won on functionality/quality/design; Fugu won on
> speed/cost/efficiency.
> *(Source: external X/Twitter comparison; treat the numbers as reported claims,
> not our measurements.)*

**Our version is the same task, run through Khala, with four things no one else
in that comparison has:**

1. **Run it through Autopilot → Khala.** A real user (or agent) submits the
   prompt to the Khala endpoint via the Autopilot cockpit; Khala's coordinator
   composes the pool (plan / write / verify) instead of one model one-shotting.
2. **Verified, not vibes.** "High quality crossy-road game" becomes a **rubric**
   (playable, correct controls, camera, difficulty ramps with progress, runs in
   one HTML file) graded by a verification class — `test_passed` (headless
   checks) and ideally a replay/parity check. The win condition is an **accepted
   outcome**, with a receipt.
3. **Settled in Bitcoin.** Whoever served the accepted outcome (Pylon worker,
   coding agent, validator) is paid, with a public receipt — the head-to-head is
   also a money loop.
4. **Watched in the Verse, and the artifact lives there too.** The whole serving
   flow renders in the world (crackling energy to the Pylons doing the work, a
   gateway portal if any external model is called, the coding-agent avatar
   building the game) — and the built three.js game is itself **playable in our
   three-effect world** (a three.js game inside our three.js Verse).

**We report the same axes plus ours:** tokens, $, wall-clock — **and**
cost-per-accepted-outcome, accepted-outcomes-per-kWh, in-world-vs-gateway split,
and verified-rate. The thesis we're proving: *composition + verification + a paid
open pool* can match frontier quality at lower cost, and — uniquely — you can
**watch and audit every piece**.

This is the demo the entire buildout below is sequenced to reach.

## Workstreams (what has to exist)

- **A — Khala serving.** The OpenAI-compatible endpoint + the coordinator ladder
  (v0 heuristic → v1 TRINITY logit router → v2 Conductor NL planner → v3 full).
  (`khala.md` §5, §11.)
- **B — Verification & economics.** Verification classes wired into the receipt
  (`test_passed` / `seeded_replication` / `exact_trace_replay`), metering,
  Bitcoin settlement to worker + validator, referral (revenue-loop RL-1/2/3).
  (`khala.md` §6–§7, §10.)
- **C — Supply & training.** Pylon serving workers in the pool (capability →
  dispatch → exact-parity receipt → payout); Psionic coordinator-training
  primitives P1–P5; TMAX-style data/stability recipe for the worker side.
  (psionic-coordinator-roadmap; tmax/synthesis.)
- **D — Verse visualization.** The crackling-energy serving view: arcs to Pylons,
  the gateway portal, coding-agent avatars, verify glow, settlement beams, HUD —
  on the Effect-centric multiplayer engine. (`khala-in-the-world.md`.)
- **E — Autopilot consumption.** The cockpit that submits prompts to Khala,
  shows the live Verse view of the serve, and renders/plays the resulting
  artifact.

## Sequenced milestones

Each milestone converges the workstreams toward the demo; later ones depend on
earlier. EPIC anchors in parentheses where they exist.

**M0 — Khala serves, metered, with a receipt.** *(A, B)*
Flip the gateway stub router to real cheapest-viable (#5482), name
`openagents/khala-mini`, turn on a real adapter (Fireworks #5479 / Vertex #5480),
real per-model decrement (#5477). *Done when:* an OpenAI SDK call to
`khala-mini` returns a metered completion with an `openagents` receipt block.

**M1 — Autopilot calls Khala.** *(E)*
Autopilot submits a prompt to the Khala endpoint and shows the completion +
receipt (route, workers, cost). *Done when:* the crossy-road prompt runs through
Autopilot end-to-end against `khala-code` and returns a single HTML file.

**M2 — Verified coding outcomes.** *(B)*
`khala-code` runs a verification command / headless checks → `test_passed`;
define the crossy-road **rubric** (playable, controls, camera, difficulty ramp,
single file) as the acceptance test. *Done when:* a run reports `verified: true`
against the rubric, with a receipt.

**M3 — Bitcoin settlement on accepted outcomes.** *(B)*
Wire verified-lane settlement (RL-2) to pay the worker + validator; accepted-
outcome pricing for `khala-code`. *Done when:* a verified crossy-road build
settles sats to a contributor with a public receipt.

**M4 — Pylon workers in the pool.** *(C)*
Pylon whole-small-model serving as a worker (capability → dispatch → exact-parity
receipt → payout) via the fabric supply adapter; some of the work now stays
in-world and paid. *Done when:* a Khala request is served by a Pylon with a
parity receipt and a Bitcoin payout.

**M5 — The Verse serving view (P0/P1 of khala-in-the-world).** *(D)*
Project inference events from the activity timeline; render crackling arcs to
assigned Pylons, the gateway portal for external calls, and a coding-agent avatar
for the build — under the evidence-bound motion contract. Build the two new
three-effect primitives (`createCracklingArc`, `createGatewayPortal`) and the
`gateway_station` row. *Done when:* submitting the crossy-road prompt in
Autopilot shows the serve happening live in the world.

**M6 — Learned coordinator (TRINITY lane), shadow-deployed.** *(A, C)*
Psionic primitives P1–P5; train the logit router by sep-CMA-ES on the
verification verdict; ship as a shadow candidate vs the heuristic router; promote
on cost-per-accepted-outcome (a gated `runtime_promotion`). *Done when:* the
learned router beats heuristic on cost-per-accepted-outcome in shadow.

**M7 — Conductor lane (compose to win the benchmark).** *(A, C, D)*
GRPO-trained NL planner (DPPO + FP32 head, TMAX recipe) that decomposes the
crossy-road task — plan (frontier via gateway) → implement (best coding worker) →
verify (rubric/replay) → refine — and the Verse shows the multi-worker fan-out.
*Done when:* `openagents/khala` solves the crossy-road task by composition, beating
single-model cost at comparable quality.

**M8 — The head-to-head demo (north-star).** *(all)*
Run the crossy-road prompt through `openagents/khala` vs a frontier baseline,
side by side: report tokens, $, time, **cost-per-accepted-outcome**,
**accepted-outcomes-per-kWh**, in-world-vs-gateway split, verified-rate; show the
live Verse serve; **play the resulting three.js game inside our three-effect
world**; and show the Bitcoin settlement receipts. *Done when:* we can publish our
own head-to-head with a verified, paid, watchable result.

## Critical path & honest gaps

```
M0 ─► M1 ─► M2 ─► M3 ─┐
            └─► M4 ───┼─► M5 ──┐
                      └─► M6 ──┴─► M7 ─► M8 (head-to-head)
```

- The **request surface, auth, balance gate, adapter registry, payment beams,
  Pylon scene, agent avatars, and multiplayer engine already exist** — M0/M1/M5
  are mostly wiring + two new render primitives.
- The genuinely new capability is the **learned coordinator** (M6/M7) — Psionic
  P1–P5 (no CMA-ES / hidden-state extraction / SVF today) — and **verified coding
  outcomes for arbitrary tasks** (M2: rubric/verification-command discovery).
- Until M6, the demo runs on the **heuristic router** (still a valid head-to-head;
  it just isn't the learned-composition story yet).
- The crossy-road rubric and headless verification harness are net-new and
  gate M2 onward — define them early; they double as a reusable
  accepted-outcome template for the Tassadar coding lane.

## Why this is the right north-star

It exercises every load-bearing claim at once: **composition over scale**
(Khala beats a single model by routing), **verification not trust** (the game is
an accepted outcome with a receipt, not a vibe), **paid open economy** (workers
and validators settle in Bitcoin), and **legibility** (you watch the serve and
play the artifact in our own world). Beating — or honestly losing to — a frontier
model on a concrete, fun task, with every piece auditable and paid, is the most
compelling possible proof that the whole system works.
