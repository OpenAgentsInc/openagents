# Plugins

A plugin is anything you add to OpenAgents. You write one, test whether
it helps Coder, and publish it so other people can check your result and
use it.

## What a plugin contains

A plugin can have any of these parts:

- **Skills.** Instructions the agent reads before a task. A skill runs no
  code.
- **Workflows.** Typed, step-by-step programs that OpenAgents runs: look
  things up, check, decide, or run Wasm.
- **Knowledge.** Cited reference entries, such as a method, an edge case,
  or how a command is used.
- **Wasm.** Sandboxed code that does one bounded job, such as reading a
  failing command's output. It is the only code a plugin can carry. It
  runs with no network, on a fixed budget of instructions and memory,
  and reads only the files its workflow grants. A workflow runs it; the model never
  calls it by name.
- **Tests.** Tasks Coder runs with the plugin and without it, and the
  checks on each run.

Not every plugin has every part. A plugin made in chat is a single skill.

## Plugins built into Coder

| Plugin | What it does |
| --- | --- |
| Claude Code | Coder hands a task to Claude Code on your computer and shows its progress as it works. |
| Codex | Coder hands a task to Codex on your computer and shows its progress as it works. |
| Cursor | Coder hands a task to Cursor's agent on your computer and shows its progress as it works. |
| Grok Build | Coder hands a task to Grok Build on your computer and shows its progress as it works. |
| OpenRouter | Use OpenRouter models in Coder with your own API key. |

Coder finds each coding agent when it's installed on your computer. In
the [Terminal](/docs/terminal), `/plugins` turns each plugin on or off.

## Why plugins

OpenAgents is one general agent made of many specialized parts. Anyone
can add a part, and a part only becomes one of Coder's defaults after it
was measured with and without, rerun by other people, and shown to help
on someone else's tests. The argument is in
[The Return of the General Agent](https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-10-01-the-return-of-the-general-agent.md).

You can also ask in any chat which plugins there are, and asking for
something no plugin does yet offers **Add a plugin**.

Next: [Write a plugin](/docs/write-a-plugin).
