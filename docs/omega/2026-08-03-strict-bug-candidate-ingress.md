# Strict bug candidate ingress

Date: 2026-08-03

Status: canonical contract and durable authority implemented; production
transport adapter and cutover remain pending.

## Purpose

The strict GitHub bug form is the only public issue-creation exception after
the internal Work cutover. Its reports are external input, not commands. Omega
must retain the report, show its trust state, and require an explicit triage
decision before it becomes canonical Work.

## Boundary

The All Work contract now defines `StrictBugCandidate`, its ledger, execute
receipts, and the negotiated `strict_bug.candidate.read` and
`strict_bug.candidate.execute` methods. The Effect authority persists the
ledger under the owner-local omega-effectd data root.

Ingress requires:

- an exact issue URL for `OpenAgentsInc/openagents` or `OpenAgentsInc/omega`;
- matching repository and issue identities;
- unique candidate, source, and GitHub delivery refs;
- an idempotency key derived from the GitHub delivery ref;
- a public-safe strict-form payload;
- all five required strict-form confirmations;
- a signature-verification evidence ref supplied by the transport adapter;
- the GitHub webhook ingress principal and its dedicated capability; and
- a structurally zero GitHub-write count.

Every accepted report starts as `untrusted` and `pending`. The authority
rejects secret-shaped values and private filesystem paths before persistence.

Triage is a separate owner-local command with optimistic candidate and ledger
revisions. `admitted`, `duplicate`, and `linked` decisions require a canonical
Work ref. `rejected` does not. A candidate-to-Work link is provenance only; it
does not grant assignment, claim, session, or command authority.

## Trust and deployment boundary

omega-effectd checks the typed evidence ref but does not perform the GitHub
webhook signature ceremony. A production transport adapter must verify the
delivery against its configured GitHub secret without exporting the secret or
raw payload, then call this boundary. Until that adapter is installed and its
journey is proven, this implementation is not a live-ingress claim.

The candidate ledger also does not activate the native Work writer. The
one-writer cutover remains in shadow until the separate reconciliation,
two-client, rollback, and policy/tool gates pass.

## Deferred execution evidence

Authored tests cover untrusted ingress, public-safe refusal, explicit triage,
zero-GitHub-write receipts, persistence, and restart recovery. Per the serial
omega#208 execution instruction, these tests and the aggregate build remain
deferred to the single final gate.
