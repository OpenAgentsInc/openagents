# Episode 223 Local CS336 A1 Dry Run After Assignment-Scoped Manifest Fix

Date: 2026-04-14
Repo: `openagents`
Commit: `33a489ec4`

This report records the retained local Episode 223 dry run after changing the
training `run_manifest` contract from one run-scoped object to one
assignment-scoped object.

The concrete contract change is:

- old path: `manifests/run_manifest.json`
- new path: `windows/<window_id>/assignments/<assignment_id>/run_manifest.json`

That change matters because the Episode 223 acceptance bar is a real dual-host
run where both the Mac Pylon and the Linux Pylon are doing the homework on the
same live lane. A single run-scoped manifest object could not honestly model
two assignment-specific worker manifests for the same window.

## Command

```bash
scripts/release/check-pylon-episode-223-cs336-a1-local.sh
```

## Retained Output

- output root:
  `target/pylon-episode-223-cs336-a1-local/20260414T154240Z-12955/`
- summary:
  `target/pylon-episode-223-cs336-a1-local/20260414T154240Z-12955/SUMMARY.md`

## Result

The local dry run passed.

Passing gates:

- `psionic_machine_manifest`
- `mac_cs336_fallback`
- `linux_cs336_fallback`
- `nexus_named_run`
- `pylon_manifest_mapping`
- `pylon_assignment_intake`
- `pylon_runtime_launch`
- `pylon_terminal_sync`

## What This Proves

- the packaged `psionic-train` CS336 A1 lane still executes through the machine
  manifest path
- weak Apple and consumer-CUDA Linux hosts still promote into the bounded
  `CS336 A1 Demo` lane
- `Nexus` still schedules the named dual-host run locally
- `Pylon` can now claim, materialize, launch, inspect, publish, and terminal-sync
  the assignment-scoped manifest layout

## What It Does Not Prove

- that production `Nexus` is already serving this artifact layout
- that the public Mac and Linux proof hosts are already upgraded to a release
  containing this fix
- that a fresh live `CS336 A1 Demo` run has already completed with both hosts
  participating

## Honest Next Step

The next step is to ship a `Pylon` and `Nexus` build that both contain this
contract, upgrade the live proof hosts, and then retain one live proof bundle
showing both the Mac and Linux Pylons doing the homework on the same named
run/window.
