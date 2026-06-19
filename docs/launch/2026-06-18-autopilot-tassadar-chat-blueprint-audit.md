# Autopilot Tassadar-Chat + Blueprint Audit

Date: 2026-06-18
Repo: `OpenAgentsInc/openagents`
Scope: research/audit only. No app code was changed. `apps/autopilot-desktop/src/ui/*`
was read (read-only) but NOT edited; that directory has concurrent work.

All file paths below are repo-relative to `OpenAgentsInc/openagents` unless noted.

---

## 0. The vision being audited

> "Once you're past onboarding, I want the default screen to be the **chat** — a
> chat with the **Tassadar-enabled thing** by default, with all the **blueprint**
> stuff."

Two pieces to sync and expose through one post-onboarding default chat pane:

1. **The Tassadar piece** — chat with the model being built, drawing on the
   verified computation / modules it learned (the LLM-computer /
   learning-by-construction / verified-program registry).
2. **The blueprint piece** — our term for the **DSPy-style structured-program
   framework for chats** (signatures / modules / typed programs / optimizer /
   RLM). Sync it with Tassadar and expose it via chat.

This doc reports, with honest EXISTS-vs-NEW marking, whether each piece is real
today, what the cleanest substrate is, and a phased plan to make a
Tassadar-backed, blueprint-structured chat the post-onboarding default.

---

## 1. Executive answers (read this first)

**(a) Can you chat with Tassadar today?**
**No — there is no prompt→completion / chat / inference endpoint for Tassadar.**
What exists is a real, live **execute → replay-verify → settle** loop over a
single digest-pinned compiled workload, plus an executor that can run and compose
**dense compiled weight modules**. Tassadar in the current paradigm is *compiled
and executed*, not *trained and prompted*. "A chat that draws on all the
computation it learned" is **aspirational** and requires a new inference/serving
seam that does not exist yet. (Details: §2.)

**(b) What is "blueprint" in our code, and where does it live?**
Blueprint is a **DSPy-style typed-program framework that EXISTS and is
implemented in THIS repo** — schemas, repositories, services, fixtures, a
contract export, HTTP routes, and a live Probe-runtime consumer (signature
lookup + tool scoping). It is *real* as a typed-contract + governance system.
The **optimizer / GEPA / RLM improvement loop is schema + design only** (not a
live runtime). The standalone workspace `blueprint/` repo is **deprecated/archived
source material**; the implementation home is this repo. (Details: §3.)

**(c) Recommended architecture for the Tassadar-chat pane.**
A new lightweight desktop **chat pane** driven by the existing Foldkit
Model/update/view + the existing `session.spawn`/node-state-poll session
substrate, **structured by Blueprint signatures** (the chat selects a Blueprint
signature → typed program → tool menu → evidence-only program run), and
**grounded in Tassadar** by linking turns to verified proof-replay bundles and
(later) by composing Tassadar dense weight modules for the exact-execution
sub-steps. The chat is a *blueprint program runner* whose exact sub-computations
are *Tassadar-verified*. (Details: §4.)

**(d) Top gaps + phased plan.**
The two load-bearing gaps: (1) no Tassadar inference/serving surface — only
execute/replay/settle; (2) no live blueprint optimizer/RLM runtime — only typed
contracts + governance. Phased plan in §5 builds the chat on what is real
(Blueprint signatures + session runtime + proof-replay grounding) first, and
treats true "chat with the Tassadar model" as a later milestone gated on a real
serving path. (Details: §5, §6.)

---

## 2. The Tassadar piece — model + inference surface

### 2.1 What Tassadar is (concept)

Tassadar is OpenAgents' implementation of the Percepta "LLM-computer"
construction: transformers made to **compute exactly** (compiled, not trained)
via an Append-only Lookup Machine (ALM). The pipeline is
program → ALM gate graph → schedule → analytic weights → deterministic execution,
**verifiable by exact replay** (re-run, compare digests byte-for-byte).

Primary docs (all under `docs/tassadar/`):

- `docs/tassadar/README.md`
- `docs/tassadar/RESEARCH_PLAN.md` — unified directive; the governing question is
  "Can the exactness we can compile become something we can train, sell, and
  embed?"
