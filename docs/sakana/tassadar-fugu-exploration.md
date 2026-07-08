# Tassadar × Fugu: Exploration

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


*Speculative design — 2026-06-22. An imagined episode and a 3D Verse experience
where the Tassadar run meets Sakana's Fugu approach: apply Fugu's
"multi-agent-system-as-one-model" orchestration to Tassadar's verified,
Bitcoin-paid worker pool, and make the orchestration **walkable** in the world.
Read as a map of what's possible, not a product contract — in the spirit of the
"Speculation: One Product" section of the transcript guide.*

Companion to [`README`](README.md),
[`adapting-sakana-coordination.md`](adapting-sakana-coordination.md),
[`coordinator-as-verified-work.md`](coordinator-as-verified-work.md),
[`psionic-coordinator-roadmap.md`](psionic-coordinator-roadmap.md),
[`tassadar-run-integration.md`](tassadar-run-integration.md).

## What Fugu is

Sakana Fugu is the **product** built on the two papers in this folder
([TRINITY](trinity.md) + [Conductor](conductor.md)): *"a Multi-Agent System,
Delivered as One Model"* — one OpenAI-compatible API that "dynamically
coordinates and orchestrates a diverse pool of powerful models," learning to
*assemble* agents from a pool rather than prescribing roles in advance. Its
pitch: frontier-level performance by **composition over scale**, "without
single-vendor dependency," with opt-out control over which models join the pool
and an explicit "frontier capability without the risk of export controls" angle.
Benchmarks claimed shoulder-to-shoulder with Fable 5 and Mythos Preview.

So Fugu is the productization of exactly the coordinator we roadmapped for
Psionic. "Tassadar meets Fugu" is therefore not two strangers — it's our
coordinator work wearing a product name, plugged into the Tassadar run's
verified worker pool, its Bitcoin settlement, and its 3D world.

## The synthesis (apply the Fugu approach to Tassadar)

Fugu orchestrates **closed frontier APIs**, graded by benchmark, behind one
endpoint. Tassadar **builds an open executor-class model**, pays contributors in
Bitcoin, and grades work by **cryptographic verification** (the verification-class
registry). Put them together and each fixes the other's gap:

| | Fugu (as shipped) | Tassadar provides |
|---|---|---|
| Worker pool | closed frontier APIs | + open Pylon nodes **and** verified executor **modules** (the agentic-npm registry) |
| Grading | benchmark / LLM judge | cryptographic verdict (`exact_trace_replay` / `seeded_replication`) |
| Settlement | subscription billing | per-contribution Bitcoin to worker **and** validator |
| Growth | one vendor's pool | group-forming network, anyone plugs in |
| Trust | "trust the API" | receipts a stranger can re-check |

