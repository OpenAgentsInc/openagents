# Architecture Decision Records: Homework Network Hardening

Status: Proposed
Scope: distributed homework launch, worker execution, validator closeout, acceptance, and payout
Context: Issue #4368 and the production proof/debugging work around the bounded CS336 homework lane

---

## ADR-0001: Artifact identity is first-class and no business logic may infer remote paths from local filenames

### Status

Proposed

### Context

A recurring source of production and test failures has been ambiguity around artifact identity and path resolution.

Observed failure classes include:

* invocation manifest path vs canonical run manifest path
* logical optimizer-step checkpoint path vs actual retained bridged checkpoint-manifest path
* assignment-id keyed remote object layout vs contribution-id keyed local retained directories
* validator challenge claim responses lacking direct bindings even though contribution data existed
* replay digest mismatches caused by path-derived placeholders instead of real file content metadata
* mutable run-root references leaking into retained contribution manifests across attempts

These are not isolated bugs. They indicate that artifact identity is currently implicit, reconstructed by convention, and spread across multiple path-shaping codepaths.

### Decision

Introduce a first-class artifact identity model and require all runtime, authority, publication, and validator paths to resolve artifacts through typed artifact descriptors rather than inferred paths.

Each artifact descriptor must include at minimum:

* artifact_id
* artifact_kind
* scope_type (`run`, `window`, `assignment`, `contribution`, `challenge`)
* scope_id
* owner_assignment_id when applicable
* owner_contribution_id when applicable
* canonical_remote_locator
* local_materialization_locator
* content_digest
* byte_length
* content_encoding if applicable
* provenance_source (`worker_output`, `retained_snapshot`, `bridge_bundle`, `validator_output`, etc.)
* created_at

All upload, verification, publication, replay, and materialization code must use descriptors, not ad hoc filename conventions.

### Consequences

Positive:

* eliminates a large class of path mismatch bugs
* makes validator replay inputs explicit and portable
* reduces dependence on implicit directory shape knowledge inside the runtime
* makes closeout and auditing easier

Negative:

* requires plumbing descriptor objects through several layers that currently pass raw paths
* may require migration shims for older retained artifact layouts

### Implementation notes

1. Create a shared artifact descriptor type in a common crate/module.
2. Add artifact registry/lookup helpers for:

   * local retained discovery
   * remote publication lookup
   * validator challenge materialization
3. Replace path inference in worker closeout, validator materialization, checkpoint publication, and bridge bundle upload.
4. Keep temporary backward-compatibility adapters for older retained layouts.

### Follow-up migration

* Phase 1: emit descriptors alongside existing behavior
* Phase 2: route validator and closeout through descriptors
* Phase 3: remove raw-path business logic except inside low-level storage adapters

---

## ADR-0002: Worker contributions must be immutable, self-contained, and attempt-scoped before seal

### Status

Proposed

### Context

Validator replay failures showed that retained worker contribution metadata could still point at mutable run-root files. Once later attempts overwrote those files, validator replay correctly refused the artifacts due to digest drift.

This demonstrates that sealing a window before contribution artifacts are frozen into an attempt-scoped immutable snapshot is unsafe.

### Decision

Before a worker contribution may be used for checkpoint publication, seal, validator scheduling, or reconcile, all referenced artifacts must be stabilized into a contribution-local immutable snapshot.

Seal preconditions now include:

* worker execution has reached a successful terminal state
* contribution receipt exists
* artifact manifest exists
* all manifest-referenced artifacts have been copied or snapshotted into a contribution-local immutable root
* all descriptor digests and byte lengths are computed from actual file contents
* bridge bundles are published or publishable at canonical assignment-keyed object paths

The retained contribution root becomes the sole source of truth for validator-visible artifacts.

### Consequences

Positive:

* prevents replay drift across attempts
* makes validator replay deterministic by construction
* simplifies claim materialization because all required artifacts live under one immutable scope

Negative:

* increases local disk usage
* adds a stabilization step before closeout

### Implementation notes

1. Add a `stabilize_retained_contribution()` phase before closeout continues.
2. Rewrite retained manifest bindings to point at contribution-local snapshot paths.
3. Validate file bytes/digests before snapshotting.
4. Publish bridge bundles from assignment-keyed canonical remote paths, even if local directories are contribution-id keyed.

### Follow-up migration

* Backfill stabilization for older retained runs when feasible
* Refuse seal when required contribution-local snapshots are absent

