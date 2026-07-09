# Sarah-first subsystem implementation implications

- Date: 2026-07-09
- Status: grounded design map; implementation must still follow owning
  contracts and invariants
- Companion: [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md)

## Purpose

Sarah-first is only real when it changes subsystem seams. This document maps
the strategic thesis to concrete implementation responsibilities, prohibited
shortcuts, and nearest proofs.

## 1. `apps/sarah`: product orchestration surface

**Must become:** the authenticated relationship and presentation layer for
typed work, not a second implementation of each backend.

Implementation consequences:

- Add bounded Effect services/tools that call existing public contracts for
  coding, payment, CRM, and receipts.
- Keep the openagents.com Worker authoritative for CRM, credits, checkout, and
  promise state.
- Treat prospect, customer, and operator modes as decoded relationship state;
  never infer authority from conversation tone.
- Extend the existing SSE/event model with bounded run/progress/closeout
  projections keyed by safe durable refs.
- Preserve AI disclosure, one-question-at-a-time behavior, no-improvised-
  pricing, and cross-prospect isolation contracts.
- Keep text fully functional when avatar/video degrades.

Do not:

- import Worker internals because Sarah is now in the monorepo;
- store raw worker events, shell output, prompts, or local paths in Sarah
  transcript/Blueprint projections;
- build a Sarah-only assignment store or approval system.

Nearest proof: `SARAH-CODE-1`, then #8600 and #8607.

## 2. Khala inference and routing

**Must become:** Sarah's persona-neutral, receipted cognitive engine and the
typed router into work—not a raw provider call hidden behind Sarah branding.

Implementation consequences:

- Route both text and avatar-brain calls through the Khala gateway.
- Use a persona-neutral internal model identity; Sarah's role program belongs
  above provider/model routing.
- Coalesce VAD fragments into sane conversational turns before inference.
- Enforce per-session and per-turn cost/cap policy before provider calls.
- Emit exact usage and typed fallback events.
- Keep semantic selection centralized; no keyword routing for tools or memory.

Do not:

- count public counter movement as proof of a Sarah turn;
- leak model/provider fallback details as inconsistent persona behavior;
- let gateway routing broaden Sarah's tool authority.

Nearest proof: #8600 sustained-speech live receipt.

## 3. Blueprint Map and memory

**Must become:** the legible state model for the relationship and, later, the
company brain.

Implementation consequences:

- Distinguish supplied facts, derived facts, open questions, plans, running
  work, and verified outcomes visually and in schema.
- Require provenance refs for every evidence-backed edge.
- Add correction, deletion, and scope semantics before broad ingestion.
- Project coding runs as work nodes linked to repository/task/receipt refs,
  not as raw event dumps.
- Preserve per-prospect isolation when authenticated owner data appears in the
  same canvas.
- Design the current DOM graph model so EN-6 can lower it to the shared canvas
  renderer without changing semantics.

Do not:

- equate model confidence with fact status;
- persist private run details merely because the current viewer owns them;
- use the map as a second workflow authority.

Nearest proof: one Sarah-originated coding run whose plan, state, blocker, and
receipt remain correct through reconnect.

## 4. Khala workflow, fleet, and durable streams

**Must become:** the stable work-control plane behind Sarah and specialist
tools.

Implementation consequences:

- Expose typed programs for dispatch, status, steer, interrupt, resume, proof,
  and closeout.
- Keep assignment/durable request refs as the correlation spine across Sarah,
  mobile, and desktop.
- Normalize harness progress into the neutral chat/run event model without
  destroying provider-specific private evidence.
- Make fallback explicit and policy-driven.
- Project safe progress frequently enough that Sarah never appears to “go
  silent” during long work.

Do not:

- make Sarah scrape CLI output;
- introduce a conversation-specific runner;
- use a public stream as the canonical raw event archive.

Nearest proof: `SARAH-CODE-1` fixture and live dogfood runs.

