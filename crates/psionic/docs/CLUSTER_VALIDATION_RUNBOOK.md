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
- trust-bundle version overlap and stale-bundle refusal during key rotation
- coordinator lease expiry and stale-leader diagnostics for operator-managed
  multi-subnet clusters
- coordinator term/fence truth in clustered execution evidence for operator-
  managed failover paths

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
  - explicit configured-peer health, backoff, and late-join recovery
  - key-rotation overlap acceptance and stale trust-bundle refusal diagnostics
  - coordinator lease freshness, expiry, and stale-leader diagnostics
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

## Rotation Drill

Use this sequence before claiming explicit key-rotation overlap or stale-bundle
rollout diagnostics for operator-managed clusters:

1. Run `cargo test -p psionic-cluster --test local_cluster_transport rotated_key_overlap_is_surfaceable_during_bundle_rollout`.
2. Run `cargo test -p psionic-cluster --test local_cluster_transport stale_trust_bundle_version_is_refused_and_diagnostic_is_recorded`.
3. If either step fails, do not claim trust-bundle rollout or key-rotation truth for the current build.

Interpretation:

- overlap acceptance failure means previous-key rollout is no longer
  machine-checkable during an explicit version window
- stale-bundle refusal failure means drifted or split-brain trust bundles are
  no longer being surfaced honestly

## Multi-subnet Dial Health Drill

Use this sequence before claiming explicit configured-peer reachability or
degraded transport truth for wider-network clusters:

1. Run `cargo test -p psionic-cluster --test local_cluster_transport unreachable_configured_peer_surfaces_explicit_health_and_backoff`.
2. Run `cargo test -p psionic-cluster --test local_cluster_transport late_joining_configured_peer_recovers_health_after_degraded_attempts`.
3. If either step fails, do not claim configured-peer dial policy or degraded reachability truth for the current build.

Interpretation:

- unreachable-peer failure means configured peers are still being explained by
  implicit LAN retry behavior instead of explicit health and backoff state
- late-join recovery failure means degraded configured-peer health is not
  recovering truthfully when the peer actually becomes reachable

## Coordinator Lease Drill

Use this sequence before claiming coordinator freshness or stale-leader expiry
truth for operator-managed multi-subnet clusters:

1. Run `cargo test -p psionic-cluster leadership_lease_reports_active_then_stale`.
2. Run `cargo test -p psionic-cluster cluster_state_effective_leadership_expires_when_lease_goes_stale`.
3. Run `cargo test -p psionic-cluster leadership_lease_changes_snapshot_digest`.
4. If any step fails, do not claim coordinator lease or stale-leader truth for the current build.

Interpretation:

- active/stale lease failure means coordinator freshness is no longer
  machine-checkable from ordered state
- effective-leadership expiry failure means stale coordinators may still look
  authoritative to higher-level scheduling or failover logic
- digest-change failure means lease turnover is no longer visible in stable
  cluster-state evidence
- coordinator fence tokens and commit-authority digests should now be visible in
  clustered execution evidence; the full fenced-failover drill still closes in
  `#3314`

## Recovery Drill

Use this sequence when validating recovery behavior intentionally:

1. Run `cargo test -p psionic-cluster signed_catchup_response_verifies_and_recovers_current_state tampered_signed_catchup_response_is_refused replayed_signed_catchup_response_is_refused`.
2. Run `cargo test -p psionic-cluster --test local_cluster_transport restarted_node_rejoins_cluster_with_advanced_epoch`.
3. Run `cargo test -p psionic-cluster --test cluster_validation_matrix recovery_validation_installs_snapshot_after_compaction_boundary`.
4. If any step fails, do not claim rejoin or catchup behavior for the current build.

Interpretation:

- signed recovery failure means catchup or snapshot payloads are no longer
  authenticated and replay-checked truthfully
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
