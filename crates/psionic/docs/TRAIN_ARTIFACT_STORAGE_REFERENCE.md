# Train Artifact Storage Reference

> Status: canonical `PSI-285` / `#3590` reference record, updated 2026-03-14
> after landing the train artifact-storage layer in
> `crates/psionic/psionic-train/src/artifact_storage.rs`.

This document records the first explicit storage-lifecycle contract for train
artifacts inside Psionic.

## Canonical Runner

Run the artifact-storage harness from the repo root:

```bash
scripts/release/check-psionic-train-artifact-storage.sh
```

## What Landed

`psionic-train` now owns a typed artifact-storage controller that tracks
checkpoint, rollout, eval, and log artifacts through retention, archival,
deduplication, garbage collection, and cold restore.

The new typed surfaces include:

- `TrainArtifactClass`
- `ArtifactRetentionProfile`
- `ArtifactArchiveClass`
- `TrainArtifactLocator`
- `TrainArtifactRecord`
- `ArtifactStorageTransition`
- `ArtifactStorageSweepReceipt`
- `ArtifactColdRestoreReceipt`
- `TrainArtifactStorageController`

## What The Contract Makes Explicit

The artifact-storage layer now makes these train-specific lifecycle seams
machine-legible:

- class-scoped retention thresholds for `hot` and `warm` tiers
- explicit archival posture for ephemeral, restorable, and immutable artifacts
- digest-aware deduplication instead of implicit operator cleanup
- typed lifecycle state for active, restore-requested, and garbage-collected
  artifacts
- sweep receipts that explain every migration or deletion transition
- cold-restore receipts with restore-SLA targets

## Pass Criteria

The artifact-storage layer is green only if all of the following remain true:

- checkpoint artifacts can progress from hot storage to warm, then archive, and
  finally garbage collection when policy allows
- duplicate rollout digests can be identified and removed deterministically
- archived eval artifacts can request and complete cold restore through typed
  receipts
- storage decisions remain inspectable through typed records and receipts rather
  than logs alone

## Current Limits

This issue does not claim that train storage is complete. It does not yet
implement:

- an external blob-store client or remote archival backend
- policy synchronization with kernel or Nexus authority services
- byte-accurate cost accounting or queue-budget interaction
- artifact placement optimization across hosts or storage media

What it does do is give Psionic one Rust-owned storage lifecycle surface for
artifact retention, deduplication, archival, garbage collection, and
cold-restore truth.
