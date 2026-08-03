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

## Remaining Packet E work

This slice defines and implements the OpenAgents-owned shared authority. It
also routes the separately negotiated method through the packaged
`omega-effectd` process and stores one private, atomic state record per
digest-addressed canonical Work identity. It does not claim that Omega consumes
`work.command.execute` yet. The next slice must pin the regenerated Rust
artifact in Omega, bind Work UI commands to the Effective Principal and
capability request, and prove the installed owner journey. Until then fixture
controls remain visibly simulated and omega#214 stays open.
