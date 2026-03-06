# Audit: Autopilot Earn MVP Full Implementation Gap

> Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, `docs/OWNERSHIP.md`, and `docs/kernel/`. File paths, issue states, and implementation-status claims here may be superseded by later commits.


Date: 2026-03-05

## Scope

This audit compares the current designed Autopilot Earn MVP to what is actually implemented in this repository today.

Authoritative design inputs:

- `docs/MVP.md`
- `docs/autopilot-earn/AUTOPILOT_EARN_MVP.md`
- `docs/PANES.md`
- `docs/OWNERSHIP.md`
- `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`

Primary implementation surfaces reviewed:

- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/sync_bootstrap.rs`
- `apps/autopilot-desktop/src/spacetime_presence.rs`
- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/state/wallet_reconciliation.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`
- `crates/nostr/core/src/nip90/mod.rs`
- `crates/nostr/core/src/nip89.rs`
- `crates/autopilot-spacetime/src/reducers.rs`
- `crates/autopilot-spacetime/src/schema.rs`
- `spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs`
- `docs/SPACETIME_ROLLOUT_INDEX.md`

Verification run during this audit:

- `cargo test -p autopilot-spacetime`
- `cargo test -p autopilot-desktop provider_nip90_lane -- --nocapture`

Both passed.

## Executive Summary

The designed Autopilot Earn MVP is not fully implemented.

The repo already contains real and reusable foundations:

- real NIP-90 relay subscribe and publish transport,
- real signed NIP-90 request, feedback, and result event construction,
- real Spark wallet actions and wallet-history reconciliation,
- a real local earnings truth rule that depends on wallet evidence,
- substantial local receipt, incident, and economy-snapshot logic,
- a real Spacetime-shaped sync and presence contract layer,
- tests that prove the core provider NIP-90 lane works end to end in harness form.

But the designed product now assumes a specific launch shape that is not in the code yet:

- an earn-first Mission Control shell,
- immediate offline market preview,
- OpenAgents-hosted Nexus as the default authority and primary relay,
- a curated default public relay set,
- hosted starter-demand jobs with single-assignee leases,
- hosted proof that a provider is an authenticated Autopilot session on the OpenAgents Nexus,
- auto-accept and automated provider execution,
- live deployed Spacetime for the approved domains,
- public stats and backend authority surfaces.

The shortest accurate description is:

Autopilot Earn today is a strong desktop-heavy prototype with real Nostr and Spark plumbing, but it is still missing the hosted Nexus layer and several product/runtime integrations required to honestly claim the designed MVP is fully implemented.

## Bottom Line

If the question is "do we have the full designed MVP?", the answer is no.

If the question is "are we still at zero?", the answer is also no.

The repo is materially closer than zero because the two hardest low-level pieces for the MVP story, NIP-90 transport and wallet-confirmed payout handling, already exist in usable form.

The largest remaining gap is not protocol syntax. It is system shape:

- the product shell is not the designed shell,
- the hosted authority layer is mostly absent,
- starter-demand is still desktop-local simulation,
- Spacetime is not yet live in the way the design assumes,
- several new spec decisions are not yet reflected in runtime defaults.

## Intended System Split

To keep the audit concrete, this is the intended boundary implied by the current docs.

### What should live on Nostr

Nostr should remain the open marketplace and discovery plane:

- public NIP-90 job requests, feedback, and results,
- NIP-89 handler publication and discovery,
- multi-relay transport and public market reach,
- portable public artifacts and public-facing marketplace state.

Nostr should not be treated as the authority for:

- starter-job eligibility,
- assignment leases,
- settlement truth,
- wallet truth,
- seed-demand budget control,
- public stats authority,
- policy enforcement that depends on privileged OpenAgents knowledge.

### What should live in Nexus and other OpenAgents backend services

For the designed MVP, `Nexus` is the opinionated, open-source, self-hostable server-authoritative stack. It should own:

- OpenAgents-hosted auth and session proof,
- the default primary relay path,
- starter-demand buyer services,
- starter-job assignment and lease authority,
- hosted eligibility checks for seed jobs,
- sync token minting and control-plane endpoints,
- authoritative receipts and backend reconciliation for hosted flows,
- public `/stats`,
- operator controls and subsidy budget enforcement.

