# oa-node Managed-Machine Contract

Date: 2026-06-06

Status: implemented contract note for issue #335 / `OPENAGENTS-RUST-002`.

## Purpose

OpenAgents product surface now has a schema-first `oa-node` managed-machine contract.

The implementation lives in
`workers/api/src/oa-node-managed-machine.ts`.

This is a contract and projection layer only. It does not connect to an SHC
machine, launch a daemon, start a workroom, expose host telemetry, spend
bitcoin, or make a provider payout claim.

## Machine Model

`OpenAgentsOaNodeMachineRecord` describes a managed machine that can be a:

- desktop;
- SHC VM;
- GCP VM; or
- future Pylon candidate.

Each record includes:

- node identity refs;
- backend kind;
- supported runtimes;
- workload classes;
- maximum workload trust;
- capability refs;
- heartbeat refs;
- health refs;
- active workroom refs;
- artifact refs;
- placement eligibility refs;
- policy refs;
- receipt refs;
- operator caveat refs;
- operator diagnostic refs;
- trust tier;
- availability;
- quarantine state; and
- provider payout eligibility state.

## Liveness Is Not Payout Eligibility

The contract deliberately separates:

- `managedLiveness`;
- `managedAvailable`;
- `providerPayoutEligibility`; and
- `providerPayoutEligible`.

A node can be alive, healthy, and useful for managed work without being a Pylon
provider or having any payout eligibility. A future Pylon-capable node can be
payout-eligible only when it is reviewed or verified, not quarantined, and has
safe payout eligibility refs.

Heartbeat evidence never becomes a payment, payout, or settlement claim by
itself.

## Runtime And Workload Classes

Supported runtimes include:

- browser;
- Codex;
- container;
- OpenCode;
- Probe;
- Psionic;
- Pylon worker; and
- shell.

Workload classes include:

- coding;
- deploy;
- eval/training;
- Forum job;
- Pylon provider;
- research; and
- Site build.

These labels are contract terms for routing and display. They do not prove a
runtime is installed or approved for automatic dispatch.

## Projection And Redaction

Public, customer, and agent projections hide:

- hostnames;
- local paths;
- private network details;
- raw logs;
- operator-only diagnostics;
- auth material;
- wallet/payment material; and
- provider payout eligibility refs.

Operator/private projections can show safe operator caveats, diagnostics, and
payout eligibility refs, but still reject raw secrets and raw payload
material.

Projection times use friendly labels such as "2 minutes ago"; raw timestamps
are internal only.

## Tests

`workers/api/src/oa-node-managed-machine.test.ts` covers:

- schema/projection decoding;
- liveness versus provider payout eligibility separation;
- quarantine blocking managed availability and payout eligibility;
- operator-only payout eligibility refs;
- friendly time labels; and
- rejection of host, path, private-network, auth, wallet, payment, raw-log,
  private-repo, and raw timestamp refs.
