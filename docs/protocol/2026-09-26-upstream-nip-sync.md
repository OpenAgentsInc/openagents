# Upstream NIP sync and implementation assessment

Reviewed September 26, 2026. Both vendored lanes now match their upstream
heads at the revisions below. **The specifications are updated; the Rust
implementations have not changed.** The highest-priority implementation work
is private-data admission and visibility, followed by incompatible parsers
and incomplete relay features. Existing fixture results do not establish
conformance to the new pins.

The detailed reviews cover every changed specification, the implementation
paths, existing evidence, and the tests needed to finish:

- [Official NIP review](2026-09-26-upstream-nip-sync-official.md).
- [Block NIP review](2026-09-26-upstream-nip-sync-block.md).
- [Implementation plan](implementation-plan.md), including these upstream
  obligations alongside the OpenAgents contracts.

## Source identity and scope

| Lane | Previous pin | New pin | Retained files |
| --- | --- | --- | --- |
| Official | [`c53877571f96`](https://github.com/nostr-protocol/nips/tree/c53877571f96eb423661fc23c620d629d37b8f19), committed August 1 | [`b82211e96c6d`](https://github.com/nostr-protocol/nips/tree/b82211e96c6dad616ed2ea43034c1c621256b745), committed September 25 | 100 Markdown files: 99 specifications and the upstream index; previously 99 files |
| Block | [`8342dfcc5890`](https://github.com/block/buzz/tree/8342dfcc5890b81a269a8ec3db73a8a56f76ce79/docs/nips), committed August 4 | [`781d39510cf2`](https://github.com/block/buzz/tree/781d39510cf23cfe224e8f521ae06a23377e06de/docs/nips), committed September 26 | 17 specifications; previously 15. The additional local README is excluded from this count. |

The previous copies were synced August 5, about seven weeks before this
review. [`nips/manifest.json`](../../nips/manifest.json) records full revisions,
upstream tree URLs, file counts, and sync times: September 26 at 07:12 UTC.
The sync used the existing `scripts/sync-nips.sh`; the source-only commit is
[`393b876457`](https://github.com/OpenAgentsInc/openagents/commit/393b876457).

All 100 official and 17 Block files were independently checked against the
Git blob hashes in GitHub's recursive tree responses at those exact commits.
Neither response was truncated; no upstream Markdown file in the synchronized
directories was missing or extra. The repo-owned Block README is deliberately
separate. This establishes source identity, not implementation correctness.

Official changes affect 11 existing specifications: 01, 02, 22, 29, 30, 42,
43, 67, 78, 84, and 86. A3 is new; the official README also changes. Block
changes AP, CW, PL, and RS, and adds FI and PMA. Unchanged specifications were
checked for source identity and relevant dependencies; this is not a new
full conformance audit of every unchanged implementation.

## What needs to change

Priority here means implementation order, not a claim that a deployed service
was exploited. Findings come from repository source inspection. Live deployment
configuration and stored production records were not inspected.

| Priority | Surface | Finding | Required response |
| --- | --- | --- | --- |
| First | Block PMA, kind 30179 | New draft explicitly requires rejection until its privacy, CAS, recovery, and revocation gates exist. Generic admission currently has no such rejection or private-read rule. | Reject public writes; audit import/restore and stored-record visibility. Keep PMA unadvertised and inert until its complete migration prerequisites exist. |
| First | Official 78, kinds 78 and 30078 | Upstream now recommends authenticated admission/serving and author-only reads. Current history, live, COUNT, and reconciliation paths do not enforce owner visibility. | Apply one deliberate owner-access policy across every path, including existing rows. Global AUTH and search exclusion alone are insufficient. |
| First | Block CW, kind 39007 | New thread-bounds events must be relay-authored. Existing client rejection covers 39005/39006, not 39007. | Add explicit rejection at public ingress and review push-kind eligibility before implementing thread windows. |
| First | Source and conformance ledgers | Both code ledgers retain the old pins. Official A3 and Block FI/PMA have no inventory entries. | Separate current source inventory from verified behavior revisions and partial/unsupported roles. Do not advance a blanket conformance claim by changing hashes alone. |
| Next | Official 22 and 84 | Comments scoped to kind 1 are still rejected; highlights ignore structured `i` sources and reject text `r` sources now permitted upstream. | Update the existing parsers and admission fixtures without relaxing unrelated validation. |
| Next | Official 02 and 86 | The public petname helper emits the old dotted form. Allow/ban mutations leave opposite-list entries intact. | Adopt traversal-order slash paths; treat reverse resolution separately. Make opposite-list removal atomic for the existing management methods. |
| Next | Block PL | Configured delivery has pre-existing transaction, current-membership, durable-job, and gateway-protocol gaps. | Correct support claims now. Complete or disable the incomplete path before treating it as conformant mobile push; a new profile string does not fix it. |
| Next | Block RS and CW | RS's new atomic HTTP snapshot and CW's thread windows, batch limits, access refresh, and deletion-summary recovery are absent. | Implement explicit server/client roles and fault fixtures. Existing addressable storage or channel paging is not evidence for the new features. |
| Verify | Official 01 and 29 | Ordinary zero-history/live paths appear consistent, but the registration-to-history-cut race is unverified; 29 adds an example matching existing validation. | Add focused lifecycle evidence for 01; no behavior change identified for 29. |
| Product-dependent | Official 30, 42/67, 43/86, A3; Block AP and FI | Comment emoji, auth hints, relay-membership claims, payment targets, persona adoption, and federated identity need distinct consumers or deployment roles. | Implement only declared roles, with fixtures and accurate discovery. Their appearance in upstream is not proof that every relay must enable them. |

NIP-78's new privacy language is **SHOULD**, not a newly introduced universal
MUST. This repo's private workspace use makes following that recommendation
the appropriate implementation direction. Exact-author authentication for
publication is an additional local policy decision; author-only serving is
explicit upstream. Correct encryption does not excuse public metadata,
ciphertext, count, or identifier disclosure when an access contract forbids it.

Two important concerns predate this sync. PL already lacks atomic lease
authority, durable delivery state, current group-membership checks at wake
time, and the public gateway's authenticated delivery protocol. RS's documented
cross-process, cross-subscription EOSE barrier is not established by the
existing addressable-storage fixture. The review identifies a possible delayed
notification race; it has not reproduced a live failure. Both old claims are
narrowed in the [Block server contract](block-nips.md).

## Implications for Coder and the OpenAgents contracts

These updates strengthen the Nostr foundation without replacing our host and
application contracts:

- **Private state:** owner-gated NIP-78 storage is relevant to read positions
  and preferences. Keep application-specific mutable state separate from
  immutable RUN evidence, private artifacts, accepted orders, and execution
  authority. Previously public rows need an explicit migration assessment;
  new ACLs cannot undo prior disclosure.
- **Personas and sessions:** AP's portable ACP alias and session policy can
  feed CAP/SESS/EXT adapters. A published command names a potential local
  binding; it cannot authorize a shell command. Shared projections must not
  overwrite preserved local commands or expose private configuration.
- **Workspace clients:** CW thread pages and RS snapshots can support Coder
  views, but need their own authenticated completeness and revocation proofs.
  A timeline page or read marker cannot become a WS snapshot, RUN checkpoint,
  or task-control grant by implication.
- **Identity:** FI is an optional external-issuer identity layer. It pairs
  issuer-qualified identity with fresh Nostr key possession and community
  policy. NIP-42 alone does not implement it; FI alone grants no POL task or
  wallet authority. Coder does not need FI before a native Nostr workflow can
  work. PMA remains rejected rather than becoming a shortcut around private
  identity migration.
- **Markets and labor:** A3 can advertise payment targets, but an address is
  not an accepted MKT order, a validated invoice, permission to spend, or
  settlement evidence. It does not expand the current Bitcoin settlement
  profile into every network listed by upstream. Agent labor remains a
  priority independent of implementing a public push gateway or FI.

The official README adds index entries for 10040, 21059, 33534, and 38000
whose underlying specifications did not change in this sync. Do not interpret
index maintenance as four new runtime requirements or allocate duplicate
OpenAgents kinds for them.

## Implementation and acceptance sequence

1. **Fix admission and privacy first.** Reject PMA 30179 and forged CW 39007;
   apply NIP-78 visibility consistently. Include public writes, stored/imported
   records, historical and live queries, ID lookups, COUNT, search, and
   reconciliation. Test anonymous, unrelated authenticated, and exact-author
   callers, plus reconnect and revocation.
2. **Make the ledgers honest and current.** Record source revision separately
   from behavior evidence. Represent newly reserved/unsupported roles directly;
   do not manufacture a passing feature test from a kind-range assertion.
   Update the guard expectations only with reviewed per-role evidence.
3. **Repair supported behavior.** Update comments, highlights, petname display,
   and allow/ban transactions. Preserve regression coverage for unchanged
   valid and invalid inputs. Verify the NIP-01 zero-limit lifecycle over a
   real WebSocket and database.
4. **Complete the workspace and mobile roles.** Bound RS's atomic snapshot at
   a single writer-database cut and verify every response field. Implement CW
   thread windows with whole-batch limits and rechecked access. Resolve PL's
   existing authority/outbox gaps before public gateway integration. Advertise
   only the specific proven features.
5. **Add selected client/deployment features.** AP adoption, auth-aware
   retrieval, membership claims, emoji, A3 discovery, and FI each need a real
   consumer, exact scope, and fixtures. Follow PMA's staged prerequisites before
   lifting rejection; ordinary addressable replacement cannot substitute for
   its transactional migration.

Later Rust behavior changes require the pinned toolchain, a separate Cargo
target per worktree, the applicable Postgres tests, and the repository
[verification gate](../verification.md). The detailed appendices give the
feature-specific negative, race, replay, privacy, and recovery cases.

## Why NIP-LAB was not renamed

The requested exception applies: **NIP-LBR already exists in this repository's
public history.** It is not currently a numbered official NIP, but silently
reusing it would conflate different contracts.

- [Issue #4727](https://github.com/OpenAgentsInc/openagents/issues/4727) and
  [the June 10 introduction](https://github.com/OpenAgentsInc/openagents/commit/7c6c92c90311fc63ad864b2c3ce75aac88c54470)
  established agentic labor under that name.
- The [last retained LBR v1 specification](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/nips/LBR.md)
  uses NIP-90 kinds **5934/6934/7000**, with platform ledger/receipt authority
  and relay events carrying references. The issue's initial 5930/6930 proposal
  was not the landed coding-labor allocation.
- The [August 4 migration decision](https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/docs/nips/NIP90-MIGRATION.md)
  froze compatibility and proposed a future standalone LBR v2 without
  allocating its kinds. Deleting the old implementation and documentation
  does not erase the public protocol identity.
- Current [NIP-LAB](../../nips/openagents/NIP-LAB.md) defines
  `openagents.labor.v1` over private 3188 artifacts, MKT agreements, and CJ/RUN
  execution. It has different verification, acceptance, rework, resolution,
  and payment semantics. The document now records the historical distinction.

No LBR specification was found in either newly pinned upstream directory.
Public search found this repository's historical usage, with no independent
primary-source owner located; that search cannot prove global name absence.
LAB and its existing references remain unchanged. A deliberate future
successor rename would need explicit version and compatibility treatment.

## Source ambiguities retained

Synced files remain exact upstream copies, including ambiguities:

- NIP-43 removes the 28935 issuance definition but retains a dangling reference
  to that kind in its final implementation paragraph. Its new issuance path
  is NIP-86 `createclaim`; do not invent a return-token shape or revive the old
  API from that sentence alone.
- AP's field table still labels some behavioral defaults reserved while new
  prose says to copy them at creation. Resolve the client interpretation with
  explicit evidence before claiming interoperability.
- The [earlier cross-lane review](2026-09-26-openagents-gap-review.md#allocation-and-source-checks)
  records Block AO's encryption-description discrepancy and AE's profile
  size limit. Those unchanged files remain subject to official NIP-44
  algorithm precedence and each application's explicitly bounded payload.

## Verification and limits

The source sync and documentation correction are separate commits. Source
identity was verified against all 117 upstream Git blobs. Documentation
validation checks the edited local links, paths, and Markdown whitespace.
No product code, deployment, or protocol advertisement changes in this pass.

The focused command was run with Rust 1.97.1 and a separate target:

```sh
CARGO_TARGET_DIR=/tmp/openagents-nip-sync-target \
  cargo test -p nostr --lib every_pinned_ -- --nocapture
```

It compiled successfully, then **both tests failed** at the old-pin assertions:
`lane.rs:1412` and `block_lane.rs:63`. There were 0 passes, 2 failures, and
219 filtered tests; the command exited 101. The missing A3/FI/PMA inventory
entries are additional source-inspected mismatches behind those first
assertions. No implementation constant or fixture was changed to conceal
this result. The source refresh is therefore not a green conformance baseline.

The full Rust gate and live Postgres acceptance were not run in this
documentation-only pass. No current-pin conformance or production privacy
claim follows from this review.
