# Pi Coding Agent Teardown — 2026-07-21

Read-only architecture, product, and integration audit of the public
`earendil-works/pi` source tree at an exact commit. The audit also reads two
integration codebases: the Vercel AI SDK `@ai-sdk/harness-pi` adapter, which is
the only host-process harness adapter in that family, and the OpenAgents
`AgentHarness` contract in the extracted Effect-native SDK. The purpose is a
concrete scope for a future OpenAgents `AgentHarness` adapter for Pi. The
adapter itself is a later packet. Nothing tracked in any repository was
modified.

## Summary

Pi is a self-extensible terminal coding agent from the Earendil team, led by
the author of libGDX. It is three things at once: a polished interactive CLI, a
machine-drivable RPC process, and — most important for OpenAgents — an
**in-process Node library**. `createAgentSession` from
`@earendil-works/pi-coding-agent` builds a full agent session inside the
caller's process, with injectable session storage, settings, auth, model
registry, resource loader, and custom tools. No bridge, no subprocess, no
socket. [source]

The runtime splits into four published packages: `pi-ai` (a unified
multi-provider LLM streaming API with more than forty providers, OAuth token
planes included), `pi-agent-core` (a small provider-agnostic agent loop with a
nine-kind event union), `pi-coding-agent` (sessions, tools, persistence,
compaction, extensions, skills, modes), and `pi-tui` (terminal rendering).
[source]

Pi has **no permission system and no sandbox**. It runs with the full authority
of the user and process that launched it, and the README says to containerize
externally when boundaries matter. That host-process posture is the central
integration question for OpenAgents. The AI SDK adapter is the best evidence of
both sides: in-process embedding worked cleanly, but making Pi *appear* to live
inside a remote sandbox required a global `node:fs` monkey-patch VFS, a
mirror-scoped workspace copy, and replacement of every built-in tool with
sandbox-executing custom tools. [source] [adapter]

The central OpenAgents decision: **build the Pi adapter, and build it
owner-local first.** On the owner-local lane (Desktop, Pylon) Pi's posture is
not a defect — the workspace is genuinely local, so the entire mirror/VFS
apparatus disappears and the adapter reduces to event translation, tool
bridging, and session-file lifecycle. `promptTurn`, `interrupt`, native
`compact`, and mid-turn `steer` map losslessly. `suspendTurn`/`continueTurn`
land on the degraded `rerun` form that the OpenAgents contract explicitly
blesses for host-resident runtimes. Reject Pi as a sandboxed-labor runtime, and
reject any port of the global-fs VFS into OpenAgents code.

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact           | Identity                                                                                     | What it establishes                     |
| ------------------ | -------------------------------------------------------------------------------------------- | --------------------------------------- |
| Public repository  | `https://github.com/earendil-works/pi`                                                       | Public source and history               |
| Audited commit     | `dd6bea41efa8caa7a10fe5a6401676dc5699f83f` on `main`                                         | Exact snapshot used here                |
| Commit time        | `2026-07-21T18:40:11+02:00`                                                                  | Freshness of the audited tip            |
| Product version    | `0.81.1` (`@earendil-works/pi-coding-agent` on npm)                                          | Active pre-1.0 release train            |
| License            | MIT (copyright 2025 Mario Zechner)                                                           | Permissive reuse boundary               |
| Source scale       | ~211,000 lines across 879 TypeScript files (tests included)                                  | Mid-size, single-language monorepo      |
| History            | 5,046 commits from 2025-08-09 to the audited tip                                             | Under one year old, very high velocity  |
| Maintainers        | Mario Zechner ~3,469 commits, Armin Ronacher ~414, then a long community tail                | Two dominant maintainers                |
| Dependency posture | Exact-pinned direct deps, shrinkwrap for npm users, `--ignore-scripts` installs, 2-day `min-release-age` | Deliberate supply-chain hardening |
| AI SDK adapter     | `vercel/ai` local clone at `6b6a8bbe9247`, `packages/harness-pi` last touched `7120171806f3` (2026-07-20) | Host-process integration evidence |
| OpenAgents contract | `OpenAgentsInc/ai` local checkout at `a93f2e2751ec`, `packages/agent-harness-contract`      | The adapter target                      |

Local audited paths: `~/work/projects/repos/pi` (Pi),
`~/work/projects/repos/ai/packages/harness-pi/src` (AI SDK adapter), and
`~/work/ai/packages/agent-harness-contract/src` (OpenAgents contract).

