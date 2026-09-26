# Official NIP implementation review, September 26, 2026

This is the detailed official-lane appendix to the
[upstream sync assessment](2026-09-26-upstream-nip-sync.md). The comparison is
`c53877571f96eb423661fc23c620d629d37b8f19` to
`b82211e96c6dad616ed2ea43034c1c621256b745`. Code references and line numbers
refer to OpenAgents `393b876457`, whose Rust code is unchanged from its parent.
The review inspected source and existing fixtures; it did not exercise a
live deployment. References to prior documentation wording use the pre-correction snapshot.
The parent assessment records the separate verification run.

## Assessment

The most urgent implementation work is NIP-78 owner privacy across every relay read path. NIP-22 and NIP-84 contain concrete parser/admission incompatibilities with the new source. NIP-02's public petname helper still emits the superseded dotted format. Existing NIP-86 allow/ban mutations do not implement the new mutual-exclusion recommendation. The new NIP-42/67 auth hint, NIP-43/86 membership claims, NIP-30 comment emoji rendering, and NIP-A3 payment discovery are optional feature work, not proof that every relay must implement them.

The source sync itself does not fix any of these behaviors. Current fixture and ledger claims describe the previous pin. In particular, the unchanged `OFFICIAL_COMMIT` at `crates/nostr/src/lane.rs:14` is the previous SHA, and `covered_files()` at line 1172 does not include A3. The guard at lines 1409–1421 first checks the pinned commit, then exact file coverage; source inspection predicts failure at the commit assertion, and an additional missing-A3 failure after changing only the constant. The parent assessment records the focused guard test separately. This mismatch marks the source/implementation review boundary. Do not change the constant and call the entire new pin proven. Add explicit supported/partial/unsupported evidence and update affected fixtures before advancing the implementation baseline.

The closing claim in the pre-review `docs/protocol/official-nip-ledger.md:982` that every event-shaped official file was configured-and-proven is qualified in the accompanying ledger correction. It also must not imply that the shallow NIP-43 role test establishes a functioning membership service.

## Per-file findings

| Source | Change in this sync | Current implementation | Classification and priority |
| --- | --- | --- | --- |
| `01.md` | Explicit zero-limit history and continued-live subscription requirements. | Query truncation retains zero; live filtering ignores limit; EOSE follows completed history. | Appears covered by code. Add focused regression evidence; P2 verification. |
| `02.md` | Forward slash-separated petname paths, reverse resolution, absolute roots, ASCII eligibility for resolution. | Helper emits reverse dotted paths; no path resolver was found in the relevant crates. | Existing helper needs semantic update; resolver is new optional client behavior. P2. |
| `22.md` | Removes the prohibition on comments replying to kind 1. | `nostr_kind` expressly rejects 1, and admission calls that parser. | Concrete semantic incompatibility. P1/P2, depending on comment traffic. |
| `29.md` | Adds a `previous` tag example with three eight-character ID prefixes. | Domain checks prefixes and store checks recent references. | Example-only; no implementation change identified. |
| `30.md` | Adds custom emoji in kind 1111 content, with example. | Tags/content survive event storage; shortcode/reaction helpers are not a general comment renderer. | Optional client feature and evidence update. P3. |
| `42.md` | Optional EOSE `auth` hint with mandatory preceding challenge if emitted. | AUTH challenge is issued at connection setup when configured; EOSE emits only finish/more. | New optional feature. P2 if implementing auth-aware retrieval. |
| `43.md` | Replaces kind 28935 invite creation with NIP-86 `createclaim`. | No relay-wide claims/join/leave service; only shallow role-event check. | New optional capability on top of a pre-existing incomplete area. P3 unless membership product requires it. |
| `67.md` | Adds `auth` hint and combined-hint example. | Parser ignores unknown hints correctly, but exposes no Auth variant. Relay emits finish/more only. | Wire-compatible fallback, new optional interpretation/emission. P2. |
| `78.md` | Adds AUTH/owner-only serving recommendations; discourages public inter-app reuse. | Both kinds lack owner read ACLs; 30078 excluded from search, 78 searchable. | Privacy policy gap made explicit by this sync. P1. |
| `84.md` | Adds structured `i` sources, permits free-text `r` sources, removes tracker-cleaning recommendation, weakens quote display MUST to SHOULD. | Parser ignores `i`, rejects non-HTTP `r`, and requires a source. | Concrete new-source incompatibility; some overstrictness predates sync. P2. |
| `86.md` | New claims/event-list methods and mutual-exclusion semantics for allow/ban. | Limited method subset; allow/ban do not remove opposite entries. | Existing method semantic update plus optional new methods. P2. |
| `A3.md` | New kind 10133 payment-target declaration using `payto` tags. | Generic replaceable storage works by range, no typed target parser/renderer. | New optional discovery feature, missing ledger coverage. P3 feature; P1 baseline honesty. |
| `README.md` | Registers A3/10133; adds existing-kind table entries and fixes links. | No behavior is supplied by the table. | Registry/documentation updates only. |

