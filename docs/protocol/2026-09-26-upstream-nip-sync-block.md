# Block NIP implementation review, September 26, 2026

This is the detailed Block-lane appendix to the
[upstream sync assessment](2026-09-26-upstream-nip-sync.md). The comparison is
`8342dfcc5890b81a269a8ec3db73a8a56f76ce79` to
`781d39510cf23cfe224e8f521ae06a23377e06de`. The review covers the full AP, CW,
PL, RS, FI, and PMA specifications, the four existing-file diffs, relevant
Rust validators and relay paths, committed fixtures, and the existing ledgers.
Code references and one-based line numbers refer to OpenAgents `393b876457`.
Unqualified gateway/store paths are under `crates/nostr-relay/src/`; protocol
paths are under `crates/nostr/src/`. No live deployment was probed.
References to prior documentation wording use the pre-correction snapshot.
The parent assessment records the separate verification run.

## Assessment

The sync adds two specifications and extends four: 17 Block specifications now exist. It does not make the relay conformant with their new roles. The immediate new safety work is to reject reserved private aggregate kind `30179` and client-authored thread-bounds kind `39007`. Existing generic admission presently treats both as ordinary addressable events. FI and RS snapshots are absent; AP's new fields belong mainly to clients; CW needs a new query/validation path. PL has useful parsers and a configured stub delivery path, but its preexisting lifecycle and authorization gaps are larger than this sync's public-gateway changes.

| NIP | New upstream scope | Current status | First action |
| --- | --- | --- | --- |
| AP | Portable ACP aliases, `session_policy`, active copying of behavioral defaults, separate local versus catalog hashes. | Existing relay envelope/ACL support; no persona content codec/adoption/runtime implementation located. | Retain opaque relay behavior; specify and fixture the client projection and adoption rules. |
| CW | Newest-first thread windows, signed `39007`, bounded whole-batch processing and access refresh, deletion-summary recovery, documented legacy thread pagination. | Channel-window subset exists; new thread/recovery modes and `39007` rejection absent. | Reserve the new relay-only kind, then implement strict query modes and fixtures. |
| PL | Dogfood App Attest profile, recoverable enrollment, renewal and conflict semantics, corrected body limits/statuses, delivery path binding. | Generic lease parsing and HTTP stub delivery exist; public gateway protocol absent. Several baseline requirements are already unmet. | Correct support claims; close existing transaction/read-authorization/outbox gaps before public delivery. |
| RS | Optional atomic, Host/community-bound, own-author kind-30078 snapshot. | Addressable storage and ordinary history exist; snapshot descriptor, request parser, MVCC result and client validator absent. | Build the optional snapshot as an explicit role; do not infer completeness from ordinary queries. |
| FI | Optional issuer-qualified JWT identity + NIP-42/NIP-98 possession, community policy, deadlines, offline JWKS and issuer disconnect. | No FI implementation or discovery. Existing Nostr authentication is not FI. | Keep unadvertised; implement only with a concrete federated deployment and full protected-surface coverage. |
| PMA | Inert reservation for private managed-agent aggregate `30179`; future transactional migration. | No codec, rejection, privacy gate or CAS profile. | Reject all `30179` ingest now; do not implement ordinary LWW “support.” |

## NIP-AP

Source: `nips/block/NIP-AP.md:85` (fields), `:102` (behavior), `:110` (transport), `:277` (relay behavior).

New `acp_command` distinguishes the transport command from the harness/runtime. Foreign catalog values must be stock `buzz-acp` or a bounded `buzz-…-acp` alias; publication does not establish local availability. Shared writers omit machine-local commands and explicitly emit stock selection. Owner replay of a redacted shared head preserves an existing local nonportable command; explicit portable values replace it. Shared content remains plaintext. Local drift hashes cover the unredacted spawn-relevant definition, rather than the projected catalog bytes. `session_policy` is definition-authoritative and changes require linked-instance restart; respond-to defaults and parallelism copy at creation.

Already covered:

- `crates/nostr/src/domain/block.rs:75` validates the persona slug and shared-tag shape; `:91` handles team-catalog identifiers. Content is intentionally opaque, as permitted by the spec.
- `crates/nostr-relay/src/store/statements.rs:207` gates shared/own-author SQL visibility before limits. `gateway/subscription.rs:622` applies the matching live gate.
- `tests/fixtures/nipap/server.json` and `crates/nostr-relay/tests/block_fixtures.rs:37` test envelopes; `gateway_postgres.rs:1303` and `:1416` exercise persona ACL behavior.

