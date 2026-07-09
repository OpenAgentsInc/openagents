# Execution sequence and critical path

- Date: 2026-07-09
- Status: Sol analysis; explanatory companion
- Strategic source: [`MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md)
- Day-to-day plan: [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md)

## Purpose

The master roadmap contains the high-level strategic order and its latest
issue snapshot. The Sol implementation roadmap reconciles day-to-day state.
This document extracts the dependency logic beneath both so work can be
evaluated by the product loop it unlocks, not only by its issue number.

## The critical-path graph

```text
conversation reliability + persona-neutral inference
                         |
                         v
authenticated Sarah relationship + live Blueprint canvas
                         |
                         v
first typed Sarah -> Khala -> Pylon -> worker delegation
                         |
                         v
resumable progress + verification + exact receipt in canvas
                         |
                         v
mobile/desktop continuation on one Effect Native intent model
                         |
                         v
standing responsibilities + budgets + approval inbox
                         |
                         v
provenance-bearing company brain + reusable employee templates
```

There are supporting critical paths beneath it:

- Effect Native substrate → catalog/host gaps → web/mobile/desktop/canvas
  conversion.
- Brokered account custody → Agent Computer execution → continuity,
  concurrency, and harness parity.
- Approval and exact-accounting rails → in-conversation payment, outbound
  send, and standing employees.
- Outcome receipts → honest templates, public promises, and network effects.

## Convergence milestone 1: Sarah is dependable

The current queue correctly puts owned-avatar quality and hardening first.
Under Sarah-first, video, audio, turn latency, and fallback are not cosmetic.
They determine whether the front door is available.

The milestone is not “best benchmark score.” It is:

- a natural opener and stable real-time conversation;
- bounded time to first response;
- text remains fully usable if rendering fails;
- simulator and deployment gates catch freezes, invalid crops, cadence loss,
  and stale assets;
- production inference uses the persona-neutral Khala lane with exact usage,
  caps, and typed fallback.

Offline quality exploration should continue, but it must not indefinitely
delay a reliable, honest interaction tier.

## Convergence milestone 2: Sarah performs one real bounded job

With the Blueprint Map complete, the next architectural proof is the coding
vertical slice described in the Sarah-first thesis. It should be filed and
tracked explicitly if it does not already have a concrete lane; otherwise a
load-bearing product step can hide between epics.

The bounded sequence is:

1. Authenticate and resolve the owner's linked capacity.
2. Convert “run issue N” into the existing typed coding workflow.
3. Show the proposed target, repository ref, and verification command.
4. Dispatch without exposing or copying credentials.
5. Stream durable progress to Sarah's canvas.
6. Render typed failures or the verified closeout.
7. Reconcile exact token and lifecycle evidence.
8. Accept a follow-up in the same conversation.

This slice should use a public-safe fixture or bounded public issue first. It
is product integration, not a new executor.

## Convergence milestone 3: launch and transact

Sarah cannot be the commercial front door while checkout, account creation,
or launch readiness require an unrelated manual journey. The next closure is:

- in-conversation account and payment paths remain code-priced and
  receipt-backed;
- the seeded test account and unattended mobile straight line pass;
- public copy stays within promise state;
- the mobile product opens into Sarah and can resume the relationship;
- store artifacts and owner-gated submissions retain their explicit status.

This milestone converts the architecture into a usable acquisition and
activation loop.

## Convergence milestone 4: one application grammar

Effect Native work can run in parallel with the Sarah vertical slice where
paths are disjoint. The valuable order is:

1. Keep substrate/catalog work continuous.
2. Finish the Sarah and landing surfaces as real Effect Native consumers.
3. Convert the mobile relationship and approval path early enough to prove
   cross-device continuity.
4. Convert the desktop cockpit where specialist power is needed.
5. Fold graph/canvas semantics into the common contract.
6. Delete each replaced legacy path in the same conversion.

The exit proof is not merely renderer adoption. It is one material workflow
that begins on one device, continues on another, and emits the same typed
intents and state transitions.

## Convergence milestone 5: execution becomes a standing capability

Pylon extraction, typed RPC, harness parity, and Agent Computer work matter
because they remove local-shell and one-account assumptions. They should
converge on:

- named, isolated provider accounts;
- honest health, capacity, and cost classes;
- typed selection and fallback;
- durable session continuity;
- owner-local and org-cloud capacity that remain separate authority rails;
- steer, interrupt, resume, verify, and writeback from the shared work model;
- cockpit views that project the same state Sarah narrates.

Only after this is routine should the product claim that Sarah can reliably
hold ongoing responsibilities.

## Convergence milestone 6: roles and brain

Standing employees should first appear as typed responsibilities behind
Sarah, with explicit budgets and approvals. The company brain then matures the
existing Blueprint Map rather than introducing a separate knowledge product.

The dependency is deliberate:

- no durable role without a proven execution rail;
- no autonomous trigger without admission, budget, and auto-pause;
- no brain fact without provenance and access scope;
- no authority promotion without a receipt;
- no template listing without a real external outcome.

This sequencing protects the roadmap from producing persuasive simulations of
employees before it can operate them safely.

## Parallel work that is genuinely independent

The following lane families can move concurrently when they do not share hot
files or owner gates:

- avatar experiment recipes versus production simulator hardening;
- Effect Native upstream catalog work versus scoped consumer conversions;
- backend account/broker work versus UI projection work built against stable
  schemas;
- outbound deliverability preparation versus Sarah-to-coding integration;
- docs, eval fixtures, and behavior contracts ahead of implementation;
- cloud source hardening that does not mutate live infrastructure.

Parallelism is unsafe when two lanes redefine the same schema, catalog
monolith, authority boundary, or public claim. Those are serial integration
points even if many agents are available.

## Work-selection rule

When choosing between two ready tasks, prefer the task that does the most of
the following:

1. closes a currently broken user loop;
2. removes a second state or authority model;
3. produces a reusable typed contract;
4. creates a live proof rather than another designed path;
5. deletes legacy implementation after replacement;
6. reduces owner effort per outcome;
7. unlocks multiple downstream lanes;
8. has a clear falsifiable exit receipt.

This favors convergence over surface-area growth.

## Status discipline

The roadmap contains fast-moving issue snapshots. A landed implementation
slice does not automatically close a lane whose live proof or owner gate is
still open. Conversely, an issue remaining in a prose queue after a newer
commit does not prove the work is unstarted.

Execution should therefore reconcile four things before closeout:

- current `origin/main` implementation;
- current issue and pull-request state;
- the lane's explicit exit receipt;
- any owner-only proof or production action.

The strategic sequence is stable even when issue numbers move: make Sarah
dependable, prove one real job through her, close the transaction and launch
loop, unify the application surfaces, make execution durable, then generalize
into roles and a brain.
