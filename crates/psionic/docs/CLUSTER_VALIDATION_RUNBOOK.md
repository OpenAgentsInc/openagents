# Psionic Cluster Validation Runbook

This runbook is the operator entrypoint for validating the first truthful
Psionic cluster scope: trusted-LAN connectivity, membership refusal, rejoin,
recovery/catchup, remote scheduling, replicated serving, and sharded execution.

## Scope

This runbook validates two cluster trust postures:

- the shipped trusted-LAN baseline with explicit namespace/admission policy
- the widened authenticated configured-peer posture for operator-managed
  multi-subnet or otherwise non-LAN-assumed deployments
- persisted operator manifests as the rollout artifact for authenticated
  configured-peer clusters

This runbook still does not claim internet-wide adversarial safety. It validates
the current truthful cluster claims and the current homogeneous CUDA planning
lanes.

## Baseline Validation Commands

Run these from the repo root:

```bash
cargo test -p psionic-cluster --test local_cluster_transport
cargo test -p psionic-cluster --test cluster_validation_matrix
cargo test -p psionic-cluster
```

What these cover:

- `local_cluster_transport`
  - seeded hello/ping connectivity
  - admission mismatch refusal
  - stale-epoch refusal
  - restart/rejoin with advanced node epoch
  - authenticated configured-peer discovery with signed control-plane messages
  - refusal of unknown peers under configured-peer posture
  - authenticated boot from persisted operator manifests
- `cluster_validation_matrix`
  - compacted catchup and snapshot-install recovery
  - degraded whole-request scheduling with explicit artifact staging truth
  - replicated serving reroute away from a slow replica
  - layer-sharded and tensor-sharded evidence surfaces
  - fault-injected shard-mesh refusal

## Fault-Injection Coverage

`cluster_validation_matrix` uses the shared test fixture in
`crates/psionic/psionic-cluster/tests/support/mod.rs`.

The fault seam currently covers:

- artifact residency changes: resident, copy-required, pull-required, refused
- degraded backend readiness on a selected node
- low-memory pressure on candidate nodes
- degraded scheduler links
- unsuitable or missing inter-shard mesh links

If a new cluster claim cannot be validated by one of those seams, extend the
fixture before broadening the roadmap claim.

## Authenticated Membership Drill

Use this sequence before claiming cluster posture wider than the first trusted
LAN baseline:

1. Run `cargo test -p psionic-cluster --test local_cluster_transport authenticated_configured_peers_discover_each_other_with_signed_control_plane_messages`.
2. Run `cargo test -p psionic-cluster --test local_cluster_transport authenticated_nodes_can_boot_from_operator_manifest`.
3. Run `cargo test -p psionic-cluster --test local_cluster_transport unknown_authenticated_peer_is_refused_under_configured_peer_posture`.
4. Run `cargo test -p psionic-cluster tampered_authenticated_message_is_refused replay_protection_rejects_duplicate_authenticated_counters`.
5. If any step fails, do not claim authenticated configured-peer rollout readiness for the current build.

Interpretation:

- configured-peer discovery failure means the signed control-plane path is no
  longer proving authenticated membership truthfully
- manifest boot failure means operator rollout still depends on ad hoc code
  rather than a reusable machine-checkable artifact
- unknown-peer refusal failure means the widened posture is no longer explicit
  enough to support operator-managed rollout decisions
- tamper or replay failure means widened cluster trust is not machine-checkable
  enough for multi-subnet posture claims

## Recovery Drill

Use this sequence when validating recovery behavior intentionally:

1. Run `cargo test -p psionic-cluster --test local_cluster_transport restarted_node_rejoins_cluster_with_advanced_epoch`.
2. Run `cargo test -p psionic-cluster --test cluster_validation_matrix recovery_validation_installs_snapshot_after_compaction_boundary`.
3. If either step fails, do not claim rejoin or catchup behavior for the current build.

Interpretation:

- transport rejoin failure means the local membership seam is no longer proving
  epoch advancement truthfully
- recovery matrix failure means ordered-state compaction/catchup is no longer
  evidence-backed

## Release Benchmark Gates

Run the release benchmark gate before claiming cluster planner performance is
still within the expected envelope:

```bash
crates/psionic/scripts/benchmark-cluster-gates.sh
```

To persist summaries:

```bash
crates/psionic/scripts/benchmark-cluster-gates.sh --json-out /tmp/psionic-cluster-bench
```

The benchmark gate covers:

- whole-request remote scheduling
- ordered recovery catchup generation
- replicated serving planning
- layer-sharded planning
- tensor-sharded planning

Budget override env vars:

- `PSIONIC_CLUSTER_BENCH_WHOLE_REQUEST_MAX_MS`
- `PSIONIC_CLUSTER_BENCH_RECOVERY_MAX_MS`
- `PSIONIC_CLUSTER_BENCH_REPLICATED_MAX_MS`
- `PSIONIC_CLUSTER_BENCH_LAYER_MAX_MS`
- `PSIONIC_CLUSTER_BENCH_TENSOR_MAX_MS`

Do not loosen those budgets in the roadmap or issue closure comment without
recording why.

## Exit Criteria

The current cluster claim remains evidence-backed only when:

- the baseline validation commands are green
- the authenticated membership drill is green before any configured-peer or
  multi-subnet rollout claim
- the release benchmark gate is green
- roadmap and issue comments reference the exact tests and runbook paths above
