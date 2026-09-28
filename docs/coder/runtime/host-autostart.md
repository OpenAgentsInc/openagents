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
| `--model ID` | `gpt-6-luna` | The Codex model each eligible task records and its grant admits, when no `--route` is given. |
| `--route PROVIDER:MODEL` | None | An admitted provider (`codex` or `claude`) and model, in preference order. Repeat for more, up to five. The first route's model is the one each task records. Use instead of `--model`. See [Routes and capacity](#routes-and-capacity). |
| `--probe-usage` | Off | Read each admitted provider's usage windows before routing, with its local login, and prefer a route below the threshold. See [Usage probes](#usage-probes). |
| `--usage-threshold PERCENT` | `90` | The utilization, 1 to 100, at or above which a probed provider is passed over. Implies `--probe-usage`. |
| `--effort LEVEL` | `medium` | `low`, `medium`, `high`, or `xhigh`, for every route. |
| `--max-steps N` | `24` | The engine's step limit, 1 to 128. |
| `--wall-seconds N` | `1800` | Each command's wall-clock limit, 1 to 3,600. |
| `--memory-mib N` | `4096` | Each command's memory limit, 64 MiB to 8 GiB. |
| `--read-only` | Off | Grant a read-only workspace. Without it, the workspace must be an isolated Git worktree whose common Git directory is outside it. |
| `--full-access` | Off | Run each task's commands as you, with no sandbox, network access, and your login-shell environment. For your own computer only. See [Full access](#full-access). |
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

### The task workspace

Tasks run in the directory the workspace label names in the host's
`serve.json`, not in your own checkout. The recommended default is one
detached worktree per host, beside your checkout: `~/work/openagents-host-tasks`
on a Mac, `~/openagents-host-tasks` on Linux. It gives each task a real
checkout of the repository, while its commits, hooks, and your working
branch stay out of the engine's reach, even with full access, unless a task
goes looking for them. Refresh it between tasks with
`git -C ~/work/openagents-host-tasks checkout --detach origin/main` after a
fetch.

The directory must be the top level of a Git checkout. An empty or plain
directory inside another repository answers Git with that repository, so
`on` and the task owner both refuse it (`is not the top level of a Git
checkout`). A task that ran in such a directory saw an empty folder.

## Turn it off

```sh
coder host autostart off
```

The next creation and the next sweep read the file again, so no restart is
needed. Tasks already started keep running under their grants; cancel one
from a device or with `coder task cancel`. Queued tasks stay queued, as
without a policy. `coder host autostart show` prints the policy and the
latest decisions, and whether each model provider is connected, has
capacity, and how much of each usage window it has used; pass `--store DIR`
when the task store is not `~/.openagents/tasks`. It probes usage when the
policy does, or once when you pass `--probe-usage`.
Deleting `autostart.json` also turns it off.

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

## Routes and capacity

A policy admits one or more routes, each a provider and a model, in the
owner's order of preference:

```sh
coder host autostart on --workspace openagents \
  --route codex:gpt-6-luna --route claude:claude-opus-5-5
```

A Claude route's model is the exact name Claude Code reports for the served
model, because the engine refuses a reply from a model other than the admitted
one. A policy written before routes existed has one route, the Codex login
with its `model`, and keeps exactly that meaning.

Each start chooses its route when it starts, not when the task was created:

1. **Connected.** With more than one route, the host skips a provider without
   a usable local login: a Codex login in `~/.codex/auth.json` (or
   `$CODEX_HOME`) whose access token is not about to expire, or a `claude`
   binary with a Claude Code sign-in. The probe makes no network request and
   reads no credential into a log. A one-route policy skips the probe, as
   before, and a login problem shows in the task's diagnostic file.
2. **Capacity.** The host skips a provider whose usage or rate limit holds in
   the capacity book, `capacity.json` in the task store (mode `0600`). The
   engine records a refusal there when a provider refuses a generation: a
   Codex HTTP 429 `usage_limit_reached` with its `resets_at`, or a Claude Code
   error result with API status 429, which holds 30 minutes because Claude
   Code does not report the reset. The book keeps one entry per provider:
   the kind of limit, when it was observed, and until when it holds.
3. **Grant.** The grant names the first connected route with capacity. The
   other connected routes follow as its `fallbacks`, in preference order.
   The task records the policy's first model either way; the task owner
   admits a grant whose routes include it. The run's transcript opens with
   a `route_capacity` step that names each route's recorded refusal, so a
   task that runs on a later route shows why.

