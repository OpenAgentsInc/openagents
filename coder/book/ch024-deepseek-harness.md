# 24. DeepSeek Harness

> **Status: written.** Read at deepseek-ai/deepseek-harness@ce7073d51254c26cd63b22a60835b8f948deb447 on 2026-11-02.

DeepSeek Harness (`dsh`) is DeepSeek's open agent harness: MIT licensed,
TypeScript on Node, built on the [Cordis](https://github.com/cordiverse/cordis)
plugin framework, and marked as an experimental developer preview
(`SAFETY.md`). Parts I–IV of this book took their scope from it, through the
[DeepSeek Harness notes](../../psionic/docs/harness.md) written on
2026-10-29 at revision `f3803c9`. This chapter rereads the source at
`ce7073d` (committed 2026-10-30), keeps the notes' findings that still hold,
and records what changed.

The source was cloned read only into `projects/repos/deepseek-harness`.
Nothing was installed or run. Every path below is relative to that clone.

## Why study it

DeepSeek publishes its models, its serving engine and this harness, and the
harness is designed around that serving stack. The clearest sign is the
model-visible prefix. Every harness addition to a request, such as identity
headers, the plugin inventory and the session log upload, travels outside
`messages`, system prompts and tool schemas, so it "does not add model-input
tokens or alter the model-visible prefix"
(`docs/deepseek-llm-api-wire-extensions.md`). Psionic also owns both a
server and a harness, so the same design is open to Coder.

The harness is also the most complete public example of a log-first agent.
Its architecture document states one rule that the rest follows: "Every model
request must be reconstructable from the log" (`docs/architecture.md`,
"Session log"). Coder's chapter 2 adopts that rule word for word.

## What it does

| Feature | File at `ce7073d` | Coder chapter |
| --- | --- | --- |
| A turn is zero or more steps; a step is one model request plus the tools it calls; `turn/*` and `step/*` are durable events | `docs/architecture.md` ("Turn flow") | [1](ch001-the-simplest-loop.md) |
| Model history derived from an append-only session log; immutable versioned generations with one migration package per version step | `docs/architecture.md` ("Session log"), `packages/core/session/src/types.ts`, `packages/session/session-format-v3-to-v4/` | [2](ch002-model-visible-means-logged.md) |
| Request additions outside the model input; a body-extension transaction accepted only after HTTP 2xx | `docs/deepseek-llm-api-wire-extensions.md` | [3](ch003-keep-the-prefix-stable.md) |
| A scoped tool registry and a guarded `tools/pre-execute`, `execute`, `post-execute` pipeline | `docs/tool-execution-pipeline.md`, `packages/core/tools/` | [4](ch004-tools-as-typed-capabilities.md) |
| `read-only`, `workspace-write` and `danger-full-access` confinement, with a one-time escalation the person approves | `packages/sandbox/README.md` | [5](ch005-a-sandbox-and-approvals.md) |
| An advisory reminder at 3, 5 and 8 identical tool calls; cooperative per-tool timeouts | `packages/guard/repeat-tool-reminder/README.md`, `packages/guard/timeout-policy/README.md` | [5](ch005-a-sandbox-and-approvals.md) |
| Compaction bracketed by `compaction/start` and `compaction/end` in the log, with tool-result pruning first | `packages/compaction/compaction/README.md` | [7](ch007-condense-the-history.md) |
| Programmatic tool calling: a model-written program run against host bindings | `packages/ptc-runtime/README.md` | [8](ch008-programs-as-tool-calls.md) |
| Subagents in process (fresh or forked from history) and out of process: ACP, Codex, Claude Code, another `dsh` | `packages/subagent/README.md` | [10](ch010-drive-claude-code.md), [11](ch011-drive-codex.md), [14](ch014-forks-subagents-and-jobs.md) |
| Background jobs owned by one agent session; same-session goals | `packages/jobs/README.md`, `packages/goal/` | [14](ch014-forks-subagents-and-jobs.md) |
| Everything is a plugin: profiles of ordered bundles, patches by row id, registrations that unwind on unload | `docs/architecture.md` ("Profiles and bundles"), `packages/boot/` | [15](ch015-plugins-with-owned-lifecycles.md) |
| An MCP client and resources; an ACP server profile | `packages/mcp/`, `packages/acp/` | [16](ch016-speak-the-protocols.md) |
| Benchmarks for session open, conversation fold, stream reconnect, agent continuation, terminal I/O | `benchmarks/` | [21](ch021-measure-the-harness.md) |

### The loop and the log

