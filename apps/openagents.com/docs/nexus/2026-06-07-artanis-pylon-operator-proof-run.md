# Artanis/Pylon Operator Proof-Run Route

Date: 2026-06-07
Issue: #486

## Summary

OpenAgents product surface now exposes an operator-only proof-run route:

```text
POST /api/operator/nexus-pylon/proof-runs
```

The route runs the Artanis/Pylon proof trace checker before and after the
existing settlement bridge. It is the repeatable operator workflow for turning
already-recorded Pylon assignment evidence into a public Nexus/Pylon receipt
and a pre/post proof-state report.

## Required Authority

The route requires:

- OpenAgents admin browser session or admin API token;
- `Idempotency-Key`;
- public-safe request refs.

It does not:

- spend bitcoin;
- create invoices;
- mutate Pylons;
- publish Pylon releases;
- expose raw payment material;
- bypass the lower-level settlement bridge evidence checks.

## Request Shape

The request includes:

- `assignmentRef`;
- `artanisRunRef`;
- optional `settlementIntentRef`;
- `amountSats`;
- optional `spendCapSats`;
- `payoutTargetApprovalRef`;
- `payoutTargetRef`;
- `policySnapshotRef`;
- optional `providerRef`;
- optional `pylonJobRef`;
- optional `buyerPaymentRef`;
- optional `redactedDestinationRef`;
- optional `adapterKind`.

The Pylon assignment event log must already contain accepted work,
artifact/proof refs, payment evidence refs, and settlement refs. If those are
missing, the route returns the pre/post trace states plus the bridge blocker.

## Response Shape

The response includes:

- `proofRunRef`;
- pre-bridge proof trace state;
- lower-level bridge status and response body;
- post-bridge proof trace state;
- public receipt URL when available;
- idempotency state.

Raw invoices, preimages, payment hashes, mnemonics, exact wallet balances,
provider credentials, private workroom refs, private file paths, raw logs, raw
timestamps, and customer data are rejected or omitted.

## Relationship To The Release Gate

This route can produce a single complete assignment proof. It still does not
make Pylon v0.2 releasable by itself. Issue #487 requires complete proof traces
across multiple distinct Pylons before stronger public claims are allowed.