---

## ADR-0003: Launch is a durable state machine and runs are not leaseable before bootstrap verification and scheduler materialization complete

### Status

Proposed

### Context

Production exposed a race where a run could exist, workers could see it, and even lease claims could occur before bootstrap publication and verification fully completed.

This happened because launch semantics were spread across run creation, artifact publication, verification, and scheduler residency, and lease claim paths could reconstruct schedulable runs too early.

### Decision

Launch is formalized as an explicit durable state machine with these phases:

* `launch_requested`
* `bootstrap_preparing`
* `bootstrap_uploaded`
* `bootstrap_verified`
* `scheduler_materialized`
* `leaseable`
* `failed`

A run is not visible to claim paths unless it has reached `scheduler_materialized`.
A run is not leaseable unless it has reached `leaseable`.
No lazy scheduler reconstruction may bypass these gates for homework launches.

### Consequences

Positive:

* removes the class of partially materialized but claimable launches
* makes production debugging easier because launch failure reason is explicit
* preserves a clean contract between launch and scheduling

Negative:

* requires explicit launch coordinator state and persistence
* may require adjusting legacy routes that inferred readiness indirectly

### Implementation notes

1. Introduce a launch coordinator record keyed by launch id and run id.
2. Persist upload receipts and verification receipts.
3. Materialize assignments/windows only after successful bootstrap verification.
4. Gate claim_lease on scheduler residency and launch state.
5. Expose launch state directly in admin/operator surfaces.

### Follow-up migration

* Add migration shims for older runs without launch coordinator records
* Mark non-homework launch paths for later alignment if they share the same risk

---

## ADR-0004: Node presence, admission, eligibility, and claimability are separate authority-side projections

### Status

Proposed

### Context

The production trail repeatedly exposed confusion between admitted, eligible, online, claimable, worker-capable, validator-capable, and network-scoped node readiness.

Nodes could be:

* admitted but offline
* online but contributor-disabled
* validator-capable but non-heartbeating
* visible via direct claim but not scheduler leaseable
* attached to the wrong network or stale lease

This means node readiness is under-modeled.

### Decision

Authority will maintain explicit separate projections for:

* identity status
* admission status
* presence status
* inventory status
* role capability status
* network scope status
* claimability status per work type (`worker_assignment`, `validator_challenge`)

All routes that reject a node must fail against one of these explicit projections and return the blocking dimension.

### Consequences

Positive:

* makes “why can this node not get work?” directly visible
* reduces endpoint probing and guesswork during live ops
* gives scheduler and direct claim paths a shared readiness model

Negative:

* increases projection complexity
* requires normalizing existing route-level checks into shared readiness logic

### Implementation notes

1. Add a `TrainingNodeReadiness` projection.
2. Move scattered checks into a shared readiness evaluator.
3. Surface readiness in operator/admin APIs.
4. Include per-role/per-network reasons in responses and logs.

### Follow-up migration

* Deprecate ad hoc online/eligible booleans as primary operator truth
* Keep them as summaries derived from the richer readiness model

---

## ADR-0005: Presence and heartbeat run independently from runtime sync and closeout

### Status

Proposed

### Context

Validator and worker online state disappeared in production because heartbeat and intake logic were nested under unrelated sync gates. This caused nodes to age out as offline even when the runtime itself was healthy.

Presence must not depend on whether closeout sync happened to run.

### Decision

Presence becomes an independent subsystem with:

* dedicated idle heartbeat loop
* dedicated admission repair/readmit-on-eviction flow
* separate online TTL
* separate metrics and logs
* no dependency on artifact sync, retained runtime sync, or closeout processing

### Consequences

Positive:

* keeps online state fresh even when no closeout work is pending
* reduces coupling between transport health and terminal sync progress
* makes validator availability much more predictable

Negative:

* adds another background loop and state channel
* requires careful idempotency for readmit-on-eviction behavior

### Implementation notes

1. Extract heartbeat/admission repair from terminal sync and intake logic.
2. Add dedicated presence metrics:

   * last heartbeat time
   * readmit attempts
   * online TTL expiration count
3. Require presence green before claimability can become true.

### Follow-up migration

* Remove any remaining presence logic hidden under `needs_sync` or analogous gates

---

## ADR-0006: Pylon retained runtime state is a recoverable cache, not a source of truth

### Status

Proposed

### Context

Multiple restart bugs came from trusting retained local runtime state too much:

* dead PID still treated as running
* stale active assignment blocking fresh work
* restarted node wedging on heartbeat eviction
* cached lease reused across wrong network scope

This shows retained runtime state must be reconstructable and distrustful by default.

### Decision

Retained runtime state is downgraded to a recoverable cache. On restart, Pylon must reconstruct actionable state from:

* live process/supervisor reality
* retained immutable artifacts
* authority assignment/window/challenge status
* a monotonic local closeout journal

No retained `running` state may be trusted without validating the live process.
No cached lease may be trusted without validating requested network scope and current authority status.

### Consequences

Positive:

* restart recovery becomes principled rather than patchy
* stale local state cannot silently poison fresh work
* safer behavior under crashes and partial restarts

Negative:

* restart path becomes more complex initially
* more authority reads may happen during recovery

### Implementation notes

1. Split retained state into:

   * execution state
   * artifact state
   * authority sync state
2. On boot, run a reconciliation pass that validates each layer separately.
3. Require explicit network match before any lease is accepted.
4. Mark orphaned completed runtimes terminal and orphaned inflight runtimes relaunchable.

### Follow-up migration

* Replace mixed runtime-state blobs with typed layer-specific files over time

---

## ADR-0007: Closeout is an append-only workflow journal with monotonic transitions

### Status

Proposed

### Context

The most successful fixes in this effort came from introducing durable closeout stages and preventing stage regression across sync passes. The remaining complexity strongly suggests that closeout is really a workflow engine and should be modeled as one.

### Decision

Closeout is represented as an append-only journal of monotonic stage transitions.

Canonical stages include:

* `worker_running`
* `worker_exited_success`
* `artifacts_verified_local`
* `checkpoint_published`
* `window_ready_to_seal`
* `window_sealed`
* `validator_claimed`
* `validator_replay_complete`
* `validator_finalized`
* `reconcile_requested`
* `reconcile_observed`
* `accepted`
* `paid`
* `terminal_failed`

Each transition must record:

* stage
* timestamp
* transition reason
* idempotency key if applicable
* authority receipt id if applicable
* blocking reason if failed

The next runtime action is derived from the journal, not inferred ad hoc from scattered state.

### Consequences

Positive:

* restart-safe by construction
* easier to debug and audit
* prevents stage regression and metadata overwrite bugs
* aligns with the exact business flow being proven

Negative:

* requires changing some runtime loops to be journal-driven
* may duplicate some information already implied elsewhere

### Implementation notes

1. Make journal append-only.
2. Compute effective current stage from the latest valid entry.
3. Refuse backward transitions.
4. Bind every external POST to an idempotency key and receipt.
5. Expose the journal to operator surfaces.

### Follow-up migration

* Move remaining sync logic to “compute next action from journal” style

---

## ADR-0008: Scheduler leases and direct validator claims are unified under a single work-offer contract

### Status

Proposed

### Context

The system currently has two ways to acquire work:

* scheduler-issued worker/validator leases
* direct validator challenge claim fallback

In practice these paths are converging on the same downstream runtime behavior but remain implemented as different concepts.

### Decision

Introduce a unified `WorkOffer` abstraction with variants:

* `worker_assignment`
* `validator_challenge`

Each offer carries:

* offer_id
* work_type
* network_id
* role_required
* authority acquisition source (`scheduled`, `direct_claim`)
* lease/claim expiry
* retained materialization bindings
* closeout journal linkage

Pylon consumes work offers, not special-case scheduler vs direct claim semantics.

### Consequences

Positive:

* simplifies intake logic
* makes fallback claim a protocol feature rather than a hidden branch
* unifies persistence and recovery semantics

Negative:

* requires reworking some client API wrappers
* may require compatibility mapping for older lease records

### Implementation notes

1. Add an internal work-offer layer in Pylon first.
2. Map existing worker leases and validator claims into that shape.
3. Later, consider unifying authority APIs around the same concept.

### Follow-up migration

* Keep existing external routes for compatibility while converging on one internal model

---

## ADR-0009: Network scope is a hard isolation boundary

### Status

Proposed

### Context

Fresh proof nodes were poisoned by stale leases from previous networks. That is unacceptable for isolated proofing and dangerous for correctness in general.

### Decision

Training network scope is a hard isolation boundary across both Pylon and authority.

Rules:

* a node admitted to one requested training network may not accept a lease from another network
* authority claim responses must never return a lease outside the requested network scope
* local runtime must reject any mismatched-network lease before ack, heartbeat, or cache persistence
* proof environments must use dedicated isolated networks by default

### Consequences

Positive:

* prevents backlog contamination across proof runs
* makes isolated repros trustworthy
* reduces accidental state bleed between old demo runs and fresh proof runs

Negative:

* may expose latent authority bugs in shared demo environments
* requires migration/testing against any existing multi-network assumptions

### Implementation notes

1. Keep the Pylon-side mismatch rejection already added.
2. Add the same invariant authority-side.
3. Add scheduler tests for network isolation.
4. Add dedicated proof network utilities.

### Follow-up migration

* discourage using the shared demo network as a proof surface

---

## ADR-0010: Authority transport, artifact transport, and execution backend are separate capability-checked adapters

### Status

Proposed

### Context

A large amount of production debugging has involved environment-specific transport issues:

* public edge hangs
* flaky relays
* IPv4-only paths
* curl vs reqwest differences
* bearer-token support vs ADC assumptions
* Linux vs macOS binary mismatches

These are environmental facts, but the architecture currently lets them leak into runtime behavior too deeply.

### Decision

Model three separate capability-checked adapters:

* Authority transport adapter
* Artifact transport adapter
* Execution backend adapter

Each adapter must support startup self-tests and declare capabilities before the node may mark itself ready/online.

### Consequences

Positive:

* reduces mystery around environment failures
* makes “why is this node online but not actually able to contribute?” more obvious
* localizes curl/reqwest/ADC/bearer differences inside adapters

Negative:

* adds adapter abstractions and startup complexity
* requires careful operator UX to avoid over-noisy health checks

### Implementation notes

1. Add startup probes for:

   * authority read/write
   * artifact read/write/verify
   * execution launch
2. Refuse contributor/validator readiness when required adapters are unhealthy.
3. Keep a degraded mode for observation-only nodes if desired.

### Follow-up migration

* migrate existing one-off transport workarounds into adapter capability flags

---

## ADR-0011: Proof environments are a first-class operational profile

### Status

Proposed

### Context

The current production proof process has required repeated ad hoc setup:

* fresh identities
* dedicated tunnels
* explicit bearer tokens
* isolated proof networks
* manual relay swaps
* backlog avoidance on shared demo networks

This is too operationally expensive for a system that needs credible live proofs.

### Decision

Introduce a supported proof-environment profile with:

* dedicated network namespace
* fresh worker and validator identities
* stable direct authority path
* explicit artifact bearer token or equivalent supported credential path
* run naming conventions for proof runs
* zero shared backlog assumption
* operator commands for launch, inspect, replay, and teardown

### Consequences

Positive:

* makes live proofing faster and more honest
* reduces confounding variables during production debugging
* gives a repeatable smoke path for releases

Negative:

* requires some operational tooling work
* creates another supported mode to maintain

### Implementation notes

1. Add scripts or commands to spin up proof worker/validator configs.
2. Make proof network creation explicit.
3. Add a release smoke command that exercises the whole chain.

### Follow-up migration

* replace informal closeproof/prodproof patterns with one standard proof workflow

---

## ADR-0012: Observability is causal and stage-based, not just counter-based

### Status

Proposed

### Context

Counters are useful, but the hardest failures in this effort were causal-chain failures:

* admitted but offline
* launch created but not scheduler-resident
* worker finished but no checkpoint publication
* sealed but validator claim unusable
* claim succeeded but materialization failed
* replay launched but artifact binding drifted

A simple counter cannot explain the first blocking reason.

### Decision

Add stage-based causal traces for each run/window/assignment/challenge.

Each trace should show:

* current stage
* last successful transition
* first blocking reason
* authority receipt ids
* relevant node ids
* relevant artifact ids
* retry counts

This trace should be visible in operator/admin surfaces and emitted in structured logs.

### Consequences

Positive:

* much faster production debugging
* clear proof of where the economic loop stopped
* complements the closeout journal well

Negative:

* more telemetry surface area
* requires careful aggregation and UI design

### Implementation notes

1. Build traces from launch coordinator state + node readiness + closeout journal.
2. Include worker and validator subtraces.
3. Expose a “why blocked?” summary on public/admin surfaces where appropriate.

### Follow-up migration

* keep counters, but treat them as summaries derived from stage traces rather than the primary truth

---

## ADR-0013: Settlement observation is separate from worker/validator execution control