The **OpenAgents Fugu** is then: a Fugu-style orchestrator (one API, "one model
to command them all") whose pool is frontier APIs *plus* Tassadar-trained
executor modules, that picks the cheapest worker+role that still **Verifies**,
trained against the verification verdict as its terminal reward (per
[`coordinator-as-verified-work.md`](coordinator-as-verified-work.md)), settling
every contribution in sats. "Composition over scale" stops being a slogan: the
composition is literally **composed verified modules** from the learning-by-
construction registry (Episode 238's "agentic npm"), and each composition is a
receipt.

The killer property Fugu can't offer and we can: **you can watch every agent
inside the one model earn Bitcoin, and re-check the receipt.**

## The episode

Working title: **"One Model to Command Them All"** (Episode concept).

Beats:

1. **The street.** Chris walks the *Snow Crash*-style street of the Tassadar run
   board (Episode 240). Floating over the run board is a single glowing orb
   labeled **Fugu** — from out here it is *one thing*, one endpoint, one model.
2. **The query.** He fires a real coding task at the orb (the same task a buyer
   would POST to the API). The orb pulses.
3. **Stepping inside.** He walks *through* the orb's membrane — and the single
   model resolves into a **swarm**: an orchestration arena where the worker pool
   (Pylons + frontier-model nodes already in the world as markers) stands around
   a central **Coordinator**.
4. **Role assignment, made spatial.** The Coordinator fires colored beams —
   **Thinker (T)**, **Worker (W)**, **Verifier (V)** — at chosen nodes, turn by
   turn. You watch the baton pass: Thinker plans → Worker executes → Verifier
   checks. TRINITY's Figure 1, walkable. (Conductor mode: the orchestrator draws
   a living natural-language **topology graph** overhead — subtask nodes wired by
   access-list edges — and you can walk up and read each subtask.)
5. **The clearing gate.** The Verifier hands the work-item to a **replay
   chamber**: an independent validator node re-executes, digests are compared,
   the item turns **gold (Verified)** or **red (Rejected)**. This is the
   load-bearing wall (Episode 237) rendered as an actual wall the work passes
   through.
6. **The money loop.** A Verified item drops **sats** that stream visibly to the
   worker and validator avatars (Episodes 235/238 — 5 sats each). The run-board
   sats counter ticks up.
7. **The library grows.** The verified program snaps onto a shelf in the
   **module library** — the agentic npm as a physical armory. Compose two modules
   and they click together into a bigger program object. Fugu's "pool" *is* this
   library.
8. **Compliance, made tactile.** Chris opens the **pool roster** and unchecks a
   couple of worker nodes (the opt-out / export-control control); they go dark,
   and the Coordinator visibly routes around them. Ties to Tassadar's admission
   standard — earned/mined Bitcoin welcome, shitcoin-spam not.
9. **Pull back.** From the street again, the swarm collapses back into one orb.
   Tagline: *"One model to command them all — and you can watch every agent
   inside it earn Bitcoin."*

## The 3D experience design

Built on the existing surface (Episode 240): walkable avatar, run board,
floating Pylon/assignment markers, refs ticker, sats counter, multiplayer; using
`@openagentsinc/three-effect` (Three.js, per the workspace UI guidance) with
Foldkit owning structure.

**Core visual thesis — the duality.** The single most important rendering choice
is the **one-orb-outside / many-agents-inside** duality. From the street, Fugu
is a single sphere (the "as one model" claim). Crossing its membrane is the
transition into the multi-agent arena (the "multi-agent system" reality). The
camera move *is* the thesis.

**Entities and how they render:**

- **Fugu orb** — the API endpoint. Single, calm, glowing from outside; internally
  a contained swarm. Walking through it is the scene transition.
- **Coordinator** — a small, fast nucleus at the arena center (the ~0.6B TRINITY
  head, or the 7B Conductor). It is *visibly smaller / cheaper* than the workers
  it commands — the whole point. Emits role beams.
- **Worker nodes** — the pool, standing in a ring: Pylons (the ones already in
  the world), frontier-API nodes, and **module-backed executors** pulled from the
  library. Each carries a **knowledge aura** colored by its owner's local corpus
  (the diverse-corpora idea from Episode 235) so heterogeneity is legible.
- **Role beams** — T (planner) / W (executor) / V (verifier) in three distinct
  colors; a turn-by-turn relay so multi-turn coordination is something you watch,
  not infer.
- **Topology graph (Conductor mode)** — a floating DAG above the arena; nodes are
  subtasks, edges are the access-list ("who sees what"); walk up to read the
  natural-language subtask string.
- **Replay chamber (the clearing layer)** — a gate the work-item must pass.
  Independent validator re-executes; gold/red verdict; **this is where trust is
  manufactured**, so make it the most architecturally prominent structure in the
  arena (the load-bearing wall).
- **Module library / agentic npm** — a walkable armory of glowing verified
  modules; accepted outcomes add shelves; composition = snapping modules into
  larger programs. The registry as a place.
- **Sat streams** — particle flows from the treasury / buyer to worker + validator
  on each Verified outcome; grounded in real receipts (no motion without a ref,
  per the visual-language policy).

**HUD gauges:**

- **Accepted outcomes per kWh** — the north-star meter (Episodes 232/237), front
  and center: electrons in → accepted agent work out.
- **Confidence tier of the current outcome** — draft / verified / reviewed /
  bonded (Episode 237), shown as the work-item's glow intensity and price tag;
  maps directly onto verification class + sample rate.
- **Cost-per-verified-outcome** and **worker-mix** (how much went to cheap open
  workers vs frontier APIs) — the composition-over-scale story as live numbers.

**Multiplayer.** Several humans walk the same arena watching the *real* run's
orchestration as world objects; tab-target a worker to inspect its current
subtask and its last receipt; tab-target the replay chamber to watch a live
challenge. This is the Episode 189 "agentic MMORPG" direction pointed at a
concrete, money-real scene.

**Why this is more than eye candy.** Every object dereferences to a real artifact
— a worker to its receipt, the orb to the API call, a sat stream to a settlement,
a module to its verified program. The world is a *browsable projection of the
clearing layer*. That is the honest version of "spatial agent UI": not a
decoration over a chat log, but a walkable index of receipts.

## Draft README (for a `fugu/` experience lane)

> ```
> # OpenAgents Fugu — Multi-Agent System as One Model, Watchable in the Verse
>
> One API. A pool of many models — frontier APIs, open Pylons, and verified
> Tassadar executor modules. Fugu learns to assemble and coordinate them per
> task (Thinker / Worker / Verifier; learned NL topologies), picks the cheapest
> composition that still VERIFIES, and settles every contribution in Bitcoin.
>
> ## What it is
> - **One endpoint** (OpenAI-compatible) over a diverse worker pool.
> - **Composition over scale**: frontier-grade outcomes without single-vendor
>   dependency — by composing verified modules, not by training one giant model.
> - **Verified, not trusted**: every outcome clears the verification-class
>   registry (exact_trace_replay / seeded_replication / ...). The receipt is the
>   product.
> - **Paid**: worker AND validator earn sats per accepted outcome.
> - **Open lane**: opt out any model from the pool (compliance / export control);
>   earned/mined Bitcoin welcome.
>
> ## Watch it
> Open the Tassadar run board in Autopilot and walk into the Fugu orb. The
> orchestration is a world: role beams, the replay-chamber clearing wall, sat
> streams, and the agentic-npm module library — every object dereferences to a
> real receipt.
>
> ## Metric
> Accepted outcomes per kilowatt-hour.
>
> ## Build on
> TRINITY (evolved coordinator) + Conductor (RL NL coordination), trained against
> the Tassadar verification verdict. See docs/sakana/.
> ```

## What's real vs speculative

- **Real foundation:** the coordinator design (TRINITY/Conductor → our Psionic
  roadmap), the verification-class reward oracle, the Bitcoin settlement rails,
  and the walkable run board (Episode 240) all exist or are specced in this
  folder.
- **Speculative:** Fugu-as-our-product-name, the orb duality scene, the module
  library as a walkable armory, and the per-outcome 3D orchestration view. None
  of it is shipped; this is a design target.
- **Honest gap to close first:** the learned coordinator itself (Psionic
  primitives P1–P5) and the rollout/benchmark verification classes
  (`seeded_replication` / `statistical_cross_check`) being dispatched. Without
  those, the arena would animate a heuristic router, not a learned Fugu. Build
  the coordinator, then make it watchable.

## Why it matters

Fugu's whole bet is that orchestration beats scale. Ours adds the two things a
closed API can't: the work is **cryptographically verified** and **paid in
Bitcoin to everyone who contributed**, and the orchestration is **legible** —
literally walkable. "One model to command them all" is a good slogan; "and you
can walk inside it and watch every agent earn Bitcoin, then re-check the receipt"
is a moat.
