# Episode 223 Local CS336 A1 Dry Run

Date: 2026-04-13  
Repo: `openagents`

This report records the retained local dry run after combining two things on
the same branch:

- the dual-host Episode 223 lane broadening so weak Apple and consumer-CUDA
  Linux hosts can honestly take the bounded `CS336 A1 Demo` lane
- the public Nexus training-read fix so post-lease run/window/artifact reads do
  not strand live Pylons before manifest materialization

The retained bundle came from:

- script:
  `scripts/release/check-pylon-episode-223-cs336-a1-local.sh`
- output root:
  `target/pylon-episode-223-cs336-a1-local/20260414T111348Z-51711/`
- sibling Psionic checkout:
  `/Users/christopherdavid/work/.worktrees/psionic-ep223-mac-linux-a1`

## Result

The local dry run passed.

What that local pass actually proves:

- the packaged `psionic-train` CS336 A1 machine-manifest path writes retained
  checkpoint and closeout outputs
- weak Apple hosts can honestly promote into the bounded A1 trainer lane
- consumer CUDA Linux hosts can honestly promote into the bounded A1 trainer
  lane
- `Nexus` can schedule one named dual-host `CS336 A1 Demo` run, preserve the
  display name through the public summary surfaces, and answer the public run
  plus window reads needed after lease claim
- `Pylon` can map that environment into the packaged demo lane, claim it,
  launch it, and sync terminal artifacts back to `Nexus`

What it does **not** prove yet:

- that the live public fleet is already upgraded onto a release containing
  these fixes
- that a real Mac Pylon and a real Linux Pylon have both already completed the
  homework on the public network
- that the public Nexus homepage is already showing the fresh live dual-host
  run

## Step Results

The retained summary recorded these passing steps:

- `psionic_machine_manifest` in `1s`
- `mac_cs336_fallback` in `1s`
- `linux_cs336_fallback` in `0s`
- `nexus_named_run` in `0s`
- `pylon_manifest_mapping` in `0s`
- `pylon_assignment_intake` in `1s`
- `pylon_runtime_launch` in `1s`
- `pylon_terminal_sync` in `1s`

## Honest Next Step

The next honest step is live, not more local contract work:

1. cut or deploy a Nexus/Pylon build that contains both the dual-host lane
   broadening and the public training-read fix
2. upgrade one reachable Mac Pylon and one reachable Linux Pylon to that build
3. retain one live proof bundle showing both machines materialize manifests,
   launch the runtime, and contribute on the same named
   `run.cs336.a1.demo` / `trainnet.cs336.a1.demo`

That is the remaining gap between "the system works locally" and "Episode 223
can honestly show both Mac and Linux Pylons doing the homework live."
