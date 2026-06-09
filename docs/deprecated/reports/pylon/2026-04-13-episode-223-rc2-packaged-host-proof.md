# Episode 223 `rc2` Packaged Host Proof

Date: 2026-04-13  
Repo: `openagents`

This report records the first retained packaged-host proof after
`pylon-v0.1.1-rc2`.

It is narrower than the final live Episode 223 proof. It does **not** claim
that public Nexus already ran the named demo. It proves the release binaries
themselves are now honest on the two host classes we actually care about.

## Scope

Proof targets:

- one local Mac host running the shipped `darwin-arm64` binary
- one reachable Linux host running the shipped `linux-x86_64` binary
- no ad hoc `OPENAGENTS_PSIONIC_REPO` override
- isolated `OPENAGENTS_PYLON_HOME` for each proof so the run does not depend on
  prior local state

Commands exercised on each host:

- `./pylon init`
- `./pylon status --json`
- `./pylon training status --json`

## Mac Result

Host:

- `Apple M5 Max`
- `128 GiB` host memory

Binary:

- `pylon-v0.1.1-rc2-darwin-arm64`

Retained local proof bundle:

- `target/pylon-episode-223-proof/mac-local-rc2/`

Observed result:

- `runtime_surface_detected = true`
- `contributor_supported = true`
- `tier = tier3_island`
- backend families include `cpu` and `metal`
- resolved Psionic repo root:
  `/Users/christopherdavid/work/psionic`
- resolved Psionic repo source: `cwd_ancestor_sibling`

That means the shipped Mac binary now self-discovers the local Psionic runtime
surface and truthfully reports this machine as training-capable for the bounded
Episode 223 lane.

## Linux Result

Host:

- `NVIDIA GeForce RTX 4080`
- `126 GiB` host memory

Binary:

- `pylon-v0.1.1-rc2-linux-x86_64`

Retained remote proof bundle:

- `~/code/.worktrees/openagents-ep223-rc2/target/pylon-episode-223-proof/linux-remote-rc2-no-override/`

Observed result:

- `runtime_surface_detected = true`
- `contributor_supported = true`
- `tier = tier3_island`
- backend families include `cpu` and `cuda`
- resolved Psionic repo root:
  `/home/christopherdavid/code/.worktrees/psionic-223`
- resolved Psionic repo source: `code_worktree`

That means the shipped Linux binary also self-discovers a current Psionic
runtime surface without a manual override and truthfully reports this machine as
training-capable for the bounded Episode 223 lane.

## What This Proves

- `rc2` is a real multi-platform packaged release, not just a source-tree claim
- both Mac and Linux proof hosts can run the shipped binary directly
- both hosts can detect Psionic training runtime truthfully without an ad hoc
  env override
- both hosts report `contributor_supported = true` for the bounded CS336 A1
  homework lane

## What This Still Does Not Prove

- that live public Nexus already shows the fresh named run
- that a live Mac Pylon and a live Linux Pylon have both already claimed worker
  slots on the same public `CS336 A1 Demo` run
- that the payout/settlement lane is already clean enough for the final public
  proof claim

## Honest Next Step

The next honest step is the live proof bundle:

1. keep the named `CS336 A1 Demo` run as a bounded single-host lane per worker
2. configure that Episode 223 run with `worker_count = 2`
3. retain one live run where the Mac Pylon takes one worker slot and the Linux
   Pylon takes the other
