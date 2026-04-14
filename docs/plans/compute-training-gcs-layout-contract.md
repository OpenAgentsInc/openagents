# Compute Training GCS Layout Contract

Status: active  
Date: 2026-04-11

This document freezes the launch-grade Google Cloud Storage contract for
training artifacts.

The MVP already froze the basic `gs://<bucket>/networks/<network_id>/runs/<run_id>/...`
layout. This document adds the operational rules that were still implicit:
bucket-root policy, mutability boundaries, garbage-collection windows,
resumable upload posture, sharding thresholds, and the signed-URL boundary.

The canonical shared types now live in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

## Scope

This issue does four things:

- freezes the canonical bucket-root and prefix layout
- freezes the launch retention and garbage-collection classes
- freezes the immutability and overwrite policy
- freezes the launch signed-URL and sharding posture

The matching signed-access contract now lives in
[`compute-training-signed-artifact-access-contract.md`](./compute-training-signed-artifact-access-contract.md).

## Canonical Bucket Contract

Launch uses Google Cloud Storage only.

Canonical bucket-root shape:

```text
gs://<bucket>
```

Canonical bucket naming convention:

```text
<gcp-project>-openagents-training-<environment>
```

Examples:

- `gs://openagentsgemini-openagents-training-prod`
- `gs://openagentsgemini-openagents-training-staging`

Important rules:

- `bucket_uri` must be a bare bucket root, not `gs://bucket/some/prefix`
- network, run, and window isolation live in object prefixes, not in ad hoc
  per-run bucket roots
- bucket naming is stable per environment; run- and network-specific state live
  below the frozen object prefixes

The shared validator now rejects bucket URIs that attempt to smuggle a prefix
into `bucket_uri`.

## Canonical Object Prefix Layout

Run root:

```text
gs://<bucket>/networks/<network_id>/runs/<run_id>/
```

Window root:

```text
gs://<bucket>/networks/<network_id>/runs/<run_id>/windows/<window_id>/
```

Temporary upload prefix:

```text
gs://<bucket>/networks/<network_id>/runs/<run_id>/_tmp/<artifact_id>/
```

Final launch-critical objects remain:

- `checkpoints/latest_pointer.json`
- `checkpoints/step-<optimizer_step>/checkpoint_manifest.json`
- `windows/<window_id>/assignments/<assignment_id>/run_manifest.json`
- `windows/<window_id>/contributions/<assignment_id>/adapter_delta_bundle.json`
- `windows/<window_id>/contributions/<assignment_id>/proof_bundle.json`
- `windows/<window_id>/validators/<challenge_id>/verdict.json`
- `windows/<window_id>/sealed_window_bundle.json`
- `windows/<window_id>/score_snapshot.json`

## Mutability And Overwrite Policy

The launch posture is:

- accepted artifacts are immutable once finalized
- overwrite is deny-by-default
- the only mutable artifact kind is `latest_checkpoint_pointer`

That means:

- contribution bundles, proofs, verdicts, score snapshots, and
  checkpoint-manifest objects are write-once
- the latest checkpoint pointer is the narrow mutable escape hatch used to
  advertise the currently promoted starting point for the next round
- mutability is an explicit artifact-kind policy, not operator judgment

## Retention And Garbage-Collection Policy

Launch retention classes are:

| Retention class | GC policy | Notes |
| --- | --- | --- |
| `private_staging` | 72 hours | pre-acceptance local updates and similar private staging outputs |
| `accepted_authority` | no automatic GC | accepted checkpoints, aggregates, and authority-visible canonical artifacts |
| `public_verification` | 90 days | proofs, score snapshots, checkpoint pointers, and other public verification objects |
| `ephemeral_transport` | 24 hours | temporary joiner envelopes and transport-only staging prefixes |

Rules:

- rejected or stale private staging artifacts must age out automatically
- accepted authority artifacts are durable and immutable at launch
- public verification artifacts stay durable but budgeted
- transport-only prefixes are aggressively garbage-collected

## Upload And Finalization Policy

Launch upload state is:

1. local materialized
2. upload initiated
3. upload complete but unverified
4. digest verified
5. resolver registered
6. locator published
7. accepted or garbage-collectable

Launch upload posture:

- data-plane uploads must use resumable GCS uploads by default
- locators are not authoritative before digest verification and resolver
  registration complete
- temporary upload prefixes exist for transport and staging, not canonical
  identity

## Sharding Policy

Launch sharding threshold:

- shard artifacts above `4 GiB`
- target shard size: `512 MiB`

The threshold is intentionally conservative for consumer-compute and cross-node
resume. It lets ordinary manifests, pointers, proofs, and most update bundles
stay single-object, while forcing very large checkpoint-like artifacts onto one
explicit sharded path before multi-GB transfer behavior becomes operator lore.

This issue freezes the threshold and target size. Later issues still need to
implement the shard-manifest upload and rematerialization path.

## Signed-URL Policy

Launch access posture is:

- use signed URLs for the GCS data plane
- reserve `NIP-98` for authenticated HTTP control-plane actions

That means:

- `Pylon` and `Psionic` should upload and download bytes directly against GCS
  using scoped signed URLs
- `Nexus` remains the registry, resolver, and policy issuer rather than a proxy
  for every object transfer
- control-plane APIs can stay Nostr-authenticated without dragging large object
  movement through the same request path

## Public Mirror Policy

GCS is the canonical byte store for launch-critical training state.

Public mirroring stays optional and allowlisted:

- checkpoint pointers
- proof bundles
- score snapshots
- other explicitly public verification artifacts

Blossom and `NIP-94` remain optional public-distribution layers. They do not
override canonical GCS identity, digest, or TRN lineage.

## Shared Code Paths

The shared launch contract now lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`

In particular:

- the shared kernel contract now exposes `PylonTrainingArtifactGcsLayoutPolicy`
- `PylonTrainingArtifactLayout` now rejects bucket-prefix drift and path
  traversal in the canonical layout ids
- `Nexus` now serves the frozen GCS layout policy over the existing authenticated
  kernel API

## Kernel API

`Nexus` now exposes:

- `GET /v1/kernel/compute/training/artifact-storage-layout`

The response is `PylonTrainingArtifactGcsLayoutPolicy`, which freezes:

- bucket-root pattern
- bucket naming convention
- run and window prefix patterns
- temporary upload prefix pattern
- immutable acceptance posture
- mutable pointer allowlist
- resumable upload requirement
- sharding threshold and target size
- data-plane and control-plane auth policy
- retention policy by retention class

## Verification

Focused verification for this issue should cover:

- the kernel-core layout policy matching the frozen launch contract
- the artifact layout rejecting bucket-prefix drift and path traversal
- the Nexus route returning the frozen GCS layout policy
