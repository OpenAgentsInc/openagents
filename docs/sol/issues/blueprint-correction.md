# P2 BM-CORRECT: Blueprint correction, deletion, provenance export, and privacy tripwire

## Priority posture

Dependency-held until #8640 Phase A closes the owner-local Sarah Fleet cutover,
unless the tripwire below fires. It must not distract the serial P0 fleet path.

## Outcome

From Sarah/Blueprint, an authenticated user can inspect what Sarah knows,
correct or delete a fact without silently rewriting history, understand where
it propagated, and export current facts with provenance and tombstones.

## Automatic tripwire

The first real, non-fixture conversation in which a user asks Sarah to correct
or delete remembered information must create a bounded owner-private
`blueprint_correction_requested` receipt and immediately reclassify this issue
for active privacy work. The same applies to a live privacy incident involving
an incorrect or over-broad Blueprint projection. Do not wait for a periodic
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

Sarah presents and requests; the owning data/projection services authorize and
apply. Model confidence is never deletion authority. Legal retention,
service-deliverable evidence, payment records, and promise receipts remain under
their own retention/authority rules and may return an explicit cannot-delete
reason instead of false success.

## Exit

One live owner-scoped fact is inspected, corrected, propagated, exported, and
then tombstoned through Sarah. Every transition is receipted and survives
reconnect; the old value disappears from current authorized projections while
remaining only where explicit history/retention policy permits, and a second
owner's state is unchanged.