- `docs/tassadar/2026-06-11-llm-computer-full-introduction.md` — assume-nothing
  intro to the LLM-computer construction.
- `docs/tassadar/work-that-proves-itself.md` — business thesis; describes
  "learning by construction" and modules-as-artifacts.
- `docs/tassadar/2026-06-14-w3-student-program-report.md` — four-baseline sweep:
  pure learned exactness fails (`0.0` pass@1); frozen analytic core + thin
  learned shell works (`1.0` pass@1). Research/eval only; no public model claim.
- `docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`
  — the most honest current-state + gap audit. Read this before believing any
  "the model" framing.

Two-lane framing (RESEARCH_PLAN §2): **Tassadar** = compiled/exact/digest-pinned
("written, not learned"); **Psion** = the learned compact-decoder lane. They must
not borrow each other's claim language.

### 2.2 What EXISTS in code (the execution + settlement loop)

**TypeScript executor — `packages/tassadar-executor/src/`** (EXISTS, real):

- `numeric-executor.ts` — `executeTassadarNumericModel`: TS re-execution of the
  ALM numeric model (parabolic-key hard-max attention, ReGLU FFN, residual
  wiring, checked integer arithmetic), producing a `TassadarNumericTrace` with a
  `traceDigest`. Self-described claim boundary: "faithful re-execution of
  digest-pinned compiled workloads only — no softmax, no learning, no serving,
  no performance claim."
- `dense-weight-module-runtime.ts` (+ `dense-weight-module.ts`,
  `-fixture.ts`) — `executeTassadarDenseWeightModule` over a
  `TassadarDenseWeightModule` (loadable dense `W_Q/W_K/W_V/FFN`-style blocks).
  This is **more advanced than "scalar lanes only"**: dense, loadable modules
  exist here in the executor package.
- `linked-dense-module-runtime.ts` (+ `linked-dense-module.ts`, `-fixture.ts`) —
  **module composition**: links/composes multiple dense modules, pins composed
  digests, declares a marketplace listing ref
  (`listing.public.tassadar_compiled_weight_module.cc1403674fc0d388`), a consumer
  family, a claim class, a required trust posture
  (`benchmark_gated_internal`), and per-module expected compatibility digests.
  This is the closest existing thing to "composing verified modules."
- `capability-envelope.ts`, `lane.ts`, `replay.ts`, `replay-cli.ts`,
  `self-test.ts`, `compiled-program-corpus.ts` — capability/claim envelope,
  replay surface, CLI, self-test, and a small compiled-program corpus.

**Live run loop — `apps/openagents.com/workers/api/src/`** (EXISTS, real):

- `tassadar-run-admission.ts`, `tassadar-trace-pairing.ts`,
  `tassadar-replay-validator.ts`, `tassadar-exact-trace-replay.test.ts`,
  `tassadar-run-settlement.ts`, `tassadar-auto-settlement.ts`,
  `tassadar-settled-feed-sync.ts`, `tassadar-poc-fixture.ts`,
  `tassadar-adversarial-verification-market.ts`,
  `tassadar-trace-contribution-routes.test.ts`.
- Flow: contributor claims a window lease → worker executes the digest-pinned
  workload via `executeTassadarNumericModel` and submits a trace digest → a
  device-distinct validator re-executes → verdict by digest comparison
  (`Verified`/`Rejected`) → on a verified `exact_trace_replay` pair, real
  Bitcoin auto-settles to both legs and is broadcast on the public settled feed
  (`GET /api/public/training/runs/run.tassadar.executor.20260615/settlements`).
- Status (per product-promises + the gap audit): the executor PoC and the
  decentralized-training launch are GREEN — two independent contributors have
  been paid real Bitcoin (a 1,000-sat canary and a 5-sat self-serve), total
  ~1,005 sats, independently verified and settled.

**Autonomous administrator (Artanis)** — `artanis-scheduled-runner.ts`,
`artanis-work-directions.ts`, `artanis-administrator-corpus.ts` (and tests).
Bounded to a few dispatches/day of **one** fixed workload (`loop_sum` to 15);
it accumulates verified traces/verdicts/receipts. No Artanis path constructs,
composes, or admits *new* compiled modules.

### 2.3 The GAP: no inference / no chat

