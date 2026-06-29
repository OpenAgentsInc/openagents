# Artanis/Pylon Proof Trace Checker

Date: 2026-06-07
Issue: #485

## Summary

OpenAgents product surface now has an evidence-only checker for the remaining Artanis/Pylon proof
gap. The checker is implemented in:

- `workers/api/src/artanis-pylon-proof-trace.ts`
- `workers/api/src/artanis-pylon-proof-trace.test.ts`

It classifies whether one Artanis assignment has a complete public-safe proof
chain across:

- Artanis dispatch evidence;
- Pylon accepted-work evidence;
- Pylon artifact/proof evidence;
- payment evidence;
- settlement evidence;
- Nexus/Pylon public receipt evidence; and
- real bitcoin movement with terminal settlement.

## Authority Boundary

The checker cannot:

- dispatch work;
- mutate Pylon records;
- create receipts;
- publish a Pylon release;
- create or settle payments;
- spend bitcoin.

It returns authority flags with every mutation and spend permission set to
`false`.

## Public-Safe Rules

The checker rejects or redacts refs containing raw payment, wallet, provider,
runner, customer, private trace, raw timestamp, or credential material. It does
not project raw invoices, preimages, payment hashes, mnemonics, exact wallet
balances, provider tokens, private workroom logs, customer data, or raw payout
destinations.

Public projections hide operator/private refs while operator projections can
retain public-safe operator diagnostic refs.

## Classification

The projection state is:

- `complete` only when the same assignment id is observed across dispatch,
  Pylon events, and the receipt, with accepted work, artifact/proof, payment,
  settlement, public receipt, real bitcoin movement, and terminal settlement
  all present;
- `partial` when the assignment id matches but required evidence is still
  missing;
- `blocked` when assignment ids mismatch or dispatch evidence is absent.

Simulation-only receipts do not satisfy real bitcoin movement.

## Remaining Work

This issue does not close the public Pylon v0.2 proof gap by itself. The next
issues in the same epic are:

- #486: added the operator proof-run API that uses this checker around the
  settlement bridge;
- #487: requires repeatable paid-work proof across multiple distinct Pylons in
  the release gate;
- #488: publish proof status through Artanis public/Forum surfaces.
