# Claude Code integration

## Actual integration boundary

The built-in driver kind is `claudeAgent`. `ClaudeDriver.create` constructs the
V2 adapter, status/capability probes, model catalog, usage reader and a separate
text-generation helper. The interactive implementation and its
`ClaudeAgentSdkQueryRunner` live together in `ClaudeAdapterV2.ts`; the runner is
not a separate production file. The lockfile resolves
`@anthropic-ai/claude-agent-sdk` to **0.3.321**. [Driver][driver],
[adapter/runner][runner], [lockfile][lock].

```text
mobile / web / desktop renderer
  → T3 environment RPC command
  → V2 orchestration + provider instance/session
  → ClaudeAgentSdkQueryRunner
  → @anthropic-ai/claude-agent-sdk query({ prompt: AsyncIterable, options })
  → Claude Code child process on the environment
  → provider network and environment-local tools
```

This gives T3 Claude Code's agent loop, tools, settings and native sessions,
rather than merely sending messages to an Anthropic model API and rebuilding
the agent loop in the client. The source uses SDK objects/events and callbacks;
it does not parse the interactive terminal display. [Query runner][runner].

## Startup, prompts and process ownership

The runner creates a prompt queue and exposes it to `query()` as an asynchronous
iterable of `SDKUserMessage`. One query can therefore receive successive user
messages. T3 supplies a custom `spawnClaudeCodeProcess` implementation: piped
stdio, environment/cwd, process-group/descendant ownership and exit tracking.
Where supported it wraps native executables through `AgentScope`; script
executables follow the SDK/Node path. Port discovery attributes agent-launched
services to their T3 thread. [Runner startup][runner].

The adapter keeps a live query when compatible and opens a new one when its
native identity/policy/configuration requires replacement. It refuses a query
replacement that would kill still-owned background work. It reads settings
and appended instruction files before closing an existing process, so a bad
path does not needlessly destroy the working session. Native thread reuse uses
persisted history as well as in-memory state to choose resume, avoiding a
fresh-session collision after idle release. [Query reuse and opening][opening].

SDK boot and MCP registration have their own bounded startup handling. Failed
opening shuts down its prompt queue before retry so an old child cannot consume
a new prompt. Abort, interrupt, close and process exit are separate operations;
closing a UI stream is not the mechanism for terminating an agent tree.
[Runner][runner].

## Query configuration and instructions

`makeClaudeQueryOptions` translates T3 selection and policy into SDK options:

- Selected API model and effort/thinking settings, plus T3's conversation title.
- New native `sessionId` or `resume`, and optional `resumeSessionAt`.
- Claude Code preset tools and the `claude_code` system-prompt preset.
- T3 runtime/orchestration instructions, user append-system-prompt settings,
  and optional device-tool instructions.
- `includePartialMessages: true`, permission mode, tool restrictions,
  approval/question/elicitation callbacks and prompt suggestions.
- Configured executable/environment and effective settings, working directory
  and permitted attachment/additional directories.

Launch arguments are interpreted into supported SDK options/settings rather
than blindly assuming every CLI flag is a separate SDK feature. Model aliases,
auto-compaction and thinking display need explicit translation. T3 preserves
the provider's own configuration surface while adding app-owned context.
[Query option compiler][options].

## Login, account selection and configuration

Claude normally uses the login/configuration available to Claude Code **on the
environment machine**. The documented default is the normal Claude config
directory. A custom instance sets `CLAUDE_CONFIG_DIR`, not `HOME`, preserving
the system Keychain location. The user documentation says that mobile can add
an account/config-folder entry, but the actual subscription login still happens
on the environment using `claude auth login`. API-key/router variables are
instance configuration, not T3 cloud-account credentials. [Claude account guide][guide],
[home/environment code][home].

Instances with different config directories have separate native conversation
state. Existing threads can switch to compatible Claude instances sharing the
same config directory; this is not a promise of lossless native continuation
across unrelated accounts/homes. The driver derives its continuation key from
the resolved config home and keys capability probes by binary/home/cwd.
[Home identity][home], [driver][driver].

T3 removes inherited Claude host-session variables such as messaging socket,
session IDs and session token fields before launching its own Claude process.
It also removes `ELECTRON_RUN_AS_NODE` and sets the default entrypoint to
`t3-code`. This prevents an agent-launched T3 server from accidentally making
its child part of its parent's Claude session, and avoids contaminating apps
Claude launches with Electron's Node mode. Instance environment assignments are
then merged deliberately. [Environment merge][home].

This source does not establish that every provider credential is isolated from
every other process of the same OS user. In particular, preserving `HOME` and
Keychain behavior is intentional. Separate T3 instances are configuration and
account-routing boundaries, not independent OS security principals.

## Streaming and transcript construction

The adapter consumes SDK assistant/user messages, `stream_event` frames,
results and system/task events. It turns these into T3 conversation messages,
reasoning, tool nodes/results, plan/todo changes, runtime requests and lifecycle
updates. Partial output supports incremental text, while canonical messages and
results settle the transcript. Native tool-use IDs and parent-tool-use IDs
connect tool results and subagent work to the appropriate execution structure.
[Adapter][adapter].

