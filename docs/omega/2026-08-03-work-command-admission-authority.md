# All Work command admission authority

Date: 2026-08-03

Status: canonical Effect slice and packaged OpenAgents host seam implemented;
Omega consumer and installed journey are not yet complete

Issue: [omega#214](https://github.com/OpenAgentsInc/omega/issues/214), Packet E

## Outcome

`@openagentsinc/all-work-contract` now defines the shared command envelope for
owned Work control. The generated Effect and Rust shapes include assignment,
delegation, revocation, agent-session start, activity, session control, diff,
review, evidence, verification, and Owner Disposition commands. The protocol
capability is `work.command.execute`.

The handwritten Effect authority is the admission point. GitHub remains a
read-only bootstrap source and receives zero writes from this processor. A
successful read, signature check, provider event, agent result, verification,
or relay acceptance does not independently grant command authority.

## Authority rules

Each request names the Work, Organization, Effective Principal, capability,
expected revision, intent, idempotency key, and occurrence time. Admission
fails closed when any boundary does not match the current state.

The authority preserves these separations:

- a human Assignee is not an Agent Delegate;
- a Delegation Grant names its issuer, generation, capabilities, tools, Host,
  budget, expiry, privacy class, evidence policy, and conditional Repository
  Work Claim and Lease pair;
- Thread, Session, Agent Session, Run, activity, provider event, loss, and
  effect references remain distinct;
- revocation removes the active delegate, marks matching Sessions revoked, and
  fences late commands by generation;
- diff, review, evidence, verification, and Owner Disposition references do not
  imply one another; and
- only the accountable human owner or human assignee can record an Owner
  Disposition.

Repository claim and lease references are nullable because All Work includes
non-repository domains. When one is present, both are required. The Repository
Work Claim authority remains the canonical collision and takeover authority;
the command authority does not recreate or weaken it.

## Receipts and replay

Every admitted command advances the Work revision, records Intent/Event/Receipt
references, and returns an exact digest-bound receipt with
`githubWriteCount: 0`. An identical idempotency replay returns its original
result. A changed replay, stale revision, stale generation, wrong Organization,
unauthorized principal, or missing capability fails closed.

The authority retains each admitted request as its typed audit record. This
keeps revoked grant terms, control bodies, disposition decisions, and their
verification references available after the active projection changes.

Provider event and explicit loss references are retained with activity facts.
An admitted activity can name an Effect, but the receipt does not infer an
Effect from agent completion text.

`WorkSnapshot` now optionally carries the complete portable execution
projection beside its compatibility reference arrays. Each Session row binds
Thread, Session, Agent Session, Run, Delegation Grant, Host, generation, and
active/paused/stopped/revoked lifecycle. Each Agent Activity row binds its
Session, Run, generation, portable kind and summary, provider event reference,
explicit loss references, and nullable Effect reference. Semantic validation
requires every projected identity to exist in the corresponding snapshot ref
set and rejects duplicate or zero-generation rows. Raw provider payload and
hidden reasoning do not enter either projection.

## Remaining Packet E work

The authority, packaged `omega-effectd` route, private atomic per-Work state,
restart recovery, and Omega command consumer now exist. The regenerated Rust
consumer and Omega inspector still need to expose the new Session and Agent
Activity projection rows. Explicit real Organization membership provisioning,
the installed owner journey, and its separate human disposition remain open
gates. Fixture controls remain visibly simulated and omega#214 stays open.
