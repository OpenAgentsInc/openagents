# 28. Goose

> **Status: written.** Read at block/goose@0909288ed4a2053b10540bfcf6dbee3b0de66126 on 2026-11-02.

Source: [`block/goose`](https://github.com/block/goose/tree/0909288ed4a2053b10540bfcf6dbee3b0de66126),
cloned shallowly into `projects/repos/goose` and read only. Nothing was
installed or run. Paths below are relative to that revision. The repository's
own workflows name the upstream as `aaif-goose/goose`
(`.github/workflows/model-toolcall-conformance.yml`); the clone URL used here
was `github.com/block/goose`.

## Why study it

Goose is a general agent with a Rust core, a CLI and an Electron desktop app.
It builds almost every capability as an MCP extension: its own developer tools,
memory and computer control ship as MCP servers in `crates/goose-mcp/`, and a
person adds others by command or URL (`crates/goose/src/agents/extension.rs`).
It packages repeatable work as **recipes**, YAML files with instructions,
parameters, extensions, a response schema and success checks
(`crates/goose/src/recipe/mod.rs`). It also tests itself on several levels:
Rust unit and integration tests, recorded MCP sessions, provider smoke tests
across a model matrix, desktop end-to-end tests, a recipe in which Goose tests
itself, and terminal-bench runs (`crates/goose/tests/`, `ui/desktop/tests/`,
`goose-self-test.yaml`, `evals/harbor/`).

Coder's [chapter 15](ch015-plugins-with-owned-lifecycles.md) designs plugins,
[chapter 16](ch016-speak-the-protocols.md) speaks MCP, and
[chapter 21](ch021-measure-the-harness.md) measures the harness. Goose has
working answers for all three in one codebase.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| Extensions as typed configs: `stdio`, `builtin`, `platform`, `streamable_http` | `crates/goose/src/agents/extension.rs` (`ExtensionConfig`) | [15](ch015-plugins-with-owned-lifecycles.md), [16](ch016-speak-the-protocols.md) |
| An extension manager with one module per transport | `crates/goose/src/agents/extension_manager/` | [16](ch016-speak-the-protocols.md) |
| A malware check against OSV before an extension package runs | `crates/goose/src/agents/extension_malware_check.rs` | [15](ch015-plugins-with-owned-lifecycles.md) |
| Built-in MCP servers: memory, computer control, tutorial | `crates/goose-mcp/src/` | [16](ch016-speak-the-protocols.md) |
| Platform extensions inside the process: developer tools, todo, summon, code execution | `crates/goose/src/agents/platform_extensions/` | [8](ch008-programs-as-tool-calls.md), [14](ch014-forks-subagents-and-jobs.md) |
| Code mode: model-written JavaScript in an isolated Boa context that calls tools | `crates/goose/src/agents/platform_extensions/code_execution.rs`, `javascript.rs` | [8](ch008-programs-as-tool-calls.md) |
| Four permission modes: `auto`, `approve`, `smart_approve`, `chat` | `crates/goose-provider-types/src/goose_mode.rs`, `crates/goose/src/permission/` | [5](ch005-a-sandbox-and-approvals.md) |
| A repeated-call inspector | `crates/goose/src/tool_monitor.rs` | [5](ch005-a-sandbox-and-approvals.md) |
| Tool output over 200,000 characters written to a file | `crates/goose/src/agents/large_response_handler.rs` | [7](ch007-condense-the-history.md) |
| Auto-compaction capped at 225,000 tokens | `crates/goose/src/context_mgmt/mod.rs` | [7](ch007-condense-the-history.md) |
| Recipes: instructions, parameters, extensions, response schema, sub-recipes, retry with checks | `crates/goose/src/recipe/mod.rs`, `crates/goose/src/agents/types.rs` (`RetryConfig`) | [14](ch014-forks-subagents-and-jobs.md), [22](ch022-generate-candidates-then-check-them.md) |
| Recorded MCP sessions replayed in tests | `crates/goose/tests/mcp_integration_test.rs`, `crates/goose/tests/mcp_replays/` | [16](ch016-speak-the-protocols.md), [21](ch021-measure-the-harness.md) |
| Provider smoke tests over every available provider and model | `ui/desktop/tests/integration/test_providers.test.ts`, `test_providers_lib.ts` | [21](ch021-measure-the-harness.md) |
| A daily tool-call conformance sweep that is not a PR gate | `.github/workflows/model-toolcall-conformance.yml` | [21](ch021-measure-the-harness.md) |
| Desktop end-to-end tests with Playwright | `ui/desktop/tests/e2e/`, `ui/desktop/playwright.config.ts` | [36](ch036-a-terminal-ui.md) |
| A recipe in which Goose tests its own tools | `goose-self-test.yaml` | [21](ch021-measure-the-harness.md) |
| Terminal-bench runs compared across harnesses and settings | `evals/harbor/README.md` | [21](ch021-measure-the-harness.md) |

### Extensions over MCP

`ExtensionConfig` is a tagged enum. A `stdio` extension names a command,
arguments, environment values and the names of environment keys to pass
through. A `streamable_http` extension names a URL. A `builtin` extension runs
one of the servers in `crates/goose-mcp/` as a child process, and a `platform`
extension runs inside the agent with direct access to its state
(`crates/goose/src/agents/extension.rs`). The extension manager has a module
per transport: `builtin.rs`, `stdio.rs` and `streamable_http.rs`, plus
`lease.rs` (`crates/goose/src/agents/extension_manager/`). Before Goose starts
a `stdio` extension, `deny_if_malicious_cmd_args` looks at the command and its
arguments (`extension_manager/stdio.rs`); for a package it queries the OSV
database and refuses the package if any advisory has an ID that starts with
`MAL-` (`crates/goose/src/agents/extension_malware_check.rs`).

The platform extensions show what Goose keeps in process:
`developer`, `todo`, `summon`, `code_execution`, `chatrecall`, `scheduler`,
`summarize` and others (`crates/goose/src/agents/platform_extensions/`).
`code_execution` gives the model three tools: list the available functions,
read one function's details, and run JavaScript that defines `async function
run()` and calls tools with `await tools.call(...)`. The instructions state
that the JavaScript context has no filesystem, shell, network, packages or
environment, so every effect goes through a discovered tool
(`code_execution.rs`, `INSTRUCTIONS`). The engine is Boa
(`platform_extensions/javascript.rs`).

### Permissions and guards

`GooseMode` has four values: `auto` approves every tool call, `approve` asks
before each one, `smart_approve` asks only for sensitive calls, and `chat`
allows no tool calls (`crates/goose-provider-types/src/goose_mode.rs`).
Per-tool decisions persist in `crates/goose/src/permission/permission_store.rs`,
keyed by a hash of the call's arguments and with an optional expiry. In
`smart_approve` mode, `permission_inspector.rs` checks a tool's read-only
annotation, and `permission_judge.rs` gives a model a tool that sorts the
requested calls into read-only and not. `tool_monitor.rs` compares each call's name and arguments with
earlier ones to catch a loop of repeated calls.

### Context

`large_response_handler.rs` writes any text tool result over 200,000
characters (`DEFAULT_LARGE_TEXT_THRESHOLD`) to a temporary file and gives the
model a reference to it instead. Auto-compaction starts at a configured share
of the context window, capped at 225,000 tokens; the comment says the cap keeps
requests under a provider's long-context price tier
(`crates/goose/src/context_mgmt/mod.rs`, `DEFAULT_AUTO_COMPACT_TOKEN_LIMIT`).

### Recipes

A `Recipe` has a version, title and description, then optional `instructions`,
`prompt`, `extensions`, `settings`, `parameters`, `response`, `sub_recipes` and
`retry` (`crates/goose/src/recipe/mod.rs`). `response` carries a JSON schema
for the final output. A `SubRecipe` names another recipe file with parameter
values and can run sequentially when repeated. `RetryConfig` sets
`max_retries`, a list of `checks`, an optional `on_failure` shell command and
timeouts of 300 seconds per check and 600 seconds for `on_failure` by default
(`crates/goose/src/agents/types.rs`). A recipe is a task with a definition of
done that a program can check.

### Testing across desktop and CLI

Goose tests at five levels:

1. **Rust tests.** `crates/goose/tests/` has 39 entries, among
   them compaction, permission persistence, repetition inspection, subprocess
   cleanup and ACP sessions.
2. **Recorded MCP sessions.** `mcp_integration_test.rs` runs real MCP servers
   in `Record` mode and stores their traffic in `tests/mcp_replays/`; in
   `Playback` mode it replays that traffic and compares results with the
   stored `.results.json` files.
3. **Provider smoke tests.** `discoverTestCases` builds one test per available
   provider and model; each test runs `goose run` with the developer extension,
   asks the model to `cat` two files with random tokens, and checks that the
   shell tool ran and both tokens came back
   (`ui/desktop/tests/integration/test_providers.test.ts`,
   `test_providers_lib.ts`). Unavailable providers are recorded as skipped
   with a reason, and models can be marked flaky.
4. **A daily conformance sweep.** `model-toolcall-conformance.yml` runs the top
   tool-capable OpenRouter models once a day. Its header says it is "deliberately
   NOT a PR gate", because a failure there usually means a provider regressed,
   not Goose.
5. **End to end and self-test.** Playwright tests drive the desktop app
   (`ui/desktop/tests/e2e/`). `goose-self-test.yaml` is a recipe in which a
   running Goose exercises its own file, shell, extension, delegation and
   compaction features and writes a report.

`evals/harbor/README.md` reports terminal-bench 2 results over 89 tasks. On
the same model it lists stock Goose at 50.6%, Goose with code mode at 57.3%,
and other harnesses between 47.2% and 55.1%, with compute hours, tokens, turns
and cost per run. This chapter did not rerun these numbers.

## What Coder takes

- **MCP as the extension boundary.** Coder already speaks an MCP subset in
  [`psionic/lib/harness/protocols.coax`](../../psionic/lib/harness/protocols.coax)
  and serves in-process MCP tools to Claude Code in
  [`coax/lib/agent/claude/mcp.coax`](../../coax/lib/agent/claude/mcp.coax).
  Goose shows the rest of the shape: one typed config per transport, built-in
  servers that use the same protocol as third-party ones, and a short list of
  in-process extensions for what needs the agent's own state.
- **A check before an extension runs.** Goose refuses known-malicious packages
  before start. Chapter 15 signs third-party plugins and runs them in bounded
  Wasm. A recorded package-malware check before a package-based MCP server
  starts is proposed work, not a guarantee of the current finite MCP adapter.
- **Recipes as tasks with a definition of done.** `RetryConfig.checks` is a
  shell-checkable success condition attached to a task. This fits
  [chapter 22](ch022-generate-candidates-then-check-them.md)'s generate, run
  and repair loop and [chapter 21](ch021-measure-the-harness.md)'s task suites,
  where each task needs a checker. Coder takes the shape: a task file with
  instructions, parameters, a response schema and checks.
- **Recorded protocol sessions as tests.** Goose's MCP replays match how Coder
  tests its Codex driver against recorded sessions
  ([`coax/lib/agent/codex/fixtures`](../../coax/lib/agent/codex/fixtures/)).
  Coder records MCP servers the same way for chapter 16's live transports.
- **Two test tiers for models.** A small per-change smoke test on every
  available provider, and a broader sweep on a schedule that never blocks a
  change. Coder adopts the split for its gateways: offline fixtures on every
  change, live model checks on a schedule, skipped without a key.

## What Coder leaves, and why

- **An agent-wide mode that approves everything.** Goose's default mode is
  `auto`. Coder's configured policy can decide per tool, but its current
  local Claude Code and Codex defaults bypass permission prompts unless
  policy or asking options select another mode (chapters 10 and 11).
  The policy design in chapter 5 is not evidence that defaults are confined.
- **A model as the read-only judge.** `smart_approve` asks a model whether a
  call is sensitive. Coder decides by meaning through a typed System One
  decision with a labelled set ([chapter 13](ch013-choose-an-engine.md) shows
  the pattern), and keeps confinement as the guarantee rather than the
  judgment.
- **JavaScript as the program language.** Code mode runs JavaScript in Boa.
  Coder's programs as tool calls ([chapter 8](ch008-programs-as-tool-calls.md))
  currently use a small bounded tool-call language interpreted in Coax,
  with variables, a condition and fixed loops. That experiment is not an
  implementation of arbitrary Coax programs as tool calls.
- **A self-test run by the agent under test.** `goose-self-test.yaml` asks the
  model to judge its own features. Coder keeps acceptance in deterministic
  checks and scripted models, and uses model runs for measurement, not as the
  pass or fail signal.
- **An Electron desktop.** Coder's interfaces are the inline terminal UI
  ([chapter 36](ch036-a-terminal-ui.md), [`coder/tui/`](../tui/coder.coax)) and
  the OpenAgents apps; there is no separate Coder desktop shell to test.

## Where to measure a borrowed design

- **Code mode.** Goose's own numbers show code mode ahead of stock Goose on
  terminal-bench 2 (57.3% against 50.6%, `evals/harbor/README.md`).
  [Chapter 8](ch008-programs-as-tool-calls.md) builds programs as tool calls,
  and [chapter 21](ch021-measure-the-harness.md) measures them as one change
  against the simplest loop, on tasks not used to build them.
- **Large tool results to a file.** Chapter 21 can compare Microcoder's head
  and tail cut ([`state.coax`](../../coax/lib/microcoder/state.coax)) with
  Goose's approach of moving large output to a file the model can read in
  parts, measured in steps and input tokens per task.
- **A compaction cap tied to price.** [Chapter 7](ch007-condense-the-history.md)
  measures when to condense; a token cap below a provider's price tier is one
  schedule to add to its comparison.
- **Repeated calls.** [Chapter 5](ch005-a-sandbox-and-approvals.md) already
  guards against repeated failing calls. Goose's inspector compares name and
  arguments; chapter 5's fixtures can include its cases.

## Sources

- [`crates/goose/src/agents/extension.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/extension.rs)
- [`crates/goose/src/agents/extension_manager/`](https://github.com/block/goose/tree/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/extension_manager)
- [`crates/goose/src/agents/extension_malware_check.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/extension_malware_check.rs)
- [`crates/goose-mcp/src/`](https://github.com/block/goose/tree/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose-mcp/src)
- [`crates/goose/src/agents/platform_extensions/code_execution.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/platform_extensions/code_execution.rs)
- [`crates/goose/src/agents/platform_extensions/javascript.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/platform_extensions/javascript.rs)
- [`crates/goose-provider-types/src/goose_mode.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose-provider-types/src/goose_mode.rs)
- [`crates/goose/src/permission/`](https://github.com/block/goose/tree/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/permission)
- [`crates/goose/src/tool_monitor.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/tool_monitor.rs)
- [`crates/goose/src/agents/large_response_handler.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/large_response_handler.rs)
- [`crates/goose/src/context_mgmt/mod.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/context_mgmt/mod.rs)
- [`crates/goose/src/recipe/mod.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/recipe/mod.rs)
- [`crates/goose/src/agents/types.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/src/agents/types.rs)
- [`crates/goose/tests/mcp_integration_test.rs`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/crates/goose/tests/mcp_integration_test.rs)
- [`ui/desktop/tests/integration/test_providers.test.ts`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/ui/desktop/tests/integration/test_providers.test.ts)
- [`ui/desktop/tests/integration/test_providers_lib.ts`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/ui/desktop/tests/integration/test_providers_lib.ts)
- [`ui/desktop/tests/e2e/`](https://github.com/block/goose/tree/0909288ed4a2053b10540bfcf6dbee3b0de66126/ui/desktop/tests/e2e)
- [`.github/workflows/model-toolcall-conformance.yml`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/.github/workflows/model-toolcall-conformance.yml)
- [`goose-self-test.yaml`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/goose-self-test.yaml)
- [`evals/harbor/README.md`](https://github.com/block/goose/blob/0909288ed4a2053b10540bfcf6dbee3b0de66126/evals/harbor/README.md)
- Coder: [`psionic/lib/harness/protocols.coax`](../../psionic/lib/harness/protocols.coax),
  [`coax/lib/agent/claude/mcp.coax`](../../coax/lib/agent/claude/mcp.coax),
  [`coax/lib/microcoder/state.coax`](../../coax/lib/microcoder/state.coax).
- [Chapter plan](PLAN.md), Part VII; the shape follows the
  [DeepSeek Harness notes](../../psionic/docs/harness.md).