## 5. Pylon and `pylon-core`

**Must become:** a desktop-optional typed engine shared by CLI, Sarah-facing
workflow integration, and the desktop cockpit.

Implementation consequences:

- Finish custody, executor, presence, and wallet boundaries as Effect services.
- Finish typed RPC and consume it before deleting stdout parsing.
- Preserve named isolated account homes and account-health taxonomy.
- Keep Spark wallet as a live rail behind its own verified boundary.
- Consolidate MCP only after feature/authority parity is traced.
- Make run status and receipts queryable by durable refs Sarah can safely
  project.

Do not:

- move engine authority into the GUI;
- merge owner-local capacity with org-cloud custody;
- extract wallet code without the required binary/WASM verification.

Nearest proof: #8578 exit followed by #8579 cockpit consumption.

## 6. Agent Computers and Cloud

**Must become:** the managed execution option for Sarah-directed work that
cannot or should not use an owner-local Pylon.

Implementation consequences:

- Build the rootfs reproducibly from source and pin its digest.
- Redeem broker grants inside the VM into scratch-only provider homes.
- Carry `provider_credential_policy: broker_only` through placement and guest
  execution.
- Separate model-token truth from compute lifecycle charging.
- Prove grant replay failure after reclaim.
- Keep control-plane fake-VM health separate from nested-virt Firecracker
  execution evidence.

Do not:

- use the public control host as proof of in-VM Codex;
- place raw credentials or topology in Sarah-visible receipts;
- add Claude/Grok cloud variants before the Codex contract works end to end.

Nearest proof: #8547, then #8549/#8588.

## 7. Mobile

**Must become:** Sarah in the user's pocket, with work supervision as her
first deep capability.

Implementation consequences:

- Preserve the existing shipping P0 straight line throughout MB-EN.
- Make Sarah the eventual navigation root/home, not an additional menu item.
- Carry conversation, Blueprint, approvals, and active-run state through
  Khala Sync.
- Use Effect Native navigation, gesture, list, and foreign-host contracts for
  voice/STT and native modules.
- Keep provider-account connection and target selection available as bounded
  actions Sarah can guide, while retaining direct settings for experts.
- Extend the MB-EN exit proof from generic message parity to a Sarah-originated
  conversation or run visible on mobile and desktop.

Do not:

- block current owner dogfood on completion of the full rewrite;
- rewrite Khala Sync, auth, push, credits, native STT, or OTA as part of UI
  conversion;
- weaken mobile behavior contracts to simplify the port.

Nearest proofs: #8543 existing-app E2E; #8597 cross-app Sarah continuation.

## 8. Desktop cockpit

**Must become:** the specialist deep-work projection over the same state Sarah
uses.

Implementation consequences:

- Render account health, capacity, concurrent runs, approvals, and exact
  receipts from typed RPC/Sync state.
- Emit the same pause/resume/drain/stop/steer intents used elsewhere.
- Support terminals, Monaco, and raw local diagnostics through typed foreign
  hosts without making them the shared product contract.
- Ensure a run started in Sarah can be opened deeply in the cockpit and a run
  started in the cockpit can be summarized accurately by Sarah.

Do not:

- make desktop a second orchestration authority;
- reintroduce a React/Tailwind shell architecture;
- retire OpenTUI before cockpit parity and owner proof.

Nearest proofs: #8574 + #8579, then #8580.

## 9. Effect Native

**Must become:** the application grammar shared by Sarah and every projection,
not an isolated design-system program.

Implementation consequences:

- Prioritize intents and state bindings needed by real Sarah workflows.
- Route component/host gaps upstream through the demand register.
- Maintain renderer conformance, version pins, and vendoring guards.
- Reconcile the repository Effect topology guard with the vendored Effect
  Native `4.0.0-beta.94` line before describing `check:deploy` as green; the
  source snapshot still expects `4.0.0-beta.70`.
