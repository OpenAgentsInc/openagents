# Other providers and the extension boundary

## Registered integrations

The pinned `BUILT_IN_DRIVERS` list contains eleven entries. “Built in” means
registered in the server, not installed, authenticated, enabled or equally
capable on every environment. A model name and an agent driver are also
different things: one agent can offer models from several upstream providers.
[Registry][registry], [driver contract][driver].

| Driver | Server-side connection | Ownership and noteworthy differences |
| --- | --- | --- |
| Codex | `codex app-server`, bidirectional JSONL on stdio | Shared instance runtime with multiple native threads; user-managed and managed-auth paths; [dedicated audit](04-codex.md) |
| Claude | Claude Agent SDK `query()`, managed Claude Code process | One provider thread per session; native home/login and SDK callback semantics; [dedicated audit](03-claude-code.md) |
| Cursor | `@cursor/sdk` local Agent API | `Agent.create`/`Agent.resume`, local workspace/store, `agent.send`, incremental updates and cancellation/steering |
| Grok | ACP subprocess, normally `grok agent stdio` | Native permission flags vary by runtime mode; API-key or cached-token auth method |
| fx | ACP subprocess, `fx acp` | Ask/automatic-review mode mapping; supported runtime modes intentionally narrower |
| OpenCode | Local managed or configured external HTTP server plus event subscription | Version-specific adapters; 1.x per-thread managed servers versus 2.x per-instance managed server |
| Antigravity | ACP runtime with provider-specific adapter/support | Instance-scoped account profiles, managed installation, special file/turn/background-work compatibility |
| Kiro | ACP subprocess, `kiro-cli acp --agent-engine=v3 --auth-method=cli` | Uses CLI auth and provider-specific permission/cancellation behavior |
| Pi | `pi --mode rpc`, JSON command/event stream | Pi owns native sessions/extensions/trust; T3 injects an MCP/permission bridge |
| Muse | `@muse-code/sdk`, spawned `muse serve` MSP connection | SDK handshake/capabilities/schema envelope and process shutdown handling |
| ACP Registry | Registry-selected or local agent distribution, ACP | Generic discovery/config/auth; binary, npx or uvx distribution resolution; capabilities differ by agent |

Sources: [Cursor runner][cursor], [Grok support][grok], [fx support][fx],
[OpenCode runtime][opencode-runtime], [provider ownership notes][providers],
[Antigravity adapter][antigravity], [Kiro support][kiro], [Pi launch][pi-launch],
[Muse SDK host][muse], [registry distribution support][acp-registry].

## Cursor: local Agent SDK, not the editor's screen

The runner loads Cursor's SDK, creates or resumes an agent, and invokes
`agent.send` with `onDelta`; it can invoke native summarize, wait/cancel a run
and steer. Workspace-local store handling matters when resuming from a
different cwd. The adapter converts interactions/tool updates into T3's V2
events. Its use of a local SDK is distinct from scraping Cursor editor UI or
assuming a generic ACP endpoint. [Runner][cursor], [adapter][cursor-adapter].

The server also compensates for SDK lifecycle/network details, including reading
the CLI HTTP/1 configuration the SDK does not itself read and narrowly handling
known shell/closed-stream failures that would otherwise become unhandled server
rejections. The lockfile pins **1.2.0 with a patch**. Porting only the public
`Agent.create` call would miss those operational dependencies; the exact patch
and wrapper behavior must be considered together. [SDK integration][cursor-sdk],
[lockfile][lock].

## ACP: one protocol, several policy implementations

T3 has a reusable `effect-acp` client/protocol/schema package and shared
`provider-acp` session runtime/adapter. The session runtime spawns the configured
agent, initializes it (the request advertises protocol version 2), negotiates
capabilities/authentication and activates a native session. New/load/resume/fork
paths carry workspace roots and MCP configuration. Prompt, session updates,
permission requests and cancellation are translated to the common T3 contract.
[ACP runtime][acp-runtime], [ACP adapter][acp-adapter], [Effect client][effect-acp].

