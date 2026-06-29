# Autopilot Blueprint Chat + Tassadar Modules Audit

Date: 2026-06-18 (rewritten 2026-06-18 to correct a misframing — see §0.1)
Repo: `OpenAgentsInc/openagents`
Scope: research/audit only. No app code was changed. `apps/autopilot-desktop/src/ui/*`
was read (read-only) but NOT edited; that directory has concurrent work.

All file paths below are repo-relative to `OpenAgentsInc/openagents` unless noted.

---

## 0. The vision being audited

> "Once you're past onboarding, I want the default screen to be the **chat** —
> a chat driven by all the **blueprint** stuff, with **Tassadar's verified
> computation modules available as steps** inside it."

One post-onboarding default chat pane, built from two real pieces:

1. **Blueprint is the chat framework.** Blueprint is the DSPy-style
   typed-program / signature framework that EXISTS in this repo
   (`apps/openagents.com/workers/api/src/blueprint/` + the Probe consumer
   `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`). A chat
   turn runs a Blueprint program: a signature selects a typed program, which
   runs scoped tools / steps.
2. **Tassadar's verified-computation modules are steps inside Blueprint
   signatures.** Tassadar's dense-weight / linked-module runtime
   (`packages/tassadar-executor/src/dense-weight-module-runtime.ts`,
   `.../linked-dense-module-runtime.ts`, the module registry/marketplace refs,
   exact-replay digests) provides exact, replay-verifiable computation units.
   When a Blueprint program runs a step bound to a Tassadar module, that step
   executes the module's exact, replay-verifiable computation and carries the
   replay receipt as its evidence.

So the chat's language reasoning is the normal Blueprint program runner / agent
runtime; **Tassadar modules are the exact-computation building blocks it
invokes as steps.** This doc reports, with honest EXISTS-vs-NEW marking, whether
each piece is real today, what the cleanest substrate is, and a phased plan to
make a Blueprint-driven, Tassadar-module-composed chat the post-onboarding
default.

### 0.1 Correction note (why this doc was rewritten)

An earlier draft of this audit misframed the work as "chat with the Tassadar
model" and proposed a Tassadar **serving / inference seam** (an NL prompt →
Tassadar completion path, plus an aspirational "learned shell + exact core
inference" phase). **That framing was wrong and is rejected.** Tassadar is not a
chat model and there is no inference endpoint to build. Tassadar produces
**verified computation modules**; those modules **compose inside Blueprint
signatures as steps**. This rewrite removes every "chat with Tassadar /
inference / serving seam" framing and reframes around modules-as-steps. The
honest EXISTS-vs-NEW findings about Blueprint (real typed contracts in-repo;
optimizer/RLM is schema/design) and the Tassadar module runtime (real
exact-execution units) are preserved.

---

## 1. Executive answers (read this first)

**(a) What is the chat, architecturally?**
The chat is a **Blueprint program runner**. A chat turn selects a Blueprint
**signature** → a typed program → scoped **tools / steps** → an evidence-only
program run → a rendered response. This runs on the existing agent/session
substrate (`session.spawn` + node-state polling). There is **no separate chat
model and no inference endpoint** — the language reasoning is the normal
Blueprint/agent runtime. (Details: §3, §4.)

**(b) Where do Tassadar's modules fit?**
Tassadar's **verified-computation modules** are bound as **typed steps inside
Blueprint signatures**. When a program runs a Tassadar-bound step, the step
resolves and executes that module's **exact, replay-verifiable computation**
(`dense-weight-module-runtime.ts` / `linked-dense-module-runtime.ts`) and
carries the **exact-replay digest** as the step's evidence. So Tassadar modules
**compose inside Blueprint signatures** — they are the exact-computation
building blocks the chat invokes, not something you "chat with." (Details: §2,
§4.)

**(c) Is each piece real today?**
- **Blueprint:** YES as a typed-contract + governance framework — schemas,
  repositories, services, fixtures, a contract export, HTTP routes, and a live
  Probe-runtime consumer (signature lookup + tool scoping). The **optimizer /
  GEPA / RLM improvement loop is schema + design only** (not a live runtime).
  (Details: §3.)
