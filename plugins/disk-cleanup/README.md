# Disk cleanup

This local plugin uses the host's background cleaner, not a shell script or
an independent deletion process. Installation leaves it off. A running host
checks enabled rules every 5 minutes (every minute while free space is below
the start level), when a task ends, and at host start.

The rule starts below `max(200 GB, 15%)`, stops at `max(300 GB, 20%)` or after
freeing 100 GB, and treats space below `max(10 GB, 1%)` as an emergency.
GB means 1,000,000,000 bytes. The host applies a 10-minute cooldown; an
emergency bypasses it.

## Preview before activation

Use an `openagents` build from this repository that supports local background
plugins. Older installed binaries that only list Wasm plugins do not support
these commands.

1. Install from the repository root:

   ```sh
   openagents plugin install plugins/disk-cleanup
   ```

2. Enable the plugin and preview its plan. The packaged rule starts paused,
   so background checks cannot delete anything before you resume it:

   ```sh
   openagents plugin enable disk-cleanup
   openagents background run disk-cleanup --dry-run
   ```

3. After reviewing the preview, activate background cleanup:

   ```sh
   openagents background resume disk-cleanup
   ```

To stop it, run `openagents plugin disable disk-cleanup`. To pause it without
uninstalling, run `openagents background pause disk-cleanup`.

Fresh hosts also leave the built-in `disk` rule off. An existing saved rule
keeps its setting. If you previously enabled the built-in monitor, pause it
with `openagents background pause disk` to avoid two cleanup policies.

## Safety, log, and undo

The host permits only ended tasks' build folders, stale build folders,
clean and pushed linked worktrees of ended tasks or worktrees with no task
record that have been unused for at least 7 days, clean, pushed, and unlocked
Claude Code worktrees idle for 2 hours, the scratch of agent sessions that
ended and left it unchanged for 7 days, idle gate builds, a kache collection
(through kache's own collector), incremental caches, and background trash in
an emergency. Plugin worktrees without an ended task record follow the
built-in monitor’s 7-day policy. Agent target directories are stale after 6
hours.

Version 0.2.0 added Claude Code worktrees, kache, the higher levels, and the
one-minute check under pressure. A rule you resumed or edited is saved in
`~/.openagents/background/rules/disk-cleanup.json`, and that file wins over
the package, so it keeps 0.1.0's classes and levels; only the 6-hour agent
staleness applies to it without changes. To move it to 0.2.0, install the
plugin again from the repository root, remove that file, preview with
`openagents background run disk-cleanup --dry-run`, and run
`openagents background resume disk-cleanup`.

Version 0.3.0 added agent scratch under `~/.openagents/scratch`
(`openagents scratch`). A saved rule from an earlier version leaves scratch
alone until you move it to 0.3.0 the same way.

The host checks task activity, locks, open files, working directories, Git
status, remote commit coverage, symlinks, volume boundaries, and its protected
state paths. Unknown or unverifiable safety evidence keeps the candidate.
The allow roots are not permission to delete arbitrary files under `~/work`
or `~/code`. Source checkouts and unsaved or unpushed work stay protected.

Inspect the audit log and restore removed worktrees:

```sh
openagents background log disk-cleanup
openagents background undo RUN_ID
```

The host stores the log under `~/.openagents/background/runs.jsonl`, with
rotation. Undo recreates worktrees from their recorded repository, branch,
and commit; it does not restore disposable build caches. Dry runs change
nothing and do not append deletion records. A later cleanup rechecks safety
and disk space rather than treating an old preview as authorization.

The package pins the rule's content digest. After editing the packaged rule,
update its digest before reinstalling. Per-computer edits use
`openagents background edit disk-cleanup` and remain subject to host admission.
