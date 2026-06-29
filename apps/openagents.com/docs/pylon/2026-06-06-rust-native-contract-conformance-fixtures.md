# Rust And Native Contract Conformance Fixtures

Date: 2026-06-06

Status: implemented contract note for issue #340 / `OPENAGENTS-RUST-007`.

## Purpose

OpenAgents product surface now has local conformance coverage for the Epic S native-facing
contracts. The tests prove that representative Rust/native payloads can be
decoded by OpenAgents product surface schemas, projected safely for public/operator audiences, and
rejected or flagged when required refs, terminal evidence, or redaction
requirements are missing.

The implementation lives in
`workers/api/src/rust-native-contract-conformance.test.ts`.

This remains local/test-only. OpenAgents product surface does not depend on `oa-node`,
`oa-workroomd`, Probe, Psionic, Pylon, or any Rust repo at test time.

## Fixture Sources

The conformance test uses these OpenAgents product surface-side fixtures:

- `OPENAGENTS_NATIVE_CONTRACT_REGISTRY_V1`;
- `OPENAGENTS_OA_NODE_CONFORMANCE_FIXTURES`;
- `OPENAGENTS_WORKROOMD_CONFORMANCE_FIXTURES`;
- `OPENAGENTS_PROBE_CONFORMANCE_FIXTURES`;
- `OPENAGENTS_PSIONIC_CONFORMANCE_FIXTURES`; and
- `OPENAGENTS_PYLON_SETTLEMENT_BRIDGE_CONFORMANCE_FIXTURES`.

Rust/native repos should either copy these payloads into their own fixture
suites or generate byte-for-byte equivalent JSON from their native schema
types. The native tests should compare required fields, enum values, redacted
refs, and receipt links against these OpenAgents product surface contracts.

## Required Coverage

Native conformance should cover:

- shared registry coverage for every consumer and ref kind;
- `oa-node` managed-machine liveness, capability, trust, quarantine, and
  provider eligibility fields;
- `oa-workroomd` assignment intake, grant refs, grant resolution, artifact
  manifests, cancellation, closeout, archive, destroy, and audit evidence;
- Probe coding-runtime requests, turn events, tool-call summaries, terminal
  status, closeout receipts, retained failures, and public projections;
- Psionic evidence-only eval/training/optimizer/scorecard/candidate-module
  evidence without runtime mutation authority; and
- Pylon provider assignments, capability snapshots, wallet readiness
  summaries, buyer payment evidence, accepted work, reward intent, payout
  eligibility, payout dispatch, payout confirmation, payout verification, and
  settlement projections without live bitcoin spend authority.

## Negative Fixtures

The test suite includes failures or flags for:

- missing registry consumer coverage;
- successful Probe runs without closeout receipts;
- failed Probe runs without retained-failure evidence;
- settled Pylon bridges without settlement refs;
- secret-bearing refs;
- raw logs and raw payloads;
- raw timestamps;
- wallet and raw bitcoin payment material;
- private repo refs; and
- payout targets, invoices, preimages, and private channel state.

## Boundary

These tests validate schema parity and redaction. They do not launch native
processes, open sockets, resolve grants, operate wallets, dispatch payouts,
settle providers, or mutate production state.

When a stable Rust fixture package exists, OpenAgents product surface can consume that package in a
separate integration lane. Until then, OpenAgents product surface owns the canonical TypeScript
fixtures and native repos should mirror them deliberately.
