# Coder quick start

Coder is a coding agent in your terminal. Run `coder` in a project folder
and ask in plain words: explain this code, fix that test, add a feature. It
reads, edits, and runs commands in that folder, and shows each step as it
goes.

Coder works with the coding agents you already have. It can answer
through Claude Code, Codex, or OpenCode on your own login, hand work to
other agents on your computer, or use its own coding loop on OpenAgents
models. You don't need an API key to start.

## 1. Install

On macOS or Linux:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

Open a new terminal and check the version:

```sh
coder --version
```

Updates install themselves. See [Download and install](/docs/download)
for what gets installed, updating, and uninstalling.

## 2. Log in

```sh
coder login
```

Coder opens [openagents.com/device](https://openagents.com/device) in
your browser, or prints that address and a code when it can't (over SSH,
for example). Check that the page shows the same code as your terminal,
then approve. Coder then asks once where this computer's chats should live:

1. **Sync all my chats:** they show on openagents.com and in the phone
   app, earlier ones too.
2. **Keep chats on this computer:** nothing is sent.

Type `1` or `2`, or answer on the website. You can change it later with
`/sync`; see [Sync, share, and export](/docs/coder-sync).

Logging in is optional. Without it, Coder answers through Claude Code,
Codex, or OpenCode if one of them is set up on this computer. Logged in,
it can also use OpenAgents models, with a free daily allowance (see
[Pricing](/docs/pricing)).

## 3. Open a project

```sh
cd my-project
coder
```

You see the input at the bottom, with **Auto** at its top right (who
answers) and your folder and Git branch at its bottom right. Type a
message and press `Enter` to send. `/` lists every command.

## 4. Ask for something

Try a question first:

```text
What does this project do? Name the three files I should read first.
```

Then a small change:

```text
Add a short "Getting started" section to the README. Show me the diff.
```

Each step shows as a line: files read, searches, edits with their diff,
and commands with their output. Press `Ctrl+O` to show each step in
full. Press `Esc` to stop a reply.

If you don't like a change, type `/rewind`, pick the message, and Coder
puts the files back as they were before it.

## 5. Keep going

- `Ctrl+N` starts a new chat; `/resume` opens an earlier one.
- `Shift+Tab` chooses who answers: Auto, Claude Code, Codex, OpenCode,
  or another agent on this computer.
- `/models` picks the model; `/plugins` (or `Ctrl+P`) turns plugins on
  and off.
- `Ctrl+C` quits. Your chat is saved.

## The Coder guides

| Guide | What it covers |
| --- | --- |
| [How Coder works](/docs/coder-how-it-works) | Quick decisions in front of the big model, one chat in front of every agent, and the steps that wait for you. |
| [Use Coder in the terminal](/docs/coder-terminal) | The screen, keys, commands, chats, memory, and attachments. |
| [Choose who answers](/docs/coder-agents) | Auto, `Shift+Tab`, models, preferences, handing work to other agents, and background agents. |
| [Coder's plugins](/docs/coder-plugins) | What each built-in plugin does, and how to set up keys. |
| [Solve an issue](/docs/coder-solver) | From a GitHub issue to a pull request. |
| [Safety](/docs/coder-safety) | What Coder asks you before doing, and how it protects your work. |
| [Sync, share, and export](/docs/coder-sync) | Your chats on openagents.com and your phone, and saving them to files. |
| [Coder from scripts](/docs/coder-scripts) | `openagents coder` for scripts and other agents. |

Found a problem? [Report it on GitHub](https://github.com/OpenAgentsInc/openagents/issues)
with your system, Coder's version (from the version command above), and
what you saw. Leave out keys and
private code.

Next: [How Coder works](/docs/coder-how-it-works).
