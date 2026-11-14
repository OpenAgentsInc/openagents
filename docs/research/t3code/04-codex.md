# Codex integration

## A local app-server, not a phone-to-model connection

The Codex driver builds a V2 adapter around T3's
`effect-codex-app-server` package. It launches the configured `codex` executable
with `app-server -c thread_unload_delay_secs=0` plus resolved launch arguments.
The zero unload delay lets T3 own idle unloading rather than stacking a second
Codex delay on top. `CODEX_HOME` and the instance environment select native
state/authentication. Standard I/O connects the child to the typed protocol
client. [Driver][driver], [launch args][launch], [process factory][spawn].

```text
app → T3 application Effect RPC → V2 adapter
    → codex app-server (child process, JSON lines on stdin/stdout)
    → Codex native threads/turns/tools and provider network
```

OpenAI's current app-server documentation describes a bidirectional JSON-RPC
protocol that omits the usual `jsonrpc` field and supports stdio as the default.
It also documents other listeners; those are not evidence that this T3 adapter
opens an app-server WebSocket port. The app-to-T3 socket and the local
T3-to-Codex stdio channel are separate.
[OpenAI app-server protocol](https://learn.chatgpt.com/docs/app-server#protocol),
[T3 stdio transport][stdio].

## Protocol implementation and handshake

T3 supplies its own Effect client, generated schemas/method maps and patched
JSON-line transport. It correlates responses with pending requests, dispatches
server notifications, and answers reverse-direction server requests. The latter
is necessary for approvals and questions: a one-way stdout consumer would be
incomplete. It handles process termination and stderr separately from protocol
frames. [Client][client], [protocol][protocol], [stdio][stdio].

Initialization sends `initialize` with T3 client info and capabilities, then
`initialized`. A semaphore ensures one initialization for the shared app-server;
the adapter handles the race where interruption leaves Codex initialized before
T3 records that fact. Capabilities request experimental API support and the MCP
app extension, and opt out of `turn/diff/updated` because T3 derives diffs from
checkpoints. It checks the returned version against compatibility advice and
refuses known-broken versions. [Initialize][initialize], [capabilities][capabilities].

Generated schemas are a compatibility boundary, not universal future-version
support. The client extends the generated `turn/start` shape for accepted fields;
undecodable known notifications are dropped with a warning once per distinct
failure per client, while unknown notifications have an optional handler. Raw
transport frames are bounded before parsing: this revision allows a JSONL
message up to 128 MiB, uses 32-entry sliding raw notification/request queues
and limits concurrent reverse-request handlers. The outgoing queue is unbounded.
These controls are not a small overall process-memory guarantee.
[Typed client][client], [transport constants][protocol].

## Shared runtime and native identities

The adapter declares multiple provider threads per session. Its app-server can
serve an instance's Codex threads, with notifications routed by native
thread/turn/item identities to T3's owning thread/run/node. Initialization and
other unbound frames are global diagnostics. Per-thread shell configuration
carries thread attribution rather than putting one thread ID in the shared
process environment. [Capabilities][capabilities], [routing/logger][routing].

The native operations include:

| T3 operation | Codex protocol path |
| --- | --- |
| Ensure new conversation | `thread/start` |
| Restore/load native conversation | `thread/resume` |
| Send prompt | `turn/start` with native thread and compiled input/options |
| Active steering | `turn/steer` |
| Stop turn | `turn/interrupt` |
| Release thread | Native unload/unsubscribe path while preserving a shared runtime where possible |
| Fork | `thread/fork` |
| Revert conversation | `thread/revert` path in this revision, with native thread loaded first |

Sources: [adapter][adapter], [turn start][start], [thread creation][thread-start],
[fork/revert][fork]. The package's generated map also contains protocol methods
that need not be the operation the adapter actually chooses. In particular,
do not infer T3 uses `thread/rollback` merely from a schema entry or older
Codex examples.

App command identity, native thread ID, native turn ID and item ID remain
separate. Codex capabilities describe all four native identity categories as
strong, but T3 still needs its own identities for cross-provider continuity,
attempts and queued work. [Capabilities][capabilities].

## Turn input and permission modes

The adapter compiles model selection, reasoning/options, input attachments,
cwd, collaboration/plan interaction, app/developer context and effective
runtime policy for Codex. Uploads refer to environment-local files after T3's
attachment claiming; a path in a prompt does not itself grant native sandbox
permission. [Turn parameter compiler][turn-policy], [attachment boundary][attachments].

The default runtime mapping is explicit:

| T3 runtime mode | Codex approval policy | Reviewer | Sandbox policy |
| --- | --- | --- | --- |
| `approval-required` | `untrusted` | `user` | `readOnly` |
| `auto-accept-edits` | `on-request` | `user` | `workspaceWrite` |
| `auto` | `on-request` | `auto_review` | `workspaceWrite` |
| `full-access` | `never` | `user` | `dangerFullAccess` |

Explicit policies can override these defaults after decoding to supported
native shapes. Unlike a prompt saying “do not edit,” these settings are sent to
Codex's native execution controls. Their concrete security strength still
depends on Codex/OS configuration; no sandbox was exercised during this audit.
[Policy compiler][turn-policy].

## Events, plans, tools and human interaction

The adapter handles native thread/turn lifecycle, item starts/completions,
assistant/reasoning deltas, command output, file changes, plans, dynamic tools,
image views, rate limits and subagent/collaboration events. These become the
same T3 messages, nodes, items, requests and usage state the other clients use.
Native final-answer phase and async-question content need separate treatment;
not every completed agent message is the final answer.
[Adapter notification handlers][adapter].

Approvals are reverse requests such as
`item/commandExecution/requestApproval`, `item/fileChange/requestApproval` and
`item/permissions/requestApproval`. The adapter finds the active native turn,
creates durable presentation records, stores a live deferred response, waits
for a T3 decision and maps it back to the native response. Session allowances,
declines and legacy review formats have distinct paths. A native request is not
answered by emitting an ordinary assistant message.
[Approval handlers][approvals].

Blocking `item/tool/requestUserInput` similarly waits for a live answer and
maps T3 question IDs back to native question IDs. In contrast, an
`agentMessage` with `delivery: "async"` and questions creates a persisted
message-response request without a pending native RPC. Answering that request
posts a new user message. This second type survives provider/server restart;
live RPC requests do not. [Blocking input][questions], [async input][async],
[protocol notes][providers].

## Completion, steering, goals and subagents

Codex declares active steering, steering during compaction, interrupt/restart
steering and queued messages. T3 manages races between a `turn/start` response,
`turn/started` and early completion. An acknowledgment for a queued native turn
does not prove it has started, so interrupt logic waits for the appropriate
lifecycle state. [Capabilities][capabilities], [turn start][start],
[interrupt implementation][interrupt].

Native `/goal` can immediately start another turn. The adapter keeps the T3
run open and associates that new turn with it; one application run can own
several native Codex turns. Goal commands that create no native turn also need
a representation, and revert must not count those as native turns to remove.
[Protocol notes][providers], [adapter][adapter].

Codex's generic capabilities include native subagent thread IDs, lifecycle,
wait/close and child-thread fork. T3 must route child events independently and
preserve background-work lifetimes. That declaration is not a claim that every
setup enables Codex multi-agent: the managed runtime below explicitly disables
both multi-agent feature flags. [Capabilities][capabilities], [managed runtime][managed].

## Two setup/authentication families

### User-managed Codex CLI

The ordinary configured executable/home path uses the user's native Codex
installation and credentials. A direct `CODEX_HOME` selects its own config,
sessions and state. T3 also supports an **auth-overlay/shadow-home** layout:
native conversation/config/runtime directories can be shared while auth files
and model cache remain private to the shadow home. This permits account changes
for compatible shared conversations without copying the entire Codex state.
[Home layout][home], [account guide][guide].

The layout explicitly treats `auth.json`, `auth.lock` and `models_cache.json`
as private, links shared session/runtime-lock directories, and rejects invalid
or conflicting filesystem entries. A separate unrelated full home cannot
continue a thread merely because it selects the same model. Keyring/file
credential behavior also matters; the user guide calls out file storage for
shadow-home account separation. [Home implementation][home], [guide][guide].

### T3-managed Codex and ChatGPT token sharing

This source revision additionally implements a managed installation and a
T3-owned ChatGPT OAuth/token-sharing flow. It is important not to describe all
Codex integration as reading an existing CLI `auth.json`.

`CodexChatGptAuth` uses OpenID discovery, registrations/profiles, scope
`chatgpt.tokens.use.direct`, verified identity tokens, stored access/refresh
credentials and serialized refresh handling. This describes code present at
the pinned revision, not a guarantee that a new third-party application can
use the same product authorization without its own supported registration and
consent. [Auth implementation][auth].

The effective runtime pins `model_provider="openai_token_sharing"` and configures
the OpenAI Responses endpoint, an auth command, API-key model discovery and
`supports_websockets=false` on that **model-provider transport**. That flag does
not disable the apps' T3 WebSocket connection. It disables multi-agent/v2 flags
for this managed runtime and strips ambient `ACCESS_TOKEN`, `OPENAI_API_KEY`,
`OPENAI_BASE_URL` and launch-argument overrides that could redirect managed
credentials. Codex remains the coding-agent runtime; T3 is not replacing it
with a client-side Responses request. [Managed runtime][managed].

The auth command reaches a private T3 token broker over a Unix socket/named
pipe. On Unix it creates an owner-only directory, checks its identity/permissions
and bounds the socket path. Requests are bound to environment, instance and
launch profile/revision; the runtime establishes broker availability before
refresh can consume a token. This is a local credential-broker boundary, not
an unauthenticated public token HTTP server. [Broker][broker], [managed runtime][managed].

For remote managed login, a local primary environment can perform the callback
using ephemeral storage under the destination environment's identity. After
successful exchange/verification it exports the profile to the destination;
only the destination persists/refreshes the credentials. That prevents two
machines rotating the same refresh token. Without that primary route, remote
callback handling remains a separate flow. The handoff's temporary store is
cleared on finalization and refuses persistent-secret creation.
[Handoff][handoff], [provider notes][providers].

## Native persistence, recovery and compatibility

Codex retains native sessions in its home; T3 retains application projections,
native-reference mappings and receipts in its own database. Resume must reload
the native thread, while revert/fork require compatible native history and
continuation identity. T3 can perform metadata-only native reads when it only
needs identity/update time instead of downloading a full transcript.
[Home layout][home], [adapter][adapter], [provider notes][providers].

The generated protocol package is in the repository; a user's external Codex
binary is not pinned by the JavaScript lockfile. Managed installation and
compatibility policy are separate. This makes binary version, account/model
availability and feature flags part of support diagnosis, even if T3's own
version has not changed. [Installation manager][installation], [compatibility manifest][manifest].

[driver]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/Drivers/CodexDriver.ts
[launch]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/codexLaunchArgs.ts
[spawn]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L1698
[adapter]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts
[client]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/effect-codex-app-server/src/client.ts#L100
[protocol]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/effect-codex-app-server/src/protocol.ts
[stdio]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/packages/effect-codex-app-server/src/_internal/stdio.ts
[initialize]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L1922
[capabilities]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L249
[routing]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L1559
[start]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L7302
[thread-start]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L7670
[fork]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L8493
[turn-policy]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L724
[attachments]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/AttachmentClaims.ts
[approvals]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L5710
[questions]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L6130
[async]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L5619
[interrupt]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/Adapters/CodexAdapterV2.ts#L8083
[home]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/Drivers/CodexHomeLayout.ts
[guide]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/user/providers-codex.md
[managed]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/CodexManagedRuntime.ts
[auth]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/CodexChatGptAuth.ts
[broker]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/CodexManagedTokenBroker.ts
[handoff]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/CodexChatGptHandoff.ts
[providers]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/providers.md
[installation]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/CodexInstallation.ts
[manifest]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/model-manifest.json