- A full grep of `apps/openagents.com/workers/api/src/` for Tassadar surfaces
  returns only admission / pairing / replay / settlement / feed / proof-replay /
  PoC fixtures — **no `chat`, `completion`, `inference`, or `prompt` route that
  serves Tassadar.**
- The only occurrences of "prompt" near Tassadar are **redaction guards**
  (e.g. `linked-dense-module-runtime.ts` enforces that public-safe refs must
  *not* contain raw prompts/traces/secrets) — i.e., the system is explicitly
  designed *not* to carry prompts. This confirms the absence of an NL surface.
- The executor runs **pre-compiled programs**, not arbitrary user prompts. Output
  is an exact execution trace, not generated tokens.

**Honest verdict (Tassadar):** You **cannot chat with Tassadar today.** What is
real is (1) execute a digest-pinned compiled workload, (2) verify by exact
replay, (3) settle real sats, and (4) run/compose dense compiled weight modules
in the executor package. "A chat that draws on all the computation it learned"
maps, in this paradigm, to *a chat whose exact sub-steps are answered by composing
verified Tassadar modules and shown with their proof-replay digests* — that
composition substrate partially EXISTS (`linked-dense-module-runtime.ts`), but
the **NL planning → module selection → serving** layer that turns a chat turn
into a composed-module execution is **NEW/aspirational** and not built.

Documented future work (gap audit + RESEARCH_PLAN): E4 MILP optimal scheduler,
broader Wasm opcode window, softmax error bounds in owned code, corpus/curriculum
variety on the live run, on-run pricing of *construction* (not just re-execution),
and the H4 "programs-in-weights becomes a module system" registry as a live,
listable/priceable service (today it is a fixture + design, not a public registry
service).

---

## 3. The blueprint piece — DSPy-style structured-program framework

### 3.1 What "blueprint" means here

Blueprint is OpenAgents' DSPy-style framework for **typed, composable,
governed** chat/work capabilities: stable input/output **signatures**, **module
versions** behind them, **typed program** runs with strict authority boundaries,
and a (planned) **optimizer** loop. It is implemented in THIS repo.

### 3.2 What EXISTS in code (typed contracts + governance — real)

**Schemas — `apps/openagents.com/workers/api/src/blueprint/schemas/`** (EXISTS):

- `program.ts` — `BlueprintProgramSignature` (stable I/O contract) and Program
  Types (behavior families with risk class, evidence/receipt requirements, tool
  scopes, direct-mutation policy).
- `module.ts` — `BlueprintModuleVersion` (implementation artifact behind a
  signature, with lifecycle/release states).
- `optimizer-run.ts` — `BlueprintOptimizerRun` with kinds including
  `gepa_style_reflection` (DSPy/GEPA lineage). **Schema only** (see §3.3).
- Plus program-run, action-submission, continuation-decision, mission-briefing,
  release-gate, source-authority, context-pack, simulation-branch schemas.

**Repositories / services / fixtures / exports / routes** (EXISTS):

- `blueprint/repositories/` — `program-runs.ts` (evidence-only), `action-submissions.ts`
  (approval-gated write boundary), `probe-contributions.ts`.
- `blueprint/services/` — `program-run-authority.ts` (denies deploy/email/PR/spend/
  source-mutation effects), `continuation-decision.ts` (classifies a turn into
  continue/test/fix/summarize/request_context/retry/stop/escalate), `continuation-mission-briefing.ts`,
  `release-gate.ts`, `signature-contribution.ts`, `smoke-probe.ts`.
- `blueprint/fixtures/` — `autopilot-continuation-signatures.ts` (seed signatures:
  continuation, test, fix, summarize, request-context, retry, stop, escalate,
  prepare-review, routing, research-policy, email-decisioning, proof-projection),
  `program-registry.ts`.
- `blueprint/exports/contract-export.ts` — JSON-Schema/OpenAPI exports for agents
  and Rust consumers.
- `blueprint-routes.ts` — Blueprint HTTP API surface.
- `blueprint/README.md` — states Optimizer Runs is the *future* home for
  GEPA/DSPy-style improvement.

**Probe runtime consumer — `packages/probe/packages/runtime/src/blueprint/`** (EXISTS):

- `signature-lookup.ts` — **DSPy-style signature selection** from backend
  capability + risk + tool constraints. This is the live "choose the typed
  program for this turn" path.