The turn flow in `docs/architecture.md` is the shape of Coder's chapter 1. A
turn opens before its first input is claimed and closes "once nothing is
owed". Inside it, each step logs `step/start`, sends one request, records the
assistant message and the tool calls and results, then logs `step/end`. A
step that fails records the tool results it still owes. The loop derives the
model history from the log and freezes it before streaming, so a retry sends
the same request without assembling the prompt again.

The log is versioned more carefully than in the 2026-10-29 notes. The writer
is now format 4 (`SESSION_FORMAT_VERSION = 4` in
`packages/core/session/src/types.ts`). Each format lives in its own file
generation, `session.vN.jsonl`, and committed generations "are never renamed,
replaced, or deleted" (`docs/architecture.md`). Opening an old session runs a
chain of adjacent migrations, one package per step
(`packages/session/session-format-v0-to-v1/` through `v3-to-v4/`), and a
write publishes a new generation beside the old one.
`docs/session-format-status.md` records which format has shipped, because "an
alpha, beta, or release-candidate product publication establishes released
Session-format obligations."

### The prefix and what leaves the machine

The wire extensions document defines three request headers and two body
fields, both on by default. `dsh_plugin_packages` sends the active plugin
package names and versions. `dsh_session_log` sends "one contiguous suffix of
the canonical Session log", up to 8 MiB per request, picking up after the
last accepted watermark (`docs/deepseek-llm-api-wire-extensions.md`). The
same document lists what that suffix can contain: the working directory,
system prompts, user and assistant content, tool arguments and results,
compaction summaries and plugin events. A composition turns it off with
`enabled: false`.

### Safety

`packages/sandbox/README.md` describes three file-effect modes and states the
limit plainly: "Confinement is same-world only: it shares the host kernel and
filesystem." `SAFETY.md` adds that the project "has not undergone a security
audit" and recommends a disposable VM or container. The repeat-tool reminder
"never blocks or delays a legitimate repeated call"; it adds a message
asking the model to change approach (`packages/guard/repeat-tool-reminder/README.md`).

### Other agents as subagents

The largest change since the notes is in `packages/subagent/`. A `dsh`
agent can now delegate to a fresh in-process child, a child seeded with the
parent's history, or an out-of-process child over ACP, Codex or Claude Code
(`packages/subagent/README.md`). The Codex provider starts a "fresh isolated
Codex thread" through the app-server protocol, maps a `permissionMode` of
`never`, `approve-for-me` or `dangerously-bypass-approvals-and-sandbox` to
`thread/start` fields, and removes credential-shaped variables from the
child's environment before applying its own
(`packages/subagent/subagent-codex/README.md`). The Claude Code provider runs
one self-contained task through the Agent SDK and returns only the final
answer, keeping "reasoning, tool traffic, stderr, usage, and workspace diffs"
out of the parent session (`packages/subagent/subagent-claude-code/README.md`).
`packages/experimental/hooks-claude-code/` and `hooks-codex/` run a person's
existing Claude Code or Codex hooks inside `dsh`.

### The Super-agent design

The notes summarize the team's article "Exploring Super-agent Design" (a
Router and Workers, Triggers, two-stage filtering, Threads, batching, full
duplex, a watchdog), and Coder's chapters 17 and 20 build on it. The notes
also name a `dsh-superagent-prototype` plugin. No file at `ce7073d` matches
`superagent`, so this revision does not show that prototype. The article
remains the source for those chapters; the background jobs it relies on are
in `packages/jobs/`.

## What Coder takes

- **The turn and step boundaries** ([chapter 1](ch001-the-simplest-loop.md)).
  Coder's loop uses the same definition: a turn closes when nothing is owed,
  and a failed step still records the tool results it owes.
- **Model-visible means logged** ([chapter 2](ch002-model-visible-means-logged.md)).
  The bounded experiment rebuilds its token requests byte for byte; live
  Coder rebuilds submitted engine-turn inputs, not opaque engine-internal
  model history. Immutable generations are a proposed migration design:
  a format change would write a new file beside the old one. They are not
  yet qualified by chapter 2's live JSONL implementation.
- **Metadata outside the prefix** ([chapter 3](ch003-keep-the-prefix-stable.md)).
  Coder keeps identity, timing and engine metadata out of the model input, so
  appending a turn never changes earlier bytes.
- **The before, run, after pipeline** ([chapter 4](ch004-tools-as-typed-capabilities.md))
  and **three confinement modes with a one-time escalation**
  ([chapter 5](ch005-a-sandbox-and-approvals.md)). The same three names
  appear in Codex (chapter 26), so Coder's policy can map to either engine.
