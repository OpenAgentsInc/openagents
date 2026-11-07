# 2026-11-07: The owner's Mac froze and needed a hard reboot

## Summary

At 12:11 CST the Mac's system-wide open-file table (`kern.maxfiles`, 491,520
entries) filled. From then on, every new process and every file open failed
with `ENFILE` ("Too many open files in system"). macOS could not even page in
the `/usr/bin/pgrep` binary, so shells, agents, Finder and system daemons all
failed. The machine stayed unusable until the owner hard-rebooted it at 13:03.
Uptime before the reboot was about 11.5 days.

## Evidence

- `~/Library/Logs/DiagnosticReports/pgrep-2026-11-07-121152.ips`: `pgrep`
  died with `SIGBUS` because the kernel could not read its compressed binary
  (`decmpfs fetch callback failed`, `arg = 0x17`, errno 23 = `ENFILE`).
- The unified log has "Too many open files in system" from 12:11:39 onward:
  121 lines that minute, about 1,200 a minute from 12:21 to 12:25, and nothing
  after 12:33. `launchd` could not spawn `xpcproxy`, so no new services could
  start. The processes in those lines (`biomed`, `launchd`, `BiomeAgent`,
  `cfprefsd`) were victims that failed to open files, not the processes
  holding the table.
- The agents' last readings of `kern.num_files` failed because `sysctl` itself
  could not start, so no reading names the process that held the files.

## What was running

- Four Claude subagents, each in its own worktree, building Everglade
  (`wasm.sh`, `coax test`) and measuring it with headless Chrome in loops.
- Five `coax serve` dev servers (one per agent, plus port 8861).
- The `com.openagents.codex-runner-pool` Codex lane in
  `~/.codex/worktrees/issue-fixes`, also building and running headless
  Chrome flights.
- `com.openagents.coax-publish`, which runs compiler mutation testing with
  about 18 parallel workers.
- The owner's own apps: Chrome, ChatGPT, Battle.net, Epic Games, CleanMyMac.
- 245 git worktrees under `openagents`.

## Cause

The logs can't name the single process that held the table, because the table
was already full when anyone tried to look. One cause is confirmed by
reproduction, and two conditions made it possible:

1. **Orphaned headless Chromes (reproduced).** `coax/lib/net/cdp.coax`
   starts Chrome in its own process group and kills it only in `cdp_close`.
   When a tool is killed first (a command timeout, `kill`, or an agent
   stopping), `cdp_close` never runs. Chrome is reparented to `launchd` and
   keeps running indefinitely, holding about 265 files in its main and GPU
   processes. A WebGPU page holds more. Killing a `shot` run reproduced this
   on 2026-11-07 at 13:15. The agents' measurement loops ran hundreds of
   browser tool runs that day.
2. **No per-process ceiling.** Shells here run with a soft limit of 1,048,576
   files per process, and `kern.maxfilesperproc` is 245,760. Two leaking
   processes can fill the whole system table before either one fails.
3. **Too much concurrent work on one Mac.** Four agents, five servers, the
   Codex lane and a mutation fan-out all ran at once. On 2026-11-02 the owner
   reported desktop lockups under the same mix
   (`docs/verification/anime-cli-2026-11-02/resource-recovery/event.json`).

## Fixes

- `scripts/mac/fd-guard.sh`, installed as the LaunchAgent
  `com.openagents.fd-guard` and run every 30 seconds:
  - kills orphaned headless Chromes: the main process of a
    `--headless=new` Chrome with its own profile, whose parent has died.
    Tested at 13:17: it killed the orphan and left the live Chromes alone.
  - at 50% full, writes the 25 processes holding the most files to
    `~/.openagents/logs/fd-guard/`, so the next incident names its cause.
  - at 80% full, shows a notification.
- Agents count toward one machine budget: at most two heavy builds or
  browser measurement loops at a time, and no `coax-publish` mutation run
  while agents build.

## Follow-ups

- `cdp.coax`: kill a Chrome whose owner has died at its next launch (record
  the owner's pid in the profile name), so cleanup no longer depends on the
  guard.
- Lower `kern.maxfilesperproc` so a single leaking process fails by itself
  instead of taking down the system. This needs `sudo`, so it is an owner
  step.
