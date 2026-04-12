# Compute Training Artifact Resolver Contract

Status: active  
Date: 2026-04-11

This document defines the canonical training artifact identity and resolver
contract for the Transcript 222 launch-hardening program.

It exists to replace one specific bad pattern in the current path: treating a
munged relative object path as if it were the durable artifact identity.

For launch hardening, the durable identity is the logical `artifact_id`. Object
paths, bucket prefixes, signed URLs, and public mirror URLs are derived views.

## Scope

This issue does three things:

- freezes one typed artifact-class vocabulary
- freezes one resolver response shape
- maps every current launch-critical `Pylon` training artifact path onto one
  logical artifact id

This issue does not yet define the concrete GCS bucket layout or signed URL
issuance. Those land in the next issues:

- `openagents#4298` for GCS object layout, lifecycle, and retention policy
- `openagents#4299` for signed read and write URL issuance

The matching launch-grade GCS policy now lives in
[`compute-training-gcs-layout-contract.md`](./compute-training-gcs-layout-contract.md).

## Canonical Types

The shared kernel-core contract now lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

The main types are:

- `PylonTrainingArtifactClass`
- `PylonTrainingArtifactKind`
- `PylonTrainingArtifactCanonicalStore`
- `PylonTrainingArtifactRetentionClass`
- `PylonTrainingArtifactMirrorPolicy`
- `PylonTrainingArtifactStorageState`
- `PylonTrainingArtifactScope`
- `PylonTrainingArtifactResolverResponse`

The resolver schema version is:

- `openagents.pylon_training_artifact_resolver.v1`

The canonical artifact-id prefix is:

- `oa.train_artifact.v1`

## Artifact Id Grammar

Artifact ids are logical ids, not path fragments.

Current format:

```text
oa.train_artifact.v1~kind~<artifact_kind>~network~<network_id>~run~<run_id>[~window~<window_id>][~assignment~<assignment_id>][~challenge~<challenge_id>][~optimizer_step~<n>]
```

Examples:

```text
oa.train_artifact.v1~kind~run_manifest~network~trainnet.alpha~run~run.alpha
oa.train_artifact.v1~kind~latest_checkpoint_pointer~network~trainnet.alpha~run~run.alpha
oa.train_artifact.v1~kind~checkpoint_manifest~network~trainnet.alpha~run~run.alpha~optimizer_step~42
oa.train_artifact.v1~kind~local_update~network~trainnet.alpha~run~run.alpha~window~window.000123~assignment~assign.node01.window000123
oa.train_artifact.v1~kind~proof_bundle~network~trainnet.alpha~run~run.alpha~window~window.000123~assignment~assign.node01.window000123
oa.train_artifact.v1~kind~validator_verdict~network~trainnet.alpha~run~run.alpha~window~window.000123~challenge~challenge.alpha
oa.train_artifact.v1~kind~sealed_window~network~trainnet.alpha~run~run.alpha~window~window.000123
oa.train_artifact.v1~kind~score_snapshot~network~trainnet.alpha~run~run.alpha~window~window.000123
```

This format is intentionally self-describing so `Nexus` can resolve the
artifact contract without depending on a host-local path or local cache state.

## Typed Artifact Class Vocabulary

The shared typed artifact classes are:

- `config`
- `checkpoint`
- `weights`
- `optimizer`
- `local_update`
- `aggregate`
- `eval`
- `proof`
- `score`

For the current launch-critical `Pylon` path, the active kinds map to classes
as follows:

| Artifact kind | Artifact class |
| --- | --- |
| `run_manifest` | `config` |
| `latest_checkpoint_pointer` | `checkpoint` |
| `checkpoint_manifest` | `checkpoint` |
| `local_update` | `local_update` |
| `proof_bundle` | `proof` |
| `validator_verdict` | `eval` |
| `sealed_window` | `proof` |
| `score_snapshot` | `score` |

This change also updates the worker-facing expected artifact class from the old
`delta` label to `local_update`, which now matches the updated local TRN draft.