Capability negotiation determines which MCP transport/configuration the agent
can accept. The runtime prefers compatible ACP or HTTP MCP forms where
advertised and can use a stdio bridge otherwise. Client file/terminal services
are opt-in; the default capability setup does not advertise arbitrary read/write
support. Some drivers add carefully scoped file behavior. It would be incorrect
to advertise every host effect solely because ACP has a method for it.
[Runtime][acp-runtime], [T3 MCP stdio bridge][mcp-bridge],
[Antigravity support][antigravity-support].

Provider differences remain outside the common wire machinery:

- **Grok:** supervised startup overrides inherited permissive config; `auto`
  and full-access have different CLI flags. T3 does not advertise the
  unsupported auto-accept-edits mode. `XAI_API_KEY` selects API-key auth;
  otherwise the cached-token method is selected. Its status path initializes
  without calling authenticate or opening a session. [Grok support][grok],
  [status][grok-status].
- **fx:** supports supervised/auto, selects native ask versus automatic review,
  and accounts for changing native mode IDs (`code` versus `auto`). Explicit
  approval/sandbox policy must not accidentally launch automatic review. It
  waits for a cancelled prompt's native outcome where required. [fx support][fx].
- **Kiro:** fixes the launch engine/auth flags and uses its own support layer
  rather than treating every ACP agent's permission choices as identical.
  [Kiro support][kiro].
- **Antigravity:** profile/account isolation, native file reader compatibility,
  workflow/child work and prompt outcome tracking require dedicated handling.
  Its app can checkpoint files but does not advertise conversation rollback,
  so generic revert must fail before restoring files. [Support][antigravity-support],
  [adapter][antigravity], [provider notes][providers].

The ACP Registry is an extensible distribution path rather than a promise that
each catalog agent has the Claude/Codex feature set. The registry driver probes
configuration, options, models, available commands and sessions; it also exposes
provider/session management where supported. Its application text-generation
service explicitly returns unsupported. “Can run an ACP chat” therefore does
not imply “can supply every T3 helper operation.” [Registry driver][registry-driver],
[distribution resolution][acp-registry].

## OpenCode: HTTP server lifetime is part of isolation

T3 can spawn `opencode serve` or connect to a configured external server. The
runtime creates the HTTP client, validates health/version and applies server
authentication where configured. The adapter uses native session methods such
as create, asynchronous prompt and abort, while a separate event subscription
owns streamed progress/completion. An event-stream failure is not equivalent
to a successful prompt finishing. [Runtime][opencode-runtime], [1.x adapter][opencode].

The version split is architectural. OpenCode 1.x registers MCP servers with
directory scope, while T3 credentials are thread-scoped. T3 therefore owns one
managed chat server per thread, plus an idle instance helper for catalog/text
generation. In 2.x it can share one managed server per instance, naming each
thread's MCP registration `t3-code-<thread>` and using session permissions to
deny other threads' entries. External servers retain external ownership.
[Server ownership notes][providers], [1.x owner][opencode-owner], [2.x server][opencode2].

Native “always” permission can be project-wide. T3 avoids turning one automatic
approval into authority over unrelated threads: one-shot approval and
application-session rules are used where necessary. A generic “remember my
approval” UI would be unsafe if it ignored native grant scope.
[Provider notes][providers].

## Pi: native RPC plus an injected extension

The process is the user's Pi CLI in RPC mode. T3 sends commands and consumes
events through its RPC adapter, retains Pi session files and uses native
continuation/rollback where supported. Pi itself discovers extensions,
packages, settings, skills and project trust; T3 does not replace all of that
with its own synthetic runtime. Fork runs through Pi in the destination cwd
because simply switching a session through RPC can retain the old cwd.
[Pi adapter][pi], [launch config][pi-launch], [ownership notes][providers].

