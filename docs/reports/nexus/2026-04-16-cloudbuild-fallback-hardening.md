# Nexus Cloud Build Fallback Hardening

Date: 2026-04-16

Related issue:

- `#4363`

## Purpose

This note records the retained outcome of hardening the Nexus image-first
fallback lane after the April 15 production cutover stall.

The goal was not to make Cloud Build the primary hotfix path again. The goal
was to leave the fallback path materially less wasteful and less fragile.

## Landed Changes

The hardened fallback lane now does all of the following:

- pins Cloud Build machine sizing in repo config:
  - `machineType: E2_HIGHCPU_32`
  - `diskSizeGb: 200`
- uses a two-step Docker build graph for Nexus:
  - `deps` stage builds `nexus-relay` from a minimal `.nexus-build-plan`
  - `builder` stage copies the full narrowed context and re-runs the final
    build
- keeps the staged Nexus-only context and Nexus-specific lockfile path intact
- materializes real build-plan crates and their build-script inputs, including
  `third_party/nostr-rs-relay/proto/nauthz.proto`
- emits a local JSON receipt for successful fallback builds under
  `docs/reports/nexus/*-cloudbuild-image-<git_short_sha>.json`

This is a `cargo-chef` equivalent rather than a literal `cargo-chef`
integration. The dependency-isolation effect is the same: Cargo work for the
stable dependency graph is separated from fast-changing source copies.

## Timing Comparison

### Pre-hardening baseline

- build id: `bd0abf00-d8b6-46b0-8bce-f4d4411519fe`
- start: `2026-04-15T21:56:07.894795426Z`
- finish: `2026-04-15T23:48:49.300441641Z`
- duration: `1:52:41.405646`
- status: `INTERNAL_ERROR`
- image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:fec745edf254`

This was the cutover-blocking production image build that made the hotfix lane
operationally unacceptable.

### Hardened fallback success on the post-extraction tree

- build id: `54b7a3b9-e716-4113-8d4c-564d2a9da196`
- start: `2026-04-16T04:28:32.065733776Z`
- finish: `2026-04-16T04:37:28.532860Z`
- duration: `0:08:56.467127`
- status: `SUCCESS`
- image:
  `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:nexus-fallback-20260416-042731`

This build proved that the hardened lane works on the current reduced Nexus
graph. It also populated the remote registry cache for the follow-up validation
run.

### Hardened fallback validation rerun

- build id: `dad6fcf0-0c5a-4023-b78b-da7bf1a7ce16`
- create time: `2026-04-16T04:38:21.700267737Z`
- start: `2026-04-16T04:39:17.053390768Z`
- finish: `2026-04-16T04:39:35.920115Z`
- Cloud Build execution duration: `0:00:18.866725`
- wrapper wall-clock receipt duration: `80524 ms`
- status: `SUCCESS`
- image:
  `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:nexus-fallback-20260416-043812`
- retained receipt:
  `docs/reports/nexus/20260416-043812-cloudbuild-image-885f848a8fc0.json`

This rerun shows what the hardened lane looks like once the registry cache is
warm: queue delay still exists, but the remote image build itself collapses to
seconds instead of minutes.

## Failure Learnings During Hardening

Two intermediate failures were important and changed the final config:

1. `68aaa2ac-4ff2-4593-902b-b7c3961c12ff` failed in `0:10:15.224077` with
   default-enabled GCS `sccache` because the server startup timed out. Result:
   `sccache` stays opt-in, not default.
2. `8b0fbca7-b168-4797-8a94-287f0519c201` failed in `0:02:59.839891` because
   the build-plan path omitted `third_party/nostr-rs-relay/proto/nauthz.proto`.
   Result: the build-plan helper now materializes real crate inputs and crate
   local `proto/` directories.

The first successful run also exposed one local wrapper bug: receipt generation
was embedding raw `gcloud builds describe --format=json` output directly into a
Python string literal. That is now fixed by writing the build description to a
temporary file and loading it from disk.

## Conclusion

The hardened Cloud Build lane is materially better than the baseline failure
mode that triggered this roadmap work:

- roughly `1:52:41` failure before hardening
- `0:08:56` successful cold-ish hardened run
- `0:00:18` successful warm-cache remote execution on rerun

That is good enough for a fallback and validation lane.

It is still not acceptable as the primary hotfix path because queue time,
remote-build variance, and cache-warm assumptions remain outside the operator's
direct control. The binary-first warm-builder lane should remain the default
production hotfix path.
