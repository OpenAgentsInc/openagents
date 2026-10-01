# The Grok Build route

Coder can hand a repository turn to the local Grok Build CLI, as it hands
turns to Devin and OpenCode. The supported coding agents for delegation are
Claude Code, Codex, Devin, OpenCode, and Grok Build. Grok Build runs on the
host as `grok agent stdio`, the CLI's Agent Client Protocol (ACP) server,
with the CLI's own login.
[Issue #10052](https://github.com/OpenAgentsInc/openagents/issues/10052)
delivers it.

The terminal delegate door stays Microcoder, then Claude Code, then Codex
CLI. A `grok` route is selected on a repository task, the same way a
[Devin route](devin.md) or an [OpenCode route](opencode.md) is.

The pieces:

| Piece | Where |
| --- | --- |
| The ACP client and Grok Build's specifics | [`crates/acp-client`](../../../crates/acp-client/src/lib.rs), `acp_client::grok` |
| The repository turn on a Grok Build route | [`microcoder::repository`](../../../crates/microcoder/src/repository/grok.rs) |
| The `grok:MODEL` route, its connection probe, and the capacity book | `coder::task::autostart`, `microcoder_loop::capacity::Provider::Grok` |
| The steering row | `coder_delegate::steering::GROK_ACP` |

## Turn it on

Sign in to Grok Build on the host once, as you would to use it yourself
(`grok`, then log in), or set `XAI_API_KEY`. Then admit a Grok Build route
in the host's auto-start policy:

```sh
coder host autostart on --workspace openagents --full-access \
  --route grok:default --route codex:gpt-6-luna --probe-usage
```

`grok:default` keeps the model Grok Build chooses. `grok:MODEL` names an
exact model id, such as `grok:grok-4.6`. Under full access the host starts
`grok agent --always-approve --model MODEL --no-leader stdio` and refuses
the turn when the session reports a different model. A model id is 1 to 128 characters,
starts with a letter or digit, and contains only letters, digits, `.`,
`_`, and `-`.

`coder host autostart show` reports whether Grok Build is connected: a
`grok` binary (`GROK_BIN`, else `grok` on `PATH`, else `~/.local/bin/grok`,
else `~/.grok/bin/grok`) and a login. The login is
`$GROK_HOME/auth.json`, or `~/.grok/auth.json`, when that file exists and
is not empty, or a non-empty `XAI_API_KEY`. The connection probe checks
the file's size and whether the variable is set, and never logs the
variable. A turn outside full access copies the file into its private Grok
home (see [Access](#access)) and reads only its `expires_at` values.

A person's own local runs allow Grok Build by default: the default provider
list is Codex, then Claude Code, then Grok Build
([#10091](https://github.com/OpenAgentsInc/openagents/issues/10091)), so
"do a test delegation to grok" runs Grok Build when it is signed in here, and
says so plainly when it is not. The desktop's auto-start switch admits the
same three routes for a first policy. The desktop's sidebar and Settings →
Coder show Grok Build when it is installed; it has no usage endpoint, so its
row shows no meter. OpenCode (no default model) and Devin (a paid API) stay
off until named.

## What a turn does

A repository run whose route is Grok Build does not run the Microcoder
step loop. The task owner admits the grant exactly as for any other route,
then:

1. Starts `grok agent --no-leader stdio` in the task's workspace, as the
   leader of its own process group. Full access adds `--always-approve`
   and uses the owner's login-shell environment, less every variable named
   `*_API_KEY`, `*_TOKEN`, or `*_SECRET`, with `XAI_API_KEY` put back when
   that source set it. At this computer's toolchains the process runs
   inside the host's boundary, with the boundary's environment (see
   [Access](#access)).
2. Sends `initialize`. It never sends `authenticate`.
3. Opens a session with `session/new`, or, on a later turn of the same
   task under full access, reattaches the session the earlier turn used
   with `session/load`.
   The new session's `_meta` carries the engine mark,
   `{"openagents.com/engine": "openagents-coder-engine"}`.
4. Checks the reported model.
5. Sends the turn's message with `session/prompt` and records what Grok
   Build streams in the task's transcript: each reply segment, reasoning,
   each completed tool call (output capped at 16 KiB), the plan, and every
   permission request with the answer the host gave.
6. When the prompt ends, stops the process group and records the ending,
   the tokens, and the process group's cleanup.

Starting the session and the prompt are effect intents the task owner
retains before dispatch (`grok_session`, `grok_prompt`). The task's result
ending is `model_finished` when Grok Build ends the turn (`end_turn`),
`cancelled_or_host_refusal` when the task was cancelled, reached its
deadline, or the host refused the turn (a reported model other than the
admitted one is a host refusal, and that turn sends no prompt),
`engine_stopped_after_refusal` when Grok Build ended the turn itself after
the host refused a tool it asked for, `engine_cancelled` when Grok Build
reported `cancelled` with no stop from the host, and `engine_incomplete`
when the agent stops for another reason. An ACP error is an engine failure.
The adapter summary's `stopped` field says in plain words how a turn the
agent ended stopped, and the chat shows it, for example "Coder stopped:
Grok Build stopped after the host refused a tool it asked to run (Write
/etc/hosts: it would write /etc/hosts, outside the workspace)." Only the
host's own stop reads "the task was stopped, or its host refused to go on".
This route records no usage probe and no rate-limit refusal.

The task transcript is the chat. This slice keeps no separate copy of
Grok Build's private session store.

## Access

Grok Build runs its own tools. Outside full access, the host runs the
whole Grok Build process inside Coder's own boundary, the one the
Microcoder loop's commands get, so Grok Build and every tool it runs are
held to the run's access the way Codex and Claude Code runs are
([#10092](https://github.com/OpenAgentsInc/openagents/issues/10092)).

- **Full access** (`--full-access`, or `coder.access: full`) starts the
  process with `--always-approve`, with no sandbox. A permission request
  that still arrives is allowed.
- **This computer's toolchains** (`toolchains`, a person's default local
  access) omits `--always-approve` and starts the process inside the
  boundary (`coder::task::adapter::Host::engine_boundary`): a `sandbox-exec`
  profile on macOS, `bwrap` on Linux, an AppContainer on Windows. It and
  everything it runs write only the workspace and a private scratch, never
  the task store or the common Git directory; read only the workspace, the
  system directories, this computer's toolchains, the Git directory, and
  the resolved `grok` program; and have the network. `HOME` and the
  temporary directory are the scratch, `PATH` and the toolchain variables
  are the ones the loop's commands get, and `GROK_HOME` is a new Grok home
  in the scratch that holds a copy (mode `0600`) of the person's
  `auth.json`, or no file when `XAI_API_KEY` is set, which is passed
  instead. The person's own Grok home is only read, never written; the
  scratch, with the copy, is removed when the turn ends. Grok Build
  refreshes its sign-in only near its expiry, and a refresh inside the
  copy could retire the refresh token the person's login still holds, so
  a turn whose login expires within half an hour plus ten minutes is
  refused before it starts (a turn has no time limit; half an hour is
  what a long turn is expected to take) and says to run `grok` once to refresh it.
  The host answers each `session/request_permission` with the agent's
  allow-once option, because the boundary holds whatever the tool does,
  except a file-writing tool (`edit`, `delete`, `move`) that names a path
  outside the workspace and the scratch in its `locations` or `rawInput`:
  that is refused with the agent's reject option. A command (`execute`)
  carries no reliable list of what it writes, so for commands the
  operating-system boundary is the limit. Grok Build 1.0.44 ends the
  whole prompt after a refused ask, which ends the turn as
  `engine_stopped_after_refusal`. A write a command attempts outside the
  workspace fails inside the command with "Operation not permitted", and
  the turn goes on.
- **The boundary** (`boundary`) has no network, and Grok Build reaches
  xAI from its own process, so a Grok Build turn there is refused before
  the process starts, with that reason. Use toolchains or full access.

Either way the process runs in its own process group, which the host stops
when the turn ends or is cancelled. A container grant refuses a Grok Build
route. The boundary was verified live on macOS with Grok Build 1.0.44; on
Linux and Windows the same boundary applies, and is not yet verified live
with Grok Build.

## Follow-up turns

The `grok_session` step records the session id. Under full access, the
next turn of the same task sends `session/load` for that id and prompts it
with the new message only. Outside full access each turn's Grok home lasts
only that turn, so each turn opens a new session. A new session receives
the engine prompt, which includes the earlier turns the host read from its
own traces.

## Steering

ACP takes one `session/prompt` at a time. A message for a running turn is
not delivered into it. `coder_delegate::steering::GROK_ACP` says so:
`session/cancel` ends the turn, and the next turn reattaches the same
session with `session/load`. The grant records that row under
`capabilities.steering`.

## Live smoke

`repository::grok` tests replay a synthetic ACP turn, at full access and
inside the toolchains boundary. The ignored test
`live_grok_cli_runs_a_repository_turn` runs that same turn through the
installed `grok`, with the owner's login, under full access, in a scratch
repository and task store that the test removes. It spends a model request:

```sh
cargo test -p microcoder --lib live_grok -- --ignored
```

On 2026-09-30, on the owner's Mac with Grok Build 1.0.44, `grok:default`
reported `grok-4.7`, ended `model_finished` in about 30 seconds, and wrote
`result.txt`.

The ignored test `live_grok_at_toolchains_writes_only_the_workspace` runs
the installed `grok` at this computer's toolchains: Grok Build runs a
shell command that writes outside the workspace, the host allows the
command, the boundary refuses the write, and the turn finishes with
`result.txt` written. On 2026-10-01, on the owner's Mac with Grok Build
1.0.44, the command reported "Operation not permitted", nothing was
written outside the workspace, and the turn ended `model_finished` in
about 11 seconds.