Missing client behavior is not a relay regression. A whole-tree search of `crates/` found no `acp_command` or `session_policy` implementation. Add source-compatible persona parsing, canonical projection serialization, local-command preservation, alias resolution under admitted host bindings, restart marking, and separate local/catalog identities when a consumer is built. Do not turn a published command string into shell permission; CAP/POL/SESS remain the local execution boundary.

Needed fixtures: omitted/null/explicit stock; valid alias and 255-byte boundary; foreign absolute path rejection; owner replay preserving redacted local commands; explicit alias replacing local command; sharing-only hash stability; session-policy restart versus unchanged in-flight instance; behavioral defaults copied once; no secrets in projected content.

Upstream ambiguity to retain visibly: the field table still labels respond-to/parallelism “Reserved,” while the new prose says they are copied at creation. Do not silently amend the pinned upstream file or claim verified upstream runtime behavior from this prose.

## NIP-CW

Source: `nips/block/NIP-CW.md:173` (legacy mode), `:216` (thread mode), `:282` (budgets), `:301` (`39007`), `:384` (deletion-summary recovery).

Thread mode has a separate request grammar: one channel and root, 1–4 supported conversation kinds, bounded depth and row limit, paired descending cursor, no mixed modes/unknown fields, at most four windows per query. It verifies root identity/channel/community, includes eligible retained root tombstones, probes after all predicates, binds signed bounds to host, authenticated reader, channel, root, limit, depth, kinds, cursor and aux flag, and refreshes access for the complete batch before release. Query-wide row/byte/scan/time budgets include probes, tombstones and retries. Failure returns no partial page; an invalid signature or exhausted budget is not compatibility fallback. `resolve_thread_roots` recovers only owning-root summaries from retained metadata, without disclosing deleted target content.

Already covered:

- Channel-only parser/render/client validation: `crates/nostr/src/channel_window.rs:65`, `:116`, `:205`. Signed channel bounds and same-second cursors are real existing code.
- HTTP channel query: `crates/nostr-relay/src/gateway/query.rs:24`; `:18` caps its scan at 4,096. `window_events` at `:142` and `project_window` at `:215` use an authorized fetched event set.
- Existing relay-only list: `crates/nostr/src/domain/block.rs:26`; public WebSocket rejection: `gateway/server.rs:1029`.
- `block_lane_postgres.rs:124` tests real channel paging, summaries and aux; `tests/fixtures/nipcw/server.json` names only `39005`/`39006`; `block_fixtures.rs:251` tests ordinary WebSocket degradation.

Confirmed new gaps:

1. `39007` is not in the relay-only list or validators. `domain/block.rs:53` falls through to `profile::admit`; `crates/nostr/src/profile.rs:57` returns success for unrecognized kinds. Reserve/reject it at every public admission boundary, not just in a future client.
2. No `thread_window`, thread-bounds schema/binding verifier, shared batch budget, access-refresh barrier, or `resolve_thread_roots` code exists. The HTTP handler accepts a single filter object and has no batch mode. A thread request currently falls back to ordinary filtering, without valid `39007`; clients must treat that as unsupported.
3. No durable thread-root/depth metadata or soft-deleted target-to-root recovery path was found in the current store/migrations. Ordinary deletion tombstones are not sufficient retained thread metadata. A wire-tag reconstruction implementation would still need equivalent complete retained ancestry and deletion recovery.

Preexisting limits to avoid relabeling as new:

- Channel summaries/aux are derived from a bounded fetched slice, not a complete dedicated thread index. This can leave ancestry/summary completeness unproved beyond the scan. The scan refusal prevents one class of false pagination exhaustion, not all truncated-summary issues.
- A nonexistent group is treated as served (`store/mod.rs:1115`–`:1126`) and the live test explicitly expects empty bounds (`block_lane_postgres.rs:321`). The pinned old and new channel text says a nonexistent channel returns no bounds. Reconcile channel existence semantics rather than advertise complete CW conformance.
- The newly documented legacy oldest-first `depth_limit`/`thread_cursor` path is also absent; it is not an automatic fallback capability of this relay.