- **The compaction bracket** ([chapter 7](ch007-condense-the-history.md)):
  a start and end marker in the log, where an unmatched start means the
  operation is still busy.
- **Other agents behind one subagent interface**
  ([chapter 13](ch013-choose-an-engine.md), [chapter 14](ch014-forks-subagents-and-jobs.md)).
  `dsh` treats Codex and Claude Code as providers on `ctx.subagents`. Coder
  makes the same choice one level up: they are engines, and Coder's own
  clients in `coax/lib/agent/claude/` and `coax/lib/agent/codex/` drive
  them. The `dsh` Codex provider's environment scrub matches what Coder's
  Codex client already does (it removes `OPENAI_API_KEY` and
  `CODEX_API_KEY`; `coax/lib/agent/codex/PARITY.md`).
- **Format status as a record**: Coder's log format and the version that has
  shipped should be written down separately, as
  `docs/session-format-status.md` does. This is proposed compatibility work.

## What Coder leaves, and why

- **The default session log upload.** `dsh_session_log` is on by default and
  sends session content to the model provider. Coder's intended policy
  distinguishes authorized inference from optional log upload. Chapter 18
  currently gates remembered facts and secondary System One calls, not
  every conversation byte. See the [outbound inventory](CONTRACT.md#outbound-authorization-and-current-enforcement)
  for the implemented coverage and remaining integration.
- **Extension points everywhere.** `dsh` exposes waterfall events at the
  request, stream and every tool phase (`docs/architecture.md`, "Turn
  flow"). Coder starts from one loop and keeps an addition only with a
  measured gain on tasks it was not built from
  ([chapter 21](ch021-measure-the-harness.md)).
- **Host-authority plugins.** A `dsh` plugin runs in the Node process with
  the host's authority; Cordis scopes are lifecycle, not containment. Coder's
  third-party plugins run in bounded Wasm with admitted capabilities
  ([chapter 15](ch015-plugins-with-owned-lifecycles.md)).
- **Node and TypeScript.** Coder's harness logic is Coax, and the host does
  processes, files and network.
- **Advisory guards only.** The repeat reminder never blocks. Coder's guard
  against repeated failing calls can refuse, because chapter 5's policy
  already decides every call; the reminder form is kept as the first
  threshold.
- **A plugin inventory on the wire.** Coder does not tell a model provider
  which plugins a person runs.

## Where to measure a borrowed design

- Prefix stability: [chapter 3](ch003-keep-the-prefix-stable.md) checks that
  appends never change earlier bytes; Psionic measures the cache hits on the
  server side.
- Compaction: [chapter 7](ch007-condense-the-history.md) measures the prefill
  cost of a summary against the history it replaces.
- Subagents and forks: [chapter 21](ch021-measure-the-harness.md) compares a
  run with forks against the simplest loop on the same suite.
- The Super-agent results (Router and Workers against one session) are rerun
  in [chapter 17](ch017-one-agent-many-tasks.md).

## Licensing

MIT (`LICENSE`). Coder ports designs and copies no code or text.

## Sources

- [deepseek-ai/deepseek-harness at ce7073d](https://github.com/deepseek-ai/deepseek-harness/tree/ce7073d51254c26cd63b22a60835b8f948deb447)
- [docs/architecture.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/docs/architecture.md)
- [docs/deepseek-llm-api-wire-extensions.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/docs/deepseek-llm-api-wire-extensions.md)
- [docs/session-format-status.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/docs/session-format-status.md)
- [docs/tool-execution-pipeline.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/docs/tool-execution-pipeline.md)
- [packages/core/session/src/types.ts](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/core/session/src/types.ts)
- [packages/sandbox/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/sandbox/README.md)
- [packages/guard/repeat-tool-reminder/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/guard/repeat-tool-reminder/README.md)
- [packages/compaction/compaction/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/compaction/compaction/README.md)
- [packages/ptc-runtime/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/ptc-runtime/README.md)
- [packages/subagent/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/subagent/README.md)
- [packages/subagent/subagent-codex/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/subagent/subagent-codex/README.md)
- [packages/subagent/subagent-claude-code/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/subagent/subagent-claude-code/README.md)
- [packages/jobs/README.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/packages/jobs/README.md)
- [SAFETY.md](https://github.com/deepseek-ai/deepseek-harness/blob/ce7073d51254c26cd63b22a60835b8f948deb447/SAFETY.md)
- [DeepSeek Harness notes](../../psionic/docs/harness.md), the 2026-10-29 survey this chapter builds on.
- [Chapter plan](PLAN.md), Part VII.
