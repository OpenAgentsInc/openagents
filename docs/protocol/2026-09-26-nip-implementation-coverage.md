# NIP implementation coverage after the September 26 sync

The current work implements selected protocol checks, privacy boundaries, and
relay paths against the specifications in `nips/`. **It does not complete all
NIPs or all Coder application roles.** The source inventories now identify the
current upstream revisions separately from the revisions behind older fixtures.
A source-count assertion, JSON parser, or stored event does not establish a
working client, agent, payment service, or complete relay extension.

This report follows the [source-sync assessment](2026-09-26-upstream-nip-sync.md)
and its [official](2026-09-26-upstream-nip-sync-official.md) and
[Block](2026-09-26-upstream-nip-sync-block.md) reviews. Those documents describe
the gaps at the source-only sync; this report records subsequent implementation
scope. The [implementation plan](implementation-plan.md) remains the backlog.

## Subsequent application slices

This report originally described the source-sync implementation. Later September
26 work added the [durable task owner](../coder/guides/tasks.md),
[private knowledge integration](../coder/migration-status.md), and the
[free labor runtime](../coder/runtime/free-labor.md). The latter now has
[retained local relay evidence](../coder/verification/2026-09-26-free-labor/README.md).
The [read-only iOS history reader](../coder/guides/mobile-readonly.md) also adds
SESS's separately admitted retained-history observer profile through
`coder-history`, `coder-connect`, and `coder-mobile`. Its
[reader receipt](../coder/verification/2026-09-26-mobile-reader.md) and
[QR bootstrap receipt](../coder/verification/2026-09-26-world-pairing.md) record
the bounded read-only path. These are scoped implementations, not blanket conformance for
their NIPs.
Use the [migration tracker](../coder/migration-status.md) for application delivery
and this report for the lower-level source-sync boundary.

## Source identity and evidence boundaries

| Lane | Current source | What the inventory establishes |
| --- | --- | --- |
| Official | `b82211e96c6dad616ed2ea43034c1c621256b745`; 99 specifications and the upstream index. | [`lane.rs`](../../crates/nostr/src/lane.rs) accounts for every specification, including A3. Current client reviews and the prior behavior baseline are separate. |
| Block | `781d39510cf23cfe224e8f521ae06a23377e06de`; 17 specifications. | [`block_lane.rs`](../../crates/nostr/src/block_lane.rs) includes FI and PMA. Retained subset fixtures do not certify every role at the new pin. |
| OpenAgents | The [23 authored specifications](../../nips/openagents/README.md), plus [shared contracts](../../nips/openagents/contracts.md). | Each contract's implemented portion and remaining host obligations appear below. There is no blanket OpenAgents conformance claim. |

The [manifest](../../nips/manifest.json) remains the upstream source authority.
Verification must bind to the actual implementation tree. Named tests below
identify intended coverage; their presence alone is not a passing execution
record. Final gate results must distinguish pure tests, live PostgreSQL/HTTP
checks, omitted prerequisites, and deployment verification. This change does
not establish the state of a production deployment.

## Official and Block work in this change