Needed fixtures: reject forged `39007`; strict mode/batch grammar; root in wrong channel/community, unsupported diff root, deleted eligible root; tied timestamps, depth and kinds; exact binding mismatch on every field; aux target in wrong tag position; simultaneous access revocation with no partial output; raw/output byte and row limits, deadline, corrupt row versus resource error; deleted reply summary recovery; clean legacy restart only on explicit unsupported mode; no fallback after authorization/signature/budget errors.

## NIP-RS

Source: `nips/block/NIP-RS.md:381`–`:470`.

The new snapshot is optional and explicitly discovered. It accepts exactly one raw filter in an array, for authenticated-self kind `30078`, with `read_state_snapshot: 1` and no extra selectors. Success returns a versioned envelope containing every retained current own-author event at one writer MVCC statement cut, including unrelated coordinates and missing `t` tags. It is bounded at 4,096 events and 8 MiB (stored and encoded measurements), fails whole on overflow or invalid reconstruction/signature, binds a Host-resolved community UUID and reader, and gives a domain-separated content hash. It is not a CAS revision or live cursor. Every override/publication/deletion decision needs fresh snapshots from every write relay; one unreadable recognized coordinate blocks it.

Confirmed new gaps:

- `gateway/query.rs:91` expects a single object, not the required raw filter array; `ordinary_events` at `:183` supplies a capped ordinary event array, not an atomic envelope.
- `gateway/wire.rs:399` has no `read_state_snapshot` descriptor; `GatewayConfig` at `config.rs:112` has one relay URL and no Host-to-community UUID mapping for this contract.
- `gateway/db.rs:781` samples high-water then executes queries; this is not the specified single writer-database MVCC statement snapshot with strict count/byte/signature guarantees.
- No snapshot codec, digest helper, client validation or RS merge/override client implementation was found.

Existing coverage is narrower: `block_lane.rs:373` only checks that `30078` is addressable. `block_fixtures.rs:269` checks addressable coordinate and general ingest. Generic live/history machinery exists; it is not a fixture-backed RS client or snapshot implementation.

The existing server doc's claim that the required cross-process full-state EOSE barrier is established should be removed or qualified. `server.rs:258`–`:329` processes durable notifications independently of `db.rs:793` history high-water sampling. `subscription.rs:395`–`:450` emits a query's EOSE without explicitly awaiting delivery of earlier commits to every other matching subscription on the connection. Static inspection exposes a delayed-notification/replacement-above-cursor race; no dedicated test proving the RS cross-subscription barrier was found. Do not call this a tested live failure: the audit did not run that reproduction. The new snapshot avoids needing those WebSocket preconditions only for its point-in-time load, not forever.

Needed fixtures: exact filter cardinality/unknown keys; absent discovery, ordinary-array fallback and false completeness; own-key versus virtual-owner identity; wrong Host/community; duplicate coordinates and bad signatures; unrelated application coordinates and absent `t`; exact digest/order vectors; 4,096/4,097 and both byte ceilings; concurrent replacement at the statement cut; writer versus replica behavior; one required write relay unavailable; unparseable recognized read state; no destructive carry-forward after incomplete load; delayed notify and final-continuation EOSE race if baseline full-load support is claimed.

## NIP-PL

Source delta: `nips/block/NIP-PL.md:266`–`:422`.

The public profile is now `buzz-ios-dogfood`, with application identity and APNs environment controlled by the gateway. Installation enrollment has a 23,896-byte ceiling (other routes 8,192); exact already-committed attested enrollment can recover its original success after an ambiguous response. Verified live key/token conflicts return 409, while expired/revoked ownership can recover. Delegation can extend installation lifetime. Challenge/delivery quotas return 429. Delivery uses the configured HTTP(S) `/v1/deliveries/apns` URL, with signature method/path/payload binding rather than equality with a single public origin. None of this permits a generic relaxation of NIP-98 URL validation elsewhere.

Existing useful code:

- `crates/nostr/src/push_lease.rs:126` decrypts; `:211` checks closed plaintext, origin, generation, profile, endpoint uniqueness inputs, quotas and narrowed filters; `:139` refuses unregistered transport constants.
- `domain/block.rs:214` checks public envelopes. The gateway requires authenticated author and refuses unconfigured executors (`server.rs:1140`–`:1168`). Author-only read ACLs exist.
- `tests/fixtures/nippl/server.json`, `block_fixtures.rs:161`, pure push tests and `block_lane_postgres.rs:366` prove a configured encrypted lease can trigger one constant-body request to a local stub. They do not prove public APNs conformance.

