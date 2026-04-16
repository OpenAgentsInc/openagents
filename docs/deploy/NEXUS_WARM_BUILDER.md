# Nexus Warm Builder

Date: 2026-04-15

This document records the primary Nexus hotfix build lane after the April 15
Cloud Build stall.

The goal is narrow:

- keep the runtime on GCP
- keep `nexus-relay` in Rust
- stop making Cloud Build the mandatory hotfix builder
- produce versioned Linux `nexus-relay` binaries from one warm persistent
  builder host

This document covers builder bootstrap and binary artifact production only.
Release upload, activation, rollback, and deploy verification live in the
 broader Nexus deploy runbook and later issue work.

## Canonical Builder Defaults

The checked-in defaults are:

- VM: `nexus-builder-1`
- zone: `us-central1-a`
- machine type: `c3d-standard-8`
- boot disk: `100 GB pd-ssd`
- persistent cache disk: `200 GB pd-ssd`
- cache mount: `/mnt/disks/nexus-builder-cache`
- build user: `nexus-builder`
- Rust toolchain: `1.88.0`
- `sccache`: `0.8.2`

All of those can be overridden through env vars in
`scripts/deploy/nexus/common.sh`.

## Canonical Commands

Provision or refresh the warm builder host:

```bash
scripts/deploy/nexus/11-provision-warm-builder.sh
```

Build a versioned Linux `nexus-relay` binary on the warm builder:

```bash
scripts/deploy/nexus/12-build-nexus-binary.sh
```

Force a cold-cache timing run:

```bash
NEXUS_BUILDER_CLEAR_CACHES=true scripts/deploy/nexus/12-build-nexus-binary.sh
```

Follow immediately with a warm-cache run:

```bash
scripts/deploy/nexus/12-build-nexus-binary.sh
```

The first run measures the builder with empty persistent caches. The second run
measures the same revision with the retained `cargo`, `target`, and `sccache`
state intact.

## Operator Access Path

The builder is operated the same way as the Nexus VM:

- `gcloud compute ssh --tunnel-through-iap`
- `gcloud compute scp --tunnel-through-iap`

The scripts handle those transport details directly. Operators should not need
to hand-copy build contexts or re-bootstrap the toolchain manually.

## Persistent Cache Layout

The builder keeps all reusable state on the persistent cache disk:

- `${NEXUS_BUILDER_CACHE_MOUNT_POINT}/cargo-home`
- `${NEXUS_BUILDER_CACHE_MOUNT_POINT}/target`
- `${NEXUS_BUILDER_CACHE_MOUNT_POINT}/sccache`
- `${NEXUS_BUILDER_CACHE_MOUNT_POINT}/sources/<git_sha>`
- `${NEXUS_BUILDER_CACHE_MOUNT_POINT}/artifacts/<git_sha>`
- `${NEXUS_BUILDER_CACHE_MOUNT_POINT}/timings/<stamp>-<git_short_sha>.json`

The build context submitted to the builder is the same narrowed Nexus-only
workspace slice already used by the fallback image lane through
`scripts/deploy/nexus/stage-build-context.sh`.

## Builder Artifact Layout

Each built revision produces:

- `.../artifacts/<git_sha>/nexus-relay`
- `.../artifacts/<git_sha>/build-metadata.json`
- `.../artifacts/<git_sha>/nexus-relay.sha256`
- `.../artifacts/<git_sha>/rustc-version.txt`
- `.../artifacts/<git_sha>/cargo-version.txt`
- `.../artifacts/<git_sha>/sccache-stats.txt`

The local operator machine also retains the build receipt under:

- `docs/reports/nexus/<stamp>-warm-builder-build-<git_short_sha>.json`

That receipt is the stable evidence path for issue comments, timing comparisons,
and later deploy receipts.

The first retained cold versus warm timing baseline is:

- `docs/reports/nexus/2026-04-16-warm-builder-baseline.md`

## Scope Boundary

This builder lane intentionally does not modify the live Nexus VM by itself.

The next steps are:

1. upload the built release to `/opt/nexus-relay/releases/<git_sha>`
2. activate `/opt/nexus-relay/current`
3. restart `systemd`
4. verify and retain a deploy receipt

Those steps are separate because the point of the builder lane is to decouple
artifact production from VM activation first.
