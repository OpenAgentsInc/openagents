# Nexus Hotfix Lane Contract

Date: 2026-04-15

## Purpose

This document freezes the Nexus hotfix lane contract after the April 15, 2026
cutover failure mode.

The operator decision is:

- keep the GCP runtime
- keep Rust
- stop using Cloud Build as the required production hotfix lane
- make a warm persistent Linux builder the primary build path
- make binary-first deploys to the Nexus VM the primary hotfix path
- keep the existing image-first path alive as explicit fallback and validation
- split the deploy unit later so relay hotfixes stop recompiling the wrong
  graph

This document is intentionally narrower than the broader roadmap. It freezes
what future implementation issues must build.

## Primary And Fallback Paths

### Primary hotfix path

The primary Nexus hotfix path is:

1. build a versioned Linux `nexus-relay` binary on a warm Linux builder
2. upload that versioned binary release to the Nexus VM
3. activate the new release by switching a stable `current` symlink
4. restart `systemd`
5. run local and public verification gates
6. keep the previous release available for immediate rollback

### Fallback path

The fallback path remains:

1. build and push the Nexus container image
2. activate that image on the Nexus VM
3. verify the rollout with the current image-based gates

Cloud Build remains valid for:

- fallback production deploys
- validation image builds
- cases where an image artifact is explicitly required

Cloud Build is no longer the default hotfix unblock path.

## Builder Contract

The builder host must be:

- Linux
- persistent across builds
- able to retain `cargo` registry cache
- able to retain `cargo` git cache
- able to retain `target` artifacts for Nexus builds
- able to retain `sccache`

The builder path may be:

- a dedicated Linux VM
- an already-operated stable Linux host
- a self-hosted CI runner on a persistent Linux machine

The builder host must not be an ephemeral per-build environment if it is the
primary hotfix path.

## Release Layout Contract

The Nexus VM release layout is:

- `/opt/nexus-relay/releases/<git_sha>/nexus-relay`
- `/opt/nexus-relay/current -> /opt/nexus-relay/releases/<git_sha>`
- `/opt/nexus-relay/shared/` for release-local shared material if needed

The persistent runtime data remains under:

- `/var/lib/nexus-relay`

The `systemd` service must execute the stable path through:

- `/opt/nexus-relay/current/nexus-relay`

The release path is immutable. Activating a release must not overwrite the
previous release in place.

## Rollback Contract

Rollback means:

1. point `/opt/nexus-relay/current` back to the previous known-good release
2. restart `systemd`
3. re-run the local and public verification gates

The currently active release must not delete the previous release before
verification passes.

The rollback path must remain operator-visible and scriptable. It must not
depend on rebuilding artifacts.

## Verification Contract

Binary-first deploys must preserve the same honesty bar as the current
image-first lane.

The minimum contract is:

- local service restart succeeded
- local health endpoint is healthy
- local API stats endpoint responds within current thresholds
- local treasury status and live policy checks still pass when treasury is
  enabled
- public `https://nexus.openagents.com/api/stats` gate still passes
- public dry-run provider heartbeat gate still passes
- deploy receipt records the activated git revision and release path

The binary lane is not accepted if it weakens verification.

## Script Responsibilities

The implementation may choose exact filenames, but the responsibilities are
fixed.

Required build and deploy responsibilities:

- builder-side Nexus binary build entrypoint
- upload of versioned binary releases to the VM
- activation of the selected release via `current`
- rollback to the previous release
- verification and receipt emission for the binary lane

## Deploy-Unit Contract

The medium-term architecture rule is:

- routine relay hotfixes must not require rebuilding `psionic-train`

This contract does not require the deploy-unit split to land before the binary
lane.

The required order is:

1. fix the hotfix lane
2. prove the binary lane once with a real deploy
3. then shrink the deploy unit

## What This Document Changes Operationally

From this point forward:

- operators and future agents should treat the binary-first lane as the target
  primary path
- Cloud Build plus image-first deploys should be treated as fallback until the
  binary lane is fully implemented and proven
- implementation work should not reopen the image-first versus binary-first
  question without new contradictory evidence
