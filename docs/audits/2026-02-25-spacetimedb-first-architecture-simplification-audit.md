# SpacetimeDB-First Architecture Simplification Audit

Status: requested deep audit (SpacetimeDB priority, no legacy-preservation constraint)
Date: 2026-02-25
Owner: repo audit

## 1) Audit directive

This audit answers:

1. Are we currently aligned with SpacetimeDB best practices?
2. Where are we still carrying prior architecture assumptions that add complexity?
3. If we optimize for simplest/best SpacetimeDB implementation (and ignore migration conservatism), what should be changed now?

Explicit assumption for this audit:

1. We do not need to preserve legacy lanes for production safety at this stage.
2. We should prefer the cleanest end-state design over compatibility complexity.

## 2) Preflight and scope

Preflight authorities checked before this audit:

1. `docs/adr/INDEX.md`
2. `docs/plans/rust-migration-invariant-gates.md`

Key constraint framing from those docs today:

1. Repo still encodes authority-boundary invariants from the prior doctrine.
2. This audit evaluates whether those constraints should be simplified/superseded for SpacetimeDB-first design, rather than assuming they are permanently correct.

## 3) Evidence reviewed

OpenAgents implementation and docs reviewed:

1. `apps/autopilot-desktop/src/main.rs`
2. `apps/autopilot-desktop/src/runtime_codex_proto.rs`
3. `apps/runtime/src/lib.rs`
4. `apps/runtime/src/spacetime_publisher.rs`
5. `apps/runtime/src/sync_auth.rs`
6. `apps/runtime/src/config.rs`
7. `apps/runtime/sql/migrations/0001_runtime_sync_bootstrap.sql`
8. `apps/openagents.com/src/route_domains.rs`
9. `apps/openagents.com/src/sync_handlers.rs`
10. `apps/openagents.com/src/sync_token.rs`
11. `apps/openagents.com/src/openapi.rs`
12. `apps/openagents.com/src/tests.rs`
13. `crates/autopilot-spacetime/src/client.rs`
14. `crates/autopilot-spacetime/src/reducers.rs`
15. `crates/autopilot-spacetime/src/schema.rs`
16. `crates/autopilot-spacetime/Cargo.toml`
17. `proto/openagents/sync/v1/sync.proto`
18. `docs/core/ARCHITECTURE.md`
19. `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
20. `docs/plans/spacetimedb-full-integration.md`
21. `docs/plans/2026-02-25-spacetimedb-autopilot-primary-comms-integration-plan.md`

SpacetimeDB sources reviewed (local repo):

1. `/Users/christopherdavid/code/SpacetimeDB/README.md`
2. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00300-resources/00200-reference/00200-http-api/00300-database.md`
3. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00400-subscriptions/00200-subscription-semantics.md`
4. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00200-functions/00300-reducers/00300-reducers.md`
5. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00200-functions/00400-procedures.md`
6. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00300-tables/00550-event-tables.md`
7. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00300-resources/00100-how-to/00400-row-level-security.md`

Verification commands run:

1. `cargo test -p openagents-control-service spacetime_token_route_is_retired -- --nocapture`
2. `cargo test -p openagents-control-service spacetime_token_routes_match_primary_sync_token_contract_shape -- --nocapture`

Observed result:

1. `spacetime_token_route_is_retired` fails (expected 404, actual 200).
2. `spacetime_token_routes_match_primary_sync_token_contract_shape` passes.

## 4) SpacetimeDB best-practice baseline (target model)

From SpacetimeDB docs/code, the intended operating model is:

1. Clients connect directly to SpacetimeDB (`GET /v1/database/:name_or_identity/subscribe`).
2. App logic lives in module reducers; reducers are the normal mutation path.
3. Subscriptions are snapshot-then-delta (`SubscribeApplied` then `TransactionUpdate`) with transaction atomicity.
4. Event tables are first-class for transient realtime signals.
5. Procedures exist but should be used sparingly; reducers are preferred for deterministic state mutation.
6. Access filtering should prefer views; RLS is experimental.

## 5) Current-state fit assessment

### Executive scorecard

1. Transport semantics alignment with SpacetimeDB: 1/5
2. Reducer-authoritative state model alignment: 1/5
3. Subscription protocol alignment: 1/5
4. Token/auth simplicity and consistency: 2/5
5. Documentation/runtime consistency: 2/5

Overall: implementation is still largely a legacy sync architecture renamed around Spacetime terminology, with partial scaffolding for SpacetimeDB concepts.

### What is already good

1. `crates/autopilot-spacetime` defines a useful domain model for stream/checkpoint/idempotency concepts.
2. `(stream_id, seq)` and stale-cursor semantics are explicitly represented in multiple places.
3. There is active test coverage around sync token contracts and compatibility checks.

### Core reality

The current sync stack is not yet a SpacetimeDB-native architecture. It is primarily:

1. control-minted JWT + custom topic grants,
2. desktop speaking Phoenix-style frames over `/sync/socket/websocket`,
3. runtime publishing into in-process `ReducerStore` via `SpacetimePublisher::in_memory()`.

## 6) Findings (ordered by severity)

### Critical-1: No actual SpacetimeDB runtime integration in retained app paths

Evidence:

1. `apps/runtime/src/lib.rs` initializes `SpacetimePublisher::in_memory()`.
2. `apps/runtime/src/spacetime_publisher.rs` writes to `ReducerStore` (in-process), not to a SpacetimeDB host.
3. `crates/autopilot-spacetime/Cargo.toml` has no SpacetimeDB client dependency.
4. Desktop sync path in `apps/autopilot-desktop/src/main.rs` targets `/sync/socket/websocket`, not `/v1/database/:db/subscribe`.

Impact:

1. Current architecture does not match SpacetimeDB’s direct client-to-database model.
2. The codebase is still effectively running a custom sync protocol with Spacetime naming.

### Critical-2: Protocol mismatch with SpacetimeDB subscription model

Evidence:

1. Desktop uses `phx_join`, `sync:subscribe`, `sync:update_batch` frame/event vocabulary (`apps/autopilot-desktop/src/main.rs`, `apps/autopilot-desktop/src/runtime_codex_proto.rs`).
2. SpacetimeDB semantics are `Subscribe`/`SubscribeApplied`/`TransactionUpdate` with protocol negotiation (`v1/v2 ... spacetimedb`).
3. `proto/openagents/sync/v1/sync.proto` is still topic-centric and includes `SpacetimeFrame` as custom envelope.

Impact:

1. Client behavior cannot be considered SpacetimeDB-native even if naming references Spacetime.
2. Extra translation layers remain mandatory and increase failure surface.

### Critical-3: Docs/tests/routes are internally inconsistent on active sync token lanes

Evidence:

1. `route_domains.rs` mounts `/api/spacetime/token` to `sync_token`.
2. OpenAPI unit test `omits_retired_spacetime_token_route` asserts route omission.
3. Test `spacetime_token_route_is_retired` expects 404 but fails with actual 200.
4. Other tests assert `/api/spacetime/token` parity with `/api/sync/token` and pass.

Impact:

1. Operational intent is ambiguous.
2. Teams can make contradictory assumptions during implementation.

### High-1: Token claims and auth remain topic-era complexity

Evidence:

1. `sync_token.rs` still derives grants from topic semantics and legacy scope labels.
2. `apps/runtime/src/sync_auth.rs` authorizes `AuthorizedSpacetimeTopic` parsed from strings like `run:<id>:events`, `worker:<id>:lifecycle`.

Impact:

1. Keeps old topic abstractions alive instead of converging on a single stream/query model.
2. Adds unnecessary aliasing/compatibility logic for a system that is not yet live.

### High-2: Runtime DB schema still reflects old sync event journal model

Evidence:

1. `apps/runtime/sql/migrations/0001_runtime_sync_bootstrap.sql` creates `runtime.sync_stream_events` and `runtime.sync_topic_sequences`.
2. This is a runtime-owned projection journal pattern, not SpacetimeDB module-owned state.

Impact:

1. Continues “runtime as sync state engine” shape rather than “SpacetimeDB as sync state engine.”

### High-3: `autopilot-spacetime` crate is useful but currently acts as simulation layer

Evidence:

1. `SpacetimeClient` is built on `Arc<Mutex<ReducerStore>>` in-memory store.
2. Reducer/schema code is domain-rich but not wired to real SpacetimeDB reducers/subscriptions.

Impact:

1. Risk of false confidence: strong local abstractions with weak production path parity.

### Medium-1: Legacy/compatibility surface area is still large

Evidence snapshot:

1. `rg -n "\blegacy\b" ... | wc -l` -> 149 hits in key surfaces.
2. Control service still carries broad compatibility/route split/admin legacy lanes.

Impact:

1. Increased maintenance and cognitive load.
2. Slower convergence to a clean SpacetimeDB-first architecture.

## 7) Direct answer: does current implementation reflect SpacetimeDB best practice?

Short answer: no, not yet.

It reflects a transitional architecture that renamed and partially mapped legacy sync behavior onto Spacetime terminology. The core runtime/client protocol, auth surface, and storage model are still not aligned with SpacetimeDB’s simplest native pattern.

## 8) Recommended simplified target architecture (no legacy-preservation bias)

### 8.1 Topology

1. Autopilot desktop and other clients connect directly to SpacetimeDB subscribe endpoint.
2. Reducers become the canonical sync/collaboration mutation path.
3. Runtime/control remain command APIs for Hydra/Aegis/high-stakes money-trust domains.
4. Runtime/control publish projection events into SpacetimeDB reducers where cross-device sync/discovery is required.
5. Nostr remains optional bridge/interoperability lane and must not block Spacetime state progression.

### 8.2 Hydra/Aegis positioning in this simplified model

1. Keep Hydra/Aegis authoritative commands in runtime HTTP APIs for now.
2. Do not mirror old authority-boundary complexity into sync lanes.
3. Emit derived, non-authoritative collaboration/discovery state into SpacetimeDB for clients.
4. Only move additional authority logic into SpacetimeDB if explicitly justified per domain and audited for determinism/safety.

This gives a simpler system while preserving high-stakes correctness boundaries where they materially matter.

## 9) Recommended buildout order (hard cut, no dual-primary migration)

### Phase 0: Governance and contract reset

1. Update ADR/invariants/docs to one truth:
   - SpacetimeDB is current canonical sync transport.
   - Remove contradictory “retired but active” route language.
2. Collapse token contract docs to a single canonical path and explicit alias policy (or remove aliases now).

### Phase 1: Real SpacetimeDB client/server path

1. Replace desktop Phoenix socket path with SpacetimeDB subscribe protocol path.
2. Introduce actual SpacetimeDB reducer call integration for mutations.
3. Remove `/sync/socket/websocket` assumptions from desktop sync lifecycle.

### Phase 2: Runtime integration simplification

1. Replace in-memory `SpacetimePublisher` with real SpacetimeDB write path.
2. Delete runtime sync topic sequence journal tables that exist only for old fanout model.
3. Keep runtime as source for domain events, but publish to SpacetimeDB as the single sync fabric.

### Phase 3: Protocol and auth convergence

1. Replace topic-based proto surfaces with stream/query-set-based contracts aligned to SpacetimeDB semantics.
2. Remove topic parsing auth logic in runtime (`AuthorizedSpacetimeTopic` model) and move to stream/query grant model.
3. Eliminate duplicate sync token routes if not required.

### Phase 4: Dead-code and compatibility deletion

1. Remove legacy compatibility routes/tests/docs no longer serving active clients.
2. Remove contradictory OpenAPI/tests and lock one canonical behavior.
3. Keep only strict regression harnesses for replay/idempotency and auth scope enforcement.

## 10) Concrete cleanup targets now

Highest-value removals/consolidations once implementation starts:

1. Desktop: Phoenix frame helpers in `apps/autopilot-desktop/src/runtime_codex_proto.rs` (replace with SpacetimeDB wire adapter).
2. Runtime: in-memory sync publisher in `apps/runtime/src/spacetime_publisher.rs`.
3. Runtime sync schema: `runtime.sync_topic_sequences` and topic-first publish assumptions.
4. Control service: contradictory retirement semantics for `/api/spacetime/token`.
5. Proto: topic-era envelope abstractions in `proto/openagents/sync/v1/sync.proto`.

## 11) Documentation updates required after implementation begins

1. `docs/core/ARCHITECTURE.md`
2. `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
3. `docs/plans/rust-migration-invariant-gates.md`
4. `docs/sync/README.md` and related sync runbooks
5. `docs/plans/spacetimedb-full-integration.md`

## 12) Final recommendation

If the goal is best/simplest SpacetimeDB architecture (and legacy preservation is not a concern), we should do a hard convergence now:

1. stop carrying topic-era/Phoenix-era sync behavior,
2. wire real SpacetimeDB subscribe/reducer paths end-to-end,
3. reduce route/protocol aliasing,
4. keep Hydra/Aegis command boundaries only where they are genuinely required for money/trust correctness,
5. delete compatibility scaffolding aggressively once SpacetimeDB-native path is live in this repo.

That is the most streamlined and technically coherent architecture for the stated priority.
