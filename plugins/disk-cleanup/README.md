# Disk cleanup

This plugin scans. It does not delete.

Version 0.4.0 makes the plugin read-only. Its rule has no triggers, no
actions, and no delete grants, so the host never runs it on its own and a
manual run deletes nothing. What it does is a full scan: every candidate folder
in the deletable classes, with its size, how long since it was last used, and
why it qualifies; every folder kept, and why; and the folders that are not
known caches, with their sizes.

Deleting is left to an agent, which takes the scan's list and removes what the
person chooses. That step is not part of this plugin yet.

## Run a scan

Enable the plugin once, then scan:

```sh
openagents plugin enable disk-cleanup
openagents --json background scan disk-cleanup
```

Without `--json` the scan prints the same as readable lines. Its `skills/disk-scan.md`
tells Coder how to run and present it.

The scan applies the same checks a run would: it keeps anything in use, anything
with unsaved or unpushed work, anything with a live task, and anything outside
the allowed roots. The rule's classes and safety lists are the ones the built-in
`disk` rule uses, and a test keeps the two in step.

## What changed from 0.3.0

Versions 0.1.0 to 0.3.0 deleted caches on a schedule, with an audit log and
worktree undo. Version 0.4.0 removes the triggers and actions. If you enabled
an earlier version, the saved rule still has its old actions until it is
reinstalled and the saved copy removed:

```sh
openagents background pause disk-cleanup
openagents plugin install plugins/disk-cleanup
```

Reinstalling replaces the package, not your saved rule. Remove
`~/.openagents/background/rules/disk-cleanup.json` after pausing, then run
`openagents plugin enable disk-cleanup` again.

The built-in `disk` rule is separate. It still deletes when it is on. This
plugin does not change it.

## Safety

The scan changes nothing: it writes no size cache and no audit record, and it
does not move or delete any folder. Unknown or unverifiable safety evidence
keeps a candidate out of the list's reclaimable total, and it shows the reason.
