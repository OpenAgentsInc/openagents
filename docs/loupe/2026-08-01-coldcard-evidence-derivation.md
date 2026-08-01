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

The development fixture is
`fixtures/forensics/coldcard/evidence-derivation-fixture.v1.json`. Run its laws
with `pnpm --filter @openagentsinc/forensic-contract test` and type-check the
package with `pnpm --filter @openagentsinc/forensic-contract typecheck`.