### 1.2 Evidence labels

- **`[source]`** — tracked Pi source, docs, or manifests at the audited commit.
- **`[adapter]`** — the AI SDK `harness-pi` adapter source at its pinned clone.
- **`[contract]`** — the OpenAgents `agent-harness-contract` source.
- **`[history]`** — Git history at or before the audited commits.
- **`[public]`** — corroborated by a linked public source.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

This audit did not execute Pi, did not run its test suite, and did not run the
AI SDK adapter. All runtime-behavior claims trace to source reading and to the
adapter's own comments, which encode operational experience. [limitation]

## 2. What Pi is

### 2.1 The product

`pi` is an interactive terminal coding agent with differential TUI rendering,
model cycling, extended-thinking levels, session branching, HTML session
export, session sharing, and a package manager for extensions, skills, prompt
templates, and themes. The README leads with "self extensible": the agent can
explain and modify its own configuration surface. [source]

Pi ships as npm packages and as standalone Bun-compiled binaries with
SHA-256-covered release archives. The repository auto-closes issues and PRs
from new contributors by default, with daily maintainer review — an unusual
but explicit contribution posture. [source]

Permissions are explicitly out of scope. The README: "Pi does not include a
built-in permission system for restricting filesystem, process, network, or
credential access." Containerization guidance covers a Gondolin micro-VM
extension, plain Docker, and OpenShell. [source]

### 2.2 The library

The same package exports a real SDK. `createAgentSession(options)` accepts a
working directory, an agent config directory, a model plus thinking level, a
tool allowlist/denylist, custom tools, and injectable managers for sessions,
settings, resources, and model/auth runtime. It returns an `AgentSession` plus
the loaded-extensions result. Every default is overridable, which is exactly
what a foreign harness needs. [source]

The layer below, `@earendil-works/pi-agent-core`, is deliberately
provider-agnostic: an agent loop over a `StreamFn` with a documented error
contract (failures are encoded in the stream, never thrown). The coding-agent
package injects `pi-ai`'s `streamSimple` as the default stream function.
[source]

`@earendil-works/pi-ai` is a unified streaming API over roughly forty-plus
providers: OpenAI, Anthropic, Google, Bedrock, Mistral, xAI, Groq, OpenRouter,
Copilot, Codex-plan OAuth, regional token plans, and more. Auth supports both
API keys and OAuth flows, with a credential store under the agent directory.
[source]

There is also an experimental `@earendil-works/pi-server` package (explicitly
unstable) and a SQLite storage package. Neither is load-bearing for the
adapter scope. [source]

## 3. Architecture walk

### 3.1 Session lifecycle

`createAgentSession` resolves the working directory, the agent directory
(default `~/.pi/agent`, env-overridable via `PI_CODING_AGENT_DIR`), the model
runtime from `auth.json` plus `models.json`, the settings manager, and the
session manager. If the session manager holds prior entries, the model and
thinking level restore from the persisted session, with a typed fallback
message when the saved model is no longer available. [source]

`AgentSession` is the shared core across all three run modes (interactive,
print, RPC). Its main verbs [source]:

- `prompt(text, options)` — run a turn. Options carry images, prompt-template
  expansion, and the streaming-queue behavior.
- `steer(text)` / `followUp(text)` — inject input into a **running** turn.
  Steer interrupts at the next drain point, follow-up waits for turn end. Both
  have `all` and `one-at-a-time` queue modes.
- `abort()` — interrupt the in-flight turn.
- `compact(customInstructions)` — manual compaction. Automatic compaction
  triggers on threshold and on context overflow.
- `bash(command)` — session-scoped shell execution recorded into the
  transcript.
- `subscribe(listener)` — the event feed (next section), returns an
  unsubscribe function.
- `dispose()` — detach and tear down without deleting persisted state.
- State accessors: `messages`, `isStreaming`, `isIdle`, `sessionId`,
  `sessionFile`, `getSessionStats()`, tool listing and activation, scoped
  models, thinking levels.

The RPC mode wraps the same session in a JSONL stdin/stdout command union:
`prompt`, `steer`, `follow_up`, `abort`, `new_session`, `get_state`,
`set_model`, `cycle_model`, `set_thinking_level`, `compact`, `bash`,
`switch_session`, `fork`, `clone`, `get_tree`, `get_messages`, and more. This
is a second, process-isolated integration surface that an adapter could use
instead of in-process embedding. [source]

