# Serving records reconciliation — 2026-09-22

This record closes the remaining #9426 reconciliation work: it
reconciles the retained serving measurements by identity, dispositions
the recorded debug CPU HTTP failure, and states the supported matrix,
provenance, gate scope, and environment-dependent skips. Nothing here
relabels a recorded result: a failed command stays failed, and a
feature-enabled build is never reported as device execution.

## Records, reconciled

| Record | Source and artifact identity | OS, backend, device | dtype, profile | Result |
|---|---|---|---|---|
| `docs/lev/measurements/2026-09-20-apple-serving-matrix.md` (`76aa8cbb9c`) | `kev-0.5b`, `kev-0.6b`, `kev-4b`, `kev-8b` from local artifact dirs; real Lev bridge `sha256:c0a3d48c…`, `LEV_OS_BUILD=25E246` | macOS 26.4, M5 Max; Metal selected via `KEV_TEST_DEVICE=metal` | F32, `serve,metal` | Six conformance tests passed in 1,003.41s — actual Metal F32 inference, not a feature-enabled build claim. The same command's CPU HTTP round trip timed out; see the disposition below. |
| `docs/kev/measurements/2026-09-20-candidate-4b.md` (`36e6649b`) | `jaredpalmer/kev-4b` `c4bfa11b`, Qwen3-4B base `906bfd4b`, upstream `86db6d92`, lock `kev-4b-c4bfa11` | macOS, M5 Max 128 GiB; Metal serving | bf16 with fp32 heads/LoRA merge; correctness tests CPU fp32 | All 44 unit/integration tests passed; Metal bf16 workload serving measured. Correctness runs are CPU fp32 and are not claimed as Metal execution. |
| This record's reruns | `kev-0.5b` adapter + `qwen2.5-0.5b` base from `kev-artifacts/` | macOS, M5 Max; CPU | fp32, `serve` | Two standalone `jev_client_round_trips_all_types` runs failed at the client's 10 s deadline (80.1s, 83.5s wall) under load average ≈5. The same test passed standalone earlier the same day (~58s) on a quieter host. |

## Disposition of the recorded debug CPU HTTP failure

The failed command in the Apple matrix ran the whole `kev` test target
under `serve,metal` while the conformance battery saturated the host.
The only failing case was `jev_client_round_trips_all_types`, which
loads `kev-0.5b` on CPU and holds a hardcoded 10-second client deadline
(`jev::Config` default) over a real forward pass.

Disposition: **a deadline artifact under host load, not an inference or
correctness failure.** Evidence:

- The test hardcodes `Device::Cpu`, so its timeout is not evidence
  about the Metal path the command was measuring — the Apple matrix
  already records this.
- Two standalone reruns today reproduced the same failure shape: the
  request reached `system_one` and the client's 10-second deadline
  expired mid-forward under load ≈5. No wrong answers were observed;
  the request never completed.
- A standalone green exists from earlier the same day on a quieter
  host (~58s wall). The debug CPU path is not broken; the test's
  internal deadline is load-sensitive.

Per the issue's constraint, no timeout was raised to obtain a green
result, and no weights were hidden. The rerun failures are recorded
here as failures.

## Supported matrix

| Lane | Status | Evidence |
|---|---|---|
| Kev CPU fp32 serving (`kev-0.5b`) | Supported; deadline-sensitive under load | This record; `serve.rs` |
| Kev Metal F32 conformance (4 variants) | Verified on Apple silicon | Apple matrix |
| Kev Metal bf16 serving (`kev-4b` candidate) | Verified | Candidate-4B record |
| Kev conformance battery | Opt-in only: `KEV_CONFORMANCE=1`, `KEV_VARIANT=<id>` to narrow | `d2307565bb`, `docs/coder/verification/gates.md` |
| Lev Apple FoundationModels via the real bridge | Verified on macOS | Apple matrix; `docs/lev/` |

## Provenance

Artifact identities are the pinned digests and locks above; code
provenance is the commit recorded with each measurement. The
conformance battery's provenance and cost now live in
`docs/coder/verification/gates.md`.

## Manual-gate scope and environment-dependent skips

`./scripts/verify-rust.sh` runs fmt, clippy (default and feature),
workspace tests (default and feature), the Rust 1.95/1.94 floors,
dependency policy, and Postgres acceptance; metal phases need
`--with-metal` and soak is opt-in. Since `d2307565bb`, kev conformance
does not run inside the test phases.

Environment-dependent skips and soft spots:

- `crates/kev/tests/serve.rs` skips without the artifact bundle and is
  load-sensitive at its 10-second client deadline.
- `crates/kev/tests/conformance.rs` returns early unless
  `KEV_CONFORMANCE=1`.
- Lev tests skip without the bridge and the on-device model.
- `crates/coder` delegate/worktree tests flake under fd pressure and
  scheduling load; each passes in isolation.
- Postgres phases need a live PostgreSQL.

The valid bf16/release evidence from the candidate record carries
forward unchanged; the debug CPU failure is dispositioned, not erased.