When no connected route has capacity, the host does not start a run that
cannot succeed. It appends a `no_capacity` entry with the earliest reset,
then ends the task with a cancellation whose reason names that time. The
device's activity summary reads, for example, `No model capacity until
2026-10-03 18:07 UTC`, and the phone shows it under the task's `Stopped`
phase.

During a run, the engine fails over. When a provider refuses a generation for
a usage or rate limit, the engine records the refusal, appends a System step
with a `route_switch` extension to the ATIF transcript, and generates the
same step again on the next admitted route with capacity. When none is left,
the run ends with the `no_capacity` ending and the earliest reset, and the
device sees the same headline. The step's cost adds every attempt's cost, so
`usd`, `usd_upper`, and `cost_unknown` stay honest across a failover. Read
[the repository adapter](microcoder-repository.md#fallback-routes-and-capacity).

Failover only chooses among the routes the owner admitted. It never adds a
model, widens a limit, or spends beyond the policy's bounds.

## Usage probes

Refusals tell the host a provider is out only after a request fails. With
`--probe-usage`, the host also asks each admitted provider how much of its
allowance is used, before it routes a waiting task:

| Provider | Endpoint | Windows read |
| --- | --- | --- |
| Claude | `GET https://api.anthropic.com/api/oauth/usage` with `anthropic-beta: oauth-2025-04-20` | `five_hour` and `seven_day`: `utilization` (percent) and `resets_at` |
| Codex | `GET https://chatgpt.com/backend-api/wham/usage` | `rate_limit.primary_window` and `secondary_window`: `used_percent`, `limit_window_seconds`, `reset_at`; and `limit_reached` / `allowed` |

```sh
coder host autostart on --workspace openagents \
  --route codex:gpt-6-luna --route claude:claude-opus-5-5 --probe-usage
coder host autostart show
# codex: connected; capacity; usage primary 100% until 2026-10-03 18:07 UTC (limit reached)
# claude: connected; capacity; usage five_hour 4% until 2026-09-28 15:49 UTC, seven_day 66% until 2026-09-29 20:59 UTC
```

- **Typed readings.** Each answer is parsed into typed windows (a used
  fraction, a reset, a length) and kept in `usage.json` in the task store,
  mode `0600`, beside `capacity.json`. The book holds no token, account
  identifier, or response text.
- **Advisory.** Routing passes over a route whose provider is at or above the
  threshold in any window, or reports its limit reached, for a later admitted
  route with capacity below it. When every route with capacity is near its
  limit, the first still starts: only a recorded refusal ends a task as
  `no_capacity`. A probe never adds a route and never overrides a refusal.
- **Cached.** Each provider is asked at most once a minute, and only when a
  task is waiting to start. A failed probe waits five minutes; a
  `Retry-After` is honored up to six hours. A reading older than 15 minutes
  is not used.
- **Degrades to refusals.** A missing or expired credential, a refused or
  rate-limited probe, a malformed body, or no network is recorded as a typed
  failure (`no_credential`, `expired`, `unauthorized`, `rate_limited`,
  `status`, `malformed`, `network`), and routing uses recorded refusals only.
  Both endpoints are private and undocumented, so expect this path.
- **Credentials.** A probe reads the provider's OAuth access token: the Codex
  login in `~/.codex/auth.json` (or `$CODEX_HOME`), and Claude Code's
  `claudeAiOauth.accessToken` from `~/.claude/.credentials.json` or, on macOS,
  the `Claude Code-credentials` keychain item for your account, read with
  `/usr/bin/security` as Claude Code reads it. The `coder` host process reads
  it only while probes are on, sends it only to that provider's usage
  endpoint, and never writes, logs, or stores it. It never changes a
  credential store. Without `--probe-usage`, no credential is read for a
  probe.

Each start routed with probes appends a `usage` journal entry naming every
admitted provider's windows.

## Full access

The owner can give auto-started tasks the same access a terminal on the
host has:

```sh
coder host autostart on --workspace openagents \
  --route codex:gpt-6-luna --route claude:claude-opus-5-5 --full-access
```

Each grant then carries `"access": "full"`, and the engine's commands:

- run as you, with the admitted shell and no sandbox: no `sandbox-exec`
  profile on macOS, no namespaces on Linux, and no write boundary, so Git,
  `ps`, Xcode's libraries, and everything else your account can use work;