### NIP-01: zero means no stored events, not an ended subscription

The new normative paragraph is `nips/official/01.md:151`. For a mixed-filter REQ, zero applies to that filter; other filters may still return history. EOSE waits for all initial queries, and the subscription remains live.

The ordinary zero-limit and later-live paths appear compatible, but the registration-to-history-cut race remains unverified:

- `crates/nostr-relay/src/gateway/server.rs:1937` preserves zero while clamping filter limits; it does not turn zero into the default.
- `crates/nostr-relay/src/gateway/db.rs:781` (`query_history`) computes `want` per filter, probes one extra row, then truncates to `want` at lines 819–821. Zero therefore contributes no stored events.
- `crates/nostr/src/domain/filter.rs:70` (`matches`) does not apply `limit` to live matching.
- `crates/nostr-relay/src/gateway/subscription.rs:395` (`history_ready`) completes the history/live handoff, emits EOSE at 444–446, and drains buffered new events after it. It does not close the subscription.

Verification needed: a Postgres/WebSocket regression with (1) only limit zero, (2) zero plus a positive-limit filter, (3) a new event between registration and the history high-water mark, and (4) a new event after EOSE. Assert no history from the zero filter, exactly one EOSE after all initial queries, and later matching delivery. `tests/fixtures/nip01/fuzz-corpus.json` already contains a limit-zero filter, but that parser input is not end-to-end proof of this lifecycle. Existing handoff tests at `subscription.rs:751` and 828 are useful foundations. A specific source-inspected concern is that live buffering begins before the history high-water sample, and `history_ready` drops buffered persisted events at or below that mark even when a zero-limit query returned no history. An arrival in that interval might therefore be omitted from both paths, depending on the intended cut. This has not been reproduced; the race case above must resolve it before claiming full lifecycle conformance.

### NIP-02: petname path semantics

`nips/official/02.md:48` now describes traversal-order `~/erin/charlie` rather than the previous reverse dotted display. It adds name-to-key resolution component by component, roots such as `~npub1...` or `~carol@names.com`, and ASCII letters/numbers/underscore eligibility for resolution.

`crates/nostr/src/domain/follow.rs:80` documents the old scheme, and line 118 constructs `format!("{}.{}", follow.petname, suffix)`. Its direct-name behavior at line 99 still agrees with upstream. `parse_follow_list` at line 28 retains arbitrary petname text; that should remain distinct from whether a name qualifies for path resolution. The new restriction is not permission to reject an entire otherwise valid follow list or hide a non-resolvable display label. The pre-review `docs/protocol/official-nip-ledger.md:9` called the old chain configured-and-proven; the corrected ledger now labels that evidence as previous-pin behavior.

Verification needed: forward-order two/three-hop output, direct label, shortest-path tie behavior, cycles, missing lists, duplicate/ambiguous component names, invalid path characters, and explicit root binding. Absolute NIP-05 roots require the existing verified identity machinery or an explicit unresolved result; the pure protocol crate must not silently invent network resolution. Update `follow.rs:205` and `lane::tests::nip02_follow_lists_replace_and_petnames_chain`. Do not claim a renderer-only change implements reverse resolution.

### NIP-22: permit valid kind-1 scopes

The only spec change removes the blanket kind-1 prohibition. `crates/nostr/src/domain/comment.rs:101` parses kind values, then explicitly errors for 1 at lines 105–107. This affects both root and parent scopes. `crates/nostr/src/domain/expanded.rs:705` calls `open_comment` during event validation, so this is an admission incompatibility, not just an old comment in source. `docs/protocol/official-nip-ledger.md:66` and the test named at `comment.rs:376` assert the old refusal.

