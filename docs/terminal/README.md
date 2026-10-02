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
its open questions. What Coder Terminal has that this lacks, and a roadmap: [gap analysis](2026-10-02-coder-terminal-gap-analysis.md).

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

On Windows the screen keeps its files under `%USERPROFILE%\.openagents`
(the program uses the profile folder as its home when `HOME` is not set).
No host runs there, since the host's control socket is a Unix socket, so
threads stay in the program's own store, and `/connect`, `/import`, and
Ctrl+S say they need a host; `--computer` opens a paired computer's threads.
The Windows build is checked with `cargo check --target
x86_64-pc-windows-gnu -p openagents-cli` but has not been run on Windows.

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
- `--computer HOST` opens another computer's threads, as a paired phone
  does: the terminal on a laptop follows the desktop's host over NIP-HOST
  (`thread.list`, `thread.read`, `thread.send`, `thread.stop`,
  `thread.run`). Pair this computer with it first (`openagents computer
  link`, with an invitation from that computer); HOST is a name, key, or key
  prefix from `openagents computer list`. It opens on that computer's thread
  list. Messages continue its threads and its Coder runs there; new threads,
  archiving, and following a run happen on that computer or in the app.
- `--local` keeps the threads in this command's own store even when a host
  runs, and `--socket PATH` names another host control socket, as for
  [`openagents chat`](../cli/chat.md).

When you quit, it prints the thread ID and the command that reopens it.

From a checkout: `cargo run -q -p openagents-cli --bin openagents --
terminal`.

## The screen

The first thing it shows is the welcome card: the version and three rows.

- **Project:** the Git checkout you started in, where Coder would work,
  written from the home folder (`~/openagents`).
- **Agents:** every coding agent ready here, such as Codex, Claude Code, and
  Grok Build. An agent that is not signed in, at its usage limit, or not
  enabled in `coder.providers` is left off the card; the chat still knows
  each one's state.

Below the transcript sits the composer. Its top line says what is happening
(`ready`, `replying`, `Starting Grok Build…`, `Coder · step 4 · ≈40% done ·
12s`); its bottom line names the engine and the thread. A Coder start shows
one line in the transcript, "Grok Build is working.", and why only when
another engine runs than the one you asked for; `/export` keeps the task and
its worktree.