- **Tassadar modules:** YES as **exact-execution units** — the executor package
  runs and composes dense compiled weight modules with pinned, replay-verifiable
  digests (`dense-weight-module-runtime.ts`, `linked-dense-module-runtime.ts`).
  The **module registry/marketplace is a fixture + design today**, not a live
  resolvable service. (Details: §2.)

**(d) Top gaps + phased plan.**
The load-bearing NEW seams: (1) a typed **Blueprint signature step that binds +
executes a Tassadar module** (the core seam, with exact-replay carried as step
evidence); (2) a **live, resolvable module registry** (it's a fixture/listing-ref
today) so Blueprint can look modules up; (3) a **Blueprint chat-program runtime**
that drives a chat turn through signature → program → steps on the existing
session substrate; (4) the desktop **chat pane UI** and the **post-onboarding
default transition**. The phased plan (§5) builds entirely on what is real
(Blueprint signatures + Tassadar module runtime + exact-replay) and contains **no
Tassadar inference/serving phase** — that idea is rejected. (Details: §5, §6.)

---

## 2. The Tassadar piece — verified-computation modules

### 2.1 What Tassadar is (concept)

Tassadar is OpenAgents' implementation of the Percepta "LLM-computer"
construction: transformers made to **compute exactly** (compiled, not trained)
via an Append-only Lookup Machine (ALM). The pipeline is
program → ALM gate graph → schedule → analytic weights → deterministic execution,
**verifiable by exact replay** (re-run, compare digests byte-for-byte).

The product unit that matters for this chat is the **module**: a dense /
composed compiled-weight artifact that performs an exact computation and is
verifiable by exact replay. These modules are what compose **as steps inside
Blueprint signatures**. Tassadar is *not* a conversational model and has no
prompt→completion surface; treating it as one is a category error.

Primary docs (all under `docs/tassadar/`):

- `docs/tassadar/README.md`
- `docs/tassadar/RESEARCH_PLAN.md` — unified directive; the governing question is
  "Can the exactness we can compile become something we can train, sell, and
  embed?" The relevant answer for this chat: embed the modules as composable,
  verifiable steps.
- `docs/tassadar/2026-06-11-llm-computer-full-introduction.md` — assume-nothing
  intro to the LLM-computer construction.
- `docs/tassadar/work-that-proves-itself.md` — business thesis; describes
  "learning by construction" and modules-as-artifacts (the composable units this
  chat consumes).
- `docs/tassadar/2026-06-14-w3-student-program-report.md` — research/eval only;
  no public model claim.
- `docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md`
  — the most honest current-state + gap audit. Read this before believing any
  "the model" framing.

Two-lane framing (RESEARCH_PLAN §2): **Tassadar** = compiled/exact/digest-pinned
("written, not learned"); **Psion** = the learned compact-decoder lane. They must
not borrow each other's claim language. For this chat, only the Tassadar module
lane is in scope — as exact computation steps.

### 2.2 What EXISTS in code (the module runtime — real)

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
  **This is the single-module exact-execution unit** that a Blueprint step would
  bind to and run.
- `linked-dense-module-runtime.ts` (+ `linked-dense-module.ts`, `-fixture.ts`) —
  **module composition**: links/composes multiple dense modules, pins composed
  digests, declares a marketplace listing ref
  (`listing.public.tassadar_compiled_weight_module.cc1403674fc0d388`), a consumer
  family, a claim class, a required trust posture
  (`benchmark_gated_internal`), and per-module expected compatibility digests.
  **This is the composition primitive** a multi-step Blueprint program would use
  to chain Tassadar modules.
- `capability-envelope.ts`, `lane.ts`, `replay.ts`, `replay-cli.ts`,
  `self-test.ts`, `compiled-program-corpus.ts` — capability/claim envelope,
  **replay surface** (the exact-replay verification carried as step evidence),
  CLI, self-test, and a small compiled-program corpus.