## Resolver Response Shape

`Nexus` now exposes one authenticated read path:

- `GET /v1/kernel/compute/training/artifacts/{artifact_id}`

The response is JSON `PylonTrainingArtifactResolverResponse`:

- `schema_version`
- `artifact_id`
- `artifact_kind`
- `artifact_class`
- `digest_algorithm`
- `digest`
- `size_bytes`
- `storage_state`
- `canonical_store`
- `retention_class`
- `mirror_policy`
- `public_mirror_urls`
- `signed_read_url`
- `signed_write_url`
- `relative_object_path`
- `locator_kind`
- `scope`

Current launch-hardening behavior:

- `digest_algorithm` is frozen to `sha256`
- `canonical_store` is frozen to `google_cloud_storage`
- `relative_object_path` is already resolved
- `digest`, `size_bytes`, `storage_state`, and signed URLs remain optional until
  the later GCS layout and signed-URL issues land

That means the resolver contract is live now, while the richer store-backed
resolution is deliberately staged behind the next two issues.

## Storage And Publication Vocabulary

The typed storage-state vocabulary now exists even before the registry is
backed by GCS lifecycle policy:

1. `local_materialized`
2. `upload_initiated`
3. `upload_complete_unverified`
4. `digest_verified`
5. `resolver_registered`
6. `locator_published`
7. `accepted`
8. `garbage_collectable`

The typed retention classes are:

- `private_staging`
- `accepted_authority`
- `public_verification`
- `ephemeral_transport`

The typed mirror policies are:

- `never`
- `allowlisted`
- `required`

## Launch-Critical Path Mapping

The current launch-critical `Pylon` artifact objects map like this:

| Current object path suffix | Logical kind | Notes |
| --- | --- | --- |
| `manifests/run_manifest.json` | `run_manifest` | assignment and runtime handoff config |
| `checkpoints/latest_pointer.json` | `latest_checkpoint_pointer` | latest promoted checkpoint pointer |
| `checkpoints/step-<n>/checkpoint_manifest.json` | `checkpoint_manifest` | step-specific checkpoint manifest |
| `windows/<window>/contributions/<assignment>/adapter_delta_bundle.json` | `local_update` | contributor update bundle |
| `windows/<window>/contributions/<assignment>/proof_bundle.json` | `proof_bundle` | contributor proof bundle |
| `windows/<window>/validators/<challenge>/verdict.json` | `validator_verdict` | validator result bundle |
| `windows/<window>/sealed_window_bundle.json` | `sealed_window` | sealed window result bundle |
| `windows/<window>/score_snapshot.json` | `score_snapshot` | public score snapshot |

All of these now round-trip through the shared resolver in
`openagents-kernel-core`.

## Current Code Paths Updated

The following owned paths now use the shared resolver contract:

- `crates/openagents-kernel-core/src/pylon_training.rs`
- `apps/pylon/src/lib.rs`
- `apps/pylon/src/training_trn_mapping.rs`
- `apps/nexus-control/src/kernel.rs`
- `apps/nexus-control/src/lib.rs`
- `crates/nostr/nips/TRN.md`

In particular:

- `Pylon` no longer derives `artifact_id` by replacing slashes in a relative
  path
- worker receipts and expected artifact classes now use `local_update`
- `kind:39520` locator identity is separated from `url` location hints
- `Nexus` can resolve the logical contract for a launch-critical artifact id

## What This Unblocks

This issue establishes the identity contract needed for the next storage
issues:

- GCS object layout can now be defined as a projection from logical artifact id
  to object path
- signed read and write URL issuance can now bind to one stable artifact id
- later resume and replay work can depend on logical artifact ids rather than
  SCP or source-host-local path assumptions

## Verification

Focused tests for this issue should cover:

- every launch-critical object path round-tripping through the shared resolver
- `Pylon` locator publication using `local_update`
- the Nexus artifact resolver route returning the logical contract for a valid
  artifact id