When OpenAgents cannot be reached (no network, the relay down, or this
computer's host restarting), the screen says so once and the top line reads
`offline · trying again in 4s · Esc stops`. It asks again after a pause that
doubles up to 30 seconds, until the reply comes or Esc stops it; what had
streamed stays, and the reply streams on once it is back. A message sent
after the host restarted goes on a new connection at once, and one sent
while it restarts waits the same way and goes through once it answers
([#10168](https://github.com/OpenAgentsInc/openagents/issues/10168)).

The transcript looks as Grok Build's does, in its Grok Night colors (Grok
Day on a light background) on its #141414 field. No turn carries a label:
your message is a prompt, `❯` and the text on a raised band, and a reply is
Markdown under it, with its heading colors, bold, italic, inline code, links
with their address, muted bullets and numbers, quote bars, boxed tables, and
code blocks on their own band. One blank row separates turns. A Coder run's
result renders the same way, never as raw Markdown, and its tool calls
group as Grok Build groups them. Fenced code is highlighted as Grok Build
highlights it: syntect with two-face's syntaxes and Grok Build's own colors.
A run result's diffs (Ctrl+O) are drawn as Grok Build draws an edit:
numbered, highlighted, removed and added lines on red and green bands.
Suggested follow-ups are not shown here; a card
that opens in the app (a Gym result, a deck, the wallet) says where to open
it.

## Keys

| Key | Does |
| --- | --- |
| Enter | Send. On an empty line, start the Coder run OpenAgents offered. |
| Option+Enter (Alt+Enter on Linux), Ctrl+J | A new line in the message. |
| Esc | Stop the reply that is streaming, or stop the Coder run. Closes a list. |
| Ctrl+T | The thread list. |
| Ctrl+R | The thread's Coder run full screen (below). Esc or Ctrl+R goes back. |
| Ctrl+Y | Copy the last reply. Press again for each code block in it, last first. It copies through the terminal (OSC 52), so it works over SSH; in tmux, `set-clipboard on`. |
| Ctrl+O | Expand every Coder run's tool calls to each call and its output, and each run result's changed files to their diffs, or condense them again. |
| Ctrl+S | Keep this computer's chats in sync with your phone (install the host). |
| PageUp, PageDown | Scroll the transcript. |
| Up, Down | Move in the message, then through your earlier messages, kept across restarts. With Coder runs in the rail under the composer, Up on an empty line moves into the rail and through it, Down moves back to the composer, and Enter opens the selected run full screen. |
| Alt+1 … Alt+9 | Open that Coder run from the rail full screen. |
| Ctrl+C | Clear the message; on an empty line, press it twice to quit. |
| Ctrl+D | Quit, on an empty line. |
| Mouse | Drag to select text; letting go copies it (OSC 52, as Ctrl+Y). Click a file's path in a reply or a run to read the file. The wheel scrolls. Most terminals still select their own way with Shift (Option in iTerm2) held. |

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
| `/settings` | Turn on or off when Coder starts at once and which coding agents it may use; Enter or Space changes the selected one and saves it. |
| `/connect` | Pair a phone with this computer by QR code. |
| `/plugins` | List the plugins installed on this computer, then the published ones. Enter on an installed one, then type what to ask it: it runs once on this folder, reading files only, and its reply shows as a card. Esc cancels. |
| `/background` | The host's background rules, such as the disk cleanup monitor, each with its state and last result. Enter shows a rule, `r` shows its dry run (what it would delete and why) and `r` again runs it, `p` pauses or resumes it, `l` shows its log. A rule's notification appears as one line in the transcript. |
| `/import` | Copy this computer's Claude Code and Codex sessions (`~/.claude`, `~/.codex`) into the host's threads, each once: the messages and text replies, without tool calls. The host only reads those folders. Needs the host. |
| `/expand` | Expand or condense the tool calls (also Ctrl+O). |
| `/run` | Open the Coder run full screen (also Ctrl+R). |
| `/open N` | Open Coder run N from the rail full screen (also Alt+N). |
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
- **The run full screen.** Ctrl+R shows the run alone: every step, each
  command with its output, and the result with its diff. What you type there
  goes to the run, not to the chat: Coder reads it at its next step and says
  so ("Coder read your message."). A turn that has ended starts again with
  it. Grok Build, OpenCode, and Devin read instructions only when a turn
  starts, so their turn stops and the next starts with your message. Esc goes
  back to the chat; `/stop` stops the run.
- **The rail.** Under the composer, one row per Coder run this thread
  started: its number, its agent, and what it is doing now, with the
  spinner and its timer while it runs. A finished run keeps its row for 30
  seconds; `/open` still opens it by number after that.
- **A follow-up after a run** goes to the router with what the run did, so
  more work continues the same run.
- **An issue** ("work on #10034") runs the issue flow in a process of its
  own, as `openagents chat work --issues` does: quitting leaves it running
  through its checks and landing.

Coder needs a Git checkout here. Outside one, a host with a project of its own
runs it there instead, as `openagents chat` does.

## Files

A click on a file's path (`src/app.rs`, `src/app.rs:42`) in a reply or a run
opens it read only, with line numbers and the code highlighted as code blocks
are, at that line. A relative path is read from the run's worktree, then from
the folder you started in. Up, Down, PageUp, PageDown, Home, and End scroll;
Esc closes it. A click on anything that is not a file here does nothing.

## Threads

Ctrl+T or `/threads` lists threads in the same order as the desktop sidebar
and the phone: pinned first, then newest first, then archived. Typing
narrows it to threads whose title or project matches, Backspace widens it.
Enter opens one, Ctrl+N starts a new one, Ctrl+A archives the selected one,
and Esc closes the list. Opening a thread while a run streams stops following it; the run keeps
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
- `prompts.json` in that chat home: the last 500 messages you sent, for Up
  and Down (your text only, no replies). A scratch screen keeps its own.
- `exports/` in that chat home: what `/export` writes.
- Coder tasks: `~/.openagents/tasks` (`OPENAGENTS_TASKS` overrides it).

## Look

White in four intensities on near-black, the desktop app's and the website's
palette, with a white cursor. With `NO_COLOR` set it uses no color at all, and
on a 256-color terminal the nearest palette entries.
Code and diffs are the exception: they take Grok Build's Grok Night theme on
the near-black field (Grok Day only where the terminal's own light background
shows, per `OPENAGENTS_APPEARANCE` or `COLORFGBG`), quantized to the 256- or
16-color palette as Grok Build does (`crates/code-highlight`, feature `grok`).

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
- `view`: the run view, the file view, and mouse selection.

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
a question's answer streams in, a coding message starts a run at once,
Ctrl+R shows it full screen and a message there reaches it, a click opens a
file it names and a drag copies, Esc stops it, a second thread starts, the thread list reopens the first with its
run, and Ctrl+C twice quits. It also checks the bytes: a white cursor handed
back on exit, the near-black field, and no amber.

The same test file has a live check, ignored by default, that runs the real
program against the live chat worker with a throwaway HOME:

```sh
OA_TERMINAL_LIVE='cargo run -q -p openagents-cli --bin openagents -- terminal --scratch' \
  cargo test -p openagents-terminal --test pty -- --ignored --nocapture live
```
