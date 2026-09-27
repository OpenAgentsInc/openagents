# Host auto-start policy

A task that an enrolled device creates with NIP-HOST `task.create` is an
inert submission: the durable inbox records it, and nothing runs until the
host's local operator starts it with an execution grant. The auto-start
policy lets the host's owner say, once and locally, that tasks created in
chosen workspaces start on their own within stated bounds. It is off by
default.
[Issue #9735](https://github.com/OpenAgentsInc/openagents/issues/9735)
delivers it as part of the
[linked devices program](https://github.com/OpenAgentsInc/openagents/issues/9736).
The implementation is `coder::task::autostart` in
[`crates/coder`](../../../crates/coder/src/task/autostart.rs).

## Turn it on

The policy lives in the host root, `~/.openagents/host/autostart.json`
(mode `0600`), and only a command on the host itself changes it. No device
operation, relay message, or grant can.

```sh
coder host autostart on --workspace openagents --max-running 1
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--workspace LABEL` | Required | A workspace label the host admits, from `coder host init` or `coder link setup --workspace`. Repeat for more. |
| `--max-running N` | `1` | At most N auto-started tasks run at once, 1 to 8. The rest wait queued. |
| `--model ID` | `gpt-6-luna` | The model each eligible task records and its grant admits. |
| `--effort LEVEL` | `medium` | `low`, `medium`, `high`, or `xhigh`. |
| `--max-steps N` | `24` | The engine's step limit, 1 to 128. |
| `--wall-seconds N` | `1800` | Each command's wall-clock limit, 1 to 3,600. |
| `--memory-mib N` | `4096` | Each command's memory limit, 64 MiB to 8 GiB. |
| `--read-only` | Off | Grant a read-only workspace. Without it, the workspace must be an isolated Git worktree whose common Git directory is outside it. |
| `--controller PATH` | `microcoder` beside `coder`, else `~/.openagents/bin/microcoder` | The engine executable. |
| `--decision-endpoint URL`, `--decision-model ID` | `https://api.typesafe.ai`, `jev-1.13.0` | The Jev client the engine's grant names. Use an exact version: the engine refuses a reply whose model differs from the admitted one, so an alias such as `jev-latest` fails at the first judgment. |

Every command also takes `--root DIR` for a host root other than
`~/.openagents/host`. `on` refuses a label the host does not admit, and a
writing policy on a checkout that holds its own Git directory; create a
worktree for the host instead:

```sh
git -C ~/work/openagents worktree add --detach ~/work/openagents-host-tasks origin/main
coder link setup --workspace openagents=$HOME/work/openagents-host-tasks
```

## Turn it off

```sh
coder host autostart off
```

The next creation and the next sweep read the file again, so no restart is
needed. Tasks already started keep running under their grants; cancel one
from a device or with `coder task cancel`. Queued tasks stay queued, as
without a policy. `coder host autostart show` prints the policy and the
latest decisions. Deleting `autostart.json` also turns it off.

## What it does

While the policy is on:

1. A device holding `operate` creates a task in a listed workspace. The host
   checks the device's grant and right as always, then the inbox records the
   task with the engine's model instead of none, and appends an `eligible`
   entry naming the task, the device, and the workspace.
2. The host starts eligible tasks in creation order while fewer than
   `max_running` auto-started tasks are running. For each, it writes a
   closed execution grant, `openagents.coder.task-execution-grant.v1`, with
   the task's intent digest and revision, the canonical system shell, the
   engine's configuration, and the policy's limits, to
   `~/.openagents/host/autostart/`, and runs
   `microcoder repository --grant FILE --store ~/.openagents/tasks --detach`.
   The task owner admits it exactly as it admits a hand-written grant: the
   same filesystem boundary, supervisor, ATIF transcript, and retained
   artifacts.
3. A task that waits for a slot starts on a later sweep, every 10 seconds.
   A started task counts against the bound while it runs, or for 120
   seconds while its owner process admits it.

Each decision appends one line to `~/.openagents/host/autostart.jsonl`
(mode `0600`), with schema `openagents.coder.host-autostart-entry.v1`:

| `event` | Meaning |
| --- | --- |
| `eligible` | A device created the task under the policy. |
| `started` | The owner process started, with its process ID and the grant digest. It is not an admission receipt; read the task. |
| `skipped` | The task was cancelled or gone, the policy stopped listing its workspace, or the policy's model changed after it was created. |
| `refused` | The owner process could not start, with the reason. |
| `unadmitted` | A started task was still queued 120 seconds later: its owner process refused it. The reason is in the task store's `repository-launch-TASK-*.jsonl` diagnostic. |
| `policy_on`, `policy_off` | The owner changed the policy, with its bounds. |

Entries never hold a prompt or a title.

## Invariants

This policy relaxes one invariant, and the [invariant ledger](../../../INVARIANTS.md)
records the change:

- With no policy file, a malformed one, or `enabled: false`, `task.create`
  is exactly the inert submission it was: no model is recorded, no entry is
  written, and nothing starts.
- A device cannot turn the policy on, widen it, or choose the engine, the
  model, or a limit. It still sends only a label, a title, and a prompt.
- The grant the policy writes is a normal operator grant, so every
  admission check of the task owner still applies.
- At most `max_running` auto-started tasks run at once, and each start is
  recorded before the next decision.

`coder::task::autostart` tests cover each: creation without a policy, the
workspace allowlist, the concurrency bound and its wait, cancellation and a
policy turned off, retries across a policy change, bounds validation, and
the command line.

## Limits

- The bound counts auto-started tasks only. A task you start by hand does
  not count.
- The task owner refuses two unresolved tasks on the same workspace, so a
  `max_running` above the number of listed workspaces adds nothing: a second
  start on a busy workspace is refused at admission, stays queued, and shows
  the refusal in its diagnostic file.
- Writing tasks share one worktree. Consecutive tasks see each other's
  uncommitted changes; review and commit or reset between them.
- The engine needs the host's Codex login and Jev key, as `microcoder
  repository` does. A refusal from either shows in the task's diagnostic
  file, not in the journal.