For the narrow compute-provider MVP, `TreasuryRouter` is not the first blocker. It belongs to the broader server-authoritative economy design, but it is not required to launch the current compute-provider-only MVP loop.

### What should live in Spacetime

Per `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`, Spacetime should be authoritative only for approved app-db domains:

- provider/device online registration and liveness,
- sync replay checkpoints and stream cursor continuity,
- non-monetary projections,
- derived counters from those domains.

Spacetime should not be the first authority for:

- wallet balances,
- payout truth,
- seed-demand spending,
- starter-job assignment leases,
- policy and risk decisions,
- control-plane auth.

### What should stay on desktop

The desktop should own:

- local keys and signing,
- local execution via Codex and local tools,
- wallet UX and withdrawal UX,
- relay fanout and local event correlation,
- read-only market preview while offline,
- provider runtime presentation,
- deterministic local caches and replay-safe projections.

## What Is Already Built And Reusable

### 1. Real NIP-90 provider transport exists

`apps/autopilot-desktop/src/provider_nip90_lane.rs` is a real relay lane, not a stub.

It already provides:

- multi-relay connection handling through a relay pool,
- ingress polling for live NIP-90 requests,
- correlation of tracked buyer feedback and result events,
- best-effort fanout publishing for signed request, feedback, and result events,
- relay health tracking and diagnostics,
- support for `status_extra`-style feedback metadata.

This is reinforced by passing tests in `provider_nip90_lane` including:

- live relay request ingestion,
- signed feedback publish,
- a desktop earn harness that reaches wallet-confirmed settlement in test form.

### 2. A buyer path already exists

`apps/autopilot-desktop/src/input/actions.rs` and `apps/autopilot-desktop/src/state/operations.rs` already support:

- signed NIP-90 request creation,
- request publication through the provider relay lane,
- feedback and result correlation,
- payment-required handling,
- handoff into Spark payment logic.

So the repo is not only able to consume jobs. It can already originate them in a meaningful way.

### 3. Spark wallet integration is real enough for MVP foundations

`apps/autopilot-desktop/src/spark_wallet.rs` supports:

- refresh,
- invoice generation,
- payment sending,
- recent payment history.

`apps/autopilot-desktop/src/state/wallet_reconciliation.rs` and related app state reconcile payouts against wallet evidence instead of just trusting a UI state transition. That is a strong and correct foundation for the current MVP direction.

### 4. Local receipts and projections are more advanced than the backend story

Two desktop-local systems are already doing a large amount of kernel-like work:

- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`

They already model receipt-like records, incidents, policy metadata, rollback-related records, and economy snapshots. The problem is not lack of domain modeling. The problem is that this logic still lives as desktop-local state instead of an authoritative backend service.

### 5. Spacetime work exists, but only part of it is deployable today

`crates/autopilot-spacetime` includes reducer primitives for:

- sync events,
- checkpoints,
- presence,
- provider capabilities,
- compute assignments,
- bridge outbox.

But the actual module under `spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs` currently exposes only:

- active connections,
- Nostr presence claim binding,
- stream heads,
- sync events,
- stream checkpoints.

That means the crate-level Spacetime design is ahead of the currently deployed module surface.

## What Is Missing Or Still Simulated

## 1. The product shell is still not the designed MVP shell

The design now assumes an earn-first Mission Control experience.

Current implementation evidence:

- `apps/autopilot-desktop/src/pane_registry.rs` starts with `Autopilot Chat` as a startup pane.
- `apps/autopilot-desktop/src/pane_registry.rs` also starts `CadDemo` by default.
- `GoOnline` is not the startup shell.
- earnings, starter jobs, job inbox, wallet, and relay panes remain separated as operational panes.

What is missing:

- a single Mission Control shell centered on earn readiness,
- the current first-run path where the user immediately sees market activity and a clear `Go Online` CTA,
- first-sats progression and celebration inside the main shell,
- a unified main earn/job view where starter jobs appear as normal jobs with visible source markers,
- de-emphasis of non-MVP startup panes for first launch.

This is not cosmetic. The launch product described in `docs/MVP.md` is a different product shape from the current startup layout.

## 2. Offline market preview is designed, but not implemented

The current docs now say the app should show live observed market activity immediately, even while offline.

Current implementation evidence:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs` explicitly stops polling when `wants_online` is false.
- the lane loops past ingress work if the provider is offline.
- current inbox behavior is still driven by online ingestion.

