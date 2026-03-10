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
- explicit non-LAN discovery posture and refusal diagnostics for LAN-only,
  configured-peer-only, and future wider-network-requested cluster claims
- trust-bundle version overlap and stale-bundle refusal during key rotation
- coordinator lease expiry and stale-leader diagnostics for operator-managed
  multi-subnet clusters
- coordinator term/fence truth in clustered execution evidence for operator-
  managed failover paths
- declared cluster execution capability profiles and their stable digest linkage
  to planner refusals plus provider-facing execution evidence
- command-authorization refusal truth and payout-facing cluster provenance
  surfaces for operator-managed multi-subnet clusters

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
  - explicit discovery posture truth for LAN-only and configured-peer-only transport
  - explicit configured-peer health, backoff, and late-join recovery
  - key-rotation overlap acceptance and stale trust-bundle refusal diagnostics
  - coordinator lease freshness, expiry, and stale-leader diagnostics
- `cluster_validation_matrix`
  - compacted catchup and snapshot-install recovery
  - degraded whole-request scheduling with explicit artifact staging truth
  - wider-network discovery intake, refusal, expiry, and admission reconciliation
  - replicated serving reroute away from a slow replica
  - layer-sharded and tensor-sharded evidence surfaces
  - declared capability-profile truth versus planner refusal and evidence digest
    surfaces
  - allowed versus refused cluster-command authorization coverage
  - whole-request and sharded command provenance surfaced for settlement/audit use
  - fault-injected shard-mesh refusal
  - stale-leader diagnostics, split-brain refusal, and failover fence rotation

## Fault-Injection Coverage

`cluster_validation_matrix` uses the shared test fixture in
`crates/psionic/psionic-cluster/tests/support/mod.rs`.

The fault seam currently covers:

- artifact residency changes: resident, copy-required, pull-required, refused
- degraded backend readiness on a selected node
- low-memory pressure on candidate nodes
- degraded scheduler links
- unsuitable or missing inter-shard mesh links
- command-authorization refusal and provenance-preservation checks

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

## Discovery Posture Drill

Use this sequence before claiming explicit non-LAN discovery posture or any
future wider-network discovery story:

1. Run `cargo test -p psionic-cluster non_lan_discovery_assessment_refuses_trusted_lan_seed_peers`.
2. Run `cargo test -p psionic-cluster non_lan_discovery_assessment_refuses_operator_managed_configured_peers`.
3. Run `cargo test -p psionic-cluster explicit_wider_network_discovery_request_is_bounded_until_implemented`.
4. Run `cargo test -p psionic-cluster --test local_cluster_transport authenticated_configured_peers_discover_each_other_with_signed_control_plane_messages`.
5. If any step fails, do not claim non-LAN discovery posture truth or future wider-network discovery readiness for the current build.

Interpretation:

- trusted-LAN posture failure means LAN-only discovery is no longer being
  surfaced as an explicit bounded posture
- configured-peer posture failure means operator-managed multi-subnet discovery
  is no longer distinguished from wider-network discovery claims
- explicit wider-network request failure means the code no longer keeps future
  non-LAN discovery requests machine-checkably refused until a real transport
  implementation exists
- configured-peer transport failure means the current wider-than-LAN story is
  no longer backed by real operator-managed transport behavior

## Wider-Network Discovery Drill

Use this sequence before claiming wider-network discovery intake or admission
reconciliation beyond the current explicit refusal boundary:

1. Run `cargo test -p psionic-cluster --test cluster_validation_matrix discovery_validation_covers_intake_refusal_expiry_and_reconciliation`.
2. Run `cargo test -p psionic-cluster signed_cluster_introduction_verifies_under_matching_policy signed_cluster_introduction_refuses_untrusted_source signed_cluster_introduction_refuses_ttl_that_exceeds_policy`.
3. Run `cargo test -p psionic-cluster replay_keeps_discovery_candidates_separate_from_membership_truth snapshot_recovery_preserves_discovery_candidate_truth_and_provenance`.
4. If any step fails, do not claim wider-network discovery readiness or candidate-admission rollout truth for the current build.

Interpretation:

- discovery validation matrix failure means signed introductions, refusal
  boundaries, expiry, or admission reconciliation are no longer covered by one
  operator-repeatable gate
- introduction-policy failure means wider-network intake is no longer bounded by
  explicit source trust and TTL policy
- replay or recovery failure means discovered-candidate truth is no longer
  staying separate from admitted membership across deterministic rebuilds

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

## Coordinator Failover Drill

Use this sequence before claiming fenced coordinator failover readiness for
operator-managed multi-subnet clusters:

1. Run `cargo test -p psionic-cluster --test cluster_validation_matrix coordinator_authority_validation_surfaces_stale_leader_and_failover_fence_rotation`.
2. Run `cargo test -p psionic-cluster --test cluster_validation_matrix split_brain_validation_refuses_conflicting_same_term_leadership`.
3. If either step fails, do not claim fenced coordinator failover readiness for the current build.

Interpretation:

- stale-leader and failover-rotation failure means operator validation no longer
  proves that current authority differs from stale coordinator truth after
  turnover
- split-brain refusal failure means the ordered-state failover path can no
  longer reject conflicting same-term authority explicitly

## Authorization And Payout Provenance Drill

