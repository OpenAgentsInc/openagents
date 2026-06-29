# Probe GEPA Unpaid Pylon Worker Lease Proof

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#514`.

OpenAgents product surface now has a public-safe unpaid lease proof for Probe GEPA benchmark
rollouts. The proof creates three demo Pylon workers and runs three
metric-call assignments through the existing OpenAgents product surface lifecycle:

1. assignment created;
2. demo Pylon worker accepts a lease;
3. worker reports progress refs;
4. worker submits artifact, proof, verifier, closeout, and resource refs;
5. evaluator closeout records accepted or rejected work;
6. OpenAgents product surface emits Psionic coordinator imports.

The implementation lives in
`workers/api/src/probe-gepa-unpaid-pylon-lease-proof.ts`.

## Worker Set

The proof uses these demo Pylons:

- `pylon.public.demo.alpha`
- `pylon.public.demo.beta`
- `pylon.public.demo.gamma`

They are public-safe demo workers for the OpenAgents product surface/Pylon assignment protocol. This
does not claim a production Pylon fleet, paid work, settlement, or public
benchmark score.

Paid-work movement now belongs to the settlement-readiness gate documented in
`docs/2026-06-08-probe-gepa-settlement-readiness-gate.md`. The unpaid lease
proof remains the no-spend receipt path; `operator_credit`,
`payable_pending_settlement`, and `settled_bitcoin` require separate accounting
and receipt evidence.

## Evidence Preserved

The proof preserves:

- OpenAgents product surface assignment refs;
- demo Pylon worker refs;
- lease refs;
- progress refs;
- artifact manifest refs;
- proof bundle refs;
- resource usage refs, including the live SHC resource-unavailable receipt;
- verifier result refs;
- accepted closeout refs;
- rejected closeout refs;
- Psionic coordinator import refs.

One assignment imports the live SHC Harbor `db-wal-recovery` failure closeout
from the public `benchmark-cloud` Stage 0 live receipt bundle:

- `artifact_manifest.probe.shc_harbor.db_wal_recovery.20260608`
- `proof_bundle.probe.shc_harbor.db_wal_recovery.20260608`
- `resource_usage_unavailable.probe.benchmark_run_probe_shc_harbor_db_wal_recovery_20260608`
- `probe_closeout.shc_harbor.db_wal_recovery.20260608`
- `verifier_result.terminal_bench.db_wal_recovery.shc_harbor.20260608.reward_0`

## Claim Boundary

Allowed payment states in this proof are:

- `unpaid_smoke`
- `rejected_no_pay`

The proof rejects:

- `operator_credit`
- `payable_pending_settlement`
- `settled_bitcoin`
- payment receipt refs;
- settlement receipt refs;
- paid-work claims;
- settlement claims;
- automatic promotion claims.

This is Pylon-distributed GEPA rollout optimization evidence, not distributed
neural-network training and not a public Terminal-Bench score.

## Verification

Run:

```sh
bun run --cwd workers/api test -- pylon-gepa-metric-call-assignments.test.ts probe-gepa-unpaid-pylon-lease-proof.test.ts
```
