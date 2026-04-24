# Episode 223 Fleet Upgrade Status

Date: 2026-04-13  
Repo: `openagents`

This report records the real status of `openagents#4338` after the
`pylon-v0.1.1-rc1` release was cut.

It is intentionally blunt:

- the multi-platform release exists
- the reachable Mac and Linux devices can hold the new binary
- the reachable fleet still does **not** honestly satisfy the "ready to take
  the `CS336 A1 Demo` run" bar

## Update After `#4345`

The specific cross-repo contract mismatch that originally blocked the bounded
`CS336 A1 Demo` lane is now fixed on `main`.

Current truth after `openagents#4345`:

- `nexus-control` resolves the packaged A1 demo environment through the
  canonical `psionic-train` lane contract instead of treating it like a CUDA
  lane by heuristic
- the same environment now maps to backend family `cpu` on the public/scheduler
  side
- `pylon` now advertises that environment as the CPU reference demo lane on
  compatible runtime-surface hosts instead of limiting it to admitted `H100`
  trainers

That closes the "same environment means CPU in Psionic but CUDA in Nexus/Pylon"
bug.

The remaining Episode 223 blocker from this audit is not the lane contract
anymore. It is the packaged runtime-detection / live-fleet proof path tracked
in `openagents#4346`, `openagents#4338`, and `openagents#4343`.

## Update After `#4346`

The packaged runtime-detection blocker is now fixed on `main`.

Current truth after `openagents#4346`:

- packaged `pylon` training runtime detection no longer relies on one
  compile-time-relative `../psionic` guess
- the runtime probe now searches the common real layouts we actually use:
  sibling checkouts, common home checkout roots, and `psionic*` worktrees
- `pylon training status --json` and `pylon doctor` now surface either:
  - the resolved Psionic repo root and source
  - or the exact candidate failure reason when runtime detection still fails

That closes the "packaged release only works if the operator manually exports
OPENAGENTS_PSIONIC_REPO" blocker.

The remaining Episode 223 blockers from this audit are now the live-fleet and
live-Nexus proof steps tracked in `openagents#4338`, `openagents#4343`, and
the workspace tracker.

## Update After Mac/Linux CS336 A1 Lane Broadening

The next cross-repo blocker is now fixed in code on `main`, but it is not yet
cut into a fresh Pylon release.

Current truth after the latest `psionic-train` and `pylon` changes:

- the bounded `CS336 A1 Demo` lane is still the same small four-step host-CPU
  homework lane
- that lane's minimum machine class is now
  `cross_platform_cpu_compatible_operator`, which means the claim boundary is
  honest host-CPU system-memory posture, not "must be a pure CPU box with no
  accelerators attached"
- `pylon` now advertises that bounded lane on Mac and Linux hosts that can run
  the host-CPU path, even when those machines also expose Apple-silicon or
  CUDA accelerators
- admitted Apple or H100-class hosts still advertise their stronger native
  lanes, but weak Apple and consumer CUDA hosts now fall back into the bounded
  CS336 lane instead of getting stranded below trainer tier

That closes the last contract bug that made the recording goal impossible in
principle.

The remaining blocker is now operational:

- cut a fresh `pylon-v...` release that contains these lane-broadening changes
- upgrade one Mac and one Linux host to that release
- retain one proof bundle showing both machines actually participating in the
  bounded A1 homework path

## Update After `rc2` Packaged Host Proof And Two-Slot Run Clarification

Two more things are now true:

- `pylon-v0.1.1-rc2` exists and both the local Mac proof host plus the
  reachable Linux proof host now pass packaged `training status` with no
  manual `OPENAGENTS_PSIONIC_REPO` override
- the Episode 223 named-run bar is now frozen more explicitly: the live
  `CS336 A1 Demo` run should use `worker_count = 2` while staying
  `replica_type = single_node`, so the Mac and Linux Pylons each take one
  worker slot and both fully do the bounded homework path

That means the remaining blocker is not packaged runtime discovery anymore.
It is the retained proof bundle for the actual upgraded hosts plus the live
two-slot named run.

The retained packaged-host proof for that narrower `rc2` claim now lives at:

- `docs/reports/pylon/2026-04-13-episode-223-rc2-packaged-host-proof.md`

Local dry-run closure for that code path is now retained separately at:

- `docs/reports/pylon/2026-04-13-episode-223-local-cs336-a1-dry-run.md`

## What Was Confirmed

### 1. The release exists

GitHub now has one shared release tag:

- `pylon-v0.1.1-rc1`

with both required assets:

- `pylon-v0.1.1-rc1-darwin-arm64.tar.gz`
- `pylon-v0.1.1-rc1-linux-x86_64.tar.gz`

### 2. A real Mac device has the new binary

On `macbook-pro-m2`:

- the `darwin-arm64` archive was downloaded from the release
- the published SHA-256 file verified successfully
- the archive extracted under `~/code/pylon-v0.1.1-rc1-darwin-arm64/`
- `./pylon status --json` ran successfully from that extracted bundle

What that same status output also showed:

- desired mode was `offline`
- local Gemma runtime at `127.0.0.1:11434` was not reachable
- training runtime surface was not detected from that host's current local
  state
- host memory was only `16 GiB`
- the machine therefore remained `tier0_presence`

So the Mac version proof is real, but it is not a training-ready Episode 223
host.

### 3. A real Linux x86_64 device has the new binary

On `archlinux`:

- the `linux-x86_64` archive is present under
  `~/code/pylon-v0.1.1-rc1-linux-x86_64/`
- the extracted `pylon` binary runs and returns `status --json`
- legacy local-runtime lane and `gemma4-e4b-gguf:latest` are present locally

The first training-status read was misleading because the host's existing
`~/code/psionic` checkout was stale enough that the released Pylon binary did
not detect the machine `psionic-train` surface.

After staging one fresh Psionic worktree from `origin/main` at:

- `~/code/.worktrees/psionic-223`

and pointing the binary at it with:

- `OPENAGENTS_PSIONIC_REPO=~/code/.worktrees/psionic-223`

the same Linux node reported:

- `runtime_surface_detected = true`
- `benchmark_lane_available = true`
- `replay_capability = short_window`

But it still remained:

- `contributor_supported = false`
- `tier1_validation`

because the reachable Linux box is an `RTX 4080` with `16 GiB`, not an admitted
`H100 80 GiB` trainer.

So the Linux version proof is real, but this retained proof still predates the
later lane-broadening changes on `main`.

## The Main Blocker

The blocker is no longer "the lane contract is impossible."

The blocker is that the live fleet has not yet been re-proven on a release that
contains the later fixes.

Current truth in the code:

- `psionic-train` defines the CS336 A1 demo environment as:
  - `psionic.environment.psion_cs336_a1_demo.host_cpu.operator@v1`
  - backend family `cpu`
  - topology `single_host_cpu_reference`
- minimum machine class `cross_platform_cpu_compatible_operator`
- `nexus-control` now maps that same environment ref to backend family `cpu`
- `pylon` now advertises that same environment on compatible Mac, Linux CPU,
  consumer-CUDA, and Apple hosts through the bounded host-CPU path

That means the accessible real devices do not line up with the demo contract:

- `macbook-pro-m2` is on the right architecture for the release asset but the
  retained proof in this report was still captured before the new lane
  broadening landed
- `archlinux` is on the right architecture for the release asset and that
  retained proof was also captured before the new lane broadening landed
- the stored SSH credential path to `imac-pro-bertha` did not produce a
  trustworthy login in this pass, so it could not be used as the Mac proof host

## Honest Conclusion For `#4338`

`#4338` is only partially complete.

Done:

- release assets exist for both Mac and Linux
- one Mac device has the `darwin-arm64` bundle and passes a binary smoke path
- one Linux device has the `linux-x86_64` bundle and passes a binary smoke path
- Linux runtime-surface detection was proven to depend on a fresh Psionic
  checkout or explicit `OPENAGENTS_PSIONIC_REPO`
- the code on `main` now honestly allows the bounded A1 homework lane to run on
  both Mac and Linux Pylons through the host-CPU path

Not done:

- a fresh Pylon release that actually contains the later contract/runtime
  fixes and the Mac/Linux lane broadening
- a reachable Mac node and a reachable Linux node both re-verified on that new
  release line
- a live fleet proof bundle showing the named A1 demo lane can actually be
  claimed and executed on the upgraded hardware by both machines

## What Has To Change Next

All of these must happen before `#4338` can close honestly:

1. cut and deploy a fresh Pylon release that includes the current `main`
   lane/runtime fixes
2. re-run fleet proof on one reachable Mac and one reachable Linux host with
   that build
3. retain one proof bundle showing both hosts actually taking the bounded A1
   homework path

Without that full sequence, `#4343` is also blocked because there is no honest
live fleet proof path for the named run yet.
