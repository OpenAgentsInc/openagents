# Use Coder on Windows

Coder, its chats, and the `openagents` command work on Windows, with the
same keys and commands as on macOS and Linux. This guide installs Coder in
PowerShell, sets up an agent, and covers what's different on Windows.

## 1. Install

Open PowerShell (in Windows Terminal, if you have it) and run:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

The installer puts `coder`, the `openagents` command, the engine, and
`coder-boundary.exe` in `%USERPROFILE%\.openagents\bin`, and adds that
folder to your user `PATH`. The last one keeps the commands Coder runs
from writing outside the folders they're given. Windows on ARM runs the
x86_64 build.

Close PowerShell, open a new window, and check:

```powershell
coder --version
```

## 2. Set up an agent

Coder can answer through Claude Code, Codex, or OpenCode on your own
login. Agents installed with npm start fine from Coder on Windows:

```powershell
npm install -g @anthropic-ai/claude-code
claude
```

Log in when it asks, then quit it. For Codex, use
`npm install -g @openai/codex`, then `codex`. Or log in to openagents.com
and use our models with the free daily allowance:

```powershell
coder login
```

## 3. Run it

```powershell
cd $env:USERPROFILE\code\my-project
coder
```

From here everything is the same as on a Mac: see
[Start here](/docs/start-here) for a first task.

## What's different on Windows

- **Use Windows Terminal.** It tells `Ctrl+Enter` (queue for later) apart
  from `Enter`. In a terminal that doesn't, press `Ctrl+Q` to queue; the
  hint under the input names the key that works.
- **No `Cmd` keys.** Use `Ctrl+P` (or `F2`) for plugins.
- **Background automation needs macOS or Linux for now**, and so do the
  local task services. Coder, its chats, and the `openagents` command
  work on Windows.
- **The Verse page** of the desktop app is on macOS and Linux, not
  Windows; the [Everglade](/everglade) world runs in your browser instead.
- **Your files** live under `%USERPROFILE%\.openagents`: chats in
  `coder\sessions`, exports in `exports`, and memory in `memory`.

## Update and uninstall

`coder update` installs the newest version now; Coder also updates
itself, once a day while it's open. To uninstall, run `coder logout`,
then:

```powershell
Remove-Item -Recurse "$env:USERPROFILE\.openagents\bin"
```

Then remove `.openagents\bin` from your user `PATH` in **Settings → System
→ About → Advanced system settings → Environment Variables**. Full
details: [Download and install](/docs/download).

Problems? See the Windows notes in [Troubleshooting](/docs/troubleshooting).

Next: [Export and share a chat](/docs/export-and-share-a-chat).
