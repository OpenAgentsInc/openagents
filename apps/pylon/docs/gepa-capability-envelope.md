# Pylon GEPA Capability Envelope

Status: implemented for `0.3.0-rc1` admission and fake-assignment tests.

`src/gepa-capability.ts` defines the GEPA-first worker capability envelope that
connects Pylon assignment leases to the in-repo
`@openagentsinc/pylon-runtime` benchmark contracts.

The default envelope advertises:

- runtime contracts: `probe.benchmark_assignment.v1`,
  `probe.benchmark_run.v1`, and `probe.benchmark_closeout.v1`;
- GEPA retained benchmark runner capability;
- Terminal-Bench retained fixture support;
- artifact upload support;
- proof/receipt support;
- assignment closeout support;
- local sandbox isolation;
- Probe runtime backend support.

Payout readiness is separate from worker readiness. A worker can be GEPA-ready
while still not ready for paid settlement. Training is not supported in this
envelope; Qwen and neural training claims remain postponed.

Admission blockers include:

- `blocker.gepa.wrong_capability`;
- `blocker.gepa.unsupported_backend`;
- `blocker.gepa.missing_isolation_profile`;
- `blocker.gepa.artifact_upload_unavailable`;
- `blocker.gepa.proof_receipts_unavailable`;
- `blocker.gepa.closeout_unavailable`;
- `blocker.gepa.payout_readiness_stale`;
- `blocker.gepa.training_claim_postponed`;
- `blocker.gepa.wall_clock_budget_exceeded`;
- `blocker.gepa.cost_budget_exceeded`;
- `blocker.gepa.unsupported_benchmark_suite`;
- `blocker.gepa.unsupported_retained_fixture`.

`computeAssignmentAdmission` merges these GEPA blockers with the normal Pylon
lease blockers for lifecycle, heartbeat freshness, capability refs, backend
support, lease expiry, and paid-mode wallet readiness.
