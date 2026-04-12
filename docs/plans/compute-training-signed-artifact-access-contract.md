# Compute Training Signed Artifact Access Contract

Status: active  
Date: 2026-04-11

This document freezes the launch-grade signed read and write access contract
for training artifacts.

The resolver contract already established durable artifact identity, and the
GCS layout contract already froze bucket, prefix, retention, and mutability
rules. This document adds the missing time-bounded access layer that lets
`Pylon` upload artifacts and lets joiners recover artifacts without smuggling
host-local paths through the control plane.

## Scope

This issue does four things:

- freezes one typed signed-access request and response schema
- freezes the `Nexus` API route that mints signed artifact access
- freezes the launch TTL, digest-expectation, and upload-verification posture
- implements Google Cloud Storage V4 signed URL issuance from `Nexus`

This issue does not yet wire `Pylon` or `Psionic` to consume the route
automatically. That lands in the next automation issues.

## Canonical Types

The shared kernel-core contract now lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

The main types are:

- `PylonTrainingArtifactSignedAccessMode`
- `PylonTrainingArtifactSignedAccessMethod`
- `PylonTrainingArtifactSignedAccessRequest`
- `PylonTrainingArtifactSignedAccessResponse`

The signed-access schema version is:

- `openagents.pylon_training_artifact_signed_access.v1`

## Nexus Route

`Nexus` now exposes one authenticated minting route:

- `POST /v1/kernel/compute/training/artifacts/{artifact_id}/signed-access`

Current auth posture:

- the route follows the existing authenticated kernel-control pattern
- it uses the existing session bearer path today
- the returned data-plane URL is a Google Cloud Storage V4 signed URL

The request body is:

```json
{
  "mode": "write",
  "ttl_seconds": 900,
  "digest": "sha256:adapter-delta",
  "size_bytes": 4096
}
```

Rules:

- `mode=write` currently requires `digest`
- `ttl_seconds` is optional and is clamped by `Nexus` to the configured max
- `size_bytes` is optional but echoed back when present

The response shape is:

```json
{
  "schema_version": "openagents.pylon_training_artifact_signed_access.v1",
  "artifact_id": "oa.train_artifact.v1~kind~local_update~network~trainnet.alpha~run~run.alpha~window~window.000123~assignment~assign.node01.window000123",
  "artifact_kind": "local_update",
  "artifact_class": "local_update",
  "canonical_store": "google_cloud_storage",
  "mode": "write",
  "method": "put",
  "signed_url": "https://storage.googleapis.com/...",
  "digest_algorithm": "sha256",
  "expected_digest": "sha256:adapter-delta",
  "expected_size_bytes": 4096,
  "issued_at_unix": 1775900000,
  "expires_at_unix": 1775903600,
  "ttl_seconds": 3600,
  "relative_object_path": "windows/window.000123/contributions/assign.node01.window000123/adapter_delta_bundle.json",
  "upload_completion_state": "upload_complete_unverified",
  "verification_success_state": "digest_verified",
  "verification_failure_state": "garbage_collectable",
  "verification_failure_reason": "upload_digest_verification_failed"
}
```

## Launch Behavior

Launch posture is:

- `Nexus` derives the object path from the logical `artifact_id`
- `Nexus` signs the Google Cloud Storage URL against the canonical bucket root
- `Nexus` returns the expected digest and size contract alongside the URL
- write access always lands in `upload_complete_unverified` first
- successful digest verification promotes the object to `digest_verified`
- verification failure marks the object `garbage_collectable`

That keeps the signed URL as a pure access grant rather than pretending URL
minting itself is final acceptance.

## Environment Contract

The launch config surface in `apps/nexus-control` is:

- `NEXUS_CONTROL_TRAINING_GCS_BUCKET_URI`
- `NEXUS_CONTROL_TRAINING_GCS_ENDPOINT`
- `NEXUS_CONTROL_TRAINING_GCS_SIGNED_URL_TTL_SECONDS`
- `NEXUS_CONTROL_TRAINING_GCS_SIGNED_URL_MAX_TTL_SECONDS`
- `NEXUS_CONTROL_TRAINING_GCS_SIGNING_CREDENTIALS_PATH`

`NEXUS_CONTROL_TRAINING_GCS_SIGNING_CREDENTIALS_PATH` falls back to
`GOOGLE_APPLICATION_CREDENTIALS`.

Current implementation detail:

- `Nexus` reads a service-account JSON file
- it signs Google Cloud Storage V4 URLs locally with the RSA private key in
  that file

That is enough for launch-grade bucket access on the existing GCS path. A
future metadata / IAMCredentials signer can extend this without changing the
public signed-access contract.

## Verification

This contract is covered by:

- kernel-core tests for signed-access request validation, TTL clamping, and
  upload verification state transitions
- `nexus-control` route tests that assert Google Cloud Storage V4 signed URL
  issuance for a launch-critical local-update artifact