The declared capabilities matter: Claude streams assistant text and reasoning,
but declares **no general streaming tool output and no streaming plan text**.
It does emit tool start/completion/output records. A tool's eventual result
existing in the transcript is different from a continuous live stdout stream.
Native thread IDs are strong; native turn IDs are weak/not exposed as a stable
provider turn ID. T3 supplies its own run/turn identities.
[Capabilities][capabilities].

T3 also handles synthetic command output, thinking summaries, attachment
representations, context usage and usage-limit/reset reporting. Claude's native
`/goal` state is inferred from synthetic command output and Stop-hook transcript
feedback because SDK mode does not emit the same goal events as Codex.
Those compatibility paths are precisely why the adapter is much larger than
the SDK startup call. [Adapter][adapter], [provider notes][providers].

## Human approval and questions

`canUseTool` is a real SDK callback. It checks for an active T3 turn, preserves
`toolUseID`, builds normalized request/node/item records and waits on a deferred
decision or answer. `AskUserQuestion` is recognized structurally and answered
through the SDK's expected input. `ExitPlanMode` has a separate plan-approval
path. The adapter also provides SDK user-dialog and MCP-elicitation handlers;
not every human interaction is a shell-command approval. Cancellation observes
the callback's signal and cleans pending requests. [Permission callback][permissions].

Runtime modes map to native modes with additional policy handling:

| T3 mode | Normal Claude mapping | Important qualification |
| --- | --- | --- |
| `approval-required` | `default` | Callback asks about operations Claude routes to it |
| `auto-accept-edits` | `acceptEdits` | Edits may be accepted before the callback; other requests still ask |
| `auto` | `auto` | Subject to native availability/configuration |
| `full-access` | `bypassPermissions` | Requires the explicit dangerous-skip option; unavailable bypass can downgrade |
| Plan interaction | `plan` | Callback remains installed for plan-mode writes/transition requests |

Explicit sandbox/approval policy can alter that table, including read-only
tool restrictions and `dontAsk`. Enterprise settings can disable bypass/auto
and change the effective mode. It would be incorrect to describe this as an
independent T3 sandbox equivalent to Codex's native sandbox policy.
[Runtime-policy mapping][policy].

Claude declares live-only callbacks, pending requests that do not survive
restart, no native apply-patch approval capability, and no subagent-originated
approval capability in its generic capability table. Displaying a persisted
request after process death cannot make its SDK callback answerable again.
[Capabilities][capabilities].

## T3's tools and MCP

T3 injects its orchestration/device/browser tool surface as MCP configuration
bound to the calling thread. The query runner deliberately removes MCP config
from the initial SDK options and registers it using `setMcpServers` after
initialization. The source explains the reason: keep authorization headers out
of SDK argv and inherited command environments. Logs of registration failure
contain server names rather than error payloads that may echo headers.
[Runner MCP registration][runner].

The adapter uses T3 runtime instructions plus available tool metadata; MCP apps
also have a distinct bridge for app-facing tool/resource calls. This is not
evidence that T3 replaces every provider/user MCP configuration, or that tool
descriptions enforce permissions. Managed MCP policies and per-tool server
authorization remain relevant. [Adapter][adapter], [Claude home handling][home],
[MCP security](06-security-and-reliability.md).

## Resume, fork, rollback and subagents

- **Resume:** reopen the native Claude session using `resume`, effective home,
  cwd/settings and any saved conversation-head reference.
- **Fork:** call the SDK's `forkSession`, optionally up to a message, and record
  the new native session/head. Custom-home history/fork work is isolated in a
  worker because the SDK reads `CLAUDE_CONFIG_DIR` from process environment.
- **Rollback:** stop the live query, resolve a native resume boundary and use
  `resumeSessionAt`; a thread-start reset may require a new native identity.
- **History:** the adapter does not advertise general native conversation
  snapshot reading even though it uses native transcript/history helpers for
  particular recovery/fork/task operations.
- **Subagents:** track native task/tool relationships and background completion,
  while the generic capability table does not promise standalone native
  subagent-thread IDs, waiting/closing/forking of those threads. Specialized
  task tracking/stop code should not be mistaken for full Codex-style child-thread
  parity.

Sources: [capabilities][capabilities], [history worker][history],
[rollback/fork implementation][fork], [background query guard][opening].

The implementation supports active steering, but declares that it interrupts
tools and does not provide the generic interrupt/restart steering capability.
Provider behavior must be represented as it is, instead of applying Codex's
steering semantics to the Claude UI. [Capabilities][capabilities].

## What this integration depends on

The host must have a usable Claude executable/account configuration, SDK/CLI
compatibility, filesystem/tool permissions and network access to the configured
provider. Native hooks, plugins, skills and MCP may execute on startup or before
a prompt. A status probe or short-lived title generator is therefore not
automatically harmless just because no chat message was sent. T3's provider
guidance explicitly treats these lifecycle effects as part of the boundary.
[Provider guidance][providers], [Claude guide][guide].

[driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/Drivers/ClaudeDriver.ts
[adapter]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts
[runner]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L1325
[opening]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L12090
[options]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L1970
[capabilities]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L489
[home]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/Drivers/ClaudeHome.ts
[guide]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/user/providers-claude.md
[permissions]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L11486
[policy]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L3056
[fork]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts#L13539
[history]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/claudeHistoryWorker.ts
[providers]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/providers.md
[lock]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/pnpm-lock.yaml#L2750
