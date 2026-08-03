# Native Repository Work Claims

- Date: 2026-08-03
- Parent: [Omega #208](https://github.com/OpenAgentsInc/omega/issues/208)
- Implementation issue: [Omega #224](https://github.com/OpenAgentsInc/omega/issues/224)
- Contract owner: `@openagentsinc/all-work-contract`
- Runtime owner: `@openagentsinc/omega-effectd`

## Outcome

The All Work service now owns a persistent Work Packet and Repository Work
Claim ledger. The generated Effect and Rust contract exposes
`repository.claim.read` and `repository.claim.execute`. Omega must use these
methods through its supervised generated client. It must not call GitHub, a
relay, or a planning service to claim repository work.

The ledger is separate from Work assignment and execution. A claim does not
create an Assignee, Agent Delegate, Lease, process, Session, Receipt,
Verification, merge decision, Release Candidate, Release, or owner acceptance.

## Canonical records

A Work Packet names its canonical Work and repository, bounded scope, owned
paths, hot files, hot contracts, verification command, revision, state, and
times. A Repository Work Claim copies the collision-bearing scope and adds its
holder, claim generation, last evidence time, evidence references, state, and
explicit release facts. Audit entries record each admitted command and each
stale-takeover refusal.

The service persists the ledger in
`all-work/repository-claims.v1.json` under the injected application data root.
Writes use optimistic ledger revision and an atomic file replacement. Request
idempotency stores the canonical command digest. A retry with the same key and
bytes returns its first receipt. A retry with different bytes fails.

## Admission laws

- `repository.claim.execute` requires the generated v2 capability, an
  Effective Principal, and `capability:repository-claim:write`.
- A Work Packet must name canonical Work before the reference process admits
  it.
- A claim refuses before mutation when active work in the same repository has
  the same Work identity or overlapping owned paths, hot files, or hot
  contracts. The refusal names the conflicting claim and collision classes,
  but not its private scope.
- Generated artifacts, migrations, route tables, lockfiles, and shared schemas
  have explicit collision classes.
- A heartbeat, status, block, or release must use the active holder and exact
  claim generation. A late generation cannot revive released or superseded
  work.
- Elapsed time alone never permits takeover. At least 90 minutes without claim
  evidence and an explicit process/worktree audit that found no active work
  are both required. A failed takeover is retained in audit history.
- Release is explicit and requires evidence. It does not assert landing or
  verification.

## GitHub history and one-writer boundary

The bounded bootstrap recognizes old `CLAIM`, `CLAIM-STATUS`, and
`CLAIM-RELEASE` comments. It projects them as source-linked, canceled Work
Packets plus `historical_import` audit entries. It never creates a native
claim from them and never overwrites native state. Incomplete source pages set
an explicit claim-ledger gap. The native execute path has no GitHub write and
every receipt fixes `githubWriteCount` at zero, so a GitHub outage cannot block
native claim, status, heartbeat, block, or release.

Signed Nostr records are a later transport and audit projection. A signature
or relay acceptance cannot become canonical claim authority.

## Deferred verification

The implementation includes authored tests for the complete command journey,
collision classes, non-conflicting work, idempotency, 90-minute-plus-audit
takeover, stale-generation fencing, historical import gaps, persistence, and
the generated reference-process read/execute/restart path. Per the owner
request, execute these tests and the Omega build once at the end of the #208
series.