- reach the network;
- get your login-shell environment, read once when the task is admitted by
  running your shell (`$SHELL -l -i`, else the account's shell) with
  `env -0`: the `PATH` with Homebrew, `~/.cargo/bin`, a Node version
  manager, and the rest, and your real `HOME`, `USER`, and `LOGNAME`. A
  shell that cannot answer in 20 seconds leaves a fixed fallback `PATH` of
  the usual tool directories. Variables whose names end in `_API_KEY`,
  `_TOKEN`, or `_SECRET` are left out.

A Claude route's generation call also passes `--permission-mode
bypassPermissions`. That call has every Claude Code tool off, as before, so
Claude Code itself still runs nothing; Microcoder runs the commands. The
Codex route calls the Codex Responses endpoint directly and has no sandbox
or approval setting of its own.

The admission record says `network: host_network` and `read_scope:
host_user`, and the trace's admission step names the environment's source,
shell, `PATH`, and variable names, never another value. The engine is told
that it runs with full access, so it uses the network and your tools
instead of working around a sandbox.

What full access gives up: the task store, the repository's Git directory,
and everything else your account can write are no longer protected from
the engine's commands, so a task's retained evidence is only as trustworthy
as the commands it ran. Use it only on your own computers, for tasks your
own admitted devices create. Every other check still applies: a device
still needs `operate` under a host-signed grant, the workspace must be one
the policy lists and the host admits, and the routes, limits, and
concurrency bound are unchanged. Turn it back off by running `on` again
without `--full-access`. `access` is absent from a policy or grant written
without it, which keeps the boundary.

## The decision journal

Each decision appends one line to `~/.openagents/host/autostart.jsonl`
(mode `0600`), with schema `openagents.coder.host-autostart-entry.v1`:

| `event` | Meaning |
| --- | --- |
| `eligible` | A device created the task under the policy. |
| `started` | The owner process started, with its process ID, the grant digest, and the chosen route and fallbacks. It is not an admission receipt; read the task. |
| `skipped` | The task was cancelled or gone, the policy stopped listing its workspace, or the policy's model changed after it was created. |
| `refused` | The owner process could not start, or no admitted provider is connected, with the reason. |
| `no_capacity` | No connected admitted provider had capacity. `resets_at` is the earliest reset, in Unix seconds, when known. The task was cancelled with that reason. |
| `usage` | With usage probes on: each admitted provider's probed windows, or why it has none, when the task was routed. |
| `unadmitted` | A started task was still queued 120 seconds later: its owner process refused it. The reason is in the task store's `repository-launch-TASK-*.jsonl` diagnostic. |
| `policy_on`, `policy_off` | The owner changed the policy, with its bounds. |

Entries never hold a prompt or a title. An entry about a later turn of a
continued task carries `turn`, the task revision that turn started at; the
first turn leaves it out.

## Follow-up turns

A device continues a chat with NIP-HOST `task.command` (`send`, a `queue`
promotion, or an emulated `steer`). Each starts the task's next turn, which
is queued exactly like a new task: the inbox records it as `eligible` with
its `turn`, and the sweep starts it only when the policy is on, lists the
task's workspace, still names the task's model, and has a free slot, through
the same admitted routes and a fresh operator grant at the new revision.
With the policy off, a follow-up turn waits, inert, like any submission. The
engine gets the new message after the earlier turns it read from the task's
own traces (at most eight, each message at most 4 KiB), and the new turn's
trace, `TASK.N.atif.jsonl`, carries them as steps marked `carried_from`.

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
- A task generates only through routes the policy admits. Choosing a route
  at start and failing over during a run pick among those routes only, and
  a task with no admitted provider that has capacity ends instead of
  starting.
- Usage probes read a provider credential only when the owner turned them
  on, and a probed reading only reorders admitted routes.
- A device's follow-up turn starts under exactly these bounds, or not at
  all; a command that waits for a turn to end runs only while its sender
  still holds `operate` under the same grant and epoch.
- Full access is off unless the owner turns it on with a command on the
  host. It changes what the engine's commands may reach, never who may
  create a task or which workspaces, routes, and limits apply.

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
- The engine needs a login for each route it uses and the host's Jev key, as
  `microcoder repository` does. A refusal from either shows in the task's
  diagnostic file, not in the journal.
- Without usage probes, capacity is learned from refusals, so the first task
  after a limit is reached still makes one refused request. A Claude refusal
  holds for 30 minutes at a time because its reset is not reported. With
  probes, a reading only reorders routes; a probed Claude reset is not
  copied into the capacity book.
- The phone does not show probed windows yet; read them with
  `coder host autostart show`.
- A task the policy ended for lack of capacity stays ended. Create it again
  after the reset.
