# Calibration accounting

Status: **complete**. No model calls. Four reserve tasks and one development task.

| Task | Attempt | Role | Build (s) | Check (s) | Disposition |
| --- | --- | --- | ---: | ---: | --- |
| reserved-a | 1 | base | 1.467 | 0.064 | semantic_base_failure |
| reserved-a | 1 | reference | 0.715 | 0.036 | passed |
| reserved-b | 1 | base | 161.704 | 0.817 | semantic_base_failure |
| reserved-b | 1 | reference | 51.305 | 1.618 | passed |
| reserved-c | 1 | base | 42.094 | 0.365 | semantic_base_failure |
| reserved-c | 1 | reference | 44.067 | 1.416 | passed |
| reserved-d | 1 | base | 42.333 | 0.615 | semantic_base_failure |
| reserved-d | 1 | reference | 41.396 | 0.464 | passed |
| development | 1 | base | 2.190 | 20.051 | semantic_base_failure |
| development | 1 | reference | 1.567 | 83.079 | invalid_extra_oracle_requirement |
| development | 2 | base | 0.314 | 145.051 | invalid_cache_provenance |
| development | 2 | reference | 0.665 | unknown | invalid_cache_provenance |
| development | 3 | base | 1.166 | 20.047 | semantic_base_failure |
| development | 3 | reference | 1.066 | 145.051 | passed |
| reserved-a | 1 | negative-control-1 | 0.715 | 0.032 | mutant_rejected |
| reserved-a | 1 | negative-control-2 | 0.665 | 0.032 | mutant_rejected |
| reserved-b | 1 | negative-control-1 | 42.906 | 1.517 | mutant_rejected |
| reserved-b | 1 | negative-control-2 | 42.951 | 0.966 | mutant_rejected |
| reserved-c | 1 | negative-control-1 | 47.059 | 1.517 | mutant_rejected |
| reserved-c | 1 | negative-control-2 | 43.950 | 1.016 | mutant_rejected |
| reserved-d | 1 | negative-control-1 | 40.281 | 1.166 | mutant_rejected |
| reserved-d | 1 | negative-control-2 | 40.627 | 1.216 | mutant_rejected |
| development | 1 | negative-control-1 | 1.266 | 30.076 | mutant_rejected |
| development | 1 | negative-control-2 | 1.216 | 30.068 | mutant_rejected |

## Corrections and validity

The first development reference failure came from an extra display-timing expectation. The corrected oracle checks actual automatic behavior across a complete interval. Original checker bytes and failure records remain retained.

The second development attempt exposed stale Cargo artifact reuse when revisiting an older source tree with a shared target. That attempt is invalid, including its apparent base pass. Its remaining process was stopped; recorded time is retained and unrecorded time is unknown. The repaired runner refreshes all tracked source-file timestamps before revisited builds.

The reserve checkers remain unchanged. Their initial calibration used newly created worktrees before each build. Cargo records show the owning library was recompiled (`fresh=false`) for every reserve variant and for repaired development attempt3; only invalid development attempt2 reports `fresh=true`. Negative controls change one behavior at a time; rejecting them establishes sensitivity to those seeded variants, not exhaustive correctness. Their details and reference solutions remain private during the panel.

## Fixed checker digests

- reserved-a: `49e196854c3be1f4d036385e9d33aed176decb61b151672197e7c6d7dc8fe7cc`
- reserved-b: `848306b4956e095b19ccdb1d937280f20433d024232e86cf668f549c40e55529`
- reserved-c: `bf90c92d28495a552d44214c90bb7eb9baa15dc13cf73978cd39b05b455ee09d`
- reserved-d: `3baf235cafef8bda5e7b3428dd5f4d626e9898c674fd2f488660d4308cd6f4ed`
- development: `faf4031386cbe53d4bd28e9d92e8e6e21fd596b3dc55ebe20850a59fdd209935`

## Coverage limits

- reserved-a: Does not cover full desktop launch, admission diagnostics, resource-budget refusal, hostile concurrent directory replacement, or macOS parity.
- reserved-b: Linux path selection only; does not execute engines, validate their versions, or cover macOS bundle paths.
- reserved-c: Bounded owner-admission contention only; does not cover every store/index/migration/adapter lock path or the full timeout. The upstream issue was open at metadata collection.
- reserved-d: Local Git fixtures and immediate offline failure; does not force stalled transport, concurrent fetch-lock contention, or spare-worktree races.

## Timing limits

Known subprocess timings only; missing/interrupted test time and orchestration, source preparation, transport, inspection, and human/agent authoring time are not zero and are not included in these sums. These are trusted-cache calibration timings, not clean candidate runtime.

Known build time: 653.686s. Known check time: 486.281s. These include invalid attempts and detected negative controls; they are not engineering ROI or executor latency.
