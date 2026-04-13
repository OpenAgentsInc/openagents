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
- Ollama and `gemma4-e4b-gguf:latest` are present locally

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

So the Linux version proof is real, but it is still not a worker that can
honestly claim the current bounded A1 demo lane as currently wired.

## The Main Blocker

The blocker is not "the release is missing."

The blocker is that the bounded CS336 A1 demo contract is internally stricter
and more inconsistent than the recording plan implies.

Current truth in the code:

- `psionic-train` defines the CS336 A1 demo environment as:
  - `psionic.environment.psion_cs336_a1_demo.host_cpu.operator@v1`
  - backend family `cpu`
  - topology `single_host_cpu_reference`
- `nexus-control` currently maps that same environment ref to backend family
  `cuda`
- `pylon` currently only advertises that same environment ref when the host is
  an admitted `H100`-class CUDA trainer

That means the accessible real devices do not line up with the demo contract:

- `macbook-pro-m2` is on the right architecture for the release asset but is a
  `16 GiB` Apple machine with no local runtime ready
- `archlinux` is on the right architecture for the release asset and has the
  right Psionic surface once pointed at a fresh checkout, but it is still a
  consumer `RTX 4080` validation-tier node, not an admitted strong trainer
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

Not done:

- a reachable Mac node and a reachable Linux node that can both honestly be
  described as "ready to take the `CS336 A1 Demo` run"
- a live fleet proof bundle showing the named A1 demo lane can actually be
  claimed on the upgraded hardware

## What Has To Change Next

One of these must happen before `#4338` can close honestly:

1. provide access to a real admitted strong trainer that matches the current A1
   demo contract
2. or change the bounded A1 demo contract so it matches the real Episode 223
   hardware we actually have available

Without one of those, `#4343` is also blocked because there is no honest live
fleet proof path for the named run yet.