The targeted change should remove only the obsolete prohibition while preserving required root/parent tags, author binding, addressable references, and external-source validation. Add valid kind-1 top-level/reply fixtures, malformed-kind-1 negative cases, and a relay admission round trip. No evidence supports rewriting existing NIP-10 notes or changing their threading behavior as part of this sync.

### NIP-29: example-only update

The new `previous` example shows several eight-character lowercase event-ID prefixes in one tag. `crates/nostr/src/domain/expanded.rs:681` already iterates every argument and validates that form. `crates/nostr-relay/src/store/mod.rs:2331` checks every supplied reference against the recent group prefix set. Existing behavior is consistent. A fixture mirroring the new example would improve source alignment, but it is not a discovered behavior gap. Do not confuse this group-scoped invite/history mechanism with NIP-43 relay membership.

### NIP-30: emoji on comments

`nips/official/30.md:9` adds kind 1111, and lines 59–72 describe rendering its content. Current reusable code has `crates/nostr/src/lane.rs:121` for shortcode syntax, custom-reaction parsing in `domain/reaction.rs:103`, and emoji-list shapes in `domain/lists.rs`. These do not constitute a general comment emoji renderer. `Comment.content` and raw event tags are retained; adding emoji does not itself cause admission refusal through the comment parser.

This is optional presentation support. Test preservation of content/tags in a valid kind-1111 event and, when implemented, token substitution scoped to that event's emoji tags, unrecognized shortcodes, optional set addresses, and bounded external media fetching. The added upstream illustrative event omits normal NIP-22 scope tags; use a complete valid comment when writing an admission fixture rather than copying that abbreviated example as a conformant full event. The general lack of image emoji rendering predates this update.

### NIP-42 and NIP-67: authentication hint

`nips/official/67.md:33` now recognizes `auth`; line 37 requires the AUTH challenge before any EOSE carrying it. NIP-42 adds the same optional behavior. Multiple hints may coexist; the new example includes `["auth", "finish"]`, so finish should not be interpreted as proof that authentication can reveal nothing else.

Current wire compatibility is safe but incomplete as a feature: `crates/nostr/src/domain/eose.rs:21` has only Finish/More; `open_eose` at line 70 ignores unknown strings at line 97 as required. `Eose::complete` at line 44 answers finish/more completeness, not whether authentication can reveal another authorized view. `gateway/wire.rs:372` emits finish or more only. `gateway/server.rs:576` queues a challenge on configured connections before ordinary request handling. `gateway/config.rs:528` advertises 67, and 42 is conditional on relay URL configuration.

If implemented, add an Auth variant or separate accessor, preserve unknown-hint tolerance, and let clients distinguish current-view exhaustion from an authentication opportunity. The relay should emit auth only under a declared policy; it must not reveal unauthorized event counts/content while deciding whether to hint. Verify auth-only, finish+auth, more+auth, unknown values, legacy two-element EOSE, challenge ordering, and an authenticated re-query. Emitting the hint is optional, so absence is not itself a conformance bug. The corrected NIP-67 ledger entry states that the implementation supports the old two hints and ignores the new one until feature work lands. Existing created_at boundary-tie limitations were already documented and are unrelated to this sync.

### NIP-43: invite issuance moved to management

The sync removes the ephemeral kind-28935 invite creation flow and directs clients to NIP-86 `createclaim`. Existing code search finds no relay-wide 28934 join, 28935 issue, or 28936 leave handlers. `crates/nostr/src/lane.rs:1560` merely signs a kind-33534 role-shaped event and checks the spec still mentions that kind. It does not validate relay self-key authority or execute a membership lifecycle. NIP-43 is absent from `gateway/config.rs:527`'s advertised NIP list. Group invitations at `migrations/0002_nip_expansion.sql:43` and `store/mod.rs:2316` are NIP-29, not NIP-43.

A feature would need authenticated, permission-limited management claims, durable redemption/revocation policy, bounded freshness checks, idempotent already-member results, relay-signed membership projections, protected publication, and join/leave integration with actual admission. Test expired/invalid/replayed/revoked claims and restart behavior. These are largely pre-existing missing membership features, now with a different issuance API; do not present them as newly broken shipping code.

Upstream inconsistency: the final Implementation paragraph in the synced `43.md` still tells clients to request kind 28935, despite deleting its definition above. Preserve the vendor copy and record this ambiguity. Do not recreate the removed API based solely on that dangling sentence. The new `createclaim` accepts a claim argument and returns true; do not invent a server-generated returned token shape.

### NIP-78: owner privacy needs an end-to-end policy