| Surface | Implementation scope | Remaining limitation |
| --- | --- | --- |
| Official 02 | [`follow.rs`](../../crates/nostr/src/domain/follow.rs) renders traversal-order slash petnames and resolves supplied follow graphs. | NIP-05 roots require caller-verified bindings; no directory or network resolution is implied. |
| Official 22 and 84 | [Comments](../../crates/nostr/src/domain/comment.rs) accept kind-1 roots and parents under the existing scope checks. [Highlights](../../crates/nostr/src/domain/highlight.rs) handle structured `i`, text `r`, optional sources, and quote URL markers. | Parsing does not fetch or authenticate external source content. |
| Official 30 | [`emoji.rs`](../../crates/nostr/src/domain/emoji.rs) scopes replacement tokens to the declaring event, including comments. | No remote image fetching, rendering, or UI integration. |
| Official 42 and 67 | [`eose.rs`](../../crates/nostr/src/domain/eose.rs) distinguishes an authentication opportunity from current-view completeness and tolerates unrelated hints. | No automatic client authentication or new relay hint-emission policy. |
| Official 43 | [`relay_access.rs`](../../crates/nostr/src/domain/relay_access.rs) checks signed declarations and fresh join/leave requests. | This is not a claim issuer, durable membership service, or claim-redemption implementation. The upstream `createclaim` ambiguity remains documented. |
| Official A3 | [`payment_target.rs`](../../crates/nostr/src/domain/payment_target.rs) parses typed targets and produces escaped, inert URIs; event admission checks the kind's syntax. | No network-specific address validation, invoice creation, wallet authority, or payment. |
| Private event visibility | [Stored queries](../../crates/nostr-relay/src/store/statements.rs), [gateway delivery](../../crates/nostr-relay/src/gateway/server.rs), and [migration 0010](../../migrations/0010_private_protocol_search.sql) apply owner/recipient boundaries and remove private content from the search index. NIP-78 owner state, private `3188` artifacts, PMA refusal, and mailbox/group separation have explicit paths. | Existing disclosure cannot be undone. History, COUNT, ID lookup, search, live delivery, import/restore, and revocation require the corresponding integration evidence; one query test cannot stand in for all paths. |
| Block AP | [`agent_persona.rs`](../../crates/nostr/src/agent_persona.rs) handles typed persona fields, portable aliases, catalog redaction, foreign adoption, preserved local commands, source digests, and session-restart decisions. | No agent launcher, team-projection consumer, or persona UI. A published command grants no execution authority. |
| Block CW | [`thread_window.rs`](../../crates/nostr/src/thread_window.rs) checks whole batches, request-bound signed `39007` records, and shared sticky budgets. Client-written `39007` is refused. The channel path rechecks access after reading rows. | The [HTTP query path](../../crates/nostr-relay/src/gateway/query.rs) refuses unsupported thread modes. Thread SQL, auxiliary reconstruction, thread-batch access refresh, and live thread-window delivery remain unimplemented. |
| Block RS | [`read_state_snapshot.rs`](../../crates/nostr/src/read_state_snapshot.rs) validates raw requests, descriptors, signed events, coordinate uniqueness, digests, and byte/count bounds. The [store](../../crates/nostr-relay/src/store/mod.rs) and [HTTP query path](../../crates/nostr-relay/src/gateway/query.rs) add a configured writer-database snapshot with Host binding and replay-checked NIP-98 authorization. | Live HTTP/PostgreSQL snapshot acceptance passed, including restart and retained expiration. This does not establish the separate cross-process, cross-subscription WebSocket EOSE barrier or a synchronized Coder view. |
| Block FI | [`federated_identity.rs`](../../crates/nostr/src/federated_identity.rs) bounds compact tokens and checks configured community/issuer, token class, asymmetric algorithm allowlists, audience, time, key, session deadline, and issuer-partitioned deny state. | A mandatory verifier supplies cryptographic evidence. No concrete JWT implementation, JWKS lifecycle, session disconnects, or HTTP/NIP-42 integration is supplied; FI remains unadvertised. |
| Block PL | [`push_lease.rs`](../../crates/nostr/src/push_lease.rs) excludes `39007` from push kinds. [Gateway configuration](../../crates/nostr-relay/src/gateway/config.rs) rejects the incomplete delivery configuration. | Transactional lease authority, durable delivery jobs, current-membership checks, and the public gateway protocol remain required before re-enabling delivery. |
| Block PMA | [`validate_block_ingest`](../../crates/nostr/src/domain/block.rs) refuses reserved kind `30179`; stored visibility and search also exclude it. | No PMA privacy/CAS/migration/recovery service is implemented. Rejection is the supported behavior. |

Official 01/29 lifecycle checks, NIP-86 allow/ban consistency, and the live
privacy/snapshot tests must retain their individual results. Do not advance
unchanged behavior to a new conformance status merely because its inventory pin
matches. The current code also carries the minor NIP-73 Ethereum identifier
example correction in [external IDs](../../crates/nostr/src/domain/external_id.rs).

The RS snapshot checks retained expiration in the same database statement as
its rows and admission policy. An expired own-author coordinate that remains
stored causes HTTP 503 until the expiration sweep physically removes it; the
relay never calls a cut complete after silently omitting that retained state.
The HTTP fixture verifies refusal, isolation from other authors, and recovery
after the real sweep.

## OpenAgents contracts: pure checks and remaining runtime work