- `tool-menu.ts` (scope tools from a signature), `program-run-evidence.ts`
  (emit evidence), `action-submission.ts` (propose approval-gated writes),
  `registry-client.ts`, `contracts.ts`, `contribution.ts`.
- Reference: `packages/probe/docs/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`.

**Core invariants enforced today:** program runs are evidence-only (not write
authority); action submissions are the only write-side boundary (deploy / email /
PR / spend / source mutation / public claims); release gates required before
promotion; no direct mutation in v1 signatures; tool access is read / evidence /
propose_action.

### 3.3 What is DESIGN-ONLY / FUTURE

- **Optimizer / GEPA / RLM runtime loop** — `BlueprintOptimizerRun` schema exists
  with `gepa_style_reflection`, but there is **no live optimizer runtime** that
  proposes candidates and re-evaluates modules. The README marks it as the future
  home. GEPA references elsewhere are job records / fixtures / smoke tests, not a
  runtime.
- **Study packets / structured learning artifacts** — design only
  (`docs/research/machine-studying/2026-06-17-blueprint-marketplace-ties.md`,
  `.../2026-06-17-tassadar-openagents-repo-studying-roadmap.md`).
- **Signature-marketplace monetization** — gated / non-authoritative
  (`apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.ts`).

### 3.4 Deprecated source material (concepts only)

- The standalone workspace repo `blueprint/` is **deprecated/archived** per the
  workspace AGENTS.md (it was not present/readable in this worktree). Mine it for
  *terminology/concepts only* (Source Authority, Program, Engine/Toolchain,
  Evidence, Receipt, optimizer, RLM, generated-SDK). **The implementation home is
  THIS repo.**
- This repo's own legacy note:
  `apps/openagents.com/docs/blueprint/2026-06-05-legacy-blueprint-primitives-openagents-inventory.md`
  states the OpenAgents product surface must NOT add a production dependency on
  the old Rust Blueprint workspace.
- Adjacent reference (concepts only, not a dependency):
  `projects/repos/gepa` (text-program / reflective-prompt optimization) and the
  broader DSPy lineage. Relevant if/when the blueprint optimizer loop is built.

**Honest verdict (blueprint):** A **working DSPy-style typed-contract +
governance system EXISTS in this repo** and is actively consumed by Probe for
signature lookup and tool scoping. The **optimizer/RLM learning loop is
schema+design only.** It is the natural framework to *structure* a chat (pick a
signature → typed program → scoped tools → evidence-only run), which is exactly
what the vision asks for.

---

## 4. Existing chat surfaces + the recommended architecture

### 4.1 Post-onboarding default screen today (EXISTS)

