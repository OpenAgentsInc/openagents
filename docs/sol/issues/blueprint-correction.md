# WONTDO: Blueprint correction/deletion product backlog

- Issue: #8642
- Resolution: closed not-planned on 2026-07-10; label `wontfix`

## Priority posture

This speculative product lane is closed. A real correction/deletion/privacy/
data-integrity request or incident creates a new bounded P0 privacy issue
immediately; it does not reactivate this broad backlog. The presentation surface
may be direct Desktop/mobile software; Sarah is not required.

## Outcome

From OpenAgents/Blueprint, an authenticated user can inspect retained facts,
correct or delete a fact without silently rewriting history, understand where
it propagated, and export current facts with provenance and tombstones.

## Automatic tripwire

The first real, non-fixture conversation in which a user asks OpenAgents to
correct or delete remembered information must create a bounded owner-private
`blueprint_correction_requested` receipt and immediately open a new scoped
privacy issue. The same applies to a live privacy incident involving an
incorrect or over-broad Blueprint projection. Do not wait for a periodic
roadmap review to notice either event.

## Scope

1. Inspect a fact's value, source, relationship/owner scope, derivation state,
   revisions, and authorized downstream projections.
2. Correct by writing a new provenance-bearing revision; preserve the prior
   revision as history rather than mutating it invisibly.
3. Delete through a scoped tombstone and propagation/rebuild contract. Never
   treat hiding one UI node as deletion.
4. Propagate revisions/tombstones through Khala Sync and authorized read models,
   with idempotency and retry receipts.
5. Export current facts, provenance, revision history, tombstone ledger, and
   could-not-delete residuals in a portable owner-private format.
6. Prove another prospect/owner is unaffected and cannot observe the action.
7. Render a one-minute-readable action receipt while retaining exact private
   audit evidence underneath.

## Authority and safety

Desktop/mobile present and request; the owning data/projection services authorize and
apply. Model confidence is never deletion authority. Legal retention,
service-deliverable evidence, payment records, and promise receipts remain under
their own retention/authority rules and may return an explicit cannot-delete
reason instead of false success.

## Exit

One live owner-scoped fact is inspected, corrected, propagated, exported, and
then tombstoned through the authenticated product surface. Every transition is receipted and survives
reconnect; the old value disappears from current authorized projections while
remaining only where explicit history/retention policy permits, and a second
owner's state is unchanged.