What is missing:

- a read-only market observer mode when offline,
- a clear preview/unclaimable label for offline rows,
- a first-launch activity feed that is alive before the user opts into provider mode.

This is a direct gap against the product decision that the app should not feel empty before `Go Online`.

## 3. Auto-accept is not implemented yet

The docs now say matching jobs should auto-accept by default.

Current implementation evidence:

- `apps/autopilot-desktop/src/input/reducers/jobs.rs` still accepts work through `JobInboxPaneAction::AcceptSelected`.
- the active-job lifecycle is still user-driven from the pane layer.
- I did not find a production ingress path that takes a matching live request directly from inbox to accepted/running based on provider policy.

What is missing:

- provider admission control on ingress,
- automatic accept for matching jobs,
- automatic reject/ignore reasons for jobs that do not fit policy,
- enforcement of the documented `max_inflight = 1` default.

## 4. Job execution is still manual-stage-driven

The designed MVP assumes a real execution loop.

Current implementation evidence:

- `apps/autopilot-desktop/src/input/reducers/jobs.rs` advances the active job through manual `AdvanceStage` actions.
- feedback and result publication are coupled to those stage transitions.
- the lifecycle can progress because the operator clicks through the pane, not because a real provider executor advanced it from actual work completion.

What is missing:

- a provider executor that automatically drives `accepted -> running -> delivered -> paid`,
- clear integration between NIP-90 jobs and the local Codex execution lane,
- timeouts, abort rules, retry rules, and post-execution settlement behavior grounded in actual runtime events,
- honest failure handling for execution errors instead of pane-driven happy-path progression.

This is one of the most important implementation gaps.

## 5. Starter demand is still desktop-local simulation, not hosted Nexus authority

The current starter-demand path is not the designed OpenAgents-hosted Nexus system.

Current implementation evidence:

- `apps/autopilot-desktop/src/state/operations.rs` defines `StarterJobsState` with local templates, local budget, local kill switch, local dispatch interval, and a desktop-local `STARTER_DEMAND_DEFAULT_MAX_INFLIGHT_JOBS`.
- `apps/autopilot-desktop/src/input/actions.rs` runs `run_auto_starter_demand_generator` inside the desktop process.
- `queue_starter_demand_request` inserts starter requests directly into the local `job_inbox` and records local projections/receipts.
- starter jobs are completed through local actions and local wallet-pointer checks.

This is the clearest design-vs-built mismatch in the repo.

What the design now requires instead:

- starter demand originates from the OpenAgents-hosted Nexus only,
- starter jobs are only available when the provider is connected to the OpenAgents-hosted Nexus,
- starter jobs are assigned one provider at a time by a hosted lease authority,
- start-confirm should be aggressive, then execution should get a more forgiving window,
- reassignment should happen on missed confirm, lost heartbeat, or lease expiry,
- the desktop should consume hosted assignment decisions instead of inventing starter jobs locally.

What is missing:

- a hosted starter-demand buyer service,
- a hosted assignment and lease manager,
- a hosted heartbeat or progress-ack path tied to starter-job leases,
- a hosted subsidy budget ledger,
- a hosted operator control surface for starter-demand health.

## 6. Nexus and other backend services are mostly absent from the repo

For the designed MVP, a real backend is required. That backend does not exist in this repo today.

Evidence:

- repo search did not find a Rust HTTP server surface for `control-api`, `kernel-authority`, `stats-api`, or `treasury-router`.
- `apps/autopilot-desktop/src/sync_bootstrap.rs` defines the client contract for `POST /api/sync/token`, but there is no repo-local server implementation.
- `apps/autopilot-desktop/src/render.rs` calls `bootstrap_sync_session_from_env(&client, None)`, which means there is no integrated bearer-auth session being supplied there today.
- there is no repo-local Nostr relay server implementation even though the design now expects the OpenAgents-hosted Nexus to be the default primary relay path.

What is missing for the designed MVP:

- a Rust `control-api` for hosted sessions and auth-bound desktop identity,
- a Rust `nexus-relay` or equivalent relay service for the hosted primary relay path,
- a Rust starter-demand service,
- a Rust lease/assignment service,
- a Rust sync token mint service at `/api/sync/token`,
- a Rust stats service for public `/stats`,
- a canonical backend receipt store and reconciliation layer for hosted operations.

This is the key reason the current system still behaves like a desktop-centered harness instead of the designed launch product.

## 7. All remaining backend work can be Rust, but not all of it should be Spacetime

Nothing in the current MVP design forces a non-Rust backend.

The missing hosted pieces can all be implemented in Rust:

- hosted auth/session service,
- control and sync-token endpoints,
- Nexus relay service,
- starter-demand service,
- lease and assignment service,
- receipt ingestion and `/stats` service.

The important constraint is not language. It is authority shape.

Per the current ADR and the product decisions already documented, Spacetime is a good fit for:

- presence,
- checkpoints,
- replay continuity,
- selected non-monetary projections.

It is not the right first authority for:

- hosted starter-job spending,
- starter-job assignment leases,
- payout truth,
- wallet truth,
- control-plane auth,
- public stats truth.

So yes, the entire remaining backend can be Rust. No, the entire remaining backend should not be implemented as Spacetime reducers.

## 8. Current relay configuration does not match the designed relay model

The product decision is now:

- OpenAgents-hosted Nexus is the default primary relay path,
- Autopilot should ship with a curated default public relay set,
- normal NIP-90 intake should span the configured reachable relay set,
- provider publication should fan out across healthy configured relays.

Current implementation evidence:

- `apps/autopilot-desktop/src/app_state.rs` still defaults `relay_url` to `wss://relay.damus.io`.
- `SettingsDocumentV1` remains single-relay-centric.
- `configured_provider_relay_urls()` can merge in-memory relay rows, but the persistent settings model is not yet shaped like a first-class curated relay set plus Nexus default.

What is missing:

- a real default relay bundle shaped around OpenAgents Nexus plus curated public backups,
- durable multi-relay configuration as a first-class settings model,
- relay health UX that is aligned to primary-plus-backups rather than single-default-relay thinking.

## 9. Current runtime defaults drift from the documented MVP

Two defaults are now clearly out of sync with the spec.

Current code:

- `apps/autopilot-desktop/src/app_state.rs` defaults `provider_max_queue_depth` to `4`.
- `apps/autopilot-desktop/src/state/operations.rs` defaults local starter demand `max_inflight_jobs` to `3`.

Current docs:

- MVP default `max_inflight` should be `1` until we know the desktop/Codex path can safely handle more than one concurrent job.

What is missing:

- runtime default alignment,
- settings UI and admission logic that actually enforce the intended MVP concurrency limit.

## 10. NIP-89 and NIP-42 are mostly library-level, not product-level

The current design assumes the hosted Nexus is the primary relay path and that the provider can be discovered correctly on the open network.

What exists today:

- `crates/nostr/core/src/nip89.rs` and the NIP-90 module document the right discovery model.
- NIP-89 helper usage appears in simulation code under `apps/autopilot-desktop/src/app_state_domains.rs`.
- relay auth parsing exists in library code and the broader codebase knows about NIP-42 concepts.

What does not exist as a product path:

- live handler publication from the desktop on `Go Online`,
- real provider capability advertisement bound to current desktop/provider state,
- real relay-auth flow integrated into the desktop for the hosted Nexus relay path,
- hosted enforcement logic that uses authenticated Autopilot session plus bound Nostr identity for starter-job eligibility.

For MVP, the docs now correctly avoid trusting a `client` tag as the proof basis. That is good. But the actual hosted proof path still needs to be built.

## 11. Spacetime is not live in the way the design assumes

The current docs and ADR now allow Spacetime authority for selected domains. The codebase is not fully there yet.

Current implementation evidence:

- `docs/SPACETIME_ROLLOUT_INDEX.md` still labels the current state as Phase 1 mirror/proxy discipline.
- `apps/autopilot-desktop/src/spacetime_presence.rs` uses a local in-memory `ProviderPresenceRegistry`, not a remote Spacetime subscription.
- `apps/autopilot-desktop/src/render.rs` bootstraps sync from env, but it does not show a full hosted auth path and still passes `None` for bearer auth.
- the deployable module only exposes sync and connection tables, not the broader provider capability and compute-assignment surfaces that exist in the crate primitives.