| Contract | Implemented foundation | Concrete remaining work |
| --- | --- | --- |
| [Shared contracts](../../nips/openagents/contracts.md) | Strict JSON/JCS; exact artifact/schema/definition/event references, locks, effects, evidence, context, receipts, and outcomes in [`contracts`](../../crates/nostr/src/contracts/mod.rs). [`private_artifact`](../../crates/nostr/src/private_artifact.rs) authenticates encrypted declarations and exact inline or resolved bytes. | Complete bounded remote resolution and durable application stores. Shape, signature, and encryption checks do not grant authority. |
| [CAP](../../nips/openagents/NIP-CAP.md) | [`cap.rs`](../../crates/nostr/src/cap.rs) parses definitions/preferences, discovery tags, service contracts, and effect/bound coverage; local capability probes exist. | Actual admission and support probing for each new adapter feature, including X402. Exact host grants and verified bindings remain separate. |
| [PRG](../../nips/openagents/NIP-PRG.md) | [`prg.rs`](../../crates/nostr/src/prg.rs) validates definitions, envelopes, and results; Coder has a local program runtime. | General admitted remote invocation, complete implementation resolution, and retained execution evidence. Discovery must not activate a program. |
| [EXT](../../nips/openagents/NIP-EXT.md) | [`ext.rs`](../../crates/nostr/src/ext.rs) checks records/manifests, operations/functions/skills, lifecycle transitions, migration pairs, freshness, and closure. | Latest foreign-import provenance and component-set compatibility; complete remote distribution and host activation/rollback. |
| [CJ](../../nips/openagents/NIP-CJ.md) | [`decision.rs`](../../crates/nostr/src/decision.rs) and [`execution.rs`](../../crates/nostr/src/execution.rs) cover bodies, encrypted routing, correlation, pins, bounds, and idempotency. | Each new profile's binding admission, complete remote artifact resolution, and actual dispatch. Confirm profiles independently. |
| [RUN](../../nips/openagents/NIP-RUN.md) | [`run.rs`](../../crates/nostr/src/run.rs) checks journal shape/order, fork retention, recovery boundaries, and fencing. | Persistent multi-controller arbitration and reconciliation with actual effects; complete application-profile evidence. |
| [CTX](../../nips/openagents/NIP-CTX.md) | Shared evidence and context parsers. | Task-frame, selection, expansion, and cache contracts; source/recipient admission; complete history and unknown handling. |
| [POL](../../nips/openagents/NIP-POL.md) | CAP grants/preferences and shared evidence/bound vocabulary. | Scoped instructions, exact-action approvals, disclosure, learned-preference activation/revocation, routing/cost records, and enforcement by the acting host. |
| [COORD](../../nips/openagents/NIP-COORD.md) | RUN generation/fencing helpers and existing local coordination foundations. | Authoritative shared task claims, persistent atomic ownership, background subscriptions, findings, and cross-host deduplication. |
| [EVAL](../../nips/openagents/NIP-EVAL.md) | KB-linked signed publication/report subset in [`kb.rs`](../../crates/nostr/src/kb.rs); local Gym evidence. | General frozen suite/trial/partition/comparison and promotion contracts, leakage checks, and evaluator admission. |
| [OPT](../../nips/openagents/NIP-OPT.md) | PRG/CJ/EVAL and local optimization foundations. | Semantic signatures, exact implementations, studies, data partitions, candidates, trial accounting, and promotion. |
| [KB](../../nips/openagents/NIP-KB.md) | Signed immutable versions, heads, withdrawal, equivocation, and EVAL evidence in [`kb.rs`](../../crates/nostr/src/kb.rs); publication/sync commands. | Wider promotion and evaluator trust. Signatures establish attribution, not quality or a positive network effect. |
| [XP](../../nips/openagents/NIP-XP.md) | [`xp.rs`](../../crates/nostr/src/xp.rs) validates signed quests, awards, revocations, and the `kb-transfer` acceptance rule; [`knowledge::xp`](../../crates/knowledge/src/xp.rs) derives the reader's trusted ledger. Verse displays quests and XP. | Reader trust, available evidence, and source-task exclusions still determine whether an award counts. XP is not spendable money or proof of a paid labor market. |
| [CTRL](../../nips/openagents/NIP-CTRL.md) | [Twelve closed artifact parsers](../../crates/nostr/src/control.rs) and a [bounded local-owner bridge](../coder/runtime/nostr-task-control.md): authenticated pairing, independent rights, exact commands, expiry/revocation, finite private projections, and durable retry identity. | Complete client products, full signed RUN history, controller handoff, remote creation/start, and broader SESS/CTX integration. Partial projections and a scoped host are not complete CTRL conformance. |
| [MKT](../../nips/openagents/NIP-MKT.md) | [`market_contracts`](../../crates/nostr/src/market_contracts.rs) validates signed offerings/heads, exact terms, authenticated RFQ/quote/order/ack, OrderRef, bounded per-issuer history, replay/equivocation, and receipt-time freshness. Public admission uses these parsers. | Durable host reservations/confirmation, full domain validation, remaining fulfillment/cancellation/dispute/closure records, and live market operation. Paid negotiation and payment records explicitly refuse in this component. |
| [LAB](../../nips/openagents/NIP-LAB.md) | [`market_contracts::labor`](../../crates/nostr/src/market_contracts/labor.rs) checks closed terms, checker policies, rights, roles, deadlines, rework bounds, criterion sets, reuse restrictions, and required recipients. Its MKT adapter checks exact policy bytes. | [`coder-labor`](../../crates/coder-labor/src/lib.rs) now supplies an admitted free-only graph, durable role journals, execution linkage, delivery, independent checks, and acceptance. General profiles, public provider operation, paid settlement, and dispute resolution remain unimplemented; dispute records alone do not resolve a dispute. |
| [SESS](../../nips/openagents/NIP-SESS.md) | The [retained-history observer](../../crates/coder-connect/README.md) implements explicit root/client pairing, a short-lived single-device QR bootstrap with same-device retry recovery, NIP-42 connections, signed encrypted private `3188` reads, exact reply binding, bounded raw transcript pages, expiry/revocation, and source-change detection. The [iOS client](../coder/guides/mobile-readonly.md) supplies Rust-owned cache and a SwiftUI observation surface. | Managed engine sessions, actual adapter-feature/effective-configuration admission, durable input queues, submit/steer/interrupt, elicitation versus approval, terminal causes, and managed/imported history accounting. The observer neither invokes CJ engine work nor grants CTRL task rights; it does not recover records the original harness never retained. |
| [WS](../../nips/openagents/NIP-WS.md) | Shared artifacts, local snapshots, and the separate Block RS work. | Resource/document/checkpoint identity and conditional mutations; exact view definitions/cuts/pages/deltas and structural-gap handling. RS does not implement WS by implication. |
| [WORK](../../nips/openagents/NIP-WORK.md) | WS/CJ relationship designs. | Authority-scoped planning graph, exact-revision proposals/admission, disposition, delegation, evidence, and bounded synchronization. |
| [AUTO](../../nips/openagents/NIP-AUTO.md) | Existing local scheduling concepts. | Frozen plans, triggers, occurrence identity, bounded admission/accounting, queued-turn reconciliation, durable skip/coalesce/cancel, and checked continuation. |
| [ENV](../../nips/openagents/NIP-ENV.md) | Existing execution-boundary and subprocess mechanisms. | Lease/materialization authority, real runtime attachment, per-CJ deduplication, participant disclosure, cleanup uncertainty, and persistent reconciliation. |
| [LIVE](../../nips/openagents/NIP-LIVE.md) | Existing media concepts and shared artifacts. | Consented session/recipient admission, input/speaking floors, capture anchors, observation-bound device effects, bounded teardown, and receipts. |
| [X402](../../nips/openagents/NIP-X402.md) | [`x402.rs`](../../crates/nostr/src/x402.rs) checks exact Lightning terms, actual-request bindings for HTTP/MCP and explicitly enabled native Nostr, authenticated BOLT11 invoices, expiry, payee, preimages, and network/payment-hash consumption keys. It reproduces the seven retained digest vectors. | Wallet authority and enforceable fee bounds, durable payment/proof consumption, provider execution intent/recovery, and live HTTP/MCP/native interoperability. No payment service is established by offline cryptographic checks. |
| [MV](../../nips/openagents/NIP-MV.md) | [Verse](../../crates/verse/src/mv.rs) publishes/reads world presence and state. | Preserve the documented role limits: peers do not establish authoritative simulation or complete world-definition publication. |

