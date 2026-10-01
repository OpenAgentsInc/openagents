# Plugins

A plugin is anything you add to OpenAgents. You write a plugin, test it
with Coder and without it, and publish it so other people can check your
result and use it.

This page is the starting point for people who make plugins. The
engineering specifications behind it stay in
[`docs/extensions/`](../extensions/README.md), and the
[glossary](../glossary.md#one-vocabulary-what-you-can-add) maps the word
*plugin* onto the precise terms those specifications use.

## What a plugin contains

A plugin can contain any of these parts:

| Part | What it is | Where it lives in a plugin directory |
| --- | --- | --- |
| Skills | Instructions the agent reads before a task, as `SKILL.md` guides. A skill runs no code and grants nothing. | `skills/` |
| Workflows | Typed, step-by-step programs that a host runs: look things up, check, decide, delegate, or run Wasm. Workflows name what they use by digest. The protocol calls a workflow a *program* ([NIP-PRG](../../nips/openagents/NIP-PRG.md)). | `programs/`, pinned by `program` in `package.json` |
| Knowledge | Cited reference entries: a method, an edge case, a common slip, or how a command is used ([NIP-KB](../../nips/openagents/NIP-KB.md)). | Published as NIP-KB entries with `openagents kb publish`, beside the plugin's release |
| Wasm | Sandboxed WebAssembly code that performs one bounded operation, such as mapping a repository's files. It is the only executable code a plugin can carry. It runs with no network, under fuel and memory limits, and reads only what the host hands it. A workflow runs it; a model never calls it by name. | A guest crate built against [`crates/plugin-pdk`](../../crates/plugin-pdk/) |
| Tests | The test set that shows whether the plugin helps: tasks Coder runs with the plugin and without it, and the checks on each run. | `evals/` |

Not every plugin has every part. A plugin made in chat is a single skill.
Project map is a workflow that runs one piece of Wasm, with its tests.

Coder and the coding agents it works with (Codex, Claude Code, Grok Build,
OpenCode, and Devin) are not plugins. They are the agents plugins plug
into.

We don't call any part of a plugin a *tool*. In AI products a tool is
something a model chooses to call by name. A plugin's Wasm is chosen and run
by workflows and typed decisions, never by a name a model writes.

## Plugins you can use now

| Plugin | What it does | Source |
| --- | --- | --- |
| Project map | Shows Coder how a project is laid out before it starts: its files, languages, largest files, build files, and tests. | [`crates/plugin-repo-map`](../../crates/plugin-repo-map/) |
| Code finder | Searches a project for up to 16 patterns and shows Coder the matching lines grouped by file. | [`crates/plugin-code-search`](../../crates/plugin-code-search/) |
| Test reader | Reads a project's test reports (JUnit XML, `cargo test`, or pytest output) and shows Coder each failing test with its file, line, and message. | [`crates/plugin-test-report`](../../crates/plugin-test-report/) |

Each directory is a complete plugin to copy: `package.json`, a workflow
under `programs/`, the Wasm crate itself, and a test set under `evals/`.
More worked examples (Explain this error, Release notes, and Dependency
check) are being written in
[#10086](https://github.com/OpenAgentsInc/openagents/issues/10086) and
will be listed here when they land.

## Make a plugin in chat

In the OpenAgents app, ask "Help me make a plugin that ...". We draft the
plugin and its tests with you one step at a time, try it once, and then run
the full test set on our computers. A plugin made in chat is a skill, and
it can turn on plugins we already have, such as Project map. A plugin that
needs new code goes to Coder on a computer you connect.

## Write a plugin

A plugin is a directory with a package record, `package.json`, at its
root. The record names the plugin and pins its workflow by digest:

```json
{
  "v": 1,
  "slug": "project-map",
  "name": "Project map",
  "summary": "Shows Coder how the project is laid out before it starts.",
  "version": "0.1.0",
  "publisher": "<your 64-hex public key>",
  "program": { "name": "project-map", "digest": "<sha-256 of programs/project-map.json>" }
}
```

Add the parts your plugin needs:

1. Put guidance in `skills/<name>/SKILL.md`.
1. Put the workflow in `programs/<name>.json`. The
   [workflow guide](../programs.md) covers step kinds, sources, and
   bounds.
1. Build Wasm as a guest crate against `crates/plugin-pdk`. The
   [Wasm host specification](../extensions/plugins.md) covers profiles,
   limits, and build receipts.
1. Write knowledge entries under `knowledge/` and check them with
   `openagents kb`.

## Test a plugin

A plugin's tests live in `evals/`, one directory per test, each with a
`prompt.md` task and checks under `graders/`. Write them with the
interview, then run them with the plugin and without it:

```sh
cd my-plugin
openagents plugin test init               # write the tests with us, step by step
openagents plugin test run . --runs 1     # try each test once
openagents plugin test run .              # the full run: three times with, three without
```

A run writes `report.json` and `report.html` under `evals/results/`. The
result is a test result: for example, "passed 5 of 6 with it, 2 of 6
without", with a verdict of **Better**, **No clear change**, or **Worse**.
Nothing leaves your computer until you publish. The
[test specification](../extensions/evaluation.md) has the test format, the
checks, the sandbox, and how the verdict is decided.

## Publish a plugin and its result

```sh
openagents plugin test publish evals/results/<timestamp>/report.json
```

`publish` adds your result to the Gym: it releases your test set, signed by
your key, and publishes the result. Other trainers rerun your tests with
`openagents plugin test check <result id>` (in the app, **Check a result**)
to confirm or dispute it. When three trainers confirm a **Better** result
and the plugin also does better on a test set someone else wrote, Coder can
use the plugin for everyone; `openagents plugin defaults sync` writes the
plugins Coder uses on your computer.

`openagents plugin list` shows published plugins. `openagents ext` is the
older name for `openagents plugin` and still works.

## Words we use

| You read | It means | In the specifications |
| --- | --- | --- |
| Plugin | Anything you add | The plugin's parts, shipped in an extension package ([NIP-EXT](../../nips/openagents/NIP-EXT.md)) |
| Skill | Instructions the agent reads | A `skill` component |
| Workflow | A typed, step-by-step program | A program ([NIP-PRG](../../nips/openagents/NIP-PRG.md)), a `program` component |
| Knowledge | Cited reference entries | A knowledge entry ([NIP-KB](../../nips/openagents/NIP-KB.md)) |
| Wasm | Sandboxed code with one bounded operation | A Wasm guest, the `plugin` component kind |
| Tests | The tasks and checks that show whether a plugin helps | An eval suite, the `eval-suite` component |
| Test result | What the tests showed, with and without the plugin | A capability claim ([NIP-EVAL](../../nips/openagents/NIP-EVAL.md) `3189`) |

The [glossary](../glossary.md#one-vocabulary-what-you-can-add) explains
each internal term.
