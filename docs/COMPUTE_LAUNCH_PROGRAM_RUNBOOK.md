# Compute Launch Program Runbook

Status: active  
Date: 2026-03-13

This runbook is the release and regression program for the widened compute
stack. It exists to prove that the current OpenAgents compute claims are backed
by repeatable artifacts across the same surfaces the product and kernel
actually expose:

- desktop-control and `autopilotctl`
- headless buyer and provider flows
- Psionic sandbox, cluster, and evidence substrate
- validator challenge protocols
- kernel and `nexus-control` authority receipts

The entrypoint is:

```bash
scripts/release/check-compute-launch-program.sh
```

That script writes:

- `SUMMARY.md`
- `summary.json`
- per-step logs
- optional cluster benchmark receipts
- optional live desktop-control snapshots
- optional packaged/headless/NVIDIA sub-run artifacts

The default gate is self-contained. It does not require a GUI session, a funded
wallet, or a packaged app bundle.

## Default Gate

Run this from the repo root:

```bash
scripts/release/check-compute-launch-program.sh
```

The default gate covers seven required legs.

1. `desktop_control_and_mcp`
   Validates the app-owned control plane, the CLI action mapping, snapshot
   change signaling, sandbox-state projection, and the proof/challenge/operator
   read model exposed to users and automation.
   Commands:
   `cargo test -p autopilot-desktop desktop_control::tests::buy_mode_request_status_preserves_result_invoice_and_payable_roles -- --exact --nocapture`
   `cargo test -p autopilot-desktop desktop_control::tests::proof_history_surfaces_settlement_and_identity_review_fields -- --exact --nocapture`
   `cargo test -p autopilot-desktop desktop_control::tests::settlement_and_challenge_history_stay_linked_to_same_delivery -- --exact --nocapture`
   `cargo test -p autopilot-desktop desktop_control::tests::snapshot_change_events_emit_local_runtime_and_gpt_oss_domains -- --exact --nocapture`
   `cargo test -p autopilot-desktop desktop_control::tests::snapshot_signature_changes_when_sandbox_truth_changes -- --exact --nocapture`
   `cargo test -p autopilot-desktop compute_mcp::tests::server_maps_representative_tools_to_desktop_actions -- --exact --nocapture`
   `cargo test -p autopilot-desktop --bin autopilotctl lifecycle_commands_map_to_control_requests -- --exact --nocapture`

2. `headless_compute_units`
   Proves headless buyer/provider sequencing, payment coupling, and
   result-publication semantics without requiring a funded external smoke run.

3. `psionic_sandbox_jobs`
   Proves bounded sandbox execution, policy refusal, background-job lifecycle,
   upload/wait sequencing, and artifact publication.

4. `psionic_cluster_matrix`
   Reuses the Psionic cluster transport and validation matrix as the canonical
   proof for discovery, admission, recovery, sharding, and fault handling.

5. `psionic_evidence_and_receipts`
   Validates signed cluster evidence, delivered execution context, and provider
   receipts for cluster and sandbox compute.

6. `validator_service`
   Proves queueing, leasing, retry, timeout, verified verdict, and rejected
   verdict behavior for the current validator protocol.

7. `nexus_compute_authority`
   Validates authoritative compute market flow, evaluation runs, synthetic-data
   pipeline linkage, validator challenge projection, and index methodology /
   correction truth.

If any required leg fails, treat the build as non-launchable for widened
compute claims.

## Optional Live And Platform Legs

These are not skipped because they are unimportant. They are skipped by default
because they require host-specific or funded preconditions.

### Funded headless roundtrip

```bash
scripts/release/check-compute-launch-program.sh --include-headless-live
```

This runs:

- `scripts/autopilot/headless-compute-smoke.sh`
- `scripts/autopilot/headless-compute-roundtrip.sh`

Use this before claiming that funded buyer/provider flows still work outside
the GUI.

### Packaged macOS app

```bash
scripts/release/check-compute-launch-program.sh --include-packaged-macos
```

This runs:

- `scripts/release/check-v01-packaged-compute.sh`
- `scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh`

Use this before claiming packaged desktop-control, packaged Apple FM, or
packaged earn-loop readiness.

### Linux NVIDIA GPT-OSS Mission Control

```bash
scripts/release/check-compute-launch-program.sh --include-nvidia
```

This runs `scripts/release/check-gpt-oss-nvidia-mission-control.sh`.

Use this before claiming seller-mode readiness on supported NVIDIA hosts.

### Cluster benchmark receipts

```bash
scripts/release/check-compute-launch-program.sh --include-cluster-bench
```

This runs the ignored Psionic cluster benchmark gates and writes JSON benchmark
receipts under `cluster-bench/`.

Use this before claiming planner latency budgets or benchmark-backed cluster
sizing guidance.

## Live Desktop Snapshot Capture

If a real desktop app is already running, capture the app-owned truth surfaces
into the same artifact tree:

```bash
scripts/release/check-compute-launch-program.sh \
  --manifest ~/.openagents/autopilot/desktop-control.json \
  --autopilotctl-bin ./target/release/autopilotctl
```

This captures:

- `status`
- `local-runtime status`
- `cluster status`
- `proof status`
- `challenge status`
- `sandbox status`
- a log tail

Use this when you need a release receipt that includes the actual GUI-synced
operator surface, not only test output.

## Soak Mode

Run repeated integrated loops with:

```bash
scripts/release/check-compute-launch-program.sh --soak-iterations 3
```

Each iteration repeats three high-signal integrated checks:

1. desktop-control proof, challenge, and settlement projection
2. cluster discovery and failover matrix drills
3. kernel validator challenge routing

Increase the iteration count before widening operator claims or cutting a
release candidate for the broader compute stack.

## Failure Triage

Start with `SUMMARY.md`, then drill into the step log that failed.

- `desktop_control_and_mcp` failures usually mean the user-visible control plane
  and the programmatic control plane no longer agree.
- `psionic_sandbox_jobs` failures usually mean sandbox lifecycle or receipt
  truth drifted.
- `psionic_cluster_matrix` failures usually mean a cluster claim was widened
  without keeping the validation matrix truthful.
- `psionic_evidence_and_receipts` failures usually mean cluster or sandbox
  provenance is no longer preserved into delivery-grade evidence.
- `validator_service` failures usually mean challenge protocol or lease semantics
  are no longer deterministic.
- `nexus_compute_authority` failures usually mean authoritative market objects,
  eval bindings, or correction logic no longer match the widened compute story.

When the optional legs are enabled, treat their sub-artifact trees as first
class evidence:

- headless run summaries live under `headless-smoke/` and `headless-roundtrip/`
- packaged run summaries live under `packaged-compute/` and
  `packaged-autopilotctl/`
- NVIDIA Mission Control artifacts live under `gpt-oss-nvidia/`

## Release Expectation

Before claiming the full widened compute stack is launch-honest, run:

```bash
scripts/release/check-compute-launch-program.sh \
  --soak-iterations 3 \
  --include-cluster-bench
```

Then add the relevant platform or funded legs for the target release
environment.
