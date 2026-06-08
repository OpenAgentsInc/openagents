# Pylon Benchmark Worker Capability Envelope

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#4559`.

`benchmark-cloud` now defines
`openagents.pylon_benchmark_worker_capability.v1` for Pylons that can run Probe
benchmark and GEPA rollout work. The envelope lives in
`crates/benchmark-cloud/src/lib.rs`.

## What Workers Advertise

The envelope records public-safe worker refs, Pylon release refs, benchmark
runner support, Harbor/Terminal-Bench support, Probe runtime support, local
model support, Apple FM support, Qwen adapter support, MLX training support,
rollout/eval support, model-training support, CPU/RAM/disk/GPU capacity, max
wall-clock budget, max cost budget ref, isolation profile, artifact upload,
proof/receipt support, assignment lease support, closeout support, payout
readiness, capability refs, caveats, and redaction state.

Payout readiness is deliberately separate from worker admission. A worker can
match no-spend or unpaid GEPA rollout work without being ready for paid
settlement. Paid work still needs the Omega settlement evidence path from the
assignment lifecycle.

## Scheduler Match

`match_pylon_benchmark_worker` compares a capability envelope with
`openagents.pylon_benchmark_work_requirement.v1`. It emits a
`PylonBenchmarkWorkerMatch` containing:

- whether the worker matched;
- blocker refs when it did not match;
- whether the worker is admitted for assignment;
- whether the worker is payout-ready for paid work.

The match helper distinguishes rollout/evaluation work from model training.
GEPA metric-call rollouts require benchmark runner, Probe runtime, lease,
artifact upload, proof/receipt, and closeout support. LoRA or model-training
requirements additionally need explicit model-training support and any training
backend requirements such as MLX.

## Safety Boundary

Capability and requirement records reject private refs, raw runner logs,
provider secrets, wallet or payment material, payout targets, private repo refs,
and raw timestamps. Capability validation also rejects overclaims such as MLX
training support without explicit model-training support.

## Verification

`cargo test -p benchmark-cloud` covers:

- a no-spend SHC worker matching Probe GEPA rollout work;
- rollout/eval work remaining distinct from LoRA/model-training work;
- overclaim rejection;
- unsafe ref rejection;
- worker admission not implying payout readiness.