`nips/official/78.md:13` discourages using the kinds for public or cross-application interchange. Line 23 recommends AUTH before accepting or serving kinds 78 and 30078 and author-only serving. These are SHOULD rules. Same-author authentication on publication is a reasonable additional local policy, but the new text explicitly binds owner identity on reads; it must not be misquoted as a new upstream MUST for writes.

Current implementation only supplies opaque payloads and storage class:

- `crates/nostr/src/domain/app_data.rs:48` handles normal/addressable shapes.
- `crates/nostr-relay/src/gateway/server.rs:998` has optional global authentication, but `nostr::profile::requires_author_auth` at `crates/nostr/src/profile.rs:33` handles OpenAgents profile kinds and does not cover either app-data kind.
- `server.rs:1805` (`owner_scoped_filter_denial`) has private-kind checks for REQ/COUNT, but not 78/30078.
- `crates/nostr-relay/src/store/statements.rs:148` and 333 allow these kinds through the public branches of both history and COUNT ACLs.
- `crates/nostr-relay/src/gateway/subscription.rs:595` (`event_visible_to_reader`) falls through to true for both kinds.
- `server.rs:749` (`handle_neg_open`) supplies authenticated read keys to the shared history path; current SQL also exposes these records' IDs to unauthenticated reconciliation.
- `crates/nostr/src/domain/filter.rs:222`, `store/statements.rs:237`, and `migrations/0008_gift_wrap_search_privacy.sql:9` exclude 30078 from search, but not 78. Excluding search is not owner authorization.

Thus an author filter, an event-ID filter, a broad subscription, COUNT, or reconciliation can still expose app data under current public relay configuration. Global AUTH alone is insufficient: an unrelated authenticated account must not gain owner access. Existing Block consumers of 30078 need review against the same policy. Any deployment migration should explicitly account for previously publicly retained records rather than claiming new ACLs undo prior disclosure.

Implement shared owner visibility across stored/live/COUNT/search/reconciliation paths, plus explicit write admission and an auth-required refusal strategy when AUTH is unavailable. Decide whether both kinds should be search-excluded as a privacy policy; at minimum, any permitted owner search must enforce the same ACL. Tests need unauthenticated, unrelated authenticated, matching owner, ID-only/broad/multi-filter queries, count, live delivery, negentropy, reconnect, replacement, and legacy stored rows. Use exact authenticated identities, not merely a label identifying an account or an agent claiming to act for an owner, unless a separate explicit delegation policy is specified.

The pre-review `docs/protocol/official-nip-ledger.md:406` said kind 78 remains searchable and called the row configured-and-proven. The corrected ledger narrows that to previous-pin shape/replacement evidence until privacy behavior is tested. The unprotected implementation existed before the sync; the sync newly makes the privacy expectation explicit.

### NIP-84: source model expansion

`nips/official/84.md:17` now allows structured NIP-73 `i` sources and arbitrary URL/text `r` sources. Quote rendering at line 45 becomes SHOULD rather than MUST. URL tracker removal is no longer recommended by the source. The quote-highlight `r` source/mention marker rule at line 51 remains; do not drop it indiscriminately.

`crates/nostr/src/domain/highlight.rs:29` has Address, Event, and Url source variants. `open_highlight` at line 176 ignores `i` tags, and lines 214–229 require exactly three fields and HTTP(S) for `r`. An otherwise valid i-only highlight therefore ends in the empty-source error at lines 267–268; text r values error immediately. `crates/nostr/src/domain/external_id.rs:22` and 120 provide the existing NIP-73 primitives to reuse. `clean_source_url` at `highlight.rs:140` can remain an optional local convenience; documentation must stop presenting it as necessary conformance.

Verification needed: i-only sources, supported/unknown structured IDs according to NIP-73's actual rules, non-URL text r, mixed sources, existing a/e, quote source versus mention URLs, empty media highlights, and round-trip admission. Source omission is SHOULD in the old and new source, while the current parser requires it: that stricter relay policy predates this sync and should be separately assessed, not mislabeled a new upstream change. Marker strictness for non-quote r is similarly worth reviewing separately. Update `docs/protocol/official-nip-ledger.md:216` and `highlight.rs:324` evidence.

### NIP-86: existing mutation semantics and new optional methods

