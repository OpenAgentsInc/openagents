# 33. Toad

> **Status: written.** Read at batrachianai/toad@dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1 on 2026-11-02.

Source: [batrachianai/toad](https://github.com/batrachianai/toad/tree/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1)
at `dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1` (last commit 2026-05-26). A
shallow clone was read under `projects/repos/toad`; nothing was installed or
run. Paths below are relative to the repository root.

Toad is licensed under the GNU Affero General Public License, version 3
(`LICENSE`), with a separate commercial license (`COMMERCIAL_LICENSE.md`).
Coder takes ideas from it and copies no code.

## Why study it

Toad is not an agent. It is a terminal host for other people's agents: it
speaks the Agent Client Protocol (ACP) as the client, and agents such as
Claude Code, Codex, Gemini and OpenHands run behind it (`README.md`, "What is
Toad?"). Coder's own terminal program (chapter 36) also hosts other agents:
`coder/tui/coder.coax` drives Claude Code over stream-json, Codex as
`codex app-server`, and Microcoder as a worker process. So Toad answers the
question Coder's terminal UI also faces, from the other side of a protocol.

Toad's second idea is its shell. The README argues that most agent terminals
run `!` commands without a shell, so a `cd` or an exported variable does not
persist and interactive programs break the display. Toad runs a real shell in
a pseudo-terminal and renders its output with its own terminal emulator.
Coder chapter 37 plans a terminal emulator in Coax for the same reason, so
Toad shows one working version of that design.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| ACP client: `initialize` with file and terminal capabilities, `session/new`, `session/load`, `session/prompt`, `session/set_mode`, `session/cancel` | `src/toad/acp/agent.py` | 16 |
| Session updates streamed as user, agent and thought chunks, tool calls and plans | `src/toad/acp/agent.py` (`rpc_session_update`), `src/toad/widgets/plan.py` | 36 |
| Permission requests from the agent shown as a choice; the answer goes back as the selected option | `src/toad/acp/agent.py` (`rpc_request_permission`), `src/toad/screens/permissions.py` | 5, 16 |
| File reads and writes served to the agent | `src/toad/acp/agent.py` (`fs/read_text_file`, `fs/write_text_file`) | 4, 16 |
| Terminals created, read, waited on, killed and released at the agent's request | `src/toad/acp/agent.py` (`terminal/*`), `src/toad/widgets/terminal_tool.py` | 16, 37 |
| A catalog of agents, one TOML file each, naming the command that starts the ACP adapter | `src/toad/data/agents/*.toml` | 13, 15 |
| A persistent shell in a pseudo-terminal, resized with the view; the working directory reported after each command | `src/toad/shell.py` | 37 |
| Its own ANSI stream parser and terminal widget: alternate screen, bracketed paste, mouse modes, scroll margins | `src/toad/ansi/_ansi.py`, `src/toad/widgets/terminal.py` | 37 |
| Danger detection for shell commands: parse with `bashlex`, a set of safe commands, escalation for paths outside the project | `src/toad/danger.py`, `src/toad/widgets/prompt.py` | 5, 36 |
| Sessions in SQLite, keyed to the agent's own session ID for resume | `src/toad/db.py`, `src/toad/screens/session_resume_modal.py` | 2, 14 |
| A Textual application | `src/toad/app.py`, `src/toad/cli.py` | 36 |

### Toad as an ACP client

`src/toad/acp/agent.py` starts an agent's command as a subprocess and speaks
JSON-RPC to it (`src/toad/jsonrpc.py`). Its `initialize` call tells the agent
that Toad can read and write text files and run terminals, and records the
agent's capabilities and authentication methods from the reply. From then on
the agent drives: it streams `session/update` notifications, and it calls
back into Toad for permission (`session/request_permission`), for files
(`fs/read_text_file`, `fs/write_text_file`) and for terminals
(`terminal/create`, `terminal/output`, `terminal/wait_for_exit`,
`terminal/kill`, `terminal/release`). Each callback posts a message to the UI
and waits on a future for the answer, so the protocol layer never touches
widgets directly.

Agents that do not speak ACP natively run through adapters. The catalog
entry for Claude Code sets `protocol = "acp"` and
`run_command."*" = "claude-agent-acp"`, an adapter that Zed Industries
publishes (`src/toad/data/agents/claude.com.toml`). The catalog holds 21
entries at this revision (`src/toad/data/agents/`).

`src/toad/db.py` stores each session's agent, the agent's own session ID, a
title, the protocol, a prompt count and timestamps. Toad resumes a session
with `session/load` when the agent advertises it
(`supports_load_session` in `src/toad/acp/agent.py`).

### The shell

`src/toad/shell.py` opens a pseudo-terminal with `pty`, starts the person's
`$SHELL` in it, and keeps it for the whole conversation. To run a command it
first resizes the pseudo-terminal to the view (`TIOCSWINSZ`), then writes the
command followed by
`printf "\e]2025;$(pwd);\e\\"`, an operating system command (OSC) with a
private number that reports the working directory when the command ends
(`Shell.send`). The parser recognizes OSC 2025 and turns it into a
working-directory event (`src/toad/ansi/_ansi.py`, the `"2025"` case). Toad
hides the echo of the lines it injected by remembering their bytes and
dropping them from the output (`Shell.write`, `_hide_echo`).

The output goes to Toad's own emulator. `src/toad/ansi/_ansi.py` is a stream
parser for control sequences that tracks the alternate screen, bracketed
paste, mouse tracking, scroll margins, character sets and cursor position
requests; `src/toad/widgets/terminal.py` draws the result as a scrollable
widget. Pastes are wrapped in bracketed-paste markers only when the program
in the shell asked for them (`Shell.send_input`).

### Danger warnings

`src/toad/danger.py` parses a command line with `bashlex`, splits it into
command atoms, and gives each a level. Commands in `SAFE_COMMANDS` (`ls`,
`cat`, `grep`, `diff` and so on) are safe; a dangerous command whose target
path resolves outside the project directory becomes destructive. The prompt
uses the result to highlight the command as the person types, when the
`shell.warn_dangerous` setting is on (`src/toad/widgets/prompt.py`). It is a
warning on the person's own input, not a gate on the agent.

### The display

Toad is a Textual application. `src/toad/cli.py` calls `app.run()` without
inline mode, so the interface takes over the terminal window, and the
conversation, plans, tool calls and terminals are widgets inside it
(`src/toad/widgets/conversation.py`).

## How Coder's terminal UI differs today

Coder's terminal program is inline (chapter 36). `coax/lib/tui/tui.coax`
redraws only a live region at the bottom, and finished turns go into the
terminal's own scrollback, so the terminal's selection, search and copy keep
working. There is no widget toolkit.

Coder's shell commands are simpler than Toad's. A message that starts with
`$ ` runs as `/bin/sh -c "exec 2>&1; COMMAND"` in the working directory, with
standard input closed, output read line by line through pipes, and a kill
after two minutes (`coder/tui/coder.coax`, `cd_command_start`). Each command
is a new process, so a `cd` does not carry over and an interactive program
cannot run. Microcoder's commands work the same way, with a 30-second limit
and the head and tail of the output kept (`coax/lib/microcoder/tools.coax`).

Coder drives its engines through each engine's own protocol: Claude Code
through `coax/lib/agent/claude/` and Codex through `coax/lib/agent/codex/`.
Chapter 16 implements an ACP server for editor clients and checks it with
an editor-shaped smoke; qualification with a real named editor remains.

## What Coder takes

**A persistent shell per conversation, in a pseudo-terminal.** Toad's
design supports interactive state such as a changed directory or exported
variable. This is a proposed optional capability to compare with independent
commands, not a replacement already qualified for Coder's baseline.
Chapter 37 implements a PTY per `$ ` command and agent terminal tools;
one persistent shell across commands is a further step.

**The working directory reported in band.** Appending a `printf` of an OSC
sequence with the working directory is a small, shell-neutral way to learn
where the shell is after each command, and it also marks where one command's
output ends. Chapter 37 uses a private OSC the same way, and adds the exit
status to the same marker.

**Hide only the bytes the host injected.** Toad drops the echo of its own
injected lines and nothing else. Coder's emulator does the same, so the
transcript shows what the person and the agent typed and not the host's
plumbing.

**ACP's callback shape for files, terminals and permission.** In ACP the
host serves files, terminals and permission to the agent. That maps onto
chapter 4's typed capabilities and chapter 5's approvals: a `terminal/create`
from an agent is a capability call that passes the same policy as Coder's own
tools. When chapter 16 adds ACP, Coder can host any ACP agent as a further
engine behind the same composer, and the agent's terminals run in chapter
37's emulator.

**Resume by the agent's own session ID.** Storing the agent's session ID
next to the local one, and resuming with the agent's own load call, is the
same split Coder uses for Claude Code and Codex. `coder attach` (chapter 14)
keeps both IDs in the log.

**Highlight risk while the person types.** Parsing the command into atoms
with resolved paths, and marking paths outside the project, is a useful hint
in the composer. Coder can show it in chapter 36's composer. It stays a
display hint; chapter 5's policy decides what runs.

## What Coder leaves, and why

**The full-screen widget application.** Chapter 36 keeps Coder inline so the
terminal's scrollback stays the record. Toad's widgets for plans, tool calls
and terminals become lines committed to scrollback and a live region, not
panes.

**An agent store in the terminal program.** Toad installs and launches agents
from a catalog. In OpenAgents the Router owns plugins and their lifecycles
(chapter 15), and Coder picks its engine per task with a System One decision
(chapter 13), not from a menu.

**Danger by name lists as a gate.** A list of safe command names cannot
describe what a command does to a repository. Coder uses chapter 5's policy
for decisions and keeps lists like Toad's for display only.

**Toad's code.** The license is AGPL-3.0. Coder's emulator is written in Coax
from the published standards and checked against libghostty as an oracle
(chapter 37); it does not read Toad's parser as a source.

## Where to measure a borrowed design

- The persistent shell, the OSC marker and echo hiding: chapter 37's checks,
  which compare the Coax emulator's screen with libghostty's on recorded
  byte streams, plus a pseudo-terminal smoke test in the style of
  `coder/tui/smoke.py` that runs `cd`, an exported variable and an
  interactive program across commands.
- Composer danger highlighting: chapter 36's smoke test checks the rendered
  row; it is display only, so it needs no harness measurement.
- ACP hosting: chapter 16 checks recorded ACP sessions byte for byte, as it
  does for MCP; chapter 21 compares an ACP-hosted engine with the same agent
  driven through its native protocol.

## Sources

- [README.md](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/README.md)
- [LICENSE](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/LICENSE)
- [src/toad/acp/agent.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/acp/agent.py)
- [src/toad/jsonrpc.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/jsonrpc.py)
- [src/toad/data/agents/claude.com.toml](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/data/agents/claude.com.toml)
- [src/toad/shell.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/shell.py)
- [src/toad/ansi/_ansi.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/ansi/_ansi.py)
- [src/toad/widgets/terminal.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/widgets/terminal.py)
- [src/toad/widgets/terminal_tool.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/widgets/terminal_tool.py)
- [src/toad/danger.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/danger.py)
- [src/toad/widgets/prompt.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/widgets/prompt.py)
- [src/toad/db.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/db.py)
- [src/toad/cli.py](https://github.com/batrachianai/toad/blob/dd4f90e8b3700c3de80ad4b0eaa488ad0105e2c1/src/toad/cli.py)
- Coder's side: [coder/tui/coder.coax](../tui/coder.coax), [coax/lib/tui/tui.coax](../../coax/lib/tui/tui.coax), [coax/lib/microcoder/tools.coax](../../coax/lib/microcoder/tools.coax), [coder/tui/smoke.py](../tui/smoke.py).
- [Chapter plan](PLAN.md), Part VII; chapters [36](ch036-a-terminal-ui.md) and [37](ch037-a-terminal-inside-the-agent.md).