What is missing:

- a real deployed Spacetime service for presence and sync,
- desktop subscription and recovery behavior wired to the live service rather than local stand-ins,
- production use of Spacetime for the approved domains in `ADR-0001`,
- optional projection mirroring for selected fleet state.

What does not need to happen first:

- moving money authority or starter lease authority into Spacetime.

The current docs are right to keep that boundary strict.

## 12. SA, SKL, and AC runtime lanes are still local simulators

Current implementation evidence:

- `apps/autopilot-desktop/src/runtime_lanes.rs` creates synthetic event ids and local timing delays.
- the lanes produce local accepted/rejected command responses and local event ids like `sa:<kind>:<seq>`.

That means:

- the desktop has a useful simulation/harness layer for agent lifecycle, skill discovery/trust, and credit flows,
- but the live product equivalents are not present.

For the designed MVP, this matters most in two places:

- starter demand currently uses local AC-path concepts rather than hosted Nexus authority,
- provider presence/capability still depends on local simulated lanes rather than live backend-backed publication and policy.

## 13. Buyer-resolution policy is still partly docs-only

The docs now distinguish:

- hosted Nexus starter-job single-assignee leases,
- public `race` jobs for MVP,
- later `windowed` jobs,
- explicit loser feedback when possible.

Current implementation evidence:

- the NIP-90 lane understands buyer feedback and `status_extra`.
- I did not find product code implementing `windowed` buyer mode.
- I did not find explicit emission of `lost-race` or `late-result-unpaid` loser feedback in the product path.

What is missing:

- explicit buyer-resolution mode in hosted job posting,
- explicit loser feedback emission for `race` jobs,
- later `windowed` support,
- operator telemetry that makes these outcomes visible when relay correlation is incomplete.

## 14. Public `/stats` and backend authority extraction do not exist yet

The earlier kernel audit already concluded this, and the desktop review supports it.

What exists today:

- local earnings scoreboard and local economy snapshot logic,
- local receipt and incident state,
- local activity feed and job history.

What does not exist:

- a public `/stats` service,
- once-per-minute backend snapshot generation,
- canonical receipt ingestion from multiple clients into one authority store,
- backend audit package export built from authoritative data.

This is a major gap if the hosted Nexus is meant to be a real public authority surface rather than just a thin sync convenience.

## 15. The hosted proof path for OpenAgents starter jobs is not implemented end to end

The current product rule is:

- OpenAgents starter jobs are only for providers connected to the OpenAgents-hosted Nexus,
- the proof basis for MVP is an authenticated Autopilot session on the hosted Nexus plus bound Nostr identity,
- stronger anti-spoofing is later hardening.

Current code does not implement that full path.

Evidence:

- the sync bootstrap contract exists, but desktop bootstrap currently passes no bearer auth from `render.rs`.
- the Spacetime module can bind a Nostr presence identity to an active connection, but that is not yet surfaced as the full product eligibility path for starter jobs.
- starter-job creation and completion are still entirely local.

What is missing:

- an OpenAgents-hosted desktop session system,
- bearer-authenticated control-plane calls from the desktop,
- hosted binding between the desktop session and the Nostr identity used by the provider,
- hosted use of that proof when determining starter-job eligibility.

## Minimum Work Required To Honestly Claim The Designed MVP Is Implemented

The following is the minimum set of deliverables needed to say the current designed MVP is real.

### 1. Replace the startup shell with Mission Control

Must include:

- earn-first launch surface,
- immediate market activity preview while offline,
- `Go Online` as a central stateful control,
- wallet balance and first-sats progression in the main shell,
- normal job list with starter source markers,
- non-MVP startup panes removed from first-run prominence.

### 2. Implement offline market preview

Must include:

- read-only multi-relay observation when offline,
- preview/unclaimable labeling,
- a distinction between seeing jobs and being eligible to take jobs.

### 3. Replace local starter-demand simulation with hosted Nexus starter-demand

Must include:

- hosted seed-demand service,
- hosted provider eligibility checks,
- single-assignee starter-job leases,
- aggressive start-confirm plus longer execution window,
- reassignment on missed confirm or lost heartbeat,
- hosted subsidy budgeting and operator control.