**Live run loop — `apps/openagents.com/workers/api/src/`** (EXISTS, real):

This loop proves the modules are real and that their computation is settled
against real value. It is the **execute → replay-verify → settle** market for
the modules, and is the source of the **exact-replay receipts** a chat step
would surface.

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

### 2.3 What is fixture/design (the registry + the chat-step binding)

These are the NEW seams between the (real) module runtime and a Blueprint chat:

- **Module registry / marketplace is a fixture + design, not a live service.**
  `linked-dense-module-runtime.ts` carries a marketplace **listing ref**, a
  consumer family, a claim class, and a trust posture — but there is **no live,
  listable/resolvable registry service** a Blueprint step could query to resolve
  "this step → that module digest." Today modules are reached via fixtures and
  pinned refs. Making the registry a **live, resolvable service** is a
  prerequisite for binding modules into signatures at runtime.
- **No Blueprint-step → Tassadar-module binding yet.** Nothing today expresses
  "this Blueprint signature step resolves and executes a Tassadar module, and
  carries its exact-replay digest as the step's evidence." That typed seam is the
  **core NEW work** (§4).
- The modules carry explicit **redaction guards** (e.g.
  `linked-dense-module-runtime.ts` enforces that public-safe refs must *not*
  contain raw prompts/traces/secrets). This is correct and the chat step must
  honor it: a Tassadar step surfaces **digests and verdicts**, never raw
  internal traces.

**Honest verdict (Tassadar):** Tassadar's **verified-computation modules are
real exact-execution units** — single (`dense-weight-module-runtime.ts`) and
composed (`linked-dense-module-runtime.ts`), with pinned, replay-verifiable
digests, proven against a live execute/replay/settle market. What is NEW is (1) a
**live module registry** so they can be resolved by a signature step, and (2) the
**typed Blueprint step that binds + executes a module and carries its replay
receipt as evidence**. There is **no Tassadar chat / inference / serving surface
to build** — that is explicitly out of scope.

---

## 3. The blueprint piece — the chat framework

### 3.1 What "blueprint" means here

Blueprint is OpenAgents' DSPy-style framework for **typed, composable, governed**
chat/work capabilities: stable input/output **signatures**, **module versions**
behind them, **typed program** runs with strict authority boundaries, scoped
**tool / step** menus, and a (planned) **optimizer** loop. It is implemented in
THIS repo. **Blueprint is the chat framework**: a chat turn IS a Blueprint
program run, and Tassadar modules plug in as steps within a signature.

### 3.2 What EXISTS in code (typed contracts + governance — real)

**Schemas — `apps/openagents.com/workers/api/src/blueprint/schemas/`** (EXISTS):

- `program.ts` — `BlueprintProgramSignature` (stable I/O contract, with
  `evidenceRequirements` and `toolScopes`) and `BlueprintProgramType` (behavior
  families with risk class, evidence/receipt requirements, tool scopes,
  direct-mutation policy). The `toolScopes` / step structure here is the natural
  attach point for a Tassadar-module step.
- `module.ts` — `BlueprintModuleVersion` (implementation artifact behind a
  signature, with lifecycle/release states).
- `optimizer-run.ts` — `BlueprintOptimizerRun` with kinds including
  `gepa_style_reflection` (DSPy/GEPA lineage). **Schema only** (see §3.3).
- Plus program-run, action-submission, continuation-decision, mission-briefing,
  release-gate, source-authority, context-pack, simulation-branch schemas.

**Repositories / services / fixtures / exports / routes** (EXISTS):

- `blueprint/repositories/` — `program-runs.ts` (evidence-only),
  `action-submissions.ts` (approval-gated write boundary),
  `probe-contributions.ts`.
- `blueprint/services/` — `program-run-authority.ts` (denies
  deploy/email/PR/spend/source-mutation effects), `continuation-decision.ts`
  (classifies a turn into
  continue/test/fix/summarize/request_context/retry/stop/escalate),
  `continuation-mission-briefing.ts`, `release-gate.ts`,
  `signature-contribution.ts`, `smoke-probe.ts`.
