# `openagents.cloud_computer_checkpoint.v1`

Status: Implemented for `openagents.cloud_computer.v1` workspaces.

Owning package: `packages/khala-sync-server`

## Storage boundary

Tools run against local copy-on-write storage. A runtime mounts a pinned,
signed base image read-only and uses a local ext4 or OverlayFS upper layer.
Host and cluster caches provide separate read-only base-image, toolchain, and
dependency layers. The active dependency tree does not use a database-backed
FUSE mount.

Postgres stores workspace heads, checkpoint operations, object bindings,
reachability edges, usage events, and deletion evidence. Google Cloud Storage
stores encrypted checkpoint bytes. Public receipts contain opaque object refs,
not GCS locations, resumable upload sessions, KMS refs, or local paths.

## Manifest and encryption

A content manifest binds the owner, tenant, logical workspace, workspace
revision, parent, base image, admitted entries, deletion tombstones, and
retention policy. Its digest is authenticated data for AES-256-GCM encryption.
A separate storage-envelope manifest binds that content digest to the
ciphertext digest and size, wrapped data-encryption key, KMS key version, and
exact GCS object generation. This split avoids a circular digest dependency.

Each committed entry has a `workspace` or `git_metadata` classification.
Checkpoint creation rejects credentials, environment files, provider state,
runtime metadata, sockets, devices, FIFOs, escaping symlinks, Git credential
configuration, and hooks before bytes enter the encrypted object. Restores
preserve admitted files and Git objects, refs, index data, modes, and safe
symlinks. The local adapter disables Git hooks.

Only the workspace-key authority adapter can mint an opaque key claim. It binds
the actor, owner, tenant, workspace, key ref, and key version. Forks require the
same owner and tenant, a new logical workspace, and a new key; they reseal
content instead of copying runtime authority or credentials.

The production Node cipher uses a random per-checkpoint data-encryption key and
96-bit nonce for AES-256-GCM. An injected KMS port wraps and unwraps the data
key through an authorized, runtime-issued workspace key handle. The service
authorizes before upload or download, authenticates the content-manifest
digest, and never includes key material in its receipt.

## Durable checkpoint flow

1. Reserve an operation and exact idempotency digest in a serializable
   transaction before a storage effect.
2. Persist the resumable upload session and byte offset. A lost upload
   acknowledgement resumes the same session.
3. Finalize with a create-only GCS generation precondition, then verify the
   object generation, checksum, ciphertext, plaintext, and both manifests.
4. Commit the verified object with a compare-and-swap on the runtime
   generation, workspace revision, and expected parent.
5. Store the exact receipt with the operation. An acknowledgement retry returns
   that receipt. A failed commit can reuse the verified content-addressed
   object.

The GCS adapter persists the opaque resumable-session URL before uploading
bytes and queries the session offset after a process restart. It creates
objects with `ifGenerationMatch=0`, pins reads to the returned generation, and
keeps bucket URLs and bearer credentials behind the storage port.

The logical workspace survives stop, resume, and host replacement. Runtime
generation changes independently from the workspace revision, so a stale
runtime cannot advance the head. Explicit, bounded-interval, stop, and
host-replacement boundaries use the same operation state machine. A reconciler
marks a verified upload orphaned when either the computer generation or the
workspace head advances before commit, which makes the object eligible for
retention-aware cleanup.

The lifecycle coordinator moves an active computer through `stopping` and
reaches `cold` only when the stop checkpoint is the durable head. Resume
advances both computer and workspace generations, restores the full chain, and
then activates the replacement lease. Checkpoint or restore failure settles
the fenced generation as `failed`. Host replacement first records the lost
lease and generation as durable evidence.

Fork authorization records the source checkpoint and actor before bytes are
read. The target must use the same owner, tenant, and signed base image with a
different workspace key. The fork becomes complete only after the target
commits a resealed checkpoint.

## Incremental checkpoints and retention

Every checkpoint declares `full` or `delta`. Delta checkpoints name a parent
and carry sorted deletion tombstones. Restore resolves the complete ancestry
to a full checkpoint, applies each layer in revision order, and applies
deletions before additions. OverlayFS capture reads the upper directory and
preserves whiteouts, so a delta never recopies the read-only base image.
Each checkpoint also records a materialized workspace-state digest. A fork
reseals the resolved state into a full checkpoint under the target key and
releases its source hold only when this digest and the signed base image match;
the delta artifact digest does not stand in for the resolved state.
Durable references represent the current
head, parent ancestry, fork sources, rollback points, and legal holds. Garbage
collection selects only tombstoned objects past retention that have no live
reference or reachable descendant.

Destroy releases live references, tombstones all retained checkpoints and
objects, and records evidence before any provider deletion. GCS deletion uses
the exact object-generation precondition. Completion records all-version
absence and key disposition; only then does the workspace become destroyed.
The concrete adapter verifies the exact generation, live head, and every
paginated object version are absent. The store accepts only the adapter's
generation-bound verification result as deletion evidence.

## Local restore and metering

The Linux adapter allocates generation-specific directories, verifies and
mounts the read-only image, mounts the local overlay, restores through an
atomic content port, and wipes the allocation on failure or cleanup. It accepts
only allocations it created and invokes fixed executables with argument arrays.

Usage events record uploaded, reused, restored, retained, and collected bytes;
checkpoint, verification, restore, and garbage-collection duration; storage
age; and object count. Restore receipts separate download, decrypt, and
materialization overhead from cache warming and local dependency-install or
large-file benchmarks.

## Implementation and verification

- Contract and GCS flow:
  `packages/khala-sync-server/src/cloud-computer-checkpoint.ts`
- GCS JSON API adapter:
  `packages/khala-sync-server/src/cloud-computer-checkpoint-gcs.ts`
- AES-256-GCM and authorized checkpoint service:
  `packages/khala-sync-server/src/cloud-computer-checkpoint-crypto.ts`
- Durable Postgres authority:
  `packages/khala-sync-server/src/cloud-computer-checkpoint-store.ts`
- Local copy-on-write runtime:
  `packages/khala-sync-server/src/cloud-computer-workspace.ts`
- Stop, resume, and host-replacement coordinator:
  `packages/khala-sync-server/src/cloud-computer-lifecycle.ts`
- Schema:
  `packages/khala-sync-server/migrations/0138_cloud_computer_checkpoints.sql`

Run:

```sh
pnpm --dir packages/khala-sync-server exec vp test --run \
  src/cloud-computer-checkpoint.test.ts \
  src/cloud-computer-checkpoint-gcs.test.ts \
  src/cloud-computer-checkpoint-crypto.test.ts \
  src/cloud-computer-checkpoint-store.test.ts \
  src/cloud-computer-lifecycle.test.ts \
  src/cloud-computer-workspace.test.ts
pnpm --dir packages/khala-sync-server run typecheck
```
