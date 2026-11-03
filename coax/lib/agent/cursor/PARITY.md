# Cursor ACP client (Coax)

A client of `agent acp`, the Cursor CLI's Agent Client Protocol server
over stdio, written the way `../codex` speaks `codex app-server`:
pull-based, no callbacks, caller storage. It tracks the CLI build
`2026.10.14-941492e` (`CU_UPSTREAM_CURSOR_VERSION` in `cursor.coax`) and
the protocol documented at <https://cursor.com/docs/cli/acp> on
2026-11-03.

| File | What |
| --- | --- |
| `client.coax` | The client as a state machine over lines: requests, answers to server requests, `cu_feed` |
| `transport.coax` | `agent acp` as a child process, and which binary is the Cursor CLI |
| `cursor.coax` | The entry: load it and link `process-native.c` |
| `tests.coax` | The requests byte for byte, a scrubbed session, permissions, questions, plans |
| `smoke.coax`, `smoke.sh` | The live smoke against the installed CLI |

## The wire

- Requests: `initialize` (protocol 1, no filesystem and no terminal
  capability, client name Coder), `authenticate` with `cursor_login`,
  `session/new` or `session/load` (absolute cwd, no MCP servers),
  `session/prompt` with one text block, `session/cancel`,
  `session/set_mode`, and `session/set_config_option` when the session
  advertises the model or thought level.
- The server's lines: a notification is an event with its method and
  params; an answer reaches the request that asked (`ev.of`); the
  session id is kept from `session/new` and `session/load`.
- `session/update` is a notification. `agent_message_chunk` text is the
  reply. `agent_thought_chunk` stays out of the transcript. `tool_call`
  and `tool_call_update` are the tool rows. `cursor/task` is a dim row.
  `cursor/update_todos` and `cursor/generate_image` are ignored.
- Server requests: `session/request_permission`, `cursor/ask_question`,
  and `cursor/create_plan` are events. The host answers before the next
  line. Unanswered, a permission is rejected (or cancelled when no
  reject option was offered), a question is skipped, and a plan is
  rejected. Nothing is allowed silently.
- `initialize` failing ends the client. A refused `authenticate` is the
  host's signal that `agent login` has not been done.

## Process

The executable is `cursor-agent` on `PATH` when it is not the IDE shim,
else `agent` on `PATH` when its own text names Cursor and the path is
not another product's agent, else the newest
`~/.local/share/cursor-agent/versions/*/cursor-agent-sea`.
`CURSOR_API_KEY` and `CURSOR_AUTH_TOKEN` are removed so the person's
login is used. `BROWSER=/usr/bin/true` and `NO_OPEN_BROWSER=1` keep it
from opening a window. The child runs in its own process group, killed
as one on `cu_close_process`. `cu_poll` never blocks longer than its
timeout; initialize unanswered for 60 s fails the client.

## Not done

- `--coder-tools` as an `mcpServers` entry, importing Cursor chat
  history, plan-limit usage, and a System One route label.
- Answering `cursor/ask_question` with a real choice. The turn is
  skipped and the prompt is shown.
- Filesystem and terminal client capabilities. Coder draws tools itself.
