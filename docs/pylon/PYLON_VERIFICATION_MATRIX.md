# Pylon Verification Matrix

This document is the release gate for standalone `Pylon`.

It exists to answer one question honestly:

**Can a new operator install, run, verify, and reason about Pylon without `Autopilot`, while preserving the launch truth of the OpenAgents Compute Market?**

## Launch Truth Checklist

These statements must remain true in docs, code, and operator guidance:

- `Pylon` is a standalone provider connector, not the whole product.
- The market is still the **OpenAgents Compute Market**.
- Launch compute product families are `inference` and `embeddings`.
- `Ollama` can back `inference` and `embeddings`.
- `Apple Foundation Models` is `inference` only at launch.
- Raw accelerator trading is not live at launch.
- Capability-envelope fields refine supply; they are not the primary product identity.
- `sandbox_execution` is the next planned family, not the current generally released launch family.

If any release note, marketing text, or operator doc violates one of those statements, the rollout should stop.

## Deterministic Matrix

| Area | Verification Path | Expected Result |
| --- | --- | --- |
| Install/init | `cargo run -p pylon -- init` | config and identity are created without `Autopilot` |
| Unconfigured truth | `cargo run -p pylon -- status` before init | reports `unconfigured` |
| Backend visibility | `cargo run -p pylon -- backends --json` | returns backend entries for `ollama` and `apple_foundation_models` with explicit health states |
| Launch products | `cargo run -p pylon -- products --json` | shows launch products only; no `apple_foundation_models.embeddings` |
| Sandbox status truth | `cargo run -p pylon -- status` on a node with declared sandbox profiles | status includes sandbox execution classes, profile IDs, and scan/runtime errors when relevant |
| Sandbox runtime/profile view | `cargo run -p pylon -- sandbox --json` | returns declared runtimes/profiles, ready execution classes, and profile digests |
| Inventory | `cargo run -p pylon -- inventory` | shows inventory rows with capability summaries and explicit eligibility |
| Jobs | `cargo run -p pylon -- jobs` | shows recent jobs or an empty truthful list, including sandbox family/profile/failure detail when present |
| Earnings | `cargo run -p pylon -- earnings` | shows earnings summary or explicit none-state |
| Receipts | `cargo run -p pylon -- receipts` | shows receipt summaries or an empty truthful list, including sandbox profile/termination/failure detail when present |
| Online transition | `cargo run -p pylon -- online` | desired mode becomes `online`; unhealthy supply becomes `degraded`, not falsely healthy |
| Pause/resume | `cargo run -p pylon -- pause` then `resume` | transitions are explicit and truthful |
| Offline transition | `cargo run -p pylon -- offline` | desired mode becomes `offline` |
| No hidden auto-online | start `pylon serve` after offline state | runtime does not silently force `online` |
| Restart safety | stop and restart `pylon serve`; re-run `status`, `jobs`, `earnings`, `receipts` | persisted desired mode and persisted provider snapshot remain coherent |
| Autopilot parity | desktop provider-admin tests + shared substrate tests | `Autopilot` and `Pylon` use the same persisted provider snapshot contract |

## Repo-Level Checks

These checks should pass before a release candidate is called valid:

```bash
cargo test -p pylon -- --nocapture
cargo clippy -p pylon --all-targets -- -D warnings
cargo test -p openagents-provider-substrate -- --nocapture
cargo clippy -p openagents-provider-substrate --all-targets -- -D warnings
cargo test -p autopilot-desktop provider_admin -- --nocapture
cargo check -p autopilot-desktop
cargo clippy -p autopilot-desktop --all-targets -- -D warnings
scripts/lint/ownership-boundary-check.sh
scripts/lint/workspace-dependency-drift-check.sh
scripts/pylon/verify_standalone.sh
```

Sandbox-specific evidence that should remain green:

```bash
cargo --manifest-path ../psionic/Cargo.toml test -p psionic-sandbox execution::tests::policy_rejection_is_receipted -- --nocapture
cargo --manifest-path ../psionic/Cargo.toml test -p psionic-sandbox execution::tests::local_subprocess_success_emits_receipt_and_artifacts -- --nocapture
cargo test -p pylon sandbox_reports_surface_profiles_status_and_failures -- --nocapture
cargo test -p autopilot-desktop snapshot_signature_changes_when_sandbox_truth_changes -- --nocapture
```

## Operational Smoke Path

The minimum operator smoke path is:

```bash
cargo run -p pylon -- status
cargo run -p pylon -- init
cargo run -p pylon -- backends --json
cargo run -p pylon -- products
cargo run -p pylon -- sandbox
cargo run -p pylon -- inventory
cargo run -p pylon -- online
cargo run -p pylon -- pause
cargo run -p pylon -- resume
cargo run -p pylon -- offline
```

If those commands do not run cleanly and report truthful state transitions, the release is not ready.

## Packaging and Service Expectations

Standalone packaging is valid only if the install story is explicit:

- how the binary is started
- where config and identity live
- how desired mode is changed
- how logs/status are checked
- how restarts are handled

The current supported operational posture is a service-style `pylon serve` process managed by:

- `systemd`
- `launchd`
- `tmux` or another persistent operator-managed session

This is sufficient for a truthful first standalone release. It is not necessary to pretend there is a universal cross-platform packaging story if it has not been verified.

## Rollout Checklist

Pre-release:

- verify launch truth statements in docs and user-facing messaging
- verify `Apple Foundation Models` is not described as embeddings-capable
- verify `Pylon` still reads as a provider connector, not a monolithic local runtime
- verify lifecycle remains explicit and not hidden behind `serve`
- verify backend-health and product surfaces distinguish healthy vs degraded vs unsupported vs misconfigured vs disabled states plainly
- verify sandbox status, backend, job, and receipt surfaces expose declared execution classes, profile IDs, and failure detail truthfully

Release candidate:

- run the full repo-level checks above
- run the standalone smoke path on at least one Ollama-capable machine
- run the standalone smoke path on at least one Apple FM-capable machine if Apple FM is part of the candidate
- run the sandbox-specific evidence commands on at least one machine with declared sandbox profiles
- confirm persisted status, jobs, earnings, and receipts survive a restart

Launch approval:

- confirm docs and smoke results match current code truth
- confirm no unsupported backend or market functionality is being claimed
- confirm rollout posture remains aligned with the broader compute-market verification and observability gates

Post-launch:

- monitor operator reports for lifecycle-control confusion
- monitor backend-state misclassification
- monitor discrepancies between `Autopilot` and standalone `Pylon` snapshot truth
- monitor sandbox runtime/profile misclassification and missing failure/termination detail in job and receipt views
- update this matrix before broadening launch claims