The sync describes the old method list in more detail and adds `listclaims`, `createclaim`, `deleteclaim`, `unallowevent`, `unbanevent`, `listallowedevents`, and `listdisallowedkinds`. It recommends that adding to a pubkey/event ban list removes that entity from the allow list, and vice versa. Removing a ban should not implicitly grant access; removing an allow entry should not implicitly ban it. `supportedmethods` may be tailored to the authenticated principal.

The existing HTTP surface is owner-authenticated using NIP-98 with payload binding (`crates/nostr-relay/src/gateway/management.rs:33`). Its declared subset at line 139 is the pubkey list methods, allow/disallow/list kinds, and configured NIP-29 group methods. `parse_command` at line 163 matches that subset. `crates/nostr/src/lane.rs:185` exposes a still narrower official method-name helper. New optional methods are unsupported; existing role/event moderation/IP/metadata methods were already unsupported before this sync. The spec says methods may be supported, so this is not a demand to implement every method just to keep 86 listed.

There is an actionable update to already implemented methods: `store/mod.rs:1849` and 1872 execute only their own insertion. `store/statements.rs:558` and 565 insert into the blocked or allowed table without removing the opposite entry. Both lists can therefore retain the same key. For the new recommendation, remove the opposite entry in the same transaction. `unbanpubkey` at 562 and `unallowpubkey` at 569 already delete only and meet the no-implicit-opposite-action guidance.

Verification needed: allow→ban and ban→allow remove opposite entries atomically, unban/unallow do not grant/ban, duplicate authorization is refused, requests survive restarts without contradictory state, and concurrent mutations have a defined final ordering. Current `gateway_postgres.rs:1133` checks management discovery, group creation, ban/list/unban only; it does not prove the cross-list transitions. Extend `tests/fixtures/nip86/management.json` only for methods actually implemented. If adding listdisallowedkinds, define whether the system retains explicit disallow entries or reports the complement of an allowlist; current disallow is merely DELETE from `relay_allowed_kind` (`statements.rs:575`), with no separately retained disallowed-kind table.

### NIP-A3: payment targets are discovery, not spending authority

The new optional draft defines kind 10133 with three-element `payto` tags: literal tag name, lowercase payment type, address. Recognized types may have network validation; unknown types are allowed and may render using `payto://`. `crates/nostr/src/domain/replacement.rs:18` already classifies 10133 as replaceable, so generic signed-event storage/replacement requires no special kind-range change. No typed target parser, target renderer, or A3-specific fixture was found in the inspected protocol/product crates. That distinction matters: storing arbitrary tags is not a proven payment-target client.

Add an explicit ledger status now; do not claim support from the event range alone. If a product needs this discovery surface, build typed extraction with exact tag/lowercase rules, lossless unknown types, safe URI generation/escaping, and replacement tests. Opening a declared URI must remain user-mediated and separate from wallet authority, accepted market terms, invoice validation, payment idempotency, and settlement evidence. This does not broaden OpenAgents' Bitcoin-first settlement profile into support for every network named by A3.

### Official README

The changed registry adds A3 and kind 10133. Entries for 10040 (NIP-85), 21059 (NIP-59), 33534 (NIP-43), and 38000 (NIP-87) catch up with existing unchanged specifications; they are not new wire changes by themselves. Link fixes include the acceptance heading and Marmot references. No runtime updates should be inferred solely from the kind table.

## Recommended implementation order

1. Publish this source/implementation distinction and correct the blanket ledger wording. Retain the expected guard mismatch until an explicitly reviewed implementation baseline lands; do not use a pin-only edit as conformance evidence.
2. Close the NIP-78 privacy policy end to end, coordinated with Block's app-data consumers. Gate it on real database/WebSocket tests across all read surfaces.
3. Update the obsolete NIP-22 rejection and expand NIP-84 sources. Update NIP-02's existing public helper; scope absolute/reverse resolution separately if not required by a current consumer.
4. Fix NIP-86's existing allow/ban transition semantics; add focused concurrent/transactional evidence.
5. Add zero-limit subscription regression evidence. Keep NIP-29's example change classified as already covered.
6. Decide which optional product features justify work: auth-aware retrieval first where private views need it; NIP-43 claims only with a real relay membership product; comment emoji and A3 discovery when a consuming UI exists. Do not expand capability claims just because upstream documents a feature.

For future Rust behavior changes, use the pinned toolchain, a separate worktree target directory, the applicable Postgres fixture setup, and the repository's `./scripts/verify-rust.sh` gate. This review does not assert those checks passed.
