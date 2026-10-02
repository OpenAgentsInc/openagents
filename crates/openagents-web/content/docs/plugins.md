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

Coder and the coding agents it works with (Codex, Claude Code, Grok
Build, OpenCode, and Devin) are not plugins. They are what plugins plug
into.

## Plugins you can look at now

| Plugin | What it does |
| --- | --- |
| Project map | Shows Coder how a project is laid out before it starts. Part of Coder's defaults. |
| Code finder | Searches a project for patterns and shows the matching lines. |
| Test reader | Reads test reports and shows each failing test with its file and line. |
| Explain this error | Finds the file and line a failing command points at and says the likely cause and fix. |
| Release notes | Groups the commits between two releases into release notes, each line citing its commit. |
| Dependency check | Reads manifests and lockfiles offline and flags duplicate versions, loose ranges, and licenses your policy doesn't allow. |

The last three are worked examples written to be copied. Each one's
source, tests, and results are on
[GitHub](https://github.com/OpenAgentsInc/openagents/blob/main/docs/plugins/examples/README.md).

## Why plugins

OpenAgents is one general agent made of many specialized parts. Anyone
can add a part, and a part only becomes one of Coder's defaults after it
was measured with and without, rerun by other people, and shown to help
on someone else's tests. The argument is in
[The Return of the General Agent](https://github.com/OpenAgentsInc/openagents/blob/main/docs/essays/2026-10-01-the-return-of-the-general-agent.md).

You can also ask in any chat: "Which plugins can I test?" lists them all,
and asking for something no plugin does yet offers **Add a plugin**.

Next: [Write a plugin](/docs/write-a-plugin).