### 3.2 The event stream

`pi-agent-core` emits a nine-kind `AgentEvent` union [source]:

```text
agent_start | agent_end(messages)
turn_start  | turn_end(message, toolResults)
message_start(message)
message_update(message, assistantMessageEvent)
message_end(message)
tool_execution_start(toolCallId, toolName, args)
tool_execution_update(toolCallId, toolName, args, partialResult)
tool_execution_end(toolCallId, toolName, result, isError)
```

Streaming granularity rides inside `message_update`: the nested
`assistantMessageEvent` from `pi-ai` carries `text_start`/`text_delta`/
`text_end`, `thinking_start`/`thinking_delta`/`thinking_end`,
`toolcall_start`/`toolcall_delta`/`toolcall_end`, and terminal `done`/`error`
with a stop reason. [source]

`AgentSession` widens this to `AgentSessionEvent` with session-level kinds:
`agent_settled`, `queue_update` (steering and follow-up queues),
`compaction_start`/`compaction_end` (reason: `manual`, `threshold`,
`overflow`), `entry_appended`, `session_info_changed`,
`thinking_level_changed`, `auto_retry_start`/`auto_retry_end`, and
summarization-retry kinds. `agent_end` gains a `willRetry` flag. Settlement is
carefully defined: awaited subscribers for `agent_end` are part of run
settlement, and the agent is idle only after they finish. [source]

### 3.3 Session persistence

Sessions persist as JSONL files (version 3 header) under the sessions
directory, default `<agentDir>/sessions`. Each entry has `id`, `parentId`, and
a timestamp, so the file is a **tree**, not a list: forks, clones, and branch
summaries are first-class, and the RPC surface exposes `fork`, `clone`, and
`get_tree`. Entry kinds include messages, thinking-level changes, model
changes, compaction summaries, branch summaries, bash executions, and custom
messages. `SessionManager.open(file)` restores a session, and
`SessionManager.inMemory()` disables persistence. [source]

This file is the whole resume story. There is no daemon holding live state:
whoever owns the JSONL file can reconstruct the transcript and continue.
[source] [inferred]

### 3.4 Tools

Built-in tools are `read`, `bash`, `edit`, `write` (the default active set)
plus read-only `grep`, `find`, and `ls`. Each is a factory (`createReadTool`
and friends) parameterized by working directory, and file mutations serialize
through a queue (`withFileMutationQueue`). Tool parameters are TypeBox
schemas. [source]

Custom tools are `ToolDefinition` objects: name, label, description, TypeBox
`parameters`, optional system-prompt snippet and guideline bullets, optional
per-tool sequential/parallel execution override, and an async `execute` that
can stream partial results. `createAgentSession` accepts them directly via
`customTools`, and the `tools` allowlist plus `excludeTools` denylist control
activation. The agent loop supports sequential or parallel tool execution and
a `beforeToolCall` hook that can block a call with a typed reason — the
primitive an approval flow builds on. [source]

### 3.5 Config, auth, and discovery

Global state lives under `~/.pi/agent/` [source]:

| Path                  | Role                                                        |
| --------------------- | ----------------------------------------------------------- |
| `auth.json`           | Per-provider credentials, `api_key` or `oauth` typed        |
| `models.json`         | Model registry additions and overrides                      |
| `settings.json`       | Defaults: provider, model, thinking level, behavior toggles |
| `sessions/`           | JSONL session trees                                         |
| `prompts/`            | Prompt templates                                            |
| `themes/`             | Custom TUI themes                                           |
| `tools/`, `bin/`      | User tools and managed binaries (`fd`, `rg`)                |

Project-local discovery walks `cwd/.pi/` and `cwd/.agents/` for extensions and
skills, and loads root-level context files with the candidate order
`AGENTS.md`, `AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`. The `.agents/skills`
convention follows the public agent-skills format, and skills format into an
`<available_skills>` system-prompt section. [source]

Auth is a real runtime object, not just a file: `AuthStorage` supports
`setRuntimeApiKey` for injected credentials, `ModelRegistry.registerProvider`
takes base URL, auth-header mode, and custom headers, and OAuth is a
first-class credential type with provider-specific flows (Anthropic, Codex
plan, Copilot). [source]

### 3.6 Extensions

