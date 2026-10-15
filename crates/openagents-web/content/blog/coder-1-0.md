---
title: Coder 1.0
date: 2026-10-15
author: OpenAgents
summary: Coder 1.0 is out: one Coder in front of every coding agent you use, sending each step where it belongs, with hard-to-undo steps waiting for your Y or N.
---

Coder 1.0 is out on macOS, Linux, and Windows.

Coder is one Coder in front of every coding agent you use: Claude Code,
Codex, OpenCode, Grok Build, Devin CLI, and Cursor, or Auto, which picks
for you. It works out what each step needs and sends it where it belongs.
Reading, editing, running tests, and pushing go straight through. Steps
that are hard to undo, such as a deploy, a merge, a secret, a payment, or a
DNS change, stop and wait until you press **Y** to allow or **N** to deny.

## Install

On macOS or Linux, in a terminal:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

Then run `coder` in your project folder. [Start here](/docs/start-here)
walks you through your first half hour, and the [download page](/download)
has every platform.

## What's in 1.0

- **Every agent behind one Coder.** Press `Shift+Tab` to step through the
  agents on your computer, or leave it on Auto. Type `/models` to pick a
  model: the pick sticks for that chat, and the rail names the model that
  answered each turn. See [Choose who answers](/docs/coder-agents) and
  [Pin a model or use Auto](/docs/pin-a-model).
- **OpenCode streams.** OpenCode's replies arrive as they're written, and
  Coder handles its one-time setup the first time you use it.
- **Keys that do what you expect.** `Enter` sends, `Ctrl+Enter` queues a
  message for later, and `Shift+Enter` starts a new line. See
  [Queue messages while Coder works](/docs/queue-messages).
- **Guards on every step,** including Windows shells (`cmd` and
  PowerShell). PowerShell that Coder can't read, such as an encoded
  command or a script file, is refused. See [Safety](/docs/coder-safety)
  and [Use Coder on Windows](/docs/coder-on-windows).
- **Each chat in its own worktree** in a Git repository, on Windows too. See
  [Worktrees and changes](/docs/worktrees-and-changes).
- **Headless mode** for scripts and other agents: `openagents coder chat`,
  with `--json`, `--mode opencode`, and `--session`. See
  [Run Coder headless](/docs/agents/coder-headless).
- **Projects you can share** by GitHub username:
  `openagents projects invite PROJECT --github USER`. See
  [Share a project with a teammate](/docs/share-a-project).

The full list, with every download and its checksum, is in the
[GitHub release](https://github.com/OpenAgentsInc/openagents/releases/tag/coder-v1.0.0).

## What we measured

We ran 21 real, closed issues three times each, with the same big model
both ways: once with Claude Code on its own, and once with a short brief
and a check step in front of it. With the brief and the check, an accepted
fix cost $1.84 instead of $4.00, and the median time was 121 s instead of
284 s, and as many fixes were accepted: 19 of 63 both ways. No small model ran in this study; the saving came from the brief and
the check, in front of the same model. The setup, the numbers, and the
scripts to run it again are in
[the study](https://github.com/OpenAgentsInc/openagents/blob/main/docs/inference/briefed-agent-ab.md).

## Pricing

Free is OpenAgents on your own computer: Coder in front of the agents you
already use, on your own models, keys, and subscriptions, at no charge from
us. Log in and Coder's `auto` model also comes with a free daily allowance.
Pro, at $20 a month, is for work on our cloud: cloud environments and
agents that keep working with your laptop closed. It isn't on sale yet. See
[Pricing](/docs/pricing).

## Learn more

- [Start here](/docs/start-here), then the [tutorials](/docs#tutorials).
- Building an agent of your own? Read the [docs for agents](/docs/agents).
- Questions, or Coder for a team? [Contact us](/contact).