- Keep semantic contracts shared while allowing renderer-specific fidelity.
- Delete legacy UI and duplicate state logic per converted surface.
- Measure feature lead time across the second renderer as the leverage test.

Do not:

- create app-local “temporary” primitives;
- equate visual parity with semantic parity;
- let broad route conversion delay the first Sarah-to-outcome integration.

Nearest proofs: #8597, #8574, and #8575; #8573 remains useful parallel burn.

## 10. Khala Sync and data projections

**Must become:** the continuity plane for the relationship and work state.

Implementation consequences:

- Define owner/prospect-scoped collections for conversation, Blueprint deltas,
  active run summaries, approvals, and bounded receipts.
- Use named server-authoritative mutators for all durable effects.
- Preserve dense versions, idempotent apply, access control, and honest
  staleness.
- Separate canonical private evidence from client-safe post-images.
- Make cross-device resume a normal fixture, not a special demo path.

Do not:

- sync raw provider events or private traces to clients;
- add optimistic effects for authoritative work transitions;
- create Sarah-specific polling when a Sync scope is the correct substrate.

Nearest proof: one active Sarah run survives web→mobile→desktop continuation.

## 11. CRM, outbound, and payments

**Must become:** capabilities Sarah can explain and coordinate while the
existing authorities continue to enforce them.

Implementation consequences:

- Complete deliverability events and suppression/opt-out round trips.
- Route replies into Sarah and the CRM with one relationship timeline.
- Keep draft→approval→send as the only outbound transition.
- Keep quote→checkout→settlement→provision as separate receipted states.
- Render payment and email receipts in conversation/canvas without exposing
  private processor data.
- Measure operator minutes and end-to-end attribution.

Do not:

- let semantic cache or memory affect price computation;
- allow “reply as Sarah” to bypass continuation approval;
- call a checkout link a sale before settlement.

Nearest proofs: #8558, #8561, and #8607.

## 12. QA, behavior contracts, receipts, and promises

**Must become:** the safety envelope for the single front door.

Implementation consequences:

- Add a Sarah-to-work fixture tier before live dogfood.
- Extend deploy simulators across conversation, Blueprint, dispatch, reconnect,
  closeout, and fallback.
- Register owner-stated UX expectations verbatim in behavior contracts.
- Reconcile task evidence against exact rows, not public counters.
- Keep live/fixture/designed status explicit in docs and copy.
- Add a could-not-prove section to every integration receipt.

Do not:

- weaken existing P0/QAM gates during MB-EN;
- treat a worker closeout as independent verification;
- let Sol analysis flip a promise state.

Nearest proof: the fixture and live receipt bundle for `SARAH-CODE-1`.

## 13. Operations and observability

**Must become:** one operational view of the relationship loop across avatar,
inference, orchestration, execution, Sync, and receipts.

Implementation consequences:

- Correlate conversation, assignment, session, and receipt refs without
  copying private payloads.
- Alert on front-door failure modes: render cadence, silent audio, inference
  fallback storms, stale progress, Sync lag, and missing closeout evidence.
- Distinguish owner action required from agent-fixable failure.
- Track cost and capacity separately for avatar GPU, hosted inference,
  owner-subscription capacity, and org-cloud compute.
- Keep text fallback and power-tool access available during Sarah incidents.

Do not:

- create a single giant log containing every private plane;
- use public status projections as private operational truth;
- keep live infrastructure exposure open after the bounded smoke need ends.

Nearest proof: a rehearsed degradation drill where video or a provider fails
and a live work run remains controllable.

## Cross-subsystem definition of done

A subsystem is Sarah-first only when it:

1. exposes a bounded typed capability to the shared relationship surface;
2. preserves its own authority rather than delegating authority to the model;
3. projects durable state through shared refs and scopes;
4. emits exact evidence and an understandable safe projection;
5. can be observed or steered from another authorized surface;
6. carries tests for success, refusal, reconnect, and degradation;
7. removes or stops growing a duplicate path.
