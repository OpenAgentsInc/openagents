# Issue 4451 Packaged Training Runtime Bridge

Recorded: 2026-04-27

Issue: `OpenAgentsInc/openagents#4451`

## Summary

Current production stats still show updated Pylons online but presence-only:

```text
pylon/0.1.15 online_pylons=4
homework_worker_eligible_pylons_online_now=0
homework_worker_presence_only_blocker_counts:
  pylon/0.1.15 homework_worker_training_capability_missing online_pylons=4
```

The local proof from the prior report already showed that an isolated updated
Pylon can advertise training capability when it has a Psionic training runtime
surface available. The remaining product packaging gap was that
`scripts/release/pylon-binary-release.sh` built and shipped only `pylon` and
`pylon-tui`. A normal npm-installed standalone archive could therefore launch
as `pylon/0.1.15` while still lacking the discoverable `psionic-train` runtime
surface required for homework-worker admission.

## Change

The Pylon binary release helper now:

- requires a clean Psionic checkout through `OPENAGENTS_PSIONIC_REPO` or the
  normal sibling `../psionic`;
- builds `psionic-train` with `cargo build --manifest-path <psionic>/Cargo.toml
  --release -p psionic-train`;
- packages a minimal `./psionic` runtime surface into the Pylon archive;
- includes `psionic/target/release/psionic-train`;
- includes the minimal source files that Pylon already probes to verify the
  machine-training runtime surface:
  - `TRAIN`
  - `Cargo.toml`
  - `Cargo.lock`
  - `crates/psionic-train/Cargo.toml`
  - `crates/psionic-train/src/main.rs`
  - `crates/psionic-train/src/train_runtime.rs`
- updates the release README and release notes so operators know the packaged
  runtime is part of the homework earning lane.

This matches the existing Pylon discovery path: when the running executable is
inside an extracted release archive, Pylon searches ancestor sibling
directories named `psionic` and accepts the minimal machine runtime layout.

## Verification

Local syntax verification:

```bash
bash -n scripts/release/pylon-binary-release.sh
```

Focused tests retained from the prior #4451 attempt:

```bash
cargo test -p nexus-control cs336_homework_auto_dispatch_cycle_targets_all_compatible_online_pylons
cargo test -p pylon pylon_autonomously_closes_homework_assignment_from_worker_completion_to_paid_receipt
```

## Remaining Production Acceptance

This does not close `#4451` by itself. The issue still needs a new public Pylon
release cut from this packaging change, updated nodes restarted through the
normal npm/bootstrap path, production stats showing updated homework-worker
eligible Pylons, and a production run that reaches accepted-work payout.

Until that proof exists, public stats may continue to show existing
`pylon/0.1.15` nodes as presence-only with
`homework_worker_training_capability_missing`.