Extensions are TypeScript modules loaded in-process through `jiti` from
project-local `.pi/extensions/`, the global agent directory, and configured
paths. The `ExtensionAPI` registers tools, slash commands, event handlers, and
UI contributions against a session event bus, with a project-trust gate before
project-local code loads. This is a powerful capability surface and an
arbitrary-code-execution surface in the host process — the reason the trust
gate exists. [source]

## 4. What is genuinely good

1. **Dependency injection all the way down.** `createAgentSession` accepts
   replacement `SessionManager`, `SettingsManager`, `ResourceLoader`,
   `ModelRuntime`, `AuthStorage`, and `ModelRegistry` instances. The AI SDK
   adapter builds a fully isolated Pi (own agent dir, own auth, own session
   store, own resource loader) without touching the user's `~/.pi`. Very few
   coding agents allow this — it is the single property that makes a
   no-bridge adapter possible. [source] [adapter]
2. **A small, honest event union with settlement semantics.** Nine core kinds,
   explicit turn boundaries, an explicit `agent_settled`, and a documented
   rule that `agent_end` subscribers are part of settlement. The stream-fn
   contract ("failures are encoded in the stream, never thrown") is the same
   discipline the OpenAgents runtime event contract enforces. [source]
3. **Steer and follow-up as first-class verbs.** Mid-turn user injection with
   typed queue modes and `queue_update` events is exactly the
   `submitUserMessage` capability most harnesses lack. Pi's version is better
   specified than most: drain points, one-at-a-time mode, and queue
   projection. [source]
4. **The session tree.** `parentId`-linked JSONL entries give forks, clones,
   branch summaries, and `get_tree` for free, and the file is the complete
   resume artifact. Compaction and branch summaries persist as typed entries
   rather than lossy rewrites. [source]
5. **Runtime-owned compaction with typed reasons.** Manual, threshold, and
   overflow compaction emit `compaction_start`/`compaction_end` with results,
   abort flags, and retry state. The adapter just triggers and observes —
   the OpenAgents `compact` verb maps directly. [source]
6. **Provider breadth with OAuth token planes.** One streaming API across
   forty-plus providers including subscription OAuth planes (Codex, Copilot,
   Anthropic) matters for an owner-local lane where owners bring their own
   accounts. [source]
7. **Supply-chain discipline.** Exact-pinned deps, generated shrinkwrap,
   lockfile-change gates, `--ignore-scripts` everywhere, a lifecycle-script
   allowlist, scheduled audits, and reproducible binary builds from release
   archives. This is well above the norm for young agent repos. [source]
8. **RPC mode as a fallback integration.** The JSONL command union covers
   prompting, steering, aborts, models, compaction, forks, and state reads.
   If in-process embedding ever becomes untenable (version skew, Effect
   runtime conflicts), a process-isolated adapter lane already exists
   upstream. [source]

## 5. Weaknesses and frictions for OpenAgents

1. **Host-process authority posture.** No permission system, no sandbox, full
   user authority, in-process arbitrary-code extensions. For OpenAgents this
   is acceptable only on the owner-local lane under the same owner-executor
   invariant as local Codex work, and never for untrusted or marketplace
   labor. [source]
2. **The sandbox impedance mismatch is real and expensive.** The AI SDK
   adapter had to (a) mirror `.pi`/`.agents`/`AGENTS.md` from the sandbox to
   the host with symlink-resolving scoped copies, because a full workspace
   mirror "makes session startup take hours" on `node_modules`-sized trees,
   (b) monkey-patch global `node:fs` with a longest-prefix VFS so Pi's
   resource loading resolves a sandbox path to a host mirror, and (c) replace
   every built-in tool with sandbox-executing custom tools. The mirror-scoping
   comment is the key artifact: Pi only reads its own resource configuration
   from the host, and everything else must run as tools against the sandbox.
   [adapter]
3. **Event vocabulary distance.** Pi's deltas hide inside
   `message_update.assistantMessageEvent`, there are no stable block ids, and
   step boundaries do not exist as events. The AI SDK translator keeps
   per-turn mutable state to synthesize text/reasoning block ids, track
   pending tool calls, mark native tools `providerExecuted`, and rename
   `find` to `glob`. An OpenAgents projection to `KhalaRuntimeEvent` needs the
   same stateful translator plus sequence numbering. [adapter] [contract]