- Initial pane is hardcoded to `"onboarding"` in
  `apps/autopilot-desktop/src/ui/initial-state.ts` (recent AO-4 work, commit
  `1e84fc2b0`: the onboarding wizard is the first screen on launch; the comment
  explicitly names the chain "registered → node → wallet → presence → Tassadar →
  earning").
- Pane routing lives in `apps/autopilot-desktop/src/ui/model.ts` (`PaneId`),
  `.../ui/view.ts` (`paneView`, sidebar `NAV`), and `.../ui/update.ts`
  (`NavigatedTo` handler).
- Onboarding completion is projected in
  `apps/autopilot-desktop/src/shared/onboarding-status.ts` (`response.complete`).
- **GAP (NEW):** there is **no auto-transition** from `"onboarding"` to a default
  screen once complete; the user must click a nav button. Making chat the
  post-onboarding default requires new transition logic in `update.ts`
  (on onboarding-complete + current pane === onboarding → navigate to chat) and/or
  a conditional initial pane in `initial-state.ts`.
  *(Note: that directory has concurrent work — coordinate; this audit changes no
  code there.)*

### 4.2 Existing chat/composer substrate (EXISTS)

- **Composer pane** (#5355) — `apps/autopilot-desktop/src/ui/view.ts`
  (`composerPane`): the interactive coding loop (objective + repo/worktree +
  adapter/lane → `session.spawn` → live streamed transcript → inline approvals →
  reply/continue). It is session/objective-driven, not a persistent message-list
  chat.
- **Session/control substrate** — `apps/autopilot-desktop/src/shared/rpc.ts`
  (`DesktopRPCSchema`, `SessionEventRow`),
  `apps/autopilot-desktop/src/bun/pylon-control.ts` (`spawnSession`,
  `connectBridgeDesktop`), `apps/autopilot-desktop/src/bun/node-state-poll.ts`
  (polls node state → sessions + per-session events). The desktop does **not**
  call an LLM directly; it spawns sessions on Pylon and observes events. Pylon's
  internal LLM message model is `apps/pylon/packages/runtime/src/llm/messages.ts`
  (text/media/reasoning/tool-call/tool-result parts).
- **Web team chat** — `apps/openagents.com/apps/web/src/page/loggedIn/page/chat.ts`
  + backend `.../workers/api/src/team-chat.ts`, `team-chat-routes.ts`,
  migration `migrations/0012_team_chat_messages.sql` (`TeamChatMessage`:
  message | autopilot_intent | adjutant_intent | system). A separate surface; not
  the desktop default.
- **Tassadar in the desktop** — proof-replay scene is already wired:
  `apps/autopilot-desktop/src/shared/proof-replays.ts` and the
  `tassadarProofReplay*` scene imported into `view.ts`. This is the existing
  grounding hook for "show the verified computation."

### 4.3 Recommended architecture (the build)

A **new lightweight desktop chat pane** that is a **Blueprint program runner**
grounded in **Tassadar proof-replays**, built on the existing session substrate.
Three layers:

1. **Chat shell (NEW, desktop UI — coordinate with concurrent `src/ui/*` work):**
   a new `"chat"` `PaneId`, a `chatPane` view, and a `chatMessages` Model field
   (id, role, body, timestamp, optional `linkedSessionRef`, optional
   `proofReplayRef`). No new RPC verb for MVP — reuse `session.spawn` +
   node-state polling for turn execution. Set it as the post-onboarding default
   via the §4.1 transition.

2. **Blueprint structuring (EXISTS — wire, don't rebuild):** each chat turn runs
   through Probe's `signature-lookup.ts` to pick a Blueprint **signature** →
   typed program → scoped **tool menu** → **evidence-only** program run, with
   action submissions gating any write. This is the "all the blueprint stuff"
   from the vision: the chat is a typed program, not a freeform prompt. (Heavier
   structuring is available as schemas grow; optimizer/RLM is later, §5.)

3. **Tassadar grounding (PARTIAL — wire what's real, gate the rest):**
   - *Now (real):* link each turn that involves exact computation to a Tassadar
     **proof-replay bundle** (existing `proof-replays.ts` + scene) so the chat
     shows the verified digest, not just a claim.
   - *Next (partially real):* for exact sub-steps, compose Tassadar **dense
     weight modules** via `packages/tassadar-executor/src/linked-dense-module-runtime.ts`
     and show the composed/exact-replay digest. The composition runtime EXISTS;
     the "turn → which modules → execute → fold into answer" planner is NEW.
   - *Later (aspirational):* true "chat with the Tassadar model" — an NL
     serving/inference path — does not exist and is a separate milestone (§5
     Phase 4), gated on a real serving seam (likely the Psion learned lane as the
     conversational shell, with Tassadar modules as the exact core, per the W3
     "frozen analytic core + thin learned shell" result).

**Why this shape:** it makes the chat real *now* on substrate that exists
(Blueprint signatures + session runtime + proof-replay grounding), keeps every
honest claim boundary intact (evidence-only program runs, exact-replay digests),
and leaves a clean upgrade path to module-composed and finally NL-served Tassadar
answers without rework.

---

## 5. Phased plan

**Phase 0 — Default-screen seam (small, NEW; coordinate on `src/ui/*`).**
Add a `"chat"` pane and auto-navigate to it when onboarding completes
(`update.ts` on onboarding-complete; optional conditional in `initial-state.ts`).
Acceptance: a returning, onboarded user lands on chat, not the wizard or the
network viz.

**Phase 1 — Blueprint-structured chat over the session runtime (mostly wiring).**
Chat turn → `signature-lookup.ts` selects a Blueprint signature → typed program →
scoped tool menu → `session.spawn` executes → node-state poll streams events →
render as a chat transcript; writes go through action submissions + release gate.
Acceptance: a turn shows its selected signature, its scoped tools, and an
evidence-only program-run record.

**Phase 2 — Tassadar proof-replay grounding (real, low risk).**
When a turn touches exact computation, attach/show the existing Tassadar
proof-replay bundle (`proof-replays.ts` + scene). Acceptance: the chat displays a
verified trace digest for exact sub-steps.

**Phase 3 — Module composition for exact sub-steps (partially real).**
Build the planner that maps an exact sub-step to one or more Tassadar dense
weight modules and runs them via `linked-dense-module-runtime.ts`, folding the
exact result + composed digest into the answer. Acceptance: a chat answer
includes a result produced by composing verified modules, with the composed
digest shown. (Depends on registry/listing maturation — today a fixture/listing
ref, not a live registry service.)

**Phase 4 — "Chat with the Tassadar model" (aspirational; new serving seam).**
Stand up an NL serving path. Most credible per the W3 sweep: a learned
conversational shell (Psion lane) that *plans and narrates* while delegating
exact sub-computations to composed Tassadar modules (Phase 3). Acceptance: a
free-form prompt yields an answer whose exact parts are Tassadar-verified.
This is the only phase that needs net-new model serving and should not block
Phases 0–3.

---

## 6. Top gaps (honest)

1. **No Tassadar inference/serving / chat surface.** Only execute / replay /
   settle + dense-module run/compose. (§2.3) — blocks true "chat with the model";
   does NOT block a blueprint-structured, proof-replay-grounded chat.
2. **No live blueprint optimizer / RLM runtime.** Typed contracts + governance
   are real; the GEPA/DSPy improvement loop is schema+design only. (§3.3) — chat
   can ship structured without it; optimization is a later upgrade.
3. **No post-onboarding auto-transition** to any default screen yet. (§4.1) —
   small NEW change, but in the concurrently-edited `src/ui/*`.
4. **No persistent chat message model in the desktop** (Composer is
   session/objective-driven, not a message list). (§4.2) — NEW, small.
5. **Module registry is a fixture/listing-ref + design, not a live
   listable/priceable service.** (§2.2, §2.3) — limits Phase 3 breadth.

---

## 7. Key file references

Tassadar (executor): `packages/tassadar-executor/src/numeric-executor.ts`,
`.../dense-weight-module-runtime.ts`, `.../linked-dense-module-runtime.ts`,
`.../capability-envelope.ts`, `.../replay.ts`, `.../index.ts`.
Tassadar (live loop): `apps/openagents.com/workers/api/src/tassadar-run-admission.ts`,
`.../tassadar-replay-validator.ts`, `.../tassadar-auto-settlement.ts`,
`.../tassadar-run-settlement.ts`, `.../tassadar-poc-fixture.ts`,
`.../artanis-scheduled-runner.ts`.
Tassadar (docs): `docs/tassadar/RESEARCH_PLAN.md`,
`docs/tassadar/2026-06-11-llm-computer-full-introduction.md`,
`docs/tassadar/work-that-proves-itself.md`,
`docs/tassadar/2026-06-14-w3-student-program-report.md`,
`docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`.
Blueprint: `apps/openagents.com/workers/api/src/blueprint/` (schemas/,
repositories/, services/, fixtures/, exports/, README.md), `.../blueprint-routes.ts`,
`packages/probe/packages/runtime/src/blueprint/signature-lookup.ts` (+ siblings),
`apps/openagents.com/docs/blueprint/2026-06-05-legacy-blueprint-primitives-openagents-inventory.md`.
Chat/session substrate: `apps/autopilot-desktop/src/ui/initial-state.ts`,
`.../ui/model.ts`, `.../ui/view.ts`, `.../ui/update.ts`,
`.../shared/onboarding-status.ts`, `.../shared/rpc.ts`,
`.../bun/pylon-control.ts`, `.../bun/node-state-poll.ts`,
`.../shared/proof-replays.ts`, `apps/pylon/packages/runtime/src/llm/messages.ts`,
`apps/openagents.com/apps/web/src/page/loggedIn/page/chat.ts`,
`apps/openagents.com/workers/api/src/team-chat.ts`.
Reference (concepts only): `projects/repos/gepa` (workspace), deprecated
workspace `blueprint/` repo.