## Market and labor boundaries

The market module recognizes four closed negotiation bodies. It requires
`OpenEnvelope` provenance; supplying a pubkey beside arbitrary JSON cannot
create a trusted record. Quotes require exact provider-authenticated terms.
A mutable discovery head cannot amend an agreement. Public admission checks
`3192`/`30192` syntax, signatures, and tag agreement without asserting that
referenced artifacts are available.

The in-memory negotiation tracker retains gaps and equivocation, blocks
conflicting sequences, binds exact quote/terms/order identities, and permits
at most one confirmed order. Live expiry uses the caller's receipt time;
backdated issuer timestamps cannot force admission. Duplicate logical records
retain identity across newly encrypted envelopes. A new tracker instance does
not supply crash-safe replay protection: the host must retain original signed
declarations, resolved terms, receipt times, decisions, and reservations.

The initial negotiation tests use an explicit no-op domain fixture. LAB's
`LaborProfile` adds strict terms/policy/rights validation but requires a
`ClosureAdmission` host implementation for the remaining exact reference graph
and supported execution semantics. There is no default that accepts opaque
labor inputs. Even a valid free agreement grants no credentials, admits no
CJ job, proves no delivery, and creates no buyer acceptance.

Current payment boundaries stay unchanged: MKT/LAB's fixed-price profile pays
after acceptance; X402 purchases an exact operation upfront. NWC is wallet
transport. Zaps sign a different description-hash commitment. L402/LSAT is a
distinct protocol. None can silently substitute for another or make a payment
record into evidence of correct work. See the
[x402 integration assessment](../coder/design/x402-lightning-nostr-integration.md).