### 4. Build the real OpenAgents-hosted Nexus backend surfaces

At minimum:

- hosted auth/session service,
- `POST /api/sync/token`,
- hosted primary relay service for Nexus,
- starter-demand service,
- assignment/lease authority,
- public `/stats`,
- backend receipt and reconciliation storage.

### 5. Implement real provider automation

Must include:

- auto-accept based on policy,
- automated job execution,
- result publication driven by execution outcomes, not manual stage clicks,
- default `max_inflight = 1` enforcement,
- truthful failure and timeout handling.

### 6. Bring live Spacetime online for the approved domains

Must include:

- deployed module and runtime configuration,
- desktop remote subscription and reconnect behavior,
- real presence/checkpoint authority,
- projection usage that follows the ADR boundary.

It does not require moving money authority into Spacetime.

### 7. Finish the Nostr product path

Must include:

- default Nexus primary relay plus curated public relay set,
- NIP-89 capability publication from the provider path,
- optional NIP-42 relay auth where the hosted relay wants it,
- full-market relay fanout and deduping aligned with the current docs.

### 8. Extract backend-worthy receipt and snapshot logic from desktop-local state

Must include:

- a shared Rust domain crate or equivalent extraction from `earn_kernel_receipts` and `economy_snapshot`,
- authoritative backend receipt persistence,
- backend snapshot production for `/stats` and audits,
- consistent semantics between desktop-local projection and backend truth.

## Recommended Implementation Order

This is the shortest path that respects the current product decisions without overbuilding the broader kernel plan too early.

### Phase 1: Align the desktop to the designed product shell

Build first:

- Mission Control shell,
- offline market preview,
- unified job list with starter markers,
- startup pane cleanup,
- default `max_inflight = 1` alignment,
- relay settings reshaped around Nexus plus curated public relays.

Why first:

- it makes the product direction visible immediately,
- it forces the UI and state model to stop assuming the old pane-centric prototype shape.

### Phase 2: Build the minimum hosted Nexus MVP backend in Rust

Build next:

- hosted auth/session,
- `/api/sync/token`,
- Nexus relay service,
- starter-demand service,
- lease/assignment service,
- receipt/reconciliation storage,
- public `/stats` skeleton.

Why second:

- the designed MVP cannot be honest without a real backend authority for hosted starter jobs and hosted session proof.

### Phase 3: Rewire desktop runtime from local starter simulation to hosted authority

Build next:

- hosted provider eligibility check,
- lease ack / heartbeat / reassignment behavior,
- desktop auto-accept from hosted assignments,
- automated execution path,
- hosted payout bookkeeping and loser feedback where relevant.

Why third:

- this is the step that turns the desktop from local simulation into a real provider client for the hosted OpenAgents Nexus.

### Phase 4: Bring live Spacetime online for presence, sync, and projections

Build next:

- deploy the current sync module,
- wire remote presence and checkpoints into the desktop,
- add additional reducer/module coverage only where it fits the ADR matrix,
- optionally mirror selected assignment or fleet telemetry into Spacetime after backend authority decisions have already been made elsewhere.

Why fourth:

- it improves fleet visibility and continuity without confusing authority ownership.

### Phase 5: Finish marketplace hardening

Build after launch-critical work:

- NIP-89 publication and richer discovery UX,
- explicit `lost-race` loser feedback,
- later `windowed` buyer mode,
- stronger anti-spoofing and device-bound proof,
- self-hosted Nexus onboarding polish,
- later private/closed Nexus mode.

## Conclusion

The current repo already proves several important claims:

- Autopilot can speak real NIP-90.
- Autopilot can reconcile real wallet evidence.
- Autopilot can maintain deterministic local job and receipt state.
- Spacetime is a real candidate for presence and sync, not just a concept.

What it does not yet prove is the full launch product that the docs now describe.

Today, the biggest missing pieces are not low-level Rust primitives. They are the hosted Nexus layer and the desktop integrations that depend on it.

Until those pieces exist, the honest description is:

OpenAgents has a serious desktop prototype with real market and wallet plumbing, but not yet the full earn-first, hosted-Nexus-backed MVP product we have now designed.
