# Running `coder-worker` on a local executor

`coder-worker` answers NIP-CJ jobs through a door. Besides the Open
Responses door (`CODER_DOOR_KEY`), it can answer through an approved local
executor: a capability under `capabilities/` that the operator approved with
`capability-trust`. Set `CODER_EXECUTOR=<slug>` and leave `CODER_DOOR_KEY`
unset; setting both is refused.

The executor door builds one bounded `delegate::Task` per job and runs it
through `Delegator`, so every job runs under the same capability approval
and filesystem boundary as a `coder` fan-out. There is no unrestricted
fallback: a host with no boundary backend, an unapproved capability, or a
grant that overlaps a protected path refuses the job with a typed status
error, which the terminal reports as `the worker declined (...)`.

## Variables

| Variable | Meaning | Default |
| --- | --- | --- |
| `CODER_EXECUTOR` | Capability slug, for example `devin-local`. | unset (door not used) |
| `CODER_EXECUTOR_WORKDIR` | Directory the executor runs in. | current directory |
| `CODER_EXECUTOR_MINUTES` | Bound for one job, in whole minutes. | `10` |
| `CODER_CAPABILITY_DIR` | Where manifests are read from when the workdir is not inside this checkout. | `capabilities/` of the enclosing repository |

## Layout the boundary accepts

The work directory is protected: the delegate reads it but cannot write it.
The approval's `--writable` path is the adapter's state and must not overlap
the work directory or any ancestor of the adapter's canonical path. Use
three separate directories:

- the checkout that holds the manifest (for example `~/repos/openagents`);
- the work directory the executor runs in (for example `~/worker-exec`);
- the writable state directory (for example `~/worker-jobs`).

## Devin CLI example

The Devin CLI keeps its logs, session database, trusted-workspace list, and
credentials under `$XDG_DATA_HOME/devin` (default `~/.local/share/devin`).
That tree also holds the pinned adapter binary, so it cannot be granted
writable. Point the CLI at a data directory inside the writable grant
instead, copy only the files it needs to start, and trust the work
directory once interactively.

```sh
mkdir -p ~/worker-exec ~/worker-jobs/xdg/devin/cli
cp ~/.local/share/devin/credentials.toml ~/worker-jobs/xdg/devin/
cp ~/.local/share/devin/cli/{installation_id,trusted_workspaces.json} \
  ~/worker-jobs/xdg/devin/cli/
chmod 600 ~/worker-jobs/xdg/devin/credentials.toml
(cd ~/worker-exec && devin)   # answer "Yes, trust", then quit

./target/debug/capability-trust approve devin-local \
  --in ~/repos/openagents --writable ~/worker-jobs

cd ~/worker-exec
XDG_DATA_HOME=$HOME/worker-jobs/xdg \
CODER_CAPABILITY_DIR=$HOME/repos/openagents/capabilities \
CODER_EXECUTOR=devin-local \
CODER_EXECUTOR_WORKDIR=$HOME/worker-exec \
CODER_EXECUTOR_MINUTES=5 \
CODER_RELAY=ws://127.0.0.1:7447 \
CODER_WORKER_SECRET="$(cat ~/.openagents/worker-secret)" \
  ~/repos/openagents/target/debug/coder-worker
```

The worker prints `door executor (devin-local)`. Drive it from the terminal
with `CODER_WORKER=<worker pubkey>` and `CODER_RELAY`; the trace's `Agent`
step records `"model": "devin-local"`.

Measured on 2026-09-20, Linux host with the `bwrap` backend and a local
Postgres relay: `coder -p "Reply with exactly the word pong"` returned
`pong` in 7.4 s wall clock; the worker logged `answered in 7358 ms`.

## As a service

On a host, the worker runs under systemd from
[`deploy/systemd/coder-worker.service`](../../deploy/systemd/coder-worker.service),
with its variables in an environment file installed from
[`deploy/coder-worker.env.example`](../../deploy/coder-worker.env.example).
The executor door needs one more file, the drop-in
[`deploy/systemd/coder-worker-executor.conf`](../../deploy/systemd/coder-worker-executor.conf),
because the base unit forbids the namespaces `bwrap` builds. The approval
is recorded as the service user into the store the environment file
names. [`deploy/README.md`](../../deploy/README.md) walks through the
install, the key, the approval, and the smoke test from another machine.

## Serving delegations

The same worker is the adapter behind the `devin-relay` capability
([`delegate.md`](delegate.md#delegating-over-the-relay)). A terminal with
`CODER_DELEGATE=devin-relay` sends a probe, then one job per task; the
worker runs a reading task in a scratch checkout of
`CODER_EXECUTOR_WORKDIR` and a writing task in a retained worktree under
the writing argv, both under this host's approval and boundary.

Two settings matter here that a single-turn worker does not care about:

- **`CODER_EXECUTOR_WORKDIR` must be a checkout of the repository the
  tasks ask about.** The terminal's repository never reaches the worker;
  a job names paths and expects the worker to have them. In an empty
  directory the Devin CLI reaches for a shell command, which its
  non-interactive mode rejects, and exits `0` having printed nothing;
  the worker reports that as `devin-local exited cleanly and printed
  nothing`, a typed `internal` refusal, and the terminal records the
  delegation as failed. Clone the repository there and check out the
  commit the tasks are pinned to.
- **`CODER_WORKER_JOBS` bounds admission.** Unset, an executor door takes
  the manifest's `concurrent_max` (6 for `devin-local`). A job past the
  bound is refused `busy` before anything runs. Set it to what this host's
  executor account can carry.

The worker log for a fan-out reads `probed`, then one `delegated: reading
task, N min` per admitted job, then `answered in N ms` or `declined:
<code>` for each; the job IDs are the request event IDs the terminal's
trace records under `relayed.request`.

## Failures you will meet

- `names no capability this host can see` — the workdir is outside the
  checkout and `CODER_CAPABILITY_DIR` is unset.
- `present and unavailable here: untrusted_workspace` — the CLI refuses the
  work directory; trust it interactively.
- `writable path X overlaps protected path X` — the writable grant is the
  work directory; separate them.
- `exited 101 ... Read-only file system` — the executor writes state
  outside the grant; relocate its state directory into the grant.

## Troubleshooting

Two refusals come from the host environment rather than the job:

- **`Refusing to run in an untrusted workspace`.** The Devin CLI keeps its
  trusted-workspace list under `$XDG_DATA_HOME/devin`, so this setup leaves
  two trust stores: the worker's, under `$HOME/worker-jobs/xdg`, and the
  default `~/.local/share/devin`. A `coder -p` or `coderbench run` started
  from a shell without `XDG_DATA_HOME` consults the default store, where
  the checkout is untrusted, and the workspace probe answers
  `present_unavailable`. Run the driving command with the same
  `XDG_DATA_HOME` the worker uses, or trust the checkout in both stores.
- **Preflight reports `origin is <rewritten URL>`.** A global
  `url.<base>.insteadOf` entry rewrites the URL that
  `git remote get-url origin` returns. CoderBench preflight reads the
  checkout's configured `remote.origin.url` instead
  ([#9448](https://github.com/OpenAgentsInc/openagents/issues/9448)), so a
  `coderbench` older than that fix needs `GIT_CONFIG_GLOBAL=/dev/null`,
  which also hides every other global Git setting.
