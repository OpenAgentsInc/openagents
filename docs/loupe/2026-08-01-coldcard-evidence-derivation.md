# Coldcard evidence derivation and claim history

OFR-017 adds the deterministic evidence-graph layer after historical program
fingerprinting. It operates on synthetic or authorized evidence bundles; it
does not enumerate keys, query wallets, or turn a public transaction pattern
into identity, intent, or unauthorized-taking evidence.

## Inputs and derivation

The typed input admits three hand-maintained seed classes: victim reports,
published addresses with retained publisher confidence, and program-
fingerprint scan candidates. Immutable chain transactions provide inputs,
outputs, exact satoshi values, classifications, block heights, timestamps, and
raw transaction digests. A configuration digest binds traversal depth,
explicit-change exclusion, dust threshold, temporal gap, and minimum
shared-input threshold.

All other graph material is derived. The edge and its human explanation are
created together by one versioned rule. A parallel record binds the edge,
explanation, rule, and exact derivation input by canonical SHA-256. When an
address or transaction appears through multiple sources, the node merges and
retains every source reference instead of replacing earlier provenance.

Program-pattern candidates remain `pattern_candidate`; victim-report edges are
`victim_confirmed`; chain facts are `observed`. Connected components describe
rule-derived transaction flow or shared input. Temporal episodes describe only
configured time proximity. Neither is identity evidence.

## Honest quantities and reconciliation

Address, transaction, UTXO, and victim-report counts are separate fields. The
contract deliberately reports victim count as unavailable because reports do
not establish deduplicated people. Unknown unreachable collectors and unpooled
operators are likewise unavailable, never numeric zero. Graph completeness is
a known floor even when the configured traversal bound is not reached.

Independent published figures carry their display value and an explicit lower
and upper bound representing stated precision. Reconciliation compares the
exact derived decimal within that interval and emits `MATCH`, `DRIFT`, or
`UNAVAILABLE`. It preserves both values and never substitutes prose or a quoted
total into derivation.

## Append-only claims

Claim-history events form a dense digest chain. The first event creates one
claim at one claim rung. Promotion must append evidence references and receipt
digests. Correction must identify affected projections. Every event binds its
reason, timestamp, evidence, projection set, and prior digest; mutation fails
schema validation. Claim-gate tests isolate program fingerprint, entity
component, unauthorized movement, and identity requirements so evidence from
one rung cannot satisfy another.

## The derived incident, and the figures it is checked against

The layer is exercised on a real incident, not only on a constructed one.
`fixtures/forensics/coldcard/evidence-derivation-incident-wave6.v1.json` derives
wave 6 of the Coldcard sweeps: blocks 960,359 and 960,367, extracted read-only
from our own archival node. Its seeds are the only two things a seed set is
allowed to be — thirteen addresses a third party published, at
`published_unconfirmed`, and thirteen fingerprint candidates from the frozen
OFR-016 bundles, carried with their bundle content digest and fingerprint
revision digest. Which transactions spent those addresses, where the value went,
how much moved, what groups with what, and when are all derived from chain
records.

The figures it reconciles against are not written by us. They are transcribed
from the postmortem author's own published dataset at a pinned commit and file
digest, and every reconciliation item must name that digest. The result:

| metric | derived | published | status |
| --- | --- | --- | --- |
| wave-6 sweep transactions | 13 | 13 | MATCH |
| wave-6 swept value | 32,629,041 sat | 32,629,041 sat | MATCH |
| wave-6 published addresses observed spending | 13 | 13 | MATCH |
| wave-6 spent UTXO count | not derivable | 13 | UNAVAILABLE |
| incident total swept | 32,629,041 sat (floor) | 115,964,784,686 sat | DRIFT |
| victim count | unavailable | not published | UNAVAILABLE |

Three of those are the interesting ones. The spent-UTXO count is `UNAVAILABLE`
because the typed chain record names addresses, not outpoints, so this graph
genuinely cannot derive it — the coincidence that both numbers are thirteen is
not permitted to become a match. The incident total is `DRIFT` because our
frozen window is nine blocks of a 341-block incident, so our figure can only be
a floor; a reconciler that called that a match would be laundering a bounded
traversal into a complete one. And the victim count stays unavailable on both
sides: the published dataset counts addresses, transactions, and UTXOs, and so
do we.

Two further properties fall out of the real data rather than being asserted.
Every wave-6 sweep spends exactly one input, so there is no shared-input
evidence anywhere in the wave, and the component rule refuses to pool the
thirteen victim addresses into common ownership. The two block times are about
104 minutes apart, past the configured one-hour gap, so they separate into two
temporal episodes — a timing claim that cannot satisfy an ownership or
unauthorized-movement gate.

## Sensitivity, measured rather than declared

Every configuration variant changes the configuration digest, so every variant
changes the graph digest. That made the original receipt report sensitivity to
every rule, including rules this evidence never exercises. The report now also
carries a structure digest over its nodes, edges, components, and episodes, and
a receipt records `structureMoved` per variant, checked against that digest.

On wave 6 the dust threshold, the shared-input threshold, and the episode gap
move the answer. The change rule and the traversal depth do not, because these
sweeps have no classified change output and nothing spends their destinations
inside the frozen window. Recording those two as insensitive is the honest
result, and the unspent destinations are exactly why unknown unreachable
collectors stay unmeasurable rather than zero.

## Claim history

The claim history is derived over the frozen OFR-016 output: a
`program_fingerprint` claim created from the frozen bundle scan, promoted with
the bundle content digest and the wide-scan ledger digest as appended evidence,
then corrected because the published claim that the size estimate always exceeds
the real signed size did not reproduce. The claim never leaves its rung, and an
attempt to promote it into `unauthorized_movement` fails. Reproducing a program
fingerprint is not a finding that a movement was unauthorized, however many
times it is promoted.

The constructed boundary fixture is
`fixtures/forensics/coldcard/evidence-derivation-fixture.v1.json`. It is
retained deliberately: dust exactly at the threshold, an explicit change output,
and a victim-confirmed report are boundary cases that real evidence does not
supply on demand. Measurement uses the incident fixture; boundary construction
uses that one. Run their laws with
`pnpm --filter @openagentsinc/forensic-contract test` and type-check the package
with `pnpm --filter @openagentsinc/forensic-contract typecheck`.