4. **Resume is transcript-faithful, not turn-faithful.** The JSONL file
   restores history exactly, but a live turn cannot be frozen: suspend must
   abort the in-flight turn and rerun from the journal. The AI SDK encodes
   this as "Pi cannot freeze a live turn the way a bridge adapter can", parks
   live sessions in a process-local map for same-process detach, and treats
   the session file as the cross-process fallback. Any work in flight at a
   slice boundary is recomputed. [adapter]
5. **Process-global mutable state.** `setDefaultStreamFn` is module-global,
   the default agent dir comes from `process.cwd()`/`homedir()` helpers with
   an env-var override, and the AI SDK's VFS is process-global by necessity.
   Multiple concurrent Pi sessions in one process work, but only with
   discipline (distinct mount points, injected managers). [source] [adapter]
6. **Tool-set changes force a session rebuild.** Custom tools bind at
   `createAgentSession` time. The AI SDK rebuilds the session whenever the
   host tool signature changes and ports a verbatim 25 ms teardown sleep with
   a TODO about a teardown microtask race — evidence of a real lifecycle
   sharp edge. [adapter]
7. **No wire schema.** Events and messages are TypeScript types, not runtime
   schemas. The AI SDK defends with loose zod parsing that ignores unknown
   kinds. An OpenAgents adapter must do the same with Effect Schema, and
   version drift is a live risk at Pi's commit velocity (`.json` to `.jsonl`
   session drift is already recorded in the adapter). [source] [adapter]
8. **TypeBox, not Effect Schema.** Host-tool parameters must convert from the
   OpenAgents contract's schema world into TypeBox. The AI SDK's
   `pi-typebox-adapter` is direct prior art for the conversion. [adapter]
9. **Governance concentration.** Two maintainers dominate 5,046 commits in
   under a year, releases are frequent, and new-contributor issues auto-close.
   Excellent velocity, weak external influence. Pin exact versions and keep
   the adapter behind the OpenAgents conformance suite. [history] [source]

## 6. The adapter scope: Pi on the OpenAgents `AgentHarness`

The OpenAgents contract (`packages/agent-harness-contract`) defines an adapter
as a tagged spec with one entry method `start`, returning a `HarnessSession`
with verbs `promptTurn`, `continueTurn`, `suspendTurn`, `compact`, `detach`,
`stop`, `destroy`. Capability is signalled by method presence, refusals fail
closed with `HarnessCapabilityUnsupported`, and the stream event **is**
`KhalaRuntimeEvent` with a per-turn `sequence` cursor. The reference, ACP, and
opencode adapters are the existing patterns, and the sandbox-provider seam is
explicit. [contract]

Pi would be the first **host-process** adapter in the OpenAgents family, the
same position `harness-pi` occupies in the AI SDK family. [adapter] [contract]

### 6.1 Session verbs feasibility

| Contract verb / capability | Pi mechanism | Fidelity |
| -------------------------- | ------------ | -------- |
| `start` (fresh)            | `createAgentSession` with injected managers | Lossless |
| `start({ resumeFrom })`    | `SessionManager.open(sessionFile)` | Lossless for transcript state |
| `promptTurn`               | `session.prompt(text)` plus `subscribe` translation | Lossless |
| `PromptControl.interrupt`  | `session.abort()` | Lossless |
| `PromptControl.submitUserMessage` | `session.steer(text)` (or `followUp`) | Lossless, better than most runtimes |
| `PromptControl.submitToolResult`  | Resolve the pending custom-tool promise | Lossless |
| `PromptControl.submitToolApproval` | Adapter-emulated: approval gate inside the built-in tool wrapper, or `beforeToolCall` block | Supported (emulated) |
| `compact`                  | `session.compact(instructions)`, observe `compaction_end` | Lossless (native) |
| `suspendTurn`              | `session.abort()` + persist JSONL + continuation state | **Degraded `rerun`** (contract-blessed) |
| `continueTurn`             | Same-process: reattach parked live turn. Cross-process: re-drive with an empty prompt from the restored journal | Live attach lossless, cold path degraded `rerun` |
| `detach`                   | Same-process: park the live session in a map. Cross-process: session file | Degraded (process-scoped park) |
| `stop`                     | Persist session file, `dispose()` | Lossless |
| `destroy`                  | `dispose()` + delete adapter-owned state | Lossless |
| Builtin tool filtering     | `tools` allowlist + `excludeTools` | Native |
| Builtin tool approvals     | Emulated via wrapped built-ins | Supported (emulated) |
| Bootstrap                  | Not needed on the owner-local lane (library import) | Omit `getBootstrap` initially |

