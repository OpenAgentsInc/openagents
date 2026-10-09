# OpenAgents Terminal

OpenAgents Terminal is a full-screen chat with OpenAgents in your terminal.
When a message is coding work, Coder runs on this computer, in the folder
you started from, and its steps stream into the same screen. It works over
SSH and in tmux.

To install it on macOS or Linux, run
`curl -fsSL https://openagents.com/cli/install.sh | bash`; for Windows,
see [Download](/docs/download).

## Start it

```sh
openagents                    # or: openagents terminal
openagents terminal --continue   # the last thread you had open in this folder
openagents terminal --thread ID  # a thread by its ID
openagents terminal --scratch    # a throwaway identity, for trying it out
```

It opens on a new thread. When you quit, it prints the thread's ID and the
command that reopens it.

## The screen

The welcome card shows three rows:

- **Project:** the Git checkout you started in, where Coder would work.
- **Agents:** the coding agents ready on this computer, such as Codex,
  Claude Code, and Grok Build.
- **Chats:** `synced` when this computer's host is running (the Mac app
  runs it), so your threads are the Mac app's threads and a connected phone
  sees them; `this computer` when no host runs; `scratch` for `--scratch`.

The line above the composer says what's happening (`ready`, `replying`,
`Starting Grok Build…`, `Coder · step 4 · ≈40% done · 12s`). The line under
it names the coding agent and the thread.

## Keys

| Key | Does |
| --- | --- |
| Enter | Send. On an empty line, start the Coder run OpenAgents offered. |
| Option+Enter (Alt+Enter on Linux), Ctrl+J | A new line. |
| Esc | Stop the reply or the Coder run. Closes a list. |
| Ctrl+T | The thread list. |
| Ctrl+O | Show every step of Coder's runs in full, or fold them again. |
| Ctrl+S | Keep this computer's chats in sync with your phone. |
| PageUp, PageDown | Scroll. |
| Up, Down | Move in the message, then through your earlier messages. |
| Ctrl+C | Clear the message; on an empty line, press it twice to quit. |
| Ctrl+D | Quit, on an empty line. |

The usual line-editing keys work too: Ctrl+A, Ctrl+E, Ctrl+W, Ctrl+K,
Ctrl+U, Alt+B, and Alt+F.

## Slash commands

A message that is exactly one of these is a command. Anything else goes to
OpenAgents.

| Command | Does |
| --- | --- |
| `/new` | Start a new thread. |
| `/threads` | List threads to open, start, or archive (also Ctrl+T). |
| `/stop` | Stop the reply or the Coder run (also Esc). |
| `/export` | Save this thread as a trajectory file. |
| `/settings` | Show Coder's settings and where the file is. |
| `/connect` | Show a QR code to connect a phone. |
| `/plugins` | List published plugins. |
| `/expand` | Show or fold every step (also Ctrl+O). |
| `/help` | Show these commands and keys. |
| `/quit` | Close the screen. A Coder run keeps going. |

## Coder runs

When a message is coding work, Coder starts at once (unless your settings
say to ask first; then press Enter on an empty line). It works in its own
copy of your checkout, so your files aren't touched. See
[Worktrees and changes](/docs/worktrees-and-changes).

- Reads and searches fold into one line (`Read 3 files, Searched 2
  patterns`); each command or edit is its own line with its exit code.
- **Esc stops the run.**
- **Quitting leaves it running.** Reopen the thread and the screen follows
  it again from the start.
- **When Coder asks a question**, type your answer.
- **A follow-up after a run** continues the same work.
- **"Work on #123"** works a GitHub issue. Keep the screen open until it
  lands. See [Work on a GitHub issue](/docs/github-issues).

## Threads

Ctrl+T lists threads: pinned first, then newest, then archived, as in the
Mac app and on the phone. Enter opens one, `n` starts a new one, `a`
archives the selected one, and Esc closes the list.

## Sync and pairing

With no host running, the welcome card offers **Ctrl+S**: it installs this
computer's host as a background service, so your threads sync with a
connected phone. If something is missing, it tells you the one thing to do
instead, usually opening the Mac app. It never installs anything you didn't
ask for.

`/connect` draws a QR code in text, so it works over SSH. Scan it with the
OpenAgents app on your phone (**Account → Computers → Connect a
computer**).

## Where it keeps things

- Threads: in the host's store, or in `~/.openagents/chat/` without a host.
- Exports: `exports/` in that folder.
- Coder's tasks: `~/.openagents/tasks`.

It draws in white on near-black. Set `NO_COLOR` for no color at all.

Next: [The openagents command](/docs/cli).