Use this sequence before claiming stronger operator-audit posture or any
future payout/dispute story built on cluster provenance:

1. Run `cargo test -p psionic-cluster --test cluster_validation_matrix authorization_validation_covers_allowed_and_refused_cluster_commands`.
2. Run `cargo test -p psionic-cluster --test cluster_validation_matrix scheduling_validation_covers_staging_and_degraded_candidate sharding_validation_covers_layer_and_tensor_evidence`.
3. Run `cargo test -p psionic-provider text_generation_receipt_preserves_cluster_execution_from_provenance text_generation_receipt_surfaces_layer_sharded_cluster_execution_truth text_generation_receipt_surfaces_tensor_sharded_cluster_execution_truth`.
4. If any step fails, do not claim payout-grade cluster provenance or stronger operator-facing authorization auditability for the current build.

Interpretation:

- authorization validation failure means allowed/refused cluster-command flows
  are no longer evidence-backed enough to support operator policy claims
- cluster scheduling/sharding provenance failure means bounded admission truth
  is no longer carried through whole-request or sharded execution surfaces
- provider receipt failure means settlement-facing JSON no longer preserves the
  cluster provenance required for later audit or dispute handling

## Capability Profile Drill

Use this sequence before claiming that declared cluster execution capability
profiles, planner refusals, and provider-facing cluster evidence are still
aligned:

1. Run `cargo test -p psionic-runtime cluster_execution_capability_profile`.
2. Run `cargo test -p psionic-runtime communication_eligibility_can_be_derived_from_capability_profile`.
3. Run `cargo test -p psionic-runtime lane_communication_eligibility_refuses_undeclared_lane_even_when_profile_exists`.
4. Run `cargo test -p psionic-cluster whole_request_scheduler_refuses_metal_cluster_dispatch_explicitly`.
5. Run `cargo test -p psionic-cluster replicated_serving_builds_replicated_topology_and_selects_best_warm_replica`.
6. Run `cargo test -p psionic-cluster layer_sharded_scheduler_builds_two_shard_cuda_plan`.
7. Run `cargo test -p psionic-cluster tensor_sharded_scheduler_builds_two_shard_cuda_plan`.
8. Run `cargo test -p psionic-provider capability_envelope_can_surface_cluster_execution_context`.
9. Run `cargo test -p psionic-provider text_generation_receipt_preserves_cluster_execution_from_provenance`.
10. If any step fails, do not claim declared capability-profile-backed cluster truth for the current build.

Interpretation:

- runtime profile round-trip or digest failure means the declared capability
  contract is no longer stable enough to anchor downstream planner/evidence
  claims
- runtime eligibility failure means required communication classes or lane
  refusals are no longer derived from declared capability truth
- whole-request refusal failure means Metal or any other refused lane may be
  widening from backend labels again instead of remaining bounded by declared
  profile truth
- replicated or sharded planner failure means clustered lane admission no
  longer consumes the declared capability profile before producing a plan
- provider capability or receipt failure means declared capability-profile
  digests are no longer preserved through operator-facing execution evidence

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

To persist benchmark receipts:

```bash
crates/psionic/scripts/benchmark-cluster-gates.sh --json-out /tmp/psionic-cluster-bench
```

Stable receipt artifacts written into that directory:

- `whole_request_scheduler.json`
- `recovery_catchup.json`
- `replicated_serving.json`
- `layer_sharded_planner.json`
- `tensor_sharded_planner.json`

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

## Benchmark Receipt Drill

Use this sequence before claiming that cluster planner performance is backed by
typed benchmark receipts rather than by ad hoc timing notes:

1. Run `crates/psionic/scripts/benchmark-cluster-gates.sh --json-out /tmp/psionic-cluster-bench`.
2. Confirm the script reports `benchmark_receipt_json_out=/tmp/psionic-cluster-bench`.
3. Confirm the directory contains `whole_request_scheduler.json`, `recovery_catchup.json`, `replicated_serving.json`, `layer_sharded_planner.json`, and `tensor_sharded_planner.json`.
4. Inspect one receipt directly, for example `sed -n '1,80p' /tmp/psionic-cluster-bench/whole_request_scheduler.json`, and confirm it includes `schema_version`, the matching `benchmark_id`, and `outcome: "passed"`.
5. If any step fails, do not claim benchmark-receipt-backed performance truth for the current build.

Interpretation:

- script failure means the release benchmark gate no longer emits the typed
  receipt artifacts the roadmap depends on
- missing receipt files mean the output naming contract is no longer stable
  enough for operator or CI consumers
- schema or benchmark-id mismatch means the persisted JSON no longer matches
  the typed receipt contract
- failed receipt outcomes mean the current cluster planner performance envelope
  is not within the documented benchmark budget

## Exit Criteria

The current cluster claim remains evidence-backed only when:

- the baseline validation commands are green
- the authenticated membership drill is green before any configured-peer or
  multi-subnet rollout claim
- the coordinator failover drill is green before claiming fenced failover truth
- the capability profile drill is green before claiming declared clustered-lane
  support or refusal truth
- the authorization and payout provenance drill is green before claiming
  stronger operator audit or payout/dispute posture
- the benchmark receipt drill is green before claiming benchmark-backed
  cluster performance truth
- the release benchmark gate is green
- roadmap and issue comments reference the exact tests and runbook paths above