- `blueprint/fixtures/` — `autopilot-continuation-signatures.ts` (seed
  signatures: continuation, test, fix, summarize, request-context, retry, stop,
  escalate, prepare-review, routing, research-policy, email-decisioning,
  proof-projection), `program-registry.ts`.
- `blueprint/exports/contract-export.ts` — JSON-Schema/OpenAPI exports for
  agents and Rust consumers.
- `blueprint-routes.ts` — Blueprint HTTP API surface.
- `blueprint/README.md` — states Optimizer Runs is the *future* home for
  GEPA/DSPy-style improvement.

**Probe runtime consumer — `packages/probe/packages/runtime/src/blueprint/`** (EXISTS):

- `signature-lookup.ts` — **DSPy-style signature selection** from backend
  capability + risk + tool constraints. This is the live "choose the typed
  program for this turn" path — the entry point of a Blueprint chat turn.
- `tool-menu.ts` (scope tools from a signature), `program-run-evidence.ts`
  (emit evidence — where a Tassadar step's replay receipt would attach),
  `action-submission.ts` (propose approval-gated writes),
  `registry-client.ts` (resolve signatures from a registry — the analog the
  Tassadar module registry needs), `contracts.ts`, `contribution.ts`.
- Reference:
  `packages/probe/docs/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`.

**Core invariants enforced today:** program runs are evidence-only (not write
authority); action submissions are the only write-side boundary (deploy / email /
PR / spend / source mutation / public claims); release gates required before
promotion; no direct mutation in v1 signatures; tool access is read / evidence /
propose_action. A Tassadar-module step is a **read/evidence-class** step (exact
computation + replay receipt), so it fits cleanly inside this boundary.

### 3.3 What is DESIGN-ONLY / FUTURE

- **Optimizer / GEPA / RLM runtime loop** — `BlueprintOptimizerRun` schema exists
  with `gepa_style_reflection`, but there is **no live optimizer runtime** that
  proposes candidates and re-evaluates modules. The README marks it as the future
  home. GEPA references elsewhere are job records / fixtures / smoke tests, not a
  runtime. The chat does NOT depend on this — it can ship structured without it.
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
schema+design only.** This is the framework that *is* the chat: pick a signature
→ typed program → scoped tools/steps → evidence-only run — with Tassadar modules
available as exact-computation steps inside that program.

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
  chat — but it is the closest existing turn-execution substrate to reuse.
- **Session/control substrate** — `apps/autopilot-desktop/src/shared/rpc.ts`
  (`DesktopRPCSchema`, `SessionEventRow`),
  `apps/autopilot-desktop/src/bun/pylon-control.ts` (`spawnSession`,
  `connectBridgeDesktop`), `apps/autopilot-desktop/src/bun/node-state-poll.ts`
  (polls node state → sessions + per-session events). The desktop spawns sessions
  on Pylon and observes events; the agent/LLM reasoning lives in the Pylon
  runtime. Pylon's internal LLM message model is
  `apps/pylon/packages/runtime/src/llm/messages.ts`
  (text/media/reasoning/tool-call/tool-result parts) — the place a Tassadar
  tool-call/tool-result step would render.
- **Web team chat** — `apps/openagents.com/apps/web/src/page/loggedIn/page/chat.ts`
  + backend `.../workers/api/src/team-chat.ts`, `team-chat-routes.ts`,
  migration `migrations/0012_team_chat_messages.sql` (`TeamChatMessage`:
  message | autopilot_intent | adjutant_intent | system). A separate surface; not
  the desktop default.
- **Tassadar in the desktop** — proof-replay scene is already wired:
  `apps/autopilot-desktop/src/shared/proof-replays.ts` and the
  `tassadarProofReplay*` scene imported into `view.ts`. This is the existing
  rendering hook for showing a Tassadar step's verified computation inline.

### 4.3 Recommended architecture (the build)

A **new lightweight desktop chat pane** that is a **Blueprint program runner**,
with **Tassadar verified-computation modules available as steps**, built on the
existing session substrate. Three layers:

1. **Chat shell (NEW, desktop UI — coordinate with concurrent `src/ui/*` work):**
   a new `"chat"` `PaneId`, a `chatPane` view, and a `chatMessages` Model field
   (id, role, body, timestamp, optional `linkedSessionRef`, and per-step refs
   including any `tassadarModuleStepRef` / `proofReplayRef`). No new RPC verb for
   MVP — reuse `session.spawn` + node-state polling for turn execution. Set it as
   the post-onboarding default via the §4.1 transition. The pane renders the
   program's steps, and for any Tassadar step shows the module + its
   exact-replay verification inline.

2. **Blueprint as the chat runtime (EXISTS — wire, don't rebuild):** each chat
   turn runs through Probe's `signature-lookup.ts` to pick a Blueprint
   **signature** → typed program → scoped **tool/step menu** → **evidence-only**
   program run on the existing session substrate, with action submissions gating
   any write. The chat IS a typed Blueprint program, not a freeform prompt.
   (Optimizer/RLM is later, §5, and not required.)

3. **Tassadar modules as steps (core NEW seam — wire what's real):**
   - *The seam:* extend a Blueprint signature so a step can be **bound to a
     Tassadar module** (a `toolScopes` entry of a Tassadar-module kind). At run
     time the step **resolves the module from the registry, executes it**
     (`dense-weight-module-runtime.ts` for single, `linked-dense-module-runtime.ts`
     for composed), and emits the **exact-replay digest** as the step's evidence
     via `program-run-evidence.ts`. The exact computation is born verified.
   - *Registry prerequisite:* make the Tassadar module registry a **live,
     resolvable service** (today a fixture/listing-ref, §2.3) so the step can
     resolve "this step → that module digest" at run time.
   - *Inline evidence:* render the Tassadar step's module + verdict + digest in
     the chat using the existing `proof-replays.ts` scene, honoring the redaction
     guards (digests/verdicts only, never raw traces).

**Why this shape:** the chat is a real Blueprint program runner *now* on
substrate that exists (Blueprint signatures + session runtime), and Tassadar
modules plug in as exact, born-verified steps via the real module runtime — every
honest claim boundary intact (evidence-only program runs, exact-replay digests).
**There is no Tassadar serving/inference layer in this architecture; modules
compose inside signatures.**

---

## 5. Phased plan

**Phase 0 — Default-screen seam (small, NEW; coordinate on `src/ui/*`).**
Add a `"chat"` pane and auto-navigate to it when onboarding completes
(`update.ts` on onboarding-complete; optional conditional in `initial-state.ts`).
Acceptance: a returning, onboarded user lands on chat, not the wizard or the
network viz.

**Phase 1 — Blueprint-driven chat over the session runtime (mostly wiring).**
Chat turn → `signature-lookup.ts` selects a Blueprint signature → typed program →
scoped tool/step menu → `session.spawn` executes → node-state poll streams events
→ render as a chat transcript; writes go through action submissions + release
gate. Acceptance: a turn shows its selected signature, its scoped tools/steps,
and an evidence-only program-run record.

**Phase 2 — Live Tassadar module registry (NEW backend, real foundation).**
Promote the module registry from fixture/listing-ref to a **live, resolvable
service** so a signature step can look modules up. Acceptance: a registry query
resolves a module ref → its pinned digest + capability/claim envelope, and the
list is browsable.

**Phase 3 — Tassadar module as a Blueprint signature step (the core seam).**
Add a typed step kind that binds a step to a Tassadar module, resolves it from
the registry (Phase 2), executes it via
`dense-weight-module-runtime.ts` / `linked-dense-module-runtime.ts`, and emits the
**exact-replay digest as the step's evidence**. Acceptance: a Blueprint program
run includes a step whose result is produced by a Tassadar module, with the
exact-replay digest carried as that step's evidence (born verified).

**Phase 4 — Inline evidence/verification surface in the chat.**
Render a Tassadar-module step in the chat with its module ref, verdict, and
exact-replay receipt inline (reuse `proof-replays.ts`), honoring redaction
guards. Acceptance: a chat answer that used a Tassadar step shows the
born-verified computation (module + digest + verdict) inline, with no raw traces
leaked.

**Phase 5 (optional, later) — Blueprint optimizer/RLM loop.**
Stand up the live `BlueprintOptimizerRun` runtime (GEPA/DSPy-style) to improve
signatures/modules over evidence. Independent of Phases 0–4; the chat ships
structured without it.

*(Removed from this plan vs. the earlier draft: the rejected "chat with the
Tassadar model via a serving seam / learned shell + exact core inference" phase.
Tassadar is not served; its modules compose as steps inside Blueprint
signatures.)*

---

## 6. Top gaps (honest)

1. **No typed Blueprint step that binds + executes a Tassadar module.** The
   module runtime is real and the signature `toolScopes`/step structure is real,
   but the seam between them (resolve → execute → carry exact-replay as step
   evidence) is NEW. (§2.3, §4.3) — this is the core build.
2. **Module registry is a fixture/listing-ref + design, not a live resolvable
   service.** (§2.2, §2.3) — prerequisite for binding modules into signatures at
   run time.
3. **No Blueprint chat-program runtime in the desktop** (Composer is
   session/objective-driven, not a chat-turn → signature → program → steps loop,
   and there is no persistent chat message model). (§4.1, §4.2) — NEW, builds on
   existing session substrate.
4. **No post-onboarding auto-transition** to any default screen yet. (§4.1) —
   small NEW change, but in the concurrently-edited `src/ui/*`.
5. **No live blueprint optimizer / RLM runtime.** Typed contracts + governance
   are real; the GEPA/DSPy improvement loop is schema+design only. (§3.3) — chat
   ships structured without it; optimization is a later, optional upgrade.

**Explicitly NOT a gap (rejected framing):** there is no missing "Tassadar
inference / serving / chat-with-the-model" endpoint to build. Tassadar produces
verified computation **modules**; those modules **compose as steps inside
Blueprint signatures**. Do not reintroduce a Tassadar serving seam.

---

## 7. Roadmap issues (GitHub)

This audit is implemented by the EPIC + child issues below (label: `roadmap`).
They encode the corrected architecture — Tassadar modules as steps inside
Blueprint signatures, surfaced through a Blueprint-driven chat pane — and contain
**no Tassadar inference/serving work** and **no GitHub Actions**.

- **EPIC #5449** — Blueprint × Tassadar chat: Tassadar computation modules as
  steps inside Blueprint signatures, surfaced as the post-onboarding chat pane.
- **#5450** — Bind Tassadar modules as typed Blueprint signature steps (core seam).
- **#5451** — Make the Tassadar module registry a live, resolvable service.
- **#5452** — Blueprint chat-program runtime (turn → signature → program → steps).
- **#5453** — Autopilot Desktop chat pane UI (Blueprint programs + inline Tassadar steps).
- **#5454** — Post-onboarding default → chat pane transition.
- **#5455** — Evidence/verification surface: Tassadar-module step carries its exact-replay receipt inline.

See EPIC #5449 for the authoritative child list and guardrails.

---

## 8. Key file references

Tassadar (module runtime): `packages/tassadar-executor/src/dense-weight-module-runtime.ts`,
`.../dense-weight-module.ts`, `.../linked-dense-module-runtime.ts`,
`.../linked-dense-module.ts`, `.../numeric-executor.ts`,
`.../capability-envelope.ts`, `.../replay.ts`, `.../index.ts`.
Tassadar (live execute/replay/settle market): `apps/openagents.com/workers/api/src/tassadar-run-admission.ts`,
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
`packages/probe/packages/runtime/src/blueprint/signature-lookup.ts` (+ siblings:
`tool-menu.ts`, `program-run-evidence.ts`, `registry-client.ts`),
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