What Pi persists is the JSONL session tree: full messages, tool results,
compaction and branch summaries, model and thinking-level changes. A resume is
therefore exactly as faithful as the last persisted entry. The unfaithful part
is only the in-flight turn: partial assistant output and running tools die
with the process and are recomputed. The contract's `lossy`/rerun continuation
form describes this precisely, so the adapter should report honest degraded
fidelity rather than simulate a freeze. [source] [adapter] [contract]

### 6.2 Pi event to `KhalaRuntimeEvent` projection sketch

Stateful translation, one translator per turn, monotonic `sequence` assigned
at emission (the durable cursor is adapter-owned, as in the reference
adapter). Real Pi kinds on the left:

| Pi event | KhalaRuntimeEvent projection |
| -------- | ---------------------------- |
| `agent_start`                                | `turn.started` |
| `turn_start` / assistant `message_start`     | `step.started` |
| `message_update` + `text_delta`              | `text.delta` (synthesized block id) |
| `message_update` + `text_end`                | `text.completed` |
| `message_update` + `thinking_delta`          | `reasoning.delta` |
| `message_update` + `thinking_end`            | `reasoning.completed` |
| `message_update` + `toolcall_delta`          | `tool.input.delta` |
| `tool_execution_start`                       | `tool.call` (`providerExecuted: true` for the native seven, host dispatch otherwise) |
| `tool_execution_update`                      | `provider.metadata` progress ref (no lossy projection into results) |
| `tool_execution_end` (`isError` false/true)  | `tool.result` / `tool.error` |
| `turn_end` / `message_end`                   | `step.finished` |
| `agent_end` + `getSessionStats()`            | `usage.recorded` then `turn.finished` |
| `agent_end` after `abort()`                  | `turn.interrupted` |
| `compaction_start` / `compaction_end`        | `compaction.recorded` |
| `auto_retry_start` / `auto_retry_end`        | `turn.retry` projection refs |
| `queue_update`                               | `turn.queue` projection refs |
| `edit`/`write` tool completions              | Adapter-synthesized `file.change` (Pi has no native file-change event) |
| Unknown kinds                                | Ignore after loose-decode, never fail the stream |

Native tool names normalize the same way the AI SDK does (`find` maps to the
common `glob`). Usage lands once per turn from session stats, since Pi does
not stream usage per step. [source] [adapter] [contract]

### 6.3 Tool bridging

Host tools bridge through Pi's `customTools`: each OpenAgents
`HarnessHostToolSpec` becomes a `ToolDefinition` whose `execute` emits
`tool.call`, parks a deferred, and resolves when the host calls
`submitToolResult`. Parameters convert from the contract's schema world to
TypeBox (JSON-Schema-shaped conversion, `pi-typebox-adapter` is direct prior
art). The serialized text goes to the model while the exact submitted value is
retained for the `tool.result` projection, copying the AI SDK's
`hostToolResults` trick. Tool-signature changes trigger a session rebuild with
the same session file, cached by sorted-name signature. [adapter] [contract]

### 6.4 Sandbox strategy

Recommended: **full owner-local, not mirror-scoped.** The OpenAgents P0 lanes
(Desktop, Pylon) run on the owner's machine against a real local worktree.
There, Pi's `cwd` is the actual workspace, the native seven tools execute
directly, resource discovery (`.pi`, `.agents`, `AGENTS.md`) reads the real
tree, and the entire AI SDK mirror/VFS apparatus is unnecessary. The
`local-process-sandbox-provider` in the SDK is the natural pairing, and the
owner-local full-authority posture matches the existing owner-executor
invariant for local Codex assignments. Isolate the agent directory per
configured account (never default to the user's live `~/.pi/agent`, mirror
the `pylon auth` isolation rule), and inject auth through `AuthStorage`
runtime keys rather than files where possible. [contract] [adapter]
[inferred]

If a remote-sandbox Pi lane is ever wanted, adopt the AI SDK's shape —
scoped config mirror plus sandbox-executing custom tools — but implement the
resource redirection with an injected `ResourceLoader` and per-session
directories instead of a global `fs` monkey-patch wherever Pi's injection
points allow it, and treat any remaining global patch as a disqualifying
finding for that lane. [adapter] [inferred]

### 6.5 Proposed packet list