T3's namespaced extension supplies MCP bridging and a blocking `tool_call`
permission hook. The source explicitly says Pi delegates permission policy to
extensions. It checks built-in/read-only tool provenance, allows edits under
auto-accept-edits, otherwise asks through Pi's confirmation UI and blocks a
declined call. Full access skips that hook's restriction. Launch/runtime
environment preparation discards inherited T3 MCP credentials before installing
the current thread's values. [Injected extension][pi-extension], [launch][pi-launch].

This is a different enforcement mechanism from a Codex OS sandbox. A portable
permission-mode label must not hide the distinction. Extension loading and
project trust are operational prerequisites; a model prompt is not sufficient
to create this hook.

## Muse: MSP through a vendor SDK

`createMuseSdkHost` calls `spawnMspConnection` with the executable,
`serve` arguments, cwd and environment. It owns startup abort/timeout cleanup
even before an Effect resource exists, initializes the connection, checks
schema envelope version 1 and exposes granted capabilities. Read-only helper
launches disable shell/write and can disable session logging. The SDK owns
shutdown, while T3 wraps its outcomes into its session/run/item model.
[SDK host][muse], [Muse adapter][muse-adapter].

The lockfile resolves **1.4.6 with a patch**. As with Cursor, importing the
version number without the patch/wrapper is not an exact reproduction of T3's
integration. [Lockfile][lock].

## Shared extension discipline

Newer providers are workspace packages with separate settings, client, server
and testing exports. Client exports must remain browser/React Native safe.
Server implementations depend on provider-core contracts and a `ProviderHost`
for host facilities; they should not pull arbitrary application internals into
the reusable package. Older Claude/Codex/Antigravity drivers still live under
the server's `provider/Drivers` while using the same registration boundary.
[Adding a provider][adding], [driver interface][driver], [registry][registry].

Capability metadata must describe implemented behavior, native response options
must survive normalization, and only genuinely supported runtime modes should
be shown. Discovery/status should not open a provider session just to refresh
a card. Text-generation helpers are separate from interactive sessions and
must consider hooks/MCP effects before launch; denying tools only after startup
is not a sandbox for a configured profile. [Provider guidance][providers],
[extension guide][adding].

There is no universal protocol that erases provider differences. The reusable
part is the **T3 orchestration contract plus adapter boundary**, with capability
and lifetime information preserved for clients.

[registry]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/builtInDrivers.ts
[driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-core/src/server/driver.ts
[cursor]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-cursor/src/server/CursorAgentSdk.ts#L460
[cursor-adapter]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-cursor/src/server/adapter.ts
[cursor-sdk]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-cursor/src/server/CursorSdk.ts
[grok]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-grok/src/server/acpSupport.ts
[grok-status]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-grok/src/server/status.ts#L379
[fx]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-fx/src/server/acpSupport.ts
[kiro]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-kiro/src/server/acpSupport.ts
[antigravity]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/AntigravityAdapterV2.ts
[antigravity-support]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/acp/AntigravityAcpSupport.ts
[effect-acp]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/effect-acp/src/client.ts
[acp-runtime]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-acp/src/server/AcpSessionRuntime.ts#L2104
[acp-adapter]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-acp/src/server/adapter.ts
[mcp-bridge]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/mcp/AcpMcpStdioBridge.ts
[registry-driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-acp-registry/src/server/driver.ts
[acp-registry]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-acp-registry/src/server/AcpRegistrySupport.ts
[opencode-runtime]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-opencode/src/server/OpenCodeRuntime.ts
[opencode]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-opencode/src/server/adapter.ts
[opencode-owner]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-opencode/src/server/OpenCodeServerOwner.ts
[opencode2]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-opencode/src/server/v2/OpenCode2Server.ts
[pi]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-pi/src/server/adapter.ts
[pi-launch]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-pi/src/server/mcpInjection.ts#L266
[pi-extension]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-pi/src/server/mcpExtensionSource.ts#L341
[muse]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-muse/src/server/sdk.ts#L144
[muse-adapter]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/provider-muse/src/server/adapter.ts
[providers]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/providers.md
[adding]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/adding-a-provider.md
[lock]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/pnpm-lock.yaml
