# 25. Claude Code

> **Status: written.** Read at AtlantisPleb/cc@abcc00908c056a6fb2bfaa4ba8933bb65daaf5f7 on 2026-11-02.

Claude Code is Anthropic's coding agent: a terminal program, `claude`, that
also runs headless and behind the Claude Agent SDK. Coder already drives it.
The client in `coax/lib/agent/claude/` speaks the CLI's stream-json protocol
and is checked against Claude Code 2.1.312
([PARITY.md](../../coax/lib/agent/claude/PARITY.md)), and the Coder TUI runs
it as the `claude` engine (`coder/tui/coder.coax`).

This chapter reads two sources. The first is `AtlantisPleb/cc`, a private
reference mirror of Claude Code's TypeScript source, cloned read only into
`projects/repos/cc`. Its own audit says the tree has no package manifest,
build files or tests, many modules sit behind build-time feature flags, and
"a source file's presence does not mean its behavior is enabled in a
particular artifact" (`docs/REPO_AUDIT.md`). The tree does not record which
release it matches. The second source is Anthropic's published
[Claude Code documentation](https://code.claude.com/docs/en/overview), which
describes the shipped product. Where the two agree, this chapter cites both.
Paths in the form `cc:path` are in the mirror at `abcc009`. The mirror is
not public, so those paths are not links; the docs links are. Nothing was
installed or run, and no code was copied.

## Why study it

Claude Code is the agent most people who use Coder already pay for, so
Coder must drive it well (chapter 10). It is also a mature answer to the
questions Parts I–III ask: how a tool declares what it touches, how a person
grants and limits authority, how a repository tells the agent its rules, and
how the agent splits work. Its answers are configuration a person already
has on disk, such as `CLAUDE.md` files, permission rules and hooks.

## What it does

| Feature | File at `abcc009`, or docs page | Coder chapter |
| --- | --- | --- |
| The query loop: stream, run tools, follow up until no tool is owed; compaction before each request | `cc:query.ts`, `cc:services/compact/autoCompact.ts`, `cc:services/compact/microCompact.ts` | [1](ch001-the-simplest-loop.md), [7](ch007-condense-the-history.md) |
| A tool declares `isReadOnly`, `isConcurrencySafe`, `isDestructive`, `validateInput`, `checkPermissions`, a result size cap | `cc:Tool.ts` | [4](ch004-tools-as-typed-capabilities.md) |
| Read-only, concurrency-safe calls run in parallel batches; others run one at a time | `cc:services/tools/toolOrchestration.ts` | [4](ch004-tools-as-typed-capabilities.md) |
| Permission modes `default`, `plan`, `acceptEdits`, `bypassPermissions`, `dontAsk`; rules from eight sources | `cc:types/permissions.ts`, [permissions](https://code.claude.com/docs/en/permissions) | [5](ch005-a-sandbox-and-approvals.md) |
| An OS sandbox for Bash: Seatbelt on macOS, bubblewrap on Linux, a host proxy with a domain allowlist | `cc:utils/sandbox/sandbox-adapter.ts`, [sandboxing](https://code.claude.com/docs/en/sandboxing) | [5](ch005-a-sandbox-and-approvals.md) |
| Hooks on 27 events; command, HTTP, MCP tool, prompt and agent handlers | `cc:entrypoints/sdk/coreTypes.ts`, `cc:schemas/hooks.ts`, [hooks](https://code.claude.com/docs/en/hooks) | [4](ch004-tools-as-typed-capabilities.md), [15](ch015-plugins-with-owned-lifecycles.md) |
| Memory files: managed, user, project and local `CLAUDE.md`, `.claude/rules/*.md`, `@path` includes | `cc:utils/claudemd.ts`, [memory](https://code.claude.com/docs/en/memory) | [6](ch006-a-document-tries-to-become-the-system-prompt.md), [19](ch019-remember-a-fact-then-revise-it.md) |
| Skills: `SKILL.md` with frontmatter; only the description loads until the skill runs | `cc:skills/loadSkillsDir.ts`, [skills](https://code.claude.com/docs/en/skills) | [3](ch003-keep-the-prefix-stable.md), [15](ch015-plugins-with-owned-lifecycles.md) |
| Deferred tools found through a search tool | `cc:tools/ToolSearchTool/`, `cc:Tool.ts` (`shouldDefer`) | [3](ch003-keep-the-prefix-stable.md) |
| Subagents from `.claude/agents/*.md`; built-in Explore, Plan and general-purpose agents; forks that inherit the parent context | `cc:tools/AgentTool/loadAgentsDir.ts`, `cc:tools/AgentTool/built-in/`, `cc:tools/AgentTool/forkSubagent.ts`, [subagents](https://code.claude.com/docs/en/sub-agents) | [14](ch014-forks-subagents-and-jobs.md) |
| Plugins that bundle commands, agents, hooks, skills and MCP servers, installed from marketplaces | `cc:utils/plugins/`, [plugins](https://code.claude.com/docs/en/plugins) | [15](ch015-plugins-with-owned-lifecycles.md) |
| Headless runs and the stream-json control protocol | `cc:cli/structuredIO.ts`, `cc:entrypoints/sdk/controlSchemas.ts`, [headless](https://code.claude.com/docs/en/headless) | [10](ch010-drive-claude-code.md), [23](ch023-openagents-coder.md) |

### The loop

`cc:query.ts` runs one `while (true)` loop per query. Before each request it
applies cheaper reductions first: "Apply microcompact before autocompact".
Automatic compaction starts when the conversation reaches the model's
effective context window minus a 13,000-token buffer
(`AUTOCOMPACT_BUFFER_TOKENS` in `cc:services/compact/autoCompact.ts`). After
the response, the loop sets `needsFollowUp` when tool calls are owed, runs
them and sends the next request.

### Tools

Each tool in `cc:Tool.ts` declares facts about one input, not about the tool
in general: `isReadOnly(input)`, `isConcurrencySafe(input)` and
`isDestructive(input)`. The defaults assume the worst: not safe for
concurrency, not read only. `cc:services/tools/toolOrchestration.ts`
partitions a response's tool calls into batches; a batch of
concurrency-safe calls runs in parallel, up to 10 by default
(`CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY`), and the rest run one by one.
`validateInput` runs before `checkPermissions`, so a malformed call is
refused before anyone is asked about it.

### Permissions and the sandbox

Five modes are public (`EXTERNAL_PERMISSION_MODES` in
`cc:types/permissions.ts`); an `auto` mode, where a classifier reviews
actions, sits behind a feature flag in the mirror and is documented on the
[permissions](https://code.claude.com/docs/en/permissions) page. A rule names
a tool and an optional specifier, such as `Bash(git commit *)`, and comes
from one of `userSettings`, `projectSettings`, `localSettings`,
`flagSettings`, `policySettings`, `cliArg`, `command` or `session`
(`cc:types/permissions.ts`). The docs fix the order: "Rules are evaluated in
order: deny, then ask, then allow. The first match in that order determines
the outcome, and rule specificity doesn't change the order." A bare tool
name in a deny rule removes the tool from the model's context.

The docs also state where rules stop. A `Read` deny rule covers Claude's own
file tools and the file commands Claude Code recognizes in Bash, but not "a
Python or Node script that opens files itself"; for that, the docs point to
the sandbox. The sandbox wraps `@anthropic-ai/sandbox-runtime`
(`cc:utils/sandbox/sandbox-adapter.ts`). It uses Seatbelt on macOS and
bubblewrap on Linux, limits writes to the working directories and the
temporary directory, and routes network through a local proxy that checks
each host against an allowlist that starts empty
([sandboxing](https://code.claude.com/docs/en/sandboxing)).

### Hooks

`HOOK_EVENTS` in `cc:entrypoints/sdk/coreTypes.ts` lists 27 events,
including `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`,
`SubagentStart`, `PreCompact` and `PermissionRequest`. A command hook blocks
a call by exiting with code 2, or by printing a JSON decision; its stderr is
what the model sees ([hooks](https://code.claude.com/docs/en/hooks)). The
same handler can be a shell command, an HTTP POST, an MCP tool, a one-turn
prompt or an agent (`cc:schemas/hooks.ts`). Hooks live in settings files,
plugins and skill or subagent frontmatter.

### Memory files and skills

`cc:utils/claudemd.ts` loads instruction files in a fixed order: managed
(`/etc/claude-code/CLAUDE.md`), user (`~/.claude/CLAUDE.md`), project
(`CLAUDE.md`, `.claude/CLAUDE.md` and `.claude/rules/*.md` from the root
down to the working directory) and local (`CLAUDE.local.md`). Files closer
to the working directory load later and so carry more weight. `@path`
includes other files, with cycles tracked and missing files ignored.

Skills are folders with a `SKILL.md`. At startup only the name, description
and `when_to_use` enter the context; the docs say "full skill content only
loads when invoked" ([skills](https://code.claude.com/docs/en/skills)), and
the loader estimates a skill's cost "based on frontmatter only"
(`cc:skills/loadSkillsDir.ts`). Tools can be deferred the same way: a tool
with `shouldDefer` is left out of the request until the model finds it with
the tool search tool (`cc:Tool.ts`, `cc:tools/ToolSearchTool/`).

### Subagents

The Agent tool starts a child with its own context and a chosen tool set.
Definitions come from `.claude/agents/*.md`
(`cc:tools/AgentTool/loadAgentsDir.ts`), and the mirror ships built-in
Explore, Plan, general-purpose and verification agents
(`cc:tools/AgentTool/built-in/`). A gated fork mode lets a call without a
`subagent_type` start a child that "inherits the parent's full conversation
context and system prompt", and runs every spawn in the background
(`cc:tools/AgentTool/forkSubagent.ts`).

### The protocol Coder drives

`cc:entrypoints/sdk/controlSchemas.ts` defines the control requests that
travel both ways over stream-json: `initialize`, `interrupt`,
`can_use_tool`, `set_permission_mode`, `set_model`, `hook_callback`,
`mcp_message`, `rewind_files`, `stop_task`, `elicitation` and others. These
are the frames Coder's client classifies and answers
([PARITY.md](../../coax/lib/agent/claude/PARITY.md), "Control protocol").

## What Coder takes

- **Per-input tool facts** ([chapter 4](ch004-tools-as-typed-capabilities.md)).
  Coder's tool schema gains `read_only` and `concurrency_safe` as functions
  of the decoded input, defaulting to false, and the loop batches safe calls.
  `validateInput` before `checkPermissions` becomes Coder's order: decode,
  validate, then ask the policy.
- **Deny, then ask, then allow** ([chapter 5](ch005-a-sandbox-and-approvals.md)).
  Coder's policy uses the same fixed order, independent of rule specificity,
  so a broad deny cannot be carved open by a narrow allow.
- **Rules know their limits.** As Claude Code's docs do, Coder states that a
  rule on a tool does not confine a process the tool starts; the OS sandbox
  does that. Coder's real-process sandbox (still to come in chapter 5)
  takes the same split: Seatbelt on macOS, bubblewrap on Linux, a proxy
  with an empty allowlist.
- **Descriptions first, bodies on use** ([chapter 3](ch003-keep-the-prefix-stable.md)).
  Skills and deferred tools keep the request prefix short and stable: a
  newly loaded body is appended, not inserted.
- **Repository instructions as data** ([chapter 6](ch006-a-document-tries-to-become-the-system-prompt.md)).
  For Microcoder, Coder takes the loading order: `CLAUDE.md` and
  `AGENTS.md` from the repository root down to the working directory, with
  every file read recorded in the log. This is not built yet. Chapter 6's rule still holds: such a file
  is instructions from the repository, never authority over the policy.
- **Driving, not copying.** The `claude` engine already answers
  `can_use_tool` through the person's y/a/n prompt with `--ask`, and runs
  with `bypassPermissions` otherwise (`coder/tui/coder.coax`). Chapter 10
  replaces that prompt with chapter 5's policy, so the same rules decide
  every engine.

## What Coder leaves, and why

- **Hooks as shell commands with the agent's authority.** A hook runs any
  program on matching events. Coder currently supports trusted local shell
  commands/hooks as well as bounded Wasm plugins with admitted imports
  ([chapter 15](ch015-plugins-with-owned-lifecycles.md)). Wasm containment
  does not apply to the shell form. Restricting third-party execution to
  Wasm is a design direction, not a statement about every installed plugin.
- **Eight rule sources.** Coder keeps three: the machine, the repository and
  the session. Each extra source is another place a person must look to know
  why a call ran.
- **A classifier as the approver.** The `auto` mode has a model decide
  approvals. Coder does not replace a person's answer with a model's. A
  System One decision may suggest an answer, and the policy and the person
  decide ([chapter 13](ch013-choose-an-engine.md) uses System One only to
  choose engines).
- **Fork-by-default subagents.** Coder keeps forks and scoped subagents
  separate, and adds either only with a measured gain
  ([chapter 14](ch014-forks-subagents-and-jobs.md)).
- **Plugin marketplaces.** Coder's third-party plugins are signed Nostr
  events, not installs from a marketplace repository.

## Where to measure a borrowed design

- Parallel batches of read-only calls: [chapter 21](ch021-measure-the-harness.md)
  compares wall time and outcomes with and without batching.
- Microcompact before full compaction: [chapter 7](ch007-condense-the-history.md)
  measures the prefill cost of each.
- Deferred tools and skills: [chapter 3](ch003-keep-the-prefix-stable.md)
  checks that loading a body appends and never rewrites earlier bytes.
- Driving Claude Code against Microcoder on the same task:
  [chapter 13](ch013-choose-an-engine.md)'s held-out set.

## Sources

- `AtlantisPleb/cc` at `abcc00908c056a6fb2bfaa4ba8933bb65daaf5f7` (private
  reference mirror, read only): `docs/REPO_AUDIT.md`, `query.ts`, `Tool.ts`,
  `services/tools/toolOrchestration.ts`, `services/compact/autoCompact.ts`,
  `services/compact/microCompact.ts`, `types/permissions.ts`,
  `utils/sandbox/sandbox-adapter.ts`, `entrypoints/sdk/coreTypes.ts`,
  `entrypoints/sdk/controlSchemas.ts`, `schemas/hooks.ts`,
  `utils/claudemd.ts`, `skills/loadSkillsDir.ts`, `tools/ToolSearchTool/`,
  `tools/AgentTool/`, `utils/plugins/`.
- Anthropic's Claude Code docs, read 2026-11-02:
  [permissions](https://code.claude.com/docs/en/permissions),
  [sandboxing](https://code.claude.com/docs/en/sandboxing),
  [hooks](https://code.claude.com/docs/en/hooks),
  [memory](https://code.claude.com/docs/en/memory),
  [skills](https://code.claude.com/docs/en/skills),
  [subagents](https://code.claude.com/docs/en/sub-agents),
  [plugins](https://code.claude.com/docs/en/plugins),
  [headless](https://code.claude.com/docs/en/headless),
  [settings](https://code.claude.com/docs/en/settings).
- Coder's client: [PARITY.md](../../coax/lib/agent/claude/PARITY.md); the TUI
  engine in `coder/tui/coder.coax`.
- [Chapter plan](PLAN.md), Part VII.
