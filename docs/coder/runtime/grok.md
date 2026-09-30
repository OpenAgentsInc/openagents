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
is not empty, or a non-empty `XAI_API_KEY`. The host checks the file's size
and whether the variable is set. It never reads the file and never logs
the variable.

The default local provider list stays Codex, then Claude Code. Grok Build
is available when you name it, after Claude Code and before OpenCode in
the allowed order.

## What a turn does

A repository run whose route is Grok Build does not run the Microcoder
step loop. The task owner admits the grant exactly as for any other route,
then:

1. Starts `grok agent --no-leader stdio` in the task's workspace, as the
   leader of its own process group. Full access adds `--always-approve`.
   The environment is the owner's login-shell environment under full
   access, or the host process's environment otherwise, less every
   variable named `*_API_KEY`, `*_TOKEN`, or `*_SECRET`, with
   `XAI_API_KEY` put back when that source set it.
2. Sends `initialize`. It never sends `authenticate`.
3. Opens a session with `session/new`, or, on a later turn of the same
   task, reattaches the session the earlier turn used with `session/load`.
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
admitted one is a host refusal, and that turn sends no prompt), and
`engine_incomplete` when the agent stops for another reason. An ACP
error is an engine failure. This route records no usage probe and no rate-limit refusal.

The task transcript is the chat. This slice keeps no separate copy of
Grok Build's private session store.

## Access

Grok Build runs its own tools, so the task owner's command boundary does
not wrap that process.

- **Full access** (`--full-access`) starts the process with
  `--always-approve`. A permission request that still arrives is allowed.
- **The boundary and this computer's toolchains** omit
  `--always-approve`. The host answers every `session/request_permission`
  with the agent's reject option, so the turn runs no tool the agent has
  to ask for.

Either way the process runs in its own process group, which the host stops
when the turn ends or is cancelled. A container grant refuses a Grok Build
route.

## Follow-up turns

The `grok_session` step records the session id. The next turn of the same
task sends `session/load` for that id and prompts it with the new message
only. A new session receives the engine prompt, which includes the earlier
turns the host read from its own traces.

## Steering

ACP takes one `session/prompt` at a time. A message for a running turn is
not delivered into it. `coder_delegate::steering::GROK_ACP` says so:
`session/cancel` ends the turn, and the next turn reattaches the same
session with `session/load`. The grant records that row under
`capabilities.steering`.

## Live smoke

`repository::grok` tests replay a synthetic ACP turn. The ignored test
`live_grok_cli_runs_a_repository_turn` runs that same turn through the
installed `grok`, with the owner's login, under full access, in a scratch
repository and task store that the test removes. It spends a model request:

```sh
cargo test -p microcoder --lib live_grok -- --ignored
```

On 2026-09-30, on the owner's Mac with Grok Build 1.0.44, `grok:default`
reported `grok-4.7`, ended `model_finished` in about 30 seconds, and wrote
`result.txt`.