| Packet | Repo | Scope |
| ------ | ---- | ----- |
| PI-01  | openagents | This teardown (done) plus FASTFOLLOW source registration |
| PI-02  | OpenAgentsInc/ai | `@openagentsinc/harness-pi`: `AgentHarness` spec, `start`/`promptTurn`, event translator to `KhalaRuntimeEvent`, TypeBox host-tool bridge, isolated agent dir, exact-pinned Pi dependency, conformance parity with the reference adapter |
| PI-03  | OpenAgentsInc/ai | Lifecycle state: session-file resume schema, `stop`/`detach` park map, `suspendTurn`/`continueTurn` degraded-rerun conformance, interrupt semantics |
| PI-04  | OpenAgentsInc/ai | Approval emulation for the native seven, builtin filtering, `compact` passthrough, account-capacity failure classes (`account_exhausted`, `account_rate_limited`) from provider errors |
| PI-05  | openagents | Desktop/Pylon lane enablement: Pi as a selectable harness with receipts, per-account agent-dir provisioning, exact token rows, and owner-local invariant wiring |

Per the SDK repo's conventions, PI-02 through PI-04 land as issues there, and
PI-05 lands as a monorepo issue after the SDK packets are green.

### 6.6 Recommendation

**Build it.** Feasibility is high: Pi is the cheapest host-process adapter
available because the library boundary already exists and every global default
is injectable. The AI SDK adapter (~6,100 lines with tests) is a complete
crib sheet, and the OpenAgents version on the owner-local lane should be
substantially smaller because the mirror, VFS, and remote-ops layers drop
out. Order: PI-02, then PI-03, then PI-05, with PI-04 folded into PI-02/03
where cheap. Do not build a remote-sandbox Pi lane now.

## 7. Adapt versus reject

Adapt (with OpenAgents boundaries):

- In-process `createAgentSession` embedding with fully injected managers and
  an isolated per-account agent directory. [source]
- The stateful event translator pattern, loose decoding of unknown kinds, and
  `providerExecuted` marking, ported from the AI SDK translator onto
  `KhalaRuntimeEvent` with adapter-owned sequence numbers. [adapter]
- Session-file-based resume with honest degraded `rerun` continuation and a
  process-local park map for live detach. [adapter] [contract]
- `steer`/`followUp` as the `submitUserMessage` implementation, surfaced in
  product UX as queue-aware steering. [source]
- Native compaction passthrough with typed reasons. [source]
- The TypeBox conversion layer for host tools. [adapter]
- Pi's supply-chain checklist (exact pins, shrinkwrap, script allowlists) as
  a reference for OpenAgents' own published packages. [source]

Reject:

- Pi as a sandboxed or marketplace labor runtime, and any un-isolated use of
  the owner's live `~/.pi/agent` directory. [source] [inferred]
- Porting the global `node:fs` monkey-patch VFS into OpenAgents code.
  [adapter]
- Pi's extension loader as an OpenAgents extension surface: in-process
  arbitrary TS conflicts with the signed, capability-declared plugin
  direction. Adapter sessions should load no project-local Pi extensions by
  default. [source]
- Treating Pi's TUI, themes, or package manager as product surfaces to
  expose. The adapter consumes the SDK only. [inferred]
- Unpinned tracking of Pi releases. Every bump is a reviewed change gated by
  the conformance suite. [history]

## 8. Watch items

1. **`pi-server` maturation.** An official stable server surface would offer
   a process-isolated adapter lane with upstream ownership of the wire
   contract, replacing in-process embedding concerns. [source]
2. **Session format drift.** Version 3 today, `.json` to `.jsonl` drift
   already bitten once in the AI SDK. Pin, and test resume across Pi bumps.
   [adapter]
3. **The teardown-race TODO.** The AI SDK's verbatim 25 ms sleep between
   `dispose()` and re-create marks an upstream lifecycle race worth
   re-verifying per release. [adapter]
4. **Approval hooks upstream.** `beforeToolCall` blocking exists in
   agent-core. If Pi grows first-class interactive approval events, the
   emulated approval wrapper should migrate to them. [source]
5. **Event-schema publication.** Any upstream move toward a versioned runtime
   schema for `AgentSessionEvent` would shrink the loose-decode risk.
   [inferred]
6. **License and governance.** MIT today with concentrated maintainership and
   auto-close contribution policy. Watch for relicensing or a hosted-product
   pivot that changes the embedding calculus. [history] [limitation]