## Lightning validation boundary

The new [x402 validator](../../crates/nostr/src/x402.rs) is a fresh Rust
implementation of the pinned public specification, independent of the older
zap parser. It validates recoverable ECDSA invoice signatures, exact millisatoshi
amounts, currency, payee, description-hash request binding, invoice lifetime,
and the SHA-256 preimage proof. Its invoice feature interpretation cites the
reviewed BOLT commit in [the decoder](../../crates/nostr/src/x402/invoice.rs).

HTTP binding preserves exact URI/query order and raw body bytes. MCP binding
distinguishes absent, empty, and null selected metadata values; missing
arguments normalize to `{}`, and null arguments are refused. Native Nostr
support requires an explicit profile selection. Untrusted JSON must pass strict parsing before
canonicalization, including duplicate-member rejection. A consumed-proof key
includes the network and payment hash. The returned retention value is an
upstream minimum, not permission to discard a native purchase with a longer
recovery deadline or unresolved obligations.

These functions neither debit a wallet nor consume a proof. A service still
needs durable atomic proof consumption and execution intent, admitted budgets
and enforceable fees, response recovery, and actual transport interoperability.
No feature advertisement or production payment path was added.

## Verification and remaining release gates

The [retained verification record](verification/2026-09-26-nips/README.md)
binds results to the tested code, distinguishes the full run from scoped
recoveries, and retains the earlier failures and their corrections. Source
inventory counts and passing tests remain narrower than whole-NIP conformance.

The relevant pure fixtures live with the [market validator](../../crates/nostr/src/market_contracts/tests.rs),
[LAB validator](../../crates/nostr/src/market_contracts/labor/tests.rs),
[profile admission](../../crates/nostr/src/profile.rs), and the new Block and
official modules linked above. They exercise malformed fields, tampering,
wrong authority, replay, missing dependencies, expiry, unsupported profiles,
and refusal boundaries.

Live acceptance is separate:

- [Store tests](../../crates/nostr-relay/tests/store_postgres.rs) cover stored
  privacy and management behavior against PostgreSQL.
- [Gateway tests](../../crates/nostr-relay/tests/gateway_postgres.rs) cover
  authenticated versus anonymous access and WebSocket lifecycle behavior.
- [RS snapshot tests](../../crates/nostr-relay/tests/read_state_snapshot_postgres.rs)
  cover the configured HTTP/database snapshot path and its refusal cases.
- The [manual verification gate](../verification.md) records the tested tree,
  commands, feature scope, failures, and omitted prerequisites. Production
  configuration and wallet/provider interoperability require their own evidence.

The persisted **free labor rehearsal** subsequently passed in
[the retained #9679 record](../coder/verification/2026-09-26-free-labor/README.md).
It resolves an admitted LAB graph, confirms exact free terms, binds a bounded
execution, retains delivery and independent checks, and records buyer acceptance
through real loopback WebSocket transport and private local storage. Public
synthetic identities and one operator establish a reproducible fixture, not
an independently operated or paid labor market. The next market milestones are
independent provider operation, broader recovery evidence, and separately
admitted settlement.
