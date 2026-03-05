# 2026-03-05 Economy Kernel Plan vs Built System Audit

Author: Codex
Status: complete
Scope: code-driven audit of the current `openagents` workspace plus targeted review of `/Users/christopherdavid/code/SpacetimeDB`

## Objective

Compare the three kernel-plan docs:

- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`
- `docs/plans/diagram.md`

against what is actually built in this repo today, with explicit answers to:

1. Do we currently have any real server-side kernel system?
2. Do `TreasuryRouter`, `Kernel Authority API`, or related systems need backend deployment?
3. Which planned pieces are actually local desktop prototypes today?
4. Could the whole thing be implemented in SpacetimeDB instead of separate services?

This audit is aligned to the current product authority in `docs/MVP.md`, not only to the broader kernel-plan docs. A gap against the kernel plans is not automatically an MVP bug.

## Sources Reviewed

Product and ownership authority:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`
- `docs/SPACETIME_ROLLOUT_INDEX.md`

Kernel-plan targets:

- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`
- `docs/plans/diagram.md`

Current repo implementation:

- `Cargo.toml`
- `apps/autopilot-desktop/src/economy_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`
- `apps/autopilot-desktop/src/sync_bootstrap.rs`
- `apps/autopilot-desktop/src/sync_apply.rs`
- `apps/autopilot-desktop/src/spacetime_presence.rs`
- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/runtime_lanes.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `crates/autopilot-spacetime/src/client.rs`
- `crates/autopilot-spacetime/src/reducers.rs`
- `crates/autopilot-spacetime/src/schema.rs`
- `crates/autopilot-spacetime/src/auth.rs`
- `spacetime/modules/autopilot-sync/README.md`
- `spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs`

SpacetimeDB feasibility review:

- `/Users/christopherdavid/code/SpacetimeDB/README.md`
- `/Users/christopherdavid/code/SpacetimeDB/crates/bindings/src/lib.rs`
- `/Users/christopherdavid/code/SpacetimeDB/crates/bindings/src/http.rs`

Broader Spacetime ecosystem review:

- `/Users/christopherdavid/code/spacetime/README.md`
- `/Users/christopherdavid/code/spacetime/BitCraftPublic/README.md`
- `/Users/christopherdavid/code/spacetime/spacetimedb-minecraft/README.md`
- `/Users/christopherdavid/code/spacetime/spacetimedb-cookbook/web-request-example/README.md`
- `/Users/christopherdavid/code/spacetime/spacetime-docs/docs/deploying/spacetimedb-standalone.md`
- `/Users/christopherdavid/code/spacetime/SpacetimeDB/docs/docs/00300-resources/00100-how-to/00100-deploy/00200-self-hosting.md`
- `/Users/christopherdavid/code/spacetime/SpacetimeDB/docs/docs/00200-core-concepts/00500-authentication.md`
- `/Users/christopherdavid/code/spacetime/spacetime-docs/docs/http/authorization.md`

## Executive Verdict

1. We do not currently have a repo-local backend implementation of the kernel-plan systems. There is no `TreasuryRouter`, no `Kernel Authority API`, no kernel HTTP service, no proto package tree, and no public `/stats` service in this workspace.
2. What we do have is substantial desktop-local kernel modeling inside `apps/autopilot-desktop`: receipts, policy bundle parsing, incident tracking, outcome registry, audit export, safety feed export, rollback tracking, and minute-level economy snapshots.
3. We also have real Spacetime work, but it is narrowly scoped. The active remote artifact is the `autopilot-sync` Spacetime module for presence/checkpoints/sync events. The desktop still mostly operates in Phase 1 mirror/proxy semantics with local stand-ins.
4. If you want the system described in the kernel-plan docs as written, then yes: `TreasuryRouter` and `Kernel Authority API` need backend deployment. The plan docs explicitly place them server-side and outside Nostr/Spacetime.
5. After reviewing the broader Spacetime ecosystem, the stronger conclusion is: SpacetimeDB can clearly host much larger server-side state machines than this repo currently uses it for, but it is still not the recommended place to put the entire OpenAgents economy backend. It remains a strong fit for presence, checkpoints, projections, and selected pure coordination state. It remains a weak fit for money-moving settlement, external routing, underwriting, verification pipelines, and OpenAgents-specific authority and audit surfaces.

## MVP Alignment

Against `docs/MVP.md`, the current system is much closer to correct than the kernel plans alone would suggest:

- MVP ships desktop-first compute-provider flow.
- Wallet and settlement truth remain local command-authoritative.
- Spacetime is only expected to own ADR-approved domains such as presence, checkpoints, and projection/sync continuity.

So the missing backend kernel services are a gap against the broad kernel-plan docs, but not necessarily a blocker for the current MVP.

## Current Built Topology

## What is real today

### 1) Desktop-local economy kernel primitives are real

`apps/autopilot-desktop` contains a large amount of local kernel-shaped logic:

- `economy_kernel_receipts.rs` defines receipt primitives and policy/trace context.
- `state/earn_kernel_receipts.rs` persists a local receipt stream, tracks work units and incidents, records settlement and rollback actions, and exports audit/safety bundles.
- `state/economy_snapshot.rs` computes and persists minute snapshots from receipts.

This is not fake scaffolding. It is real implementation, but its authority boundary is local desktop state, usually persisted under `~/.openagents/...json`, not a shared backend service.

### 2) Wallet and NIP-90 earning flows are real

The current desktop app has real buy/sell path pieces:

- NIP-90 provider/buyer lane logic in `provider_nip90_lane.rs`.
- Auto-payment handling for `payment-required` + `bolt11` in `state/operations.rs` and input reducers.
- Spark wallet command execution in `spark_wallet.rs` and reusable wallet primitives in `crates/spark`.

This means the repo already has a real local command lane for request publication, invoice handling, and wallet-confirmed money movement.

### 3) Spacetime support is real, but narrow

There is active Spacetime implementation in this repo:

- `crates/autopilot-spacetime` contains schema metadata, auth checks, in-memory reducers, and an HTTP reducer client.
- `spacetime/modules/autopilot-sync` contains a real Spacetime module with `append_sync_event`, `ack_stream_checkpoint`, presence challenge binding, and connection lifecycle reducers.
- `sync_bootstrap.rs` enforces canonical `POST /api/sync/token` and retained subscribe target semantics.

But this is still not a full live remote sync deployment in the desktop app. The desktop currently uses:

- local presence runtime in `spacetime_presence.rs`,
- local checkpoint persistence in `sync_apply.rs`,
- bootstrap/session preparation in `sync_bootstrap.rs`.

Code search found no current desktop call sites that drive `SpacetimeReducerHttpClient` subscription or reducer append/ack flows end to end.

## What is not present today

### 1) No backend kernel service exists in this workspace

`Cargo.toml` includes:

- `apps/autopilot-desktop`
- reusable crates
- `crates/autopilot-spacetime`

It does not include any backend app or kernel service crate. There is also no `proto/` tree in the repo.

### 2) No repo-local `TreasuryRouter`

Searches for `TreasuryRouter` only hit docs. There is no implementation of a policy planner/router service in code.

### 3) No repo-local `Kernel Authority API`

Searches for `Kernel Authority API` only hit docs. There is no HTTP service handling authoritative mutations and emitting canonical receipts.

### 4) No public `/stats` service

The repo contains local snapshot computation, but not a public or operator-facing HTTP endpoint serving snapshot-derived metrics.

### 5) No proto-first wire contract implementation

`docs/plans/economy-kernel-proto.md` specifies `proto/openagents/**/v1/*`. The repo currently has no `proto/` directory and no generated kernel proto crate.

## Plan vs Built Matrix

| Planned system | Current state in repo | Deployment needed? | Spacetime fit | Notes |
| --- | --- | --- | --- | --- |
| `TreasuryRouter` | Not built | Yes, if plan is adopted | No for full role | Docs define it as policy planner behind authenticated HTTP; codebase has nothing matching it. |
| `Kernel Authority API` | Not built | Yes, if plan is adopted | Partial at best | Could move some pure state transitions into Spacetime, but the plan defines HTTP authority and receipt issuance as backend services. |
| Canonical receipt stream service | Partially prototyped locally | Yes for shared authority | Partial | Desktop has local receipt persistence and export, but no shared service/store. |
| Economy snapshot service and public `/stats` | Partially prototyped locally | Yes for plan-level `/stats` | Partial | Minute snapshot computation exists locally; no remote publication path exists. |
| Settlement engine | Local desktop command lane only | Maybe not for MVP, yes for kernel-plan backend | Poor | Wallet send/receive is local command-authoritative; external settlement is not a good reducer fit. |
| Verification engine and verdict authority | Not built as service | Yes for kernel plan | Poor to partial | Long-running and externally integrated verification is a weak Spacetime fit. |
| Liability, claims, coverage, warranty flows | Mostly not built | Yes | Poor | Local receipt code models some incidents and liabilities, but there is no backend market/underwriter/claims system. |
| Credit envelopes / treasury-like authority | Local simulation/projection only | Yes for real authority | Partial | `runtime_lanes.rs` AC lane generates local event IDs and mutates in-process snapshot state. |
| Proto contracts under `proto/openagents/**` | Not built | Yes | N/A | Entire proto layer from the plan is missing. |
| Sync bootstrap/token contract | Partially built in desktop | Yes outside desktop | N/A | Desktop expects a control endpoint for `POST /api/sync/token`, but no such app exists in this workspace. |
| Spacetime presence/checkpoints/sync events | Built and deployable | Yes, as Spacetime module | Strong | This is the one clearly deployable server-side artifact already present. |
| Provider capability / compute assignment / outbox Spacetime schema | Partially designed | Maybe | Moderate | `crates/autopilot-spacetime/src/schema.rs` defines broader tables than the current published module exposes. |

## Detailed Findings

### 1) The repo has no general backend service layer today

The strongest simple signal is the workspace shape:

- one app: `apps/autopilot-desktop`
- several reusable crates
- one Spacetime crate and one Spacetime module directory

There is no backend app for:

- treasury routing,
- kernel authority,
- stats serving,
- verification,
- claims/liability,
- sync token issuance.

`docs/MVP.md` still references `apps/openagents.com` as the retained home for auth/session flows and `POST /api/sync/token`, but that app is not present in this pruned repo.

### 2) Many kernel-plan concepts are already prototyped locally in the desktop

The repo is not starting from zero. The desktop already contains local implementations of:

- append-only receipt persistence,
- idempotency tracking,
- policy bundle evaluation inputs,
- rollback/correction receipts,
- incident tracking and resolution,
- outcome registry,
- safety feed export,
- audit package export,
- minute-level economy snapshots.

That reduces future extraction cost. But it also means the current "kernel" is embedded inside the desktop process and local storage, not deployable shared infrastructure.

### 3) Spacetime is real for sync, but the desktop is still mostly Phase 1

`docs/SPACETIME_ROLLOUT_INDEX.md` describes the current phase correctly:

- Phase 1 current: local apply/checkpoints, local Spacetime-shaped presence/projection semantics.
- Phase 2 target: live remote Spacetime authority for ADR-approved domains.

That matches the code:

- `sync_bootstrap.rs` prepares real token/target contracts.
- `spacetime_presence.rs` is still an in-process `ProviderPresenceRegistry`.
- `sync_apply.rs` still persists local checkpoint files.

This means the Spacetime track is not vapor, but it is not yet the live remote authority path the kernel diagrams would imply for broader shared state.

### 4) The only clearly deployable server-side artifact already built is the Spacetime sync module

`spacetime/modules/autopilot-sync` is real deployment material:

- schema/tables,
- reducers,
- publish scripts,
- contract verification scripts,
- handshake smoke scripts.

If you ask "what do we actually have to deploy today?" the answer is:

- the `autopilot-sync` Spacetime module,
- plus some external control service for sync token minting that this repo expects but does not contain.

That is far short of the full kernel-plan service stack.

### 5) Credit/treasury-like flows are currently local simulation, not authority

`apps/autopilot-desktop/src/runtime_lanes.rs` implements AC lane commands such as:

- `PublishCreditIntent`
- `PublishCreditOffer`
- `PublishCreditEnvelope`
- `PublishCreditSpendAuth`
- `PublishCreditSettlement`
- `PublishCreditDefault`

But the implementation is local snapshot mutation with generated event IDs such as `ac:<kind>:<seq>`. There is no backend envelope authority, no shared spend budget service, and no settlement reconciliation beyond local runtime state.

This is useful UI/runtime scaffolding, not a deployed treasury system.

### 6) The repo has no plan-defined proto layer

The kernel-proto doc is implementation-facing, but the codebase does not reflect that plan:

- no `proto/openagents/**`,
- no generated kernel crate,
- no HTTP/gRPC layer tied to those messages.

So the plan exists as architecture text, not as implemented wire contract.

### 7) Spacetime schema intent is broader than the currently published module

There is an internal mismatch worth noting:

- `crates/autopilot-spacetime/src/schema.rs` describes a broader sync schema including `provider_capability`, `compute_assignment`, `bridge_outbox`, `coordination_event`, and `conflict_event`.
- `spacetime/modules/autopilot-sync/README.md` describes a narrower actual module: `active_connection`, `nostr_presence_claim`, `stream_head`, `sync_event`, `stream_checkpoint`.

That means even inside the Spacetime track, part of the broader design remains unshipped.

## Do TreasuryRouter and Related Systems Need Backend Deployment?

For the kernel-plan docs as written: yes.

Why:

1. `docs/plans/economy-kernel.md` explicitly says authority mutations must occur over authenticated HTTP to `TreasuryRouter` or `Kernel Authority API`.
2. `docs/plans/diagram.md` explicitly says `TreasuryRouter` and `Kernel Authority API` are server-side services, not on the user machine and not on Nostr/Spacetime.
3. The current repo has no such services, which means those plan components are not deployed and not close to deployable from this workspace alone.

For the current MVP: no, not yet.

Why:

1. `docs/MVP.md` keeps money and policy command-authoritative and desktop-first.
2. ADR-0001 explicitly restricts Spacetime authority to presence/checkpoints/projections/counters.
3. The current earning loop can function with local desktop authority plus Nostr plus wallet plus optional Spacetime sync domains.

So the right answer is conditional:

- required for the broad kernel-plan architecture,
- not required for the current MVP acceptance loop.

## Could We Do All of It With SpacetimeDB?

Short answer: not as the recommended design, even though it is more technically plausible than a narrow reading of this repo alone would suggest.

## What changed after reviewing the wider Spacetime ecosystem

The additional Clockwork Labs repos change the interpretation in two useful ways:

1. `BitCraftPublic` is a real example of a large, complex server-side application structured as SpacetimeDB modules with substantial state and coordination logic. That means "large backend in Spacetime" is not theoretical.
2. `spacetimedb-minecraft` and the cookbook `web-request-example` both reinforce the opposite boundary: when you need protocol bridging or external side effects, Spacetime-based systems still commonly rely on external proxy servers or special clients/workers.

So the revised view is:

- more of a pure stateful OpenAgents backend could technically live inside Spacetime than this audit first emphasized,
- but the specific OpenAgents domains that matter most economically still push the design toward a hybrid backend.

## What SpacetimeDB is good at here

SpacetimeDB is a good fit for:

- provider/device presence,
- sync streams and replay checkpoints,
- append-only event projections,
- shared counters and queryable non-monetary state,
- some deterministic pure state machines that do not require external side effects.

That matches both:

- the repo ADRs, and
- the actual `autopilot-sync` module already built.

It also matches broader Spacetime usage patterns:

- `BitCraftPublic` shows large-scale game/server state can live in Spacetime tables and reducers,
- `spacetimedb-minecraft` shows external protocol translation can sit outside the module,
- `web-request-example` shows effectful integrations are often better modeled as queue tables plus external privileged workers.

## Why it is not a good fit for the whole kernel

The SpacetimeDB review matters here:

1. Reducers are transactional but have no direct network/filesystem access.
2. Reducers can only communicate outward through table writes and logs.
3. Procedures can perform side-effecting operations, but they are not automatically transactional.
4. Procedure HTTP is constrained and documented with a maximum timeout of 500 ms.
5. Scheduled reducers/procedures are best-effort, not a strong fit for critical external money orchestration.

Those constraints make Spacetime a poor primary home for:

- wallet settlement execution,
- FX RFQs and multi-hop liquidity routing,
- third-party underwriter/coverage calls,
- verification pipelines that may take materially longer than 500 ms,
- authenticated public/private HTTP authority surfaces defined in the kernel plan.

You could force more of the kernel into Spacetime procedures or special-client patterns, and the wider ecosystem shows that this can work for some classes of application logic. But for OpenAgents you would still be fighting:

- the actual platform constraints,
- the repo's current ADR boundaries,
- and the kernel docs themselves, which place these roles in server-side HTTP services.

The special-client pattern is especially telling. It is useful for queue-driven external effects, but once you are relying on privileged side workers to do the hard parts, you are effectively back in a hybrid architecture. At that point, canonical money and audit authority should stay in conventional backend services.

## Recommended deployment split if the kernel plan becomes active work

Use Spacetime for:

- presence authority,
- replay checkpoints,
- append-only projection streams,
- shared counters that are explicitly non-monetary.

Use conventional backend services for:

- `TreasuryRouter`,
- `Kernel Authority API`,
- receipt ingestion and canonical receipt storage if shared across clients,
- minute snapshot publication and `/stats`,
- verification/liability/coverage integrations,
- sync token minting and any authenticated control-plane endpoints.

Keep desktop-local authority for MVP-critical local execution and wallet UX until a real backend replacement is intentionally designed and shipped.

## What Is Still Missing If You Wanted the Plan Docs for Real

P0 missing systems:

1. `TreasuryRouter` service.
2. `Kernel Authority API` service.
3. Shared receipt stream store and authority mutation endpoint.
4. Public/restricted economy snapshot publication path and `/stats`.
5. Sync token issuing control service inside the retained control plane.
6. Real live desktop subscription/append/ack integration against deployed Spacetime.
7. Proto tree and generated kernel wire contracts.

P1 missing systems:

1. Verification service with explicit verdict authority and evidence ingestion.
2. Liability/claims/coverage services.
3. Treasury/credit envelope authority beyond local runtime simulation.
4. Wider Spacetime module coverage for the broader sync schema already modeled in `crates/autopilot-spacetime`.

P2 or future-only systems:

1. FX/RFQ routing and solver market services.
2. Certification/underwriter/coverage market integrations.
3. Optional liability market and warranty machinery from the broader kernel plans.

## Verification Notes

Commands run during this audit:

- `cargo test -p autopilot-spacetime`
- targeted code search across `apps/`, `crates/`, `docs/`, and `spacetime/`

Observed result:

- `autopilot-spacetime` tests passed.
- Targeted `autopilot-desktop` sync tests were not runnable in the current working tree because the branch currently has unrelated compile errors in `state/earn_kernel_receipts.rs` and `state/economy_snapshot.rs`.

That does not change the deployment conclusion, but it does mean the most advanced local kernel-modeling code could not be revalidated end to end from the current worktree state.

## Bottom Line

The repo today is:

- strong on desktop-local kernel modeling,
- real on wallet/NIP-90 earning flow,
- real but still partial on Spacetime sync/presence infrastructure,
- missing the backend service layer described by the kernel-plan docs.

If you stay within the current MVP, that is acceptable.

If you want to realize the kernel plans as written, then the missing work is not just "wire up Spacetime." It is the addition of real backend services, with Spacetime kept as a focused app-db/sync substrate rather than the whole economy kernel.

## Addendum: Recommended Rust Backend on Google Cloud

This addendum assumes:

- backend business logic is written in Rust,
- infrastructure is hosted on Google Cloud,
- the goal is to move from today's desktop-local kernel modeling toward a real shared backend without throwing away the current MVP.

## Recommended architectural stance

Use three different runtime classes, not one:

1. Stateless Rust HTTP services on Cloud Run.
2. Self-hosted SpacetimeDB on GCP for the sync/presence domains that need it.
3. Managed data stores for canonical persistence and caching.

The main mistake to avoid is trying to force all of these concerns into one platform:

- Cloud Run is good for stateless Rust APIs.
- SpacetimeDB is good for live sync and selected shared state domains.
- PostgreSQL is still the right canonical store for economic authority and audit history.

## Recommended service split

### 1) `control-api` Rust service

Role:

- issue `POST /api/sync/token`,
- own desktop auth/session integration,
- mint or broker OpenAgents-scoped sync leases for Spacetime,
- expose minimal control-plane endpoints the desktop needs.

Deployment:

- Cloud Run
- private service-to-service auth with IAM for internal callers
- external ingress only through Google Cloud load balancing

Should use Spacetime:

- no

Why:

- this is OpenAgents token/control-plane logic, not a shared projection domain.

### 2) `kernel-authority` Rust service

Role:

- accept authoritative mutations,
- persist canonical receipts,
- enforce idempotency,
- write work/settlement/incident/outcome state,
- own append-only economic truth.

Deployment:

- Cloud Run for initial rollout
- move to GKE only if sustained concurrency, custom networking, or worker colocation requires it

Should use Spacetime:

- no for authority storage
- yes only for publishing derived projections after commit

Why:

- this is the system of record and should sit on canonical transactional storage, not on reducers/procedures.

### 3) `stats-projector` Rust worker

Role:

- consume committed receipts,
- compute minute snapshots,
- materialize `/stats` tables and public-safe aggregates,
- optionally mirror selected counters into Spacetime projections.

Deployment:

- Cloud Run job for backfills and compaction
- Cloud Run service or long-lived worker for continuous projection

Should use Spacetime:

- not as authority
- yes as optional projection target for live non-monetary counters

### 4) `treasury-router` Rust service

Role:

- policy planning,
- route selection,
- approval/budget decisions,
- choose which authority operation to call.

Deployment:

- Cloud Run

Should use Spacetime:

- no

Why:

- planner/policy services should be stateless and call the authority layer over authenticated HTTP.

Important sequencing note:

Do not build `treasury-router` first. Build it only after `kernel-authority` exists. Right now the repo needs a real authority service more than it needs a separate planner.

### 5) `verification-worker` and `integration-worker` Rust services

Role:

- long-running verification,
- third-party calls,
- webhook/outbox delivery,
- liability/coverage integrations,
- async retries and compensating actions.

Deployment:

- Cloud Run if request/worker lifetimes are short and idempotent
- GKE if workloads become long-lived, connection-heavy, GPU-backed, or operationally noisy

Should use Spacetime:

- no

Why:

- these are external side-effect and integration workloads, which are a weak fit for reducers and only a mediocre fit for procedures.

This recommendation is reinforced by the cookbook `web-request-example`, which handles external requests through a privileged external client listening to module state rather than trying to keep the whole integration inside reducers.

## Recommended data and infrastructure stack

### Canonical transactional store

Start with:

- Cloud SQL for PostgreSQL, regional HA, private IP

Upgrade to:

- AlloyDB only if receipt volume, read-scaling pressure, or recovery/replica demands outgrow Cloud SQL

Why this split:

- Cloud SQL is the simpler starting point for a new Rust backend with moderate operational load.
- AlloyDB is the better fit once the economic ledger and snapshot workload become more demanding.

Store here:

- receipts,
- work units,
- contracts,
- intents,
- settlement state,
- incidents,
- outcomes,
- snapshot rows,
- idempotency records,
- audit export manifests.

### Cache and hot coordination

Use:

- Memorystore for Redis

Store here:

- short TTL idempotency hot keys,
- rate-limit counters,
- replay locks,
- projector cursors that do not need to be canonical,
- ephemeral throttling state.

Do not treat Redis as source of truth.

### Blob and evidence storage

Use:

- Cloud Storage

Store here:

- large audit bundles,
- exported evidence packages,
- redacted public packages,
- non-relational receipt attachments,
- verification artifacts too large for PostgreSQL rows.

### Async transport

Use:

- Pub/Sub for durable async fan-out between services

Use it for:

- receipt-committed events,
- projection triggers,
- verification tasks,
- retryable integration work.

Do not use Pub/Sub as the canonical ledger. It is transport, not truth.

### Container and secrets baseline

Use:

- Artifact Registry for images
- Secret Manager for credentials and signing material that cannot live only in GCP workload identity
- Cloud Logging, Cloud Monitoring, and trace export from all Rust services

## Where Spacetime should be used

Use Spacetime for domains that match ADR-0001 and the current rollout direction:

1. `session_presence` and provider/device liveness.
2. replay checkpoints and cursor continuity.
3. append-only sync/projection streams for activity and job-state views.
4. selected non-monetary counters that benefit from live subscriptions.
5. eventually, `provider_capability` and projection-grade `compute_assignment` views if you want live fleet visibility.
6. possibly additional pure coordination state machines if they remain non-monetary, replay-friendly, and free of slow external side effects.

These are all domains where:

- direct subscriptions matter,
- eventual projection is acceptable,
- money/policy authority must not drift.

## Where Spacetime should not be used

Do not use Spacetime as primary authority for:

1. wallet balances, sends, receives, and settlement truth,
2. canonical receipts and audit ledger,
3. `kernel-authority`,
4. `treasury-router`,
5. `/stats` authority,
6. policy bundle evaluation that must be bound to canonical receipt history,
7. long-running verification or external execution,
8. liability/claims/coverage orchestration,
9. sync token issuance and control-plane auth.

Spacetime may mirror some of this state after commit, but it should not own it.

## Spacetime auth and edge posture

The wider Spacetime docs also change one implementation detail in the recommendation:

- Spacetime already supports OIDC-compatible JWTs directly.
- Service-to-service access to Spacetime can therefore use normal OIDC service credentials.
- You do not need a bespoke OpenAgents auth system for every Spacetime-only surface.

What you still need `control-api` for is:

- OpenAgents-specific session/control policy,
- the repo's explicit `POST /api/sync/token` contract,
- brokering/scoping the access pattern the desktop already expects.

For self-hosted Spacetime, keep a restrictive reverse proxy in front of it and expose only the routes the product actually needs. The official self-hosting docs explicitly show exposing only selected routes like subscribe and identity instead of the full management surface.

## Recommended GCP deployment shape

### Public edge

Use:

- Google Cloud external Application Load Balancer
- Cloud Armor

Expose through it:

- public API hostname for desktop control/auth flows
- public API hostname for kernel/stat endpoints if needed

Keep internal-only services private where possible.

### Stateless Rust service runtime

Use:

- Cloud Run for `control-api`, `kernel-authority`, `treasury-router`, `stats-api`, and lightweight workers

Why:

- clean Rust container deployment,
- IAM-based service-to-service auth,
- good fit for HTTP APIs,
- no need to manage node pools for stateless components.

### Spacetime runtime

Use first:

- dedicated Compute Engine VM or small VM pair for SpacetimeDB self-hosting,
- Nginx reverse proxy in front,
- persistent disk for the Spacetime root dir/data.

Why:

- the official self-hosting guidance is VM + systemd + Nginx centric,
- it is the simplest path for a stateful direct-connection service,
- it matches Spacetime's documented route-restriction model,
- it avoids introducing Kubernetes complexity before you know your Spacetime operational envelope.

Use later, if needed:

- GKE Standard for SpacetimeDB

When GKE makes sense:

- you want containerized operational standardization,
- you need more controlled rolling operations across environments,
- your platform team already manages stateful workloads in Kubernetes.

I would no longer recommend GKE as the default first Spacetime deployment. On GCP, a hardened GCE VM is the better first move unless you already have a strong Kubernetes operating model.

## Recommended implementation sequence

### Phase 0: extract shared logic before deploying services

Create a shared Rust crate, for example `crates/economy-kernel-core`, and move into it the pure logic from:

- `apps/autopilot-desktop/src/economy_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/state/economy_snapshot.rs`

Move only pure domain logic first:

- receipt models,
- policy bundle parsing and normalization,
- snapshot computation,
- redaction/export rules,
- reason-code taxonomies.

Do not move desktop UI or local file persistence into that crate.

This is the most important first step because it prevents logic drift between desktop and backend.

### Phase 1: ship the minimal real backend

Build and deploy:

1. `control-api`
2. `kernel-authority`
3. Cloud SQL PostgreSQL
4. Redis
5. Artifact Registry and Secret Manager baseline

At this phase:

- desktop still keeps local fallback state,
- authoritative mutations start hitting `kernel-authority`,
- receipts become server-persisted,
- desktop reads back truth rather than inventing it locally.

### Phase 2: make Spacetime real for the domains it already fits

Deploy:

- `autopilot-sync` Spacetime module on the self-hosted SpacetimeDB instance

Then wire desktop to:

- mint real sync tokens from `control-api`,
- subscribe to real Spacetime streams,
- ack checkpoints remotely,
- use server-backed presence rather than local presence registry.

This is the point where current Phase 1 mirror/proxy semantics become real Phase 2.

### Phase 3: add projector and `/stats`

Build:

- `stats-projector`
- `stats-api`

Flow:

1. `kernel-authority` commits receipt.
2. receipt-committed event is published.
3. projector computes minute snapshot and materialized aggregates.
4. `/stats` serves from materialized snapshot tables.

If you want live dashboards, mirror selected public-safe counters into Spacetime after the canonical projector commit.

### Phase 4: add `treasury-router`

Only after the authority layer is stable:

- extract planner logic into its own Rust service,
- keep it stateless,
- require every planned operation to call `kernel-authority`,
- never let it mutate canonical state directly.

### Phase 5: add specialized workers

Add:

- verification workers,
- coverage/claims adapters,
- outbox/webhook relays,
- compensation and reconciliation jobs.

Keep these asynchronous and idempotent from day one.

## Recommended simplifications

Do not start with a large microservice fleet.

Start with these Rust deployables:

1. `control-api`
2. `kernel-authority`
3. `stats-projector` plus `stats-api`
4. SpacetimeDB `autopilot-sync`

Everything else can initially remain:

- inside `kernel-authority`, or
- as background workers in the same codebase with separate binaries.

This keeps the first production backend small while still respecting the right authority split.

## Concrete recommendation

If I were setting this up now, I would do this:

1. Extract receipt/snapshot/policy logic from desktop into a shared Rust crate.
2. Stand up `kernel-authority` on Cloud Run backed by Cloud SQL PostgreSQL.
3. Stand up `control-api` on Cloud Run for sync token issuance and control-plane auth.
4. Deploy SpacetimeDB on a hardened GCE VM first, only for presence/checkpoints/projection streams, with a restrictive reverse proxy and OIDC-backed access.
5. Add `stats-projector` and `/stats` once receipts are server-persisted.
6. Add `treasury-router` only when policy planning actually needs its own service boundary.

That path gets you to a real Rust backend on Google Cloud without overusing Spacetime and without prematurely building the entire economy-kernel service graph.