Confirmed preexisting implementation gaps (not introduced by this sync):

1. Lease authority is checked in `gateway/push.rs:24` by reading a capped list, then ordinary admission occurs separately (`server.rs:1165`, `:1179`, `:1534`). Endpoint uniqueness, generation watermark and event replacement do not share the required transaction. Racing addresses can both pass preflight.
2. The only prior-state source is unexpired stored events (`store/mod.rs:1136`–`:1152`, max 1,024 in `push.rs:17`). No separate durable watermark retention through expiry/tombstone recovery or complete indexed lease book exists.
3. Delivery matching calls envelope helpers only (`push.rs:179`, `push_lease.rs:346`), not current group membership or full relay membership checks. A channel lease can continue matching after membership loss. The event body remains constant, but unauthorized wake timing still violates match-time authorization.
4. Accepted publication spawns a non-durable task (`server.rs:1559`); no durable deduplicated jobs, origin-scoped outbox/retry state, crash recovery or current-generation recheck immediately before transport send exists.
5. `post_reconnect` (`push.rs:143`) sends raw constant JSON over `http://` with `X-Push-Endpoint`, no NIP-98 authorization, stable job UUID, endpoint_grant envelope, TLS public gateway support, or response handling. It returns after socket write. No installation/App Attest/delegation/custody service exists. Updating the configured profile string alone cannot make it conformant.
6. Key retirement is not implemented: one `current` executor key is advertised (`config.rs:30`); old accepted ciphertext can become unreadable after rotation.

New delta to include even in the base descriptor helper: `push_lease.rs:165` excludes old relay-only snapshot kinds but not new `39007` from eligible kinds. Current configured push kinds do not include it (`config.rs:37`), so there is no present default path; future descriptor validation must reject it.

Priority: describe configured PL as partial/prototype and do not recommend enabling it as a conformant service. Close transaction, authorization and durable-job gaps before integrating the public gateway. Needed tests: concurrent endpoint/generation races, expiry/tombstone replay after restart, >1,024 retained leases, membership removal between match/send, revocation during delivery, multi-process job deduplication, fixed-body noninterference, provider 410/429/503 semantics, unchanged job id with fresh auth on permitted retry, key retirement, exact App Attest replay recovery, enrollment conflict and renewal.

## NIP-FI

Source: `nips/block/NIP-FI.md:15`, `:152`, `:170`, `:262`, `:333`, `:580`, `:873`.

This entire profile is new to the pinned lane. It is optional and has no new event kind. It pairs `(iss, sub)` and an exact key assertion with fresh possession; it does not turn email or bare subject into identity. Host resolves community/audience/authorized issuers before any issuer dependency lookup. JWT class, asymmetric algorithm, exact issuer/audience/key, finite age and JWKS deadlines are pinned. FI requires a dedicated single header, offline authenticated JWKS with refresh/SSRF protections, bounded sessions without in-band renewal, and separately verified per-request HTTP admission. The issuer-scoped disconnect command has distinct JWT type and atomic replay/deny reservation; its close-versus-admission race, cross-process propagation and restart limits are explicit. Discovery must not promise unconditional upstream-revocation time or expose identity details.

Current code only supplies foundations: `gateway/auth.rs:18` stores NIP-42-authenticated keys/virtual owners; `:60` verifies AUTH. `server.rs:503` routes HTTP and upgrades directly to WebSocket without FI assertion/Host-community admission; `query.rs:55` verifies NIP-98 only. `config.rs:112` has no FI policy/registry/JWKS/deadline fields. Whole-tree searches found no FI/JWKS/assertion/disconnect implementation. `wire.rs:399` and `:458` correctly do not advertise FI. No FI fixtures exist.

Implement it as an explicit fail-closed deployment role, never as an inference from NIP-42 support. Reuse signature and HTTP primitives, but keep command versus assertion tokens distinct. All protected HTTP routes need the pair; validating only the WebSocket would leave the named bypass. Preserve the spec's narrow Git and Blossom exceptions only if those actual adapters are supported; do not weaken general HTTP auth for convenience. Upstream FI contains links and named implementation gaps for Buzz's own code; those are not OpenAgents implementation evidence.

