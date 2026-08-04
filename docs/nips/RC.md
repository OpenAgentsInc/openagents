> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.

NIP-RC
======

Repository Work Claims
----------------------

`draft` `optional`

This NIP defines the multi-agent collision ledger as signed events:
**Work Packets** naming a bounded repository scope, and **Repository Work
Claims** recording who currently holds that scope, with evidence
heartbeats, explicit release, audited takeover, and generation fencing.

What a claim is: coordination. What it is not: an Assignee, an Agent
Delegate, a Lease grant, merge authority, verification, or proof that
implementation is active or complete. The ledger exists so concurrent
agents do not collide on the same files and contracts — nothing more.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32300 | Addressable | Work Packet |
| 32301 | Addressable | Repository Work Claim |
| 32302 | Addressable (unique `d`) | Claim Audit Entry |
| 32303-32309 | — | Reserved for future NIP-RC use |

Authority-signed; mutations flow through NIP-WI (`claim.execute` with
`claim`, `heartbeat`, `status`, `block`, `release`, and `takeover`
arguments — mirroring the native `repository.claim.execute` boundary).

## 1. Work Packet (`kind:32300`)

The bounded scope definition. Address:

```text
32300:<authority_pubkey>:<packet_ref>
```

### 1.1 Required tags

- `d`: stable `packet_ref`
- `org`
- `work`: the canonical Work this packet decomposes (a packet without
  canonical Work is refused at admission)
- `repo`: the repository ref (NIP-34 announcement address or stable
  repository coordinate)
- `scope`: bounded scope statement ref or digest
- `state`: `planned`, `active`, `blocked`, `landed`, `superseded`, or
  `cancelled`
- `revision`, `published_at`

### 1.2 Recommended tags

- `path` with marker `owned`: repeated owned path prefixes
- `hot` with marker `file`: repeated hot files (lockfiles, route tables,
  migrations, generated catalogs)
- `hot` with marker `contract`: repeated hot contracts (shared schemas,
  protocol definitions)
- `verify`: the pinned verification command ref
- `a` with marker `claim`: the current active claim

Hot files and hot contracts carry explicit collision classes: generated
artifacts, migrations, route tables, lockfiles, and shared schemas
collide even when path prefixes do not overlap.

## 2. Repository Work Claim (`kind:32301`)

The live hold on a packet's scope. Address:

```text
32301:<authority_pubkey>:<claim_ref>
```

### 2.1 Required tags

- `d`: stable `claim_ref`
- `org`
- `a` with marker `packet`: the Work Packet
- `work` and `repo`: copied collision-bearing scope
- `p` with marker `holder`: the claiming principal
- `generation`: monotonic claim generation
- `state`: `active`, `released`, `superseded`, or `taken_over`
- `claimed_at` and `evidence_at`: claim time and last evidence time
- `revision`, `published_at`

### 2.2 Recommended tags

- `e`/`a` with marker `evidence`: heartbeat evidence refs (commits,
  NIP-EV receipts, activity refs)
- `released_at`, `p` with marker `releaser`, and `e` with marker
  `release_evidence` on release
- `reason` on supersession or takeover

## 3. Admission laws

These encode the implemented native ledger's rules:

1. **Refuse before collide.** A claim is refused when active work in the
   same repository shares the Work identity or overlaps owned paths, hot
   files, or hot contracts. The refusal names the conflicting claim and
   the collision classes — not the conflicting claim's private scope.
2. **Evidence or it is aging.** Heartbeats, status changes, and blocks
   must come from the active holder with the exact claim generation, and
   each updates `evidence_at` with an evidence ref. A late-generation
   command cannot revive released or superseded work.
3. **Takeover is earned, twice.** Elapsed time alone never permits
   takeover. Both are required: at least 90 minutes with no claim
   evidence, and an explicit process/worktree audit that found no active
   work — recorded as a Claim Audit Entry. A failed takeover attempt is
   retained in the audit history.
4. **Release is explicit.** Releasing publishes the claim with
   `state=released`, the releaser, and release evidence. Release asserts
   the hold ended; it does not assert landing, verification, or
   completion.
5. **History is append-visible.** Superseded and taken-over claims stay
   published; the packet's `claim` ref moves to the successor.

## 4. Claim Audit Entry (`kind:32302`)

The append-only audit record. Address:

```text
32302:<authority_pubkey>:<claim_ref>:aud:<n>
```

Required tags: `d`, `org`, `a` marker `claim`, `kind_label`
(`admitted`, `heartbeat`, `refused_conflict`, `takeover_attempted`,
`takeover_refused`, `takeover_completed`, `released`,
`historical_import`), `p` marker `actor`, `occurred_at`.

`historical_import` entries project legacy claim systems (for example old
issue-comment `CLAIM` ledgers) as source-linked, cancelled packets — they
never create a native claim and never overwrite native state, and
incomplete source pages are recorded as explicit ledger gaps.

## Security considerations

- **Claims are not locks on reality.** A process can write files without
  holding a claim. The ledger is honest coordination among cooperating
  actors plus an audit trail for the uncooperative case — enforcement
  lives in review and admission, not in the relay.
- **Scope disclosure.** Owned paths and hot files reveal repository
  structure. Private repositories keep packets and claims on restricted
  relays; refusals name collision classes, not foreign scopes.
- **Generation replay.** Every mutating command carries the claim
  generation; stale generations refuse. A holder key compromise is
  bounded by the grant and the audit trail.
- **Starvation and squatting.** The two-condition takeover rule protects
  active workers from clock-based eviction while preventing indefinite
  squatting on evidence-less claims.

## References

- NIP-01, NIP-34
- NIP-WK, NIP-WI (layer 0)
- NIP-AD (this layer) — grants carry their claim/lease pair
- NIP-CC (this layer) — coding sessions bind packets to worktrees
- `docs/omega/2026-08-03-native-repository-work-claims.md`
- `docs/sol/CLAIM_PROTOCOL.md` — the operational protocol this encodes

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Work Packet, Repository Work Claim, audit
  entries, collision classes, and the evidence/takeover/release laws.
