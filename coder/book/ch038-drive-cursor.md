# 38. Drive Cursor

> **Status: partly implemented.** Coder runs tasks through the installed
> Cursor CLI on the person's own login (pinned CLI build
> `2026.10.14-941492e`): `coder run "TASK" --engine cursor` and the terminal
> UI's Cursor agent. One ACP session holds the conversation. The reply
> streams, tool rows and diffs show, and chapter 5 answers approvals. Still
> to come: a live smoke against that CLI on a logged-in account.

Cursor is the third coding agent many people have. Coder drives it as a
client of `agent acp`, the Cursor CLI's Agent Client Protocol server:
JSON-RPC 2.0, one JSON object a line, on the CLI's stdin and stdout.
Print mode (`agent -p`) starts a new process for every turn and has no
per-tool approval channel, so it is not the driver. `coder acp` is the
other direction: Coder as the server an editor talks to. That path is
unchanged.

System One's router stays the four engines it was checked on (Claude Code,
Codex, Microcoder, Psionic). Cursor is chosen with `--engine cursor`,
`/engine cursor`, or Shift-Tab. It is not a route label.

## The client

[coax/lib/agent/cursor/](../../coax/lib/agent/cursor/) is a pull-based
client in caller storage. The caller finds the CLI, starts `agent acp`,
then calls `cu_initialize`, `cu_authenticate` and `cu_session_new` (or
`cu_session_load` for a fork of a Cursor session), and polls with
`cu_poll`. `cu_feed` turns one stdout line into a typed event.

| Event | What Coder does with it |
| --- | --- |
| `agent_message_chunk` | adds the text to the streaming reply |
| `agent_thought_chunk` | stays out of the transcript |
| `tool_call` | shows a row from the title |
| `tool_call_update` | shows the result, and a diff when the content is one |
| `session/request_permission` | chapter 5, then an offered option |
| `cursor/ask_question` | skipped; the prompt is a dim line |
| `cursor/create_plan` | accepted under the default trust; y/n with `--ask` |
| `session/prompt` result | ends the turn: `end_turn`, `cancelled`, or failed |

A permission left unanswered is declined before the next line. The option
has to be one the request offered. `y` is allow once, `a` is allow always,
`n` is reject once. A missing allow option is a refusal. Esc sends
`session/cancel` and answers a pending permission as cancelled. If the turn
does not end within 5 seconds, Coder ends the process.

The child never receives `CURSOR_API_KEY` or `CURSOR_AUTH_TOKEN`, and it
never opens a browser (`BROWSER=/usr/bin/true`, `NO_OPEN_BROWSER=1`). The
person's `agent login` is the login. An initialize failure fails the
client. An authenticate refusal fails the turn and says to run
`agent login`.

The binary is `cursor-agent` on `PATH` when it is not the IDE shim that
exits with "No Cursor IDE installation found", else `agent` on `PATH` when
its text names Cursor and the path is not under `~/.grok/`, else the newest
`~/.local/share/cursor-agent/versions/*/cursor-agent-sea`. The Cursor IDE
`cursor` binary is not spawned.

`--model` and `--effort` are applied only when the session advertises a
config option of category `model` or `thought_level` with that value.
Otherwise the account default stands and the session log notes that the
flag was not applied.

A fork of a Cursor session uses `session/load`. Updates and permission
requests while that history replays are not shown and are not allowed. If
load is refused, Coder starts a new session and says so. The session id is
logged once. It is not stored in Claude Code's resume buffer.

## Approvals by policy

`--allow`, `--ceiling` and `--root` work as in
[chapter 10](ch010-drive-claude-code.md#approvals-by-policy). The tool
kind is the effect, in the same order as chapter 5:

| Kind | Effect |
| --- | --- |
| `read`, `search` | read |
| `fetch` | network |
| `edit`, `delete`, `move` | write |
| `execute`, and any other kind | run |

Paths come from `locations[].path` and from `rawInput`'s `path`,
`file_path` and `cwd`. A path that contains `..` is outside the working
directory. With no policy and no `--ask`, the tool runs and the approval is
logged. With `--ask`, the y/a/n prompt answers. In a job with no one to
ask, an ask is declined.

## Checks

```sh
coax test coax/lib/agent/cursor/tests.coax --library   # requests, replay, approvals, kinds
coax/lib/agent/cursor/smoke.sh                         # one live session, a declined command
python3 coder/tui/smoke.py --live cursor               # two turns in the terminal UI
```

The library checks replay a scrubbed ACP exchange. They are the documented
protocol, not a byte capture of a live CLI. The pinned build is
`CU_UPSTREAM_CURSOR_VERSION` in
[cursor.coax](../../coax/lib/agent/cursor/cursor.coax).

## Dependencies

- [Chapter 2](PLAN.md#i-the-loop) and [Chapter 5](PLAN.md#i-the-loop).
- [Chapter 11](ch011-drive-codex.md): the same pull-based client shape.

## Done means

- [x] The request, replay, approval and kind tests pass.
- [x] `coder run "TASK" --engine cursor` and the TUI's Cursor agent start
  `agent acp` on the installed CLI.
- [ ] A live smoke passes against CLI build `2026.10.14-941492e` on a
  logged-in account.
- [x] Approvals are answered by chapter 5's policy. The decision is
  `cd_policy_decide` in [ceiling.coax](../tui/ceiling.coax); the client
  only maps kinds and offered options.

## Sources

- `coax/lib/agent/cursor/` and its [PARITY.md](../../coax/lib/agent/cursor/PARITY.md).
- `coder/tui/cursor.coax`, `coder/tui/diff.coax`.
- Cursor's ACP documentation, read 2026-11-03:
  [agent acp](https://cursor.com/docs/cli/acp),
  [parameters](https://cursor.com/docs/cli/reference/parameters),
  [headless](https://cursor.com/docs/cli/headless).
