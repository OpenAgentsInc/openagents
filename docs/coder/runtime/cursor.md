# The Cursor agent in ACP Subagents

Coder's ACP Subagents plugin can hand a task to the Cursor agent CLI
(`cursor-agent`), which serves the Agent Client Protocol (ACP) as
`cursor-agent acp`
([Cursor's documentation](https://cursor.com/docs/cli/acp)).
[Issue #10984](https://github.com/OpenAgentsInc/openagents/issues/10984)
delivers it.

| Piece | Where |
| --- | --- |
| Cursor's binary, sign-in, extension methods, and modes | [`acp_client::cursor`](../../../crates/acp-client/src/cursor.rs) |
| Discovery of the installed agent | [`coder_new::acp_discovery`](../../../crates/coder-new/src/acp_discovery.rs) |
| The delegation and its rail rows | `coder_new::bundled_runtime::acp` |
| Recorded sessions | `crates/acp-client/fixtures/cursor-2026.06.24-*.jsonl` |

## Turn it on

Install the Cursor CLI and sign in once with `cursor-agent login`, or set
`CURSOR_API_KEY` (or `CURSOR_AUTH_TOKEN`). Then refresh the ACP Subagents
settings (R) and keep **Cursor** checked. From a script:

```sh
openagents --json coder delegate cursor --task "Add a README to this folder" --in DIR
```

Discovery takes the program from `CURSOR_AGENT_BIN`, else `cursor-agent` on
`PATH` or in `~/.local/bin`. A bare `agent` counts only when it resolves into
Cursor's install tree (`~/.local/share/cursor-agent`), because other CLIs,
such as Grok Build, also install an `agent`.

## What a delegation does

1. Before it starts the agent, the host refuses the delegation with
   "Cursor is not signed in. Run `cursor-agent login`, or set
   CURSOR_API_KEY, then try again." when neither credential variable is set
   and `cursor-agent status --format json` reports `isAuthenticated: false`.
   The check reads only that field and gives up after 10 seconds.
2. Starts `cursor-agent acp` in the task's directory as its own process
   group. The environment drops every `*_API_KEY`, `*_TOKEN`, and `*_SECRET`
   variable except `CURSOR_API_KEY` and `CURSOR_AUTH_TOKEN`.
3. Sends `initialize`, then `authenticate` with `cursor_login`, then
   `session/new` and `session/set_mode` (`agent` by default; `plan` and
   `ask` are the other modes), then `session/prompt`. The reported model is
   `models.currentModelId`.
4. Approves each `session/request_permission` with the agent's allow-once
   option, as for every ACP subagent.
5. Answers Cursor's blocking extension requests so the turn never waits on
   a person: `cursor/ask_question` is skipped with a reason that says no one
   can answer, and `cursor/create_plan` is accepted. The notices
   `cursor/update_todos`, `cursor/task`, and `cursor/generate_image` are
   acknowledged. Each request and notice appears in the agent rail as a
   Question, Plan, Todos, Task, or Image row.

A signed-out agent that refuses `authenticate`, or never answers it, ends
the delegation with the same sign-in message.

## Tests

`acp_client::cursor` and `coder_new::bundled_runtime` replay recorded
sessions from Cursor agent 2026.06.24: a signed-in turn that runs one
command, a plan-mode turn, and a signed-out `authenticate`. On 2026-10-08
the installed agent ran `openagents --json coder delegate cursor` three
ways from a scratch state directory: with its stored login, with
`CURSOR_API_KEY`, and signed out. Both signed-in runs wrote the requested
file and finished; the signed-out run was refused in under a second.