### Status

Proposed

### Context

Worker execution, validator replay, reconcile, and payout observation currently touch the same broad terminal sync flow. That creates long dependency chains and makes one missing piece feel like the whole loop is broken.

### Decision

Separate the planes conceptually and in code:

* Execution plane: worker and validator runtimes produce artifacts/verdicts
* Authority closeout plane: seal, finalize, reconcile
* Settlement observation plane: accepted outcome and payout receipt tracking

Pylon participates in execution and observes settlement. It does not own treasury semantics.

### Consequences

Positive:

* simplifies reasoning about where a failure belongs
* reduces coupling between runtime loops and payout observation
* aligns better with the economic kernel direction

Negative:

* may require refactoring terminal sync into more focused components
* more explicit interfaces between planes needed

### Implementation notes

1. Keep payout observation in the closeout journal but isolate it from worker execution state.
2. Ensure settlement polling cannot block execution cleanup.
3. Expose payout as observed authority state, not local completion logic.

### Follow-up migration

* evolve terminal sync into plane-specific sync modules with shared journal state

---

## ADR-0014: Validator claim responses must be self-sufficient, but Pylon may synthesize bindings as a compatibility fallback

### Status

Proposed

### Context

Production repeatedly showed that validator challenge claim could succeed while `target_bindings` was empty. The runtime now compensates by pulling contribution outcomes and synthesizing bindings locally.

That fallback is useful, but the protocol should not rely on it forever.

### Decision

The long-term contract is:

* validator challenge claim response must be self-sufficient for validator materialization
* Pylon may temporarily synthesize missing bindings from authority contribution outcomes as a compatibility fallback
* compatibility fallback must emit structured warnings and metrics so it can be removed later

### Consequences

Positive:

* protects runtime progress against partial server response shapes
* preserves forward progress while authority contract catches up or historical state remains poisoned

Negative:

* temporary dual-path complexity
* risk of relying on fallback forever unless tracked

### Implementation notes

1. Keep local synthesis path for now.
2. Add metrics for “claim response missing target_bindings.”
3. Set removal criteria once fresh production runs prove the self-sufficient claim response consistently.

### Follow-up migration

* remove synthesis fallback when current production runs no longer need it

---

## ADR-0015: Manual sync routes must drive the real closeout chain, not only refresh caches

### Status

Proposed

### Context

One isolated reproduction showed that the operator-visible training sync route could return success while skipping the actual retained terminal closeout chain. That made “sync succeeded” a misleading signal.

### Decision

Any operator-triggered sync or recovery route must execute the real terminal closeout progression before refreshing caches and returning success.

A sync route that only refreshes authority caches is not sufficient for retained recovery.

### Consequences

Positive:

* operator actions become honest recovery tools
* avoids false confidence in manual sync commands

Negative:

* sync operations may become slower and more consequential
* requires clearer operator messaging about what sync actually does

### Implementation notes

1. Keep the recent change that runs terminal closeout before cache refresh.
2. Document sync semantics explicitly.
3. Emit stage deltas in sync responses.

### Follow-up migration

* audit other operator recovery routes for similar “cache refresh only” behavior

---

## Proposed implementation order

1. ADR-0003 launch state machine
2. ADR-0001 artifact identity model
3. ADR-0002 immutable contribution snapshots
4. ADR-0007 closeout journal as workflow engine
5. ADR-0004 and ADR-0005 node readiness + presence split
6. ADR-0009 network isolation hardening
7. ADR-0008 unified work-offer model
8. ADR-0010 transport adapters with startup capability checks
9. ADR-0012 causal observability
10. ADR-0011 proof-environment profile
11. ADR-0013 settlement observation separation
12. ADR-0014 claim self-sufficiency / fallback deprecation
13. ADR-0015 honest sync semantics

---

## Summary

These ADRs turn the recent proof/debugging lessons into structural decisions:

* artifact identity becomes explicit
* launch gets a real transaction boundary
* retained worker outputs become immutable before seal
* node readiness is modeled, not guessed
* presence is independent from closeout sync
* closeout becomes a monotonic workflow journal
* worker leases and validator claims converge on one offer model
* network scope becomes hard isolation
* transport quirks are encapsulated behind adapters
* proof environments become a supported operational mode
* observability becomes causal and stage-based

The net effect is a system that is less dependent on conventions, more restart-safe, and much easier to prove honestly in production.
