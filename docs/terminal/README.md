# OpenAgents Terminal

OpenAgents Terminal is a full-screen chat with OpenAgents in your terminal.
You type a message and the same chat router the phone and the desktop app use
answers it. When the router judges a message is coding work, Coder runs on
this computer, in the folder you started from, and its steps stream into the
same screen. It works over SSH and in tmux, and it is text only.

Status: v1, landed 2026-10-01
([#10111](https://github.com/OpenAgentsInc/openagents/issues/10111)). It ships
inside the `openagents` program, which installs on its own with one command
(below; first release `1.0.0-rc.1`,
[#10114](https://github.com/OpenAgentsInc/openagents/issues/10114)). The
[scope](scope.md) records why it is built this way and the decisions taken on
its open questions.

## Install

On macOS and Linux:

```sh
curl -fsSL https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh | sh
```

On Windows, in PowerShell:

```powershell
irm https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.ps1 | iex
```

The installer picks the build for your system (macOS on Apple silicon or
Intel; Linux on x86_64 or arm64, glibc or musl; Windows x86_64), downloads
`openagents` and the `microcoder` engine Coder runs with, checks both against
the release's SHA-256 sums, and puts them side by side in `~/.openagents/bin`
(`%USERPROFILE%\.openagents\bin` on Windows). It installs nothing whose
digest does not match. If that folder is not on your `PATH`, it prints the line
to add (Windows adds it for you). Then it opens the terminal when one is
attached; set `OPENAGENTS_NO_LAUNCH=1` to skip that.

It follows the `stable` channel, and `rc` until a stable release exists. To
choose:

```sh
curl -fsSL .../openagents/install.sh | OPENAGENTS_CHANNEL=rc sh   # a channel
curl -fsSL .../openagents/install.sh | sh -s 1.0.0-rc.1           # a version
```

In PowerShell, set `$env:OPENAGENTS_CHANNEL` or `$env:OPENAGENTS_VERSION`
first. `OPENAGENTS_BIN_DIR` installs somewhere else. Run the command again to
update. On Windows, `openagents connect`, `labor`, `service`, `ssh`, `wallet`,
and `x402` say they need macOS or Linux.

How a release is built and published: [docs/release/terminal.md](../release/terminal.md).

## Start it

```sh
openagents terminal          # or bare `openagents` on a terminal
openagents terminal --thread ID
openagents terminal --continue
openagents terminal --scratch
```

- Bare `openagents` opens the screen when it runs on a terminal. Piped, or
  with `--json`, it prints the usage as before.
- It opens on a new thread. `--continue` opens the last thread you had open
  in this folder, `--thread ID` opens that thread, and Ctrl+T lists them.
- `--scratch` uses a throwaway identity and thread store in the system
  temporary directory, for trying it out and for tests. Reopen that thread
  with `--scratch --thread ID`.
- `--local` keeps the threads in this command's own store even when a host
  runs, and `--socket PATH` names another host control socket, as for
  [`openagents chat`](../cli/chat.md).

When you quit, it prints the thread ID and the command that reopens it.

From a checkout: `cargo run -q -p openagents-cli --bin openagents --
terminal`.

## The screen

The first thing it shows is the welcome card: the version and three rows.

- **Project:** the Git checkout you started in, where Coder would work.
- **Agents:** every coding agent ready here, such as Codex, Claude Code, and
  Grok Build. An agent that is not signed in, at its usage limit, or not
  enabled in `coder.providers` is left off the card; the chat still knows
  each one's state.
- **Chats:** `synced` with this computer's host running (the OpenAgents app,
  or `openagents host serve --control`): they are the desktop app's threads,
  and a paired phone reads and continues them. Without one, `this computer`:
  they stay in this command's store until a host starts here, then move into
  it, keeping their IDs. `scratch` for `--scratch`.

Below the transcript sits the composer. Its top line says what is happening
(`ready`, `replying`, `Coder · step 4 · ≈40% done · 12s`); its bottom line
names the engine and the thread.

Replies render as Markdown. Suggested follow-ups are not shown here; a card
that opens in the app (a Gym result, a deck, the wallet) says where to open
it.

## Keys

| Key | Does |
| --- | --- |
| Enter | Send. On an empty line, start the Coder run OpenAgents offered. |
| Option+Enter (Alt+Enter on Linux), Ctrl+J | A new line in the message. |
| Esc | Stop the reply that is streaming, or stop the Coder run. Closes a list. |
| Ctrl+T | The thread list. |
| Ctrl+O | Expand every Coder run's tool calls to each call and its output, or condense them again. |
| Ctrl+S | Keep this computer's chats in sync with your phone (install the host). |
| PageUp, PageDown | Scroll the transcript. |
| Up, Down | Move in the message, then through your earlier messages. |
| Ctrl+C | Clear the message; on an empty line, press it twice to quit. |
| Ctrl+D | Quit, on an empty line. |

The composer keeps the readline keys `coder` has: Ctrl+A and Ctrl+E, Ctrl+W,
Ctrl+K, Ctrl+U, and Alt+B and Alt+F.

## Slash commands

A message that is exactly one of these words is a command. Anything else,
including text that only starts with a slash, goes to OpenAgents.

| Command | Does |
| --- | --- |
| `/new` | Start a new thread. |
| `/threads` | List threads to open, start, or archive (also Ctrl+T). |
| `/stop` | Stop the reply or the Coder run (also Esc). |
| `/export` | Save this thread as an ATIF trajectory under the chat home's `exports/`. |
| `/settings` | Show the Coder settings and where the file is. |
| `/connect` | Pair a phone with this computer by QR code. |
| `/plugins` | List published plugins. |
| `/expand` | Expand or condense the tool calls (also Ctrl+O). |
| `/help` | Show these commands and keys. |
| `/quit` | Close the screen; a Coder run keeps going. |

## Coder runs

A run's tool calls show as Grok Build shows them
([design](../coder/design/2026-10-01-tool-call-groups.md)): consecutive calls
that only look fold into one line counted by verb (`◈ Read 3 files, Searched
2 patterns`), each command or edit is one line (`◆ Run cargo test · exit
101`), and file contents and command output stay hidden until Ctrl+O.

When the router judges a message is coding work, Coder starts at once on this
computer (unless your settings say to ask first; then Enter on an empty line
starts it). It works in its own worktree of the checkout you started in, with
the first coding agent your settings allow that is signed in here with
capacity, and every step is approved: there is no permission prompt
([#10104](https://github.com/OpenAgentsInc/openagents/issues/10104)).

The run streams into the transcript: who runs it and why, its thoughts and
tool calls, each command with its exit code and last lines of output, a switch
to another agent, and the result with the files it changed and its worktree.
Progress is one live line, `step N · ≈X% done`, the share being Jev's estimate
of how much is complete. Runs have no step or time budget.

- **Esc stops the run.** Its turn ends as stopped, and the transcript says so.
- **Quitting leaves it running.** The run is its own process. Reopen the
  thread (`--thread ID`, or the thread list) and the screen follows it again
  from its first event.
- **A question from Coder** is answered by typing in the composer.
- **A follow-up after a run** goes to the router with what the run did, so
  more work continues the same run.
- **An issue** ("work on #10034") runs the issue flow inside the screen's
  process in v1: keep the screen open until it lands.

Coder needs a Git checkout here. Outside one, a host with a project of its own
runs it there instead, as `openagents chat` does.

## Threads

Ctrl+T or `/threads` lists threads in the same order as the desktop sidebar
and the phone: pinned first, then newest first, then archived. Enter opens
one, `n` starts a new one, `a` archives the selected one, and Esc closes the
list. Opening a thread while a run streams stops following it; the run keeps
going.

## Sync with your phone, and pairing

With no host running, the welcome card offers Ctrl+S: it installs this
computer's host as a user service (launchd on macOS, systemd on Linux), so
threads sync with a paired phone. It does that when this computer has what the
install needs; otherwise it says the one thing to do instead, usually opening
the OpenAgents app, which runs the host. It never installs anything unasked.

`/connect` asks the host for a single-use pairing code and draws it as a QR
code made of text, so it works over SSH. Scan it with the OpenAgents app on
your phone (Connect a computer). The screen says when the phone connected.
The code is cancelled once a phone connects, when it expires, when you press
Esc, and when the screen closes.

## What it keeps

- Threads: the host's store, or `~/.openagents/chat/` (`OPENAGENTS_CHAT_HOME`
  overrides it), as for `openagents chat`.
- `terminal.json` in that chat home: the last thread per folder (folder paths
  and thread IDs, no message text).
- `exports/` in that chat home: what `/export` writes.
- Coder tasks: `~/.openagents/tasks` (`OPENAGENTS_TASKS` overrides it).

## Look

White in four intensities on near-black, the desktop app's and the website's
palette, with a white cursor. With `NO_COLOR` set it uses no color at all, and
on a 256-color terminal the nearest palette entries.

## How it is built

`crates/openagents-terminal` is a library the `openagents` program calls
(`crates/openagents-cli/src/screen.rs`). It is a screen over the shared chat
client, `openagents_chat::client`, and adds no chat or Coder logic: the
router decides every route, and the screen draws the client's typed events
with the components in `crates/coder-terminal` (turn, card, run, and list
overlay; see [terminal behavior](../coder/runtime/terminal.md)).

- `app`: the state, and what each key and each client event does. Pure, and
  tested without a terminal.
- `draw`: one frame.
- `screen`: the input loop, which holds the terminal through
  `coder-terminal`'s panic-safe guard and runs each action against the client.
- `slash`: the closed list of commands.
- `last`: the last thread per folder.

What only this program can do (pairing over the host's control socket,
installing the host service, listing plugins, the settings) reaches the
screen through the `Extras` trait.

## Testing

```sh
cargo test -p openagents-terminal
cargo test -p coder-terminal
```

`tests/pty.rs` runs the real input loop in a pseudo-terminal, reading the
screen through `coder-vt` (the emulator the phone's terminal screen uses),
against the in-process backend with
a scripted chat worker and a fake coding engine on a scratch Git repository:
a question's answer streams in, a coding message starts a run at once, Esc
stops it, a second thread starts, the thread list reopens the first with its
run, and Ctrl+C twice quits. It also checks the bytes: a white cursor handed
back on exit, the near-black field, and no amber.

The same test file has a live check, ignored by default, that runs the real
program against the live chat worker with a throwaway HOME:

```sh
OA_TERMINAL_LIVE='cargo run -q -p openagents-cli --bin openagents -- terminal --scratch' \
  cargo test -p openagents-terminal --test pty -- --ignored --nocapture live
```
