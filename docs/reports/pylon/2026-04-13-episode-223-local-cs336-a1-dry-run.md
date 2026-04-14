# Episode 223 Local CS336 A1 Dry Run

Date: 2026-04-13  
Repo: `openagents`

This report records the latest retained local dry run after broadening the
bounded `CS336 A1 Demo` lane and strengthening the named-run proof so both Mac
and Linux Pylons can each take one worker slot on the same bounded homework
run.

The retained bundle came from:

- script:
  `scripts/release/check-pylon-episode-223-cs336-a1-local.sh`
- output root:
  `target/pylon-episode-223-cs336-a1-local/20260414T051244Z-58844/`
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
- `Nexus` can schedule one two-slot named `CS336 A1 Demo` run, preserve the
  display name through the public summary surfaces, and retain both worker
  slots on that same run
- `Pylon` can map that environment into the packaged demo lane, claim it,
  launch it, and sync terminal artifacts back to `Nexus`

What it does **not** prove yet:

- that the live public fleet is already upgraded onto a release containing
  these fixes
- that a real Mac Pylon and a real Linux Pylon have both already completed the
  homework on the public network
- that the public Nexus homepage is already showing the fresh live run

## Step Results

The retained summary recorded these passing steps:

- `psionic_machine_manifest` in `1s`
- `mac_cs336_fallback` in `186s`
- `linux_cs336_fallback` in `1s`
- `nexus_named_run` in `198s`
- `pylon_manifest_mapping` in `0s`
- `pylon_assignment_intake` in `1s`
- `pylon_runtime_launch` in `1s`
- `pylon_terminal_sync` in `0s`

## Honest Next Step

The next honest step is not more local contract work. It is operational:

1. cut a fresh `pylon-v...` release that contains the post-`rc1` runtime fixes
   plus the new Mac/Linux lane broadening
2. upgrade one reachable Mac Pylon and one reachable Linux Pylon to that build
3. retain one live proof bundle showing both machines actually taking the two
   worker slots on the bounded A1 homework run

That is the remaining gap between "the system works locally" and "Episode 223
can honestly show both Mac and Linux Pylons doing the homework live."