Needed fixtures: every `FI-TRACE-*` oracle, issuer/community cross-product, unauthorized issuer checked before JWKS, header cardinality, all time equalities/overflow, token-class confusion, JWK add/remove/hard expiry, unavailable dependencies, session closure, terminal cancel-before-admission, deny capacity per issuer, max-until merge, replay atomicity, protected HTTP body binding, route-specific exceptions, cross-process revocation and restart behavior, exact private-state denial bytes, and no assertions/claims in logs/discovery.

## NIP-PMA

Source: `nips/block/NIP-PMA.md:3` and `:92` deployment order.

PMA is a codec/kind reservation, explicitly not an enabled migration. Its owner-authored aggregate contains encrypted private runnable configuration and preserved agent identity, binds exact signed public projections and recovery material, and will require transactional multi-record CAS, anti-resurrection, private reads, backups, revocation and verified dual-write migration. Generic NIP-33 latest-write-wins is explicitly insufficient.

Current generic admission accepts unrecognized `30179` (`domain/block.rs:53` → `profile.rs:57`); the private SQL kind lists omit it (`store/statements.rs:151`, `:333`), and live visibility defaults to true (`subscription.rs:595`–`:632`). No future CAS or private codec exists. Merely retaining ciphertext is not safe PMA adoption: it violates mandatory rejection and exposes metadata/ciphertext on ordinary public surfaces. This does not imply decryption of correctly encrypted contents.

Immediate smallest implementation: explicit fail-closed rejection of `30179` in every public write path, retaining no secret-bearing candidate. Independently audit compatibility import/restore paths and any already stored rows; they cannot silently expose unsupported private records. Do not introduce a fake codec/full NIP claim just to satisfy the file inventory. Later phases must follow the upstream order, preserving existing local identities/recovery and keeping catalog browsing independent of private decryption.

Fixtures: rejected valid-looking and malformed aggregates via WebSocket/public store; no historical/live/COUNT/id/search leakage if rows are restored/imported; no `nip-pma` support claim. Future codec/CAS phases need duplicate/unknown fields, outer/inner mismatches, nsec-derived identity, unconditional OA binding, exact projection recovery hashes/signatures, definition-less migration failure preserving old identity, racing generations/predecessors, tombstone anti-resurrection, atomic fanout and restore drills.

## Ledger, advertising and delivery order

`crates/nostr/src/block_lane.rs:9` retains the old commit; `:12` lists 15 files; `:63`–`:64` asserts old pin/count. Its `check_rs` is only an addressable-class test and `check_ap` only envelope validation. The sync creates a real known test mismatch. Do not merely update constants and call the new roles covered. Keep provenance/pin inventory tests separate from per-role support tests, including explicit unsupported/reserved behavior.

Before this documentation correction, `docs/protocol/block-nip-ledger.md:3` and `block-nips.md:3` claimed 15/adoption at the old pin. Update to 17 retained targets and a status matrix. `gateway/wire.rs:458` always emits `nip-rs`, emits `nip-cw` with signer+URL, and emits `nip-pl` with push config. FI/PMA are absent. State that RS currently means storage foundation, CW is channel-only, and PL is partial; an optional snapshot or thread mode needs its own proven behavior even if the coarse label is unchanged.

Recommended implementation sequence:

1. New-kind rejection for PMA `30179` and CW `39007`; regressions across public ingress/store/read/import boundaries. Correct support claims without changing pinned upstream prose.
2. Eliminate or disable advertised PL paths that lack live read authorization and transactional authority; retain prototype evidence separately.
3. RS atomic snapshot plus consumer validation, and a deliberate baseline EOSE proof if it is still claimed.
4. CW thread windows, batch resource/access law and deletion-summary recovery; finish existing channel completeness/existence gaps.
5. AP client projection/adoption and session-policy integration when a real consumer needs it.
6. FI deployment as a separately scoped project with JWT/JWKS, all ingress paths, session/disconnect races and privacy tests. PMA remains rejected until its complete staged migration prerequisites exist.

Documentation-only sync/assessment does not require the Rust gate. Any later behavior implementation needs the pinned toolchain, applicable fixture/Postgres tests, and `scripts/verify-rust.sh` under the repository contract. This assessment claims source inspection, not a fresh passing verification run.
