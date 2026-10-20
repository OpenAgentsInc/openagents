# Disk cleanup

Scan old Cargo outputs and unused worktrees, review the list, and apply a saved
plan. The plugin itself stays read-only: no triggers, actions, or delete grants.

## Install and scan

From the repository, install the Rust tool and the package:

```sh
scripts/dev/install-disk-janitor.sh
openagents plugin install plugins/disk-cleanup
openagents-disk-cleanup migrate-plugin
openagents-disk-cleanup scan --age-hours 24 --plan cleanup-plan.json --json
```

`migrate-plugin` enables the scan-only package. If an earlier version left a
saved deleting rule, it backs that rule up beside the saved rule and replaces
it with the package's read-only settings. It refuses a package that still asks
for automatic deletion. Older hosts can install the tool without rebuilding
the desktop app. `openagents background scan disk-cleanup` remains available
for the host's rule-based scan; use the tool above for the full target and Git
worktree inventory.

The scan discovers shared targets, source-keyed build state, custom targets
registered by the Cargo build helper, and older targets in common working
folders. It asks Git for linked worktrees, including those under `~/wt` and
agent scratch. Unknown folders stay. Use `disk-janitor.keep` to protect a path.

## Review and apply

The plan contains the exact scope, minimum age, candidate sizes, and reasons
for keeping folders. Previewing removes nothing. After reviewing it:

```sh
openagents-disk-cleanup apply --plan cleanup-plan.json --receipt cleanup-receipt.json
```

To apply only numbered entries from the scan, add `--only 1,3`. Create a new plan
for another cleanup; existing plans and receipts are never overwritten.
`--builds-only` and `--worktrees-only` restrict a scan to that category.

The minimum age is 24 hours and does not shrink when free space is low. Apply
rechecks each entry's files, age, processes, locks, and Git state. A changed or
busy entry stays. Cargo target folders, lock files, and compiler state markers
stay. Fresh outputs in an otherwise old target stay. A worktree must be clean,
fully merged into `origin/main`, and saved on a remote. Ignored notes, unsaved
changes, Git locks, live tasks, spare checkouts, and active processes keep it.
A receipt records each result and the information needed to restore worktrees.

Linux reads `/proc`, including executable paths, mapped files, and build
folders from Cargo's environment. If a login service hides its process paths,
the tool tries an already-authorized, noninteractive `sudo` read. If that is
unavailable, cleanup waits. It never prompts for a password or prints process
environments. macOS uses `lsof`.

Nix garbage collection, model downloads, benchmark results, scratch sessions,
package caches, and installed application versions are outside this command.

## Optional scheduled previews

`scripts/dev/install-disk-janitor.sh --schedule` installs a read-only scan every
ten minutes with systemd on Linux or launchd on macOS. The timer never applies
a plan. Linux results are in `journalctl --user -u openagents-disk-cleanup`;
macOS results are in `~/.openagents/logs/disk-cleanup-scan.log`.

The old `disk-janitor.sh` entry point now calls this tool and previews by
default. Its `--low` flag cannot lower the minimum age or add cleanup categories.
