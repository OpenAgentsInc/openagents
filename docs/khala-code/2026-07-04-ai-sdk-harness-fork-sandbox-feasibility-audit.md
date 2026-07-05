# AI SDK Harness Fork + OpenAgents Sandbox Feasibility Audit

Date: 2026-07-04
Updated: 2026-07-05
Status: fourth-pass audit. No runtime code changed by this document.
Scope: actual published AI SDK harness package code, local Pylon Codex/Claude
runners, OpenAgents sandbox/workroom plans, opencode's Vercel AI SDK Core usage,
current Khala Sync mobile/desktop work, and whether a maintained fork or local
prototype is feasible.

## Executive Answer

Yes, we can get AI SDK Harnesses working locally with modifications, and yes,
the right long-term integration is to make an OpenAgents sandbox/workroom
implement the AI SDK sandbox-provider contract.

The opencode pass adds one important upgrade: Khala Code should copy
opencode's AI SDK Core pattern before waiting on harnesses. Opencode does not
make AI SDK stream parts its app model. It owns session state, tools,
permissions, provider catalog, telemetry, and transcript persistence; it calls
`streamText` as the default model transport; then it adapts AI SDK `stream` /
`fullStream` parts back into an internal `LLMEvent` stream. Khala should do the
same with an OpenAgents event schema.

The Khala Sync/mobile pass adds the near-term product path. Main already has a
real Khala Sync engine, desktop chat/fleet consumers, a TanStack DB adapter,
and an Expo mobile destination with Tailnet health probing and SQLite
persistence scaffolding. The upgrade is viable if we use Khala Sync as the
durable cross-device transport for OpenAgents-owned runtime events and control
intents, not as a place to serialize raw AI SDK stream parts.

No, we should not maintain a broad fork of AI SDK core. The viable ownership
shape is narrower:

- Keep upstream `ai` and `@ai-sdk/harness` as dependencies whenever possible.
- Build an OpenAgents sandbox provider that implements
  `HarnessV1SandboxProvider`.
- Maintain a shallow fork or wrapper of the Codex and Claude Code adapters only
  where OpenAgents needs account-home routing, raw private event capture,
  package version control, typed failure mapping, or stronger policy seams.
- Upstream every generic extension point we need so the fork can shrink.
- Add a first-class AI SDK Core provider runtime in Khala for normal
  provider/model calls; keep Codex/Claude agent runtimes behind their own
  app-server or harness/sandbox adapters.

The decisive code-level finding is that AI SDK already separates the agent
runtime from the sandbox provider. `@ai-sdk/harness` does not hard-code Vercel.
It asks for a provider that can create/resume a network sandbox session with
file I/O, `run`, `spawn`, an exposed WebSocket port, and lifecycle methods. That
is almost exactly the public-safe boundary our OpenAgents sandbox docs call
`openagents.sandbox.v1`, and matches the product shape we extracted from the
Amp sandbox research for Khala Code threads and workrooms.

The caution is Codex. The published Codex adapter runs the Codex SDK inside the
sandbox with `sandboxMode: "danger-full-access"` and `approvalPolicy: "never"`.
It explicitly rejects built-in tool filtering and permission modes other than
`allow-all`. That is acceptable only if the sandbox/workroom is the actual
containment authority. It is not acceptable as a policy boundary by itself.

## Published Code Reviewed

Packages fetched from npm and unpacked on 2026-07-04:

| Package | Version | Source shape reviewed |
| --- | ---: | --- |
| `@ai-sdk/harness` | `1.0.18` | 76 package files, about 9.2k TypeScript source lines. |
| `@ai-sdk/harness-codex` | `1.0.19` | 23 package files, about 2.6k TypeScript source lines. |
| `@ai-sdk/harness-claude-code` | `1.0.18` | 20 package files, about 2.8k TypeScript source lines. |
| `@ai-sdk/sandbox-vercel` | `1.0.18` | 11 package files, about 724 TypeScript source lines. |

Current npm latest checks on 2026-07-04:

| Package | Latest checked | Notable drift |
| --- | ---: | --- |
| `@openai/codex-sdk` | `0.142.5` | AI SDK Codex bridge package pins `0.130.0`; OpenAgents Pylon pins `^0.139.0`. |
| `@anthropic-ai/claude-agent-sdk` | `0.3.201` | AI SDK Claude bridge package pins `0.3.177`; OpenAgents pins `^0.3.172`. |
| `@anthropic-ai/claude-code` | `2.1.201` | AI SDK Claude bridge package pins `2.1.177`. |

The important package files are:

- `@ai-sdk/harness/src/v1/harness-v1-sandbox-provider.ts`
- `@ai-sdk/harness/src/v1/harness-v1-network-sandbox-session.ts`
- `@ai-sdk/harness/src/agent/prepare-sandbox-for-harness.ts`
- `@ai-sdk/harness/src/agent/internal/bootstrap-recipe.ts`
- `@ai-sdk/sandbox-vercel/src/vercel-sandbox.ts`
- `@ai-sdk/sandbox-vercel/src/vercel-sandbox-session.ts`
- `@ai-sdk/sandbox-vercel/src/vercel-network-sandbox-session.ts`
- `@ai-sdk/harness-codex/src/codex-harness.ts`
- `@ai-sdk/harness-codex/src/bridge/index.ts`
- `@ai-sdk/harness-codex/src/bridge/package.json`
- `@ai-sdk/harness-claude-code/src/claude-code-harness.ts`
- `@ai-sdk/harness-claude-code/src/bridge/index.ts`
- `@ai-sdk/harness-claude-code/src/bridge/package.json`

Reference repositories inspected on 2026-07-05:

| Repository | Commit | Relevant files |
| --- | --- | --- |
| `projects/repos/opencode` | `1b9b2604581bfdac263a69a2d5846bd2a91da6cc` | `packages/opencode/src/session/llm.ts`, `packages/opencode/src/session/llm/ai-sdk.ts`, `packages/opencode/src/session/llm/request.ts`, `packages/opencode/src/session/tools.ts`, `packages/opencode/src/provider/provider.ts`, `packages/opencode/src/provider/transform.ts`, `packages/opencode/src/session/processor.ts`, `packages/opencode/src/session/llm/native-runtime.ts`, `packages/opencode/src/session/llm/native-request.ts`, `packages/opencode/src/session/llm/AGENTS.md`, `packages/opencode/package.json`. |
| `projects/repos/ai` | `77f9f686dcf8873f8cc9eb1aa416e91b8b308a70` | `packages/ai/src/generate-text/stream-text.ts`, `packages/ai/src/generate-text/stream-text-result.ts`, `packages/provider/src/language-model/v4/language-model-v4.ts`, and harness package directories. |

Current OpenAgents files reviewed on 2026-07-05:

| Area | Evidence |
| --- | --- |
| Recent commits | `2526a91e7a`, `9b566511d8`, `e0bb4bc630`, `970f6d1bdd`, `5964c3a7f9`, `636eb30bd6`. |
| Khala Sync contracts/client/server | `docs/khala-sync/SPEC.md`, `packages/khala-sync`, `packages/khala-sync-client`, `packages/khala-sync-server`. |
| Desktop Sync consumer | `clients/khala-code-desktop/src/bun/khala-sync-service.ts`, `rpc-handlers.ts`, `src/shared/rpc.ts`, `tests/khala-sync-service.test.ts`, `tests/rpc-schema.test.ts`. |
| Mobile destination | `clients/khala-mobile/AGENTS.md`, `README.md`, `app/index.tsx`, `src/status/khala-code-connectivity*.ts`, `src/sync/khala-chat-feed.tsx`, `src/sync/khala-mobile-sync-runtime.ts`, `src/sync/expo-db-sqlite-persistence.ts`. |
| Sync UI/docs receipts | `docs/fable/2026-07-04-khala-sync-implementation-status.md`, `2026-07-04-khala-sync-db-collection.md`, `2026-07-04-chat-sidebar-sync-consumers.md`, `2026-07-04-khala-sync-cross-device-dogfood.md`. |

## OpenCode's Vercel AI SDK Core Pattern

Opencode's production runtime uses Vercel AI SDK Core, not AI SDK Harnesses.
The repo contains no evidence that opencode itself runs through
`@ai-sdk/harness-opencode` for its app model. Instead it imports `streamText`,
`wrapLanguageModel`, `tool`, `jsonSchema`, `asSchema`, and provider packages
from `ai` / `@ai-sdk/*` and wraps them in opencode-owned Effect services.

That distinction matters for Khala Code. AI SDK Harnesses are still the right
shape for "run Codex or Claude Code as a whole agent inside an OpenAgents
sandbox." AI SDK Core is the right shape for "normalize provider/model
streaming, tools, usage, reasoning, and provider metadata through one
model-call adapter." We should do both, in that order.

### What opencode actually does

1. `packages/opencode/src/session/llm.ts` owns runtime selection. It resolves
   model/provider/auth/config, prepares the request, optionally tries an
   experimental native runtime, and otherwise calls `streamText`.
2. `packages/opencode/src/session/llm/request.ts` builds a normalized request
   before AI SDK sees it. It merges agent, provider, user, and plugin system
   prompts; selects model variants; computes temperature/topP/topK/max tokens;
   filters tools through permission rules; applies provider-specific headers;
   sets OpenAI/Codex-style `strict: false` on function tools where needed; and
   adds compatibility tools such as Copilot's `_noop`.
3. `packages/opencode/src/provider/provider.ts` is a real provider catalog,
   not a static switch. It starts from models.dev data, merges config/env/auth
   and plugin patches, lazy-loads bundled AI SDK provider packages, installs
   non-bundled provider packages when needed, wraps `fetch` for timeout and SSE
   quirks, caches provider SDK instances, and supports provider-specific model
   loaders.
4. `packages/opencode/src/provider/transform.ts` centralizes provider quirks:
   providerOptions key mapping, reasoning variants, prompt cache keys, OpenAI /
   Azure / Bedrock option shape, Anthropic and Gemini caching hints,
   interleaved reasoning fields, unsupported media fallback text, and JSON
   schema lowering for providers with stricter tool-schema rules.
5. `packages/opencode/src/session/tools.ts` converts opencode's tool registry
   into AI SDK `tool()` definitions. Tool execution re-enters opencode's Effect
   runtime, runs plugin hooks, calls opencode permission `ask`, truncates large
   outputs, normalizes attachments, and completes interrupted tool calls
   through the session processor.
6. `packages/opencode/src/session/llm/ai-sdk.ts` is the narrow adapter seam.
   It maps AI SDK `TextStreamPart`s into `@opencode-ai/llm` `LLMEvent`s:
   start/finish step, text, reasoning, tool input, tool call, tool result, tool
   error, usage, finish reason, and provider metadata. Unsupported UI-facing
   parts are dropped. Raw provider chunks are used only for a narrow Copilot
   billing metadata extraction.
7. `packages/opencode/src/session/processor.ts` consumes only `LLMEvent`.
   It updates durable message parts, tool state, reasoning state, usage/cost,
   snapshots, patch parts, compaction, retries, and status without caring
   whether events came from AI SDK or native transport.
8. `packages/opencode/src/session/llm/native-runtime.ts` and
   `native-request.ts` prove the architecture works: native LLM support is
   opt-in, per-request, and falls back to AI SDK. Both paths converge on the
   same `LLMEvent` stream, and tool execution remains opencode-owned.

The pattern is not "let AI SDK own the agent." The pattern is "let AI SDK own
provider call normalization, while the product owns session authority."

### What the actual AI SDK code confirms

The current AI SDK Core code makes this feasible without a fork:

- `streamText` accepts a `LanguageModel`, messages, headers, tools,
  `activeTools`, `toolChoice`, `providerOptions`, telemetry, repair hooks,
  runtime/tool context, and optional tool-approval controls.
- `StreamTextResult` exposes `stream` plus deprecated-compatible `fullStream`,
  along with `textStream`, usage, steps, response messages, provider metadata,
  and UI-message conversion helpers.
- `TextStreamPart` is the same shape opencode adapts: text, reasoning, file,
  source, tool-input, tool-call, tool-result, tool-error, tool approval,
  start-step, finish-step, finish, abort, error, and raw.
- Provider packages implement the `LanguageModelV4` interface through
  `doGenerate` and `doStream`, so Khala can depend on the provider/model
  abstraction while keeping its own transcript schema.
- AI SDK Core also has an `experimental_sandbox` parameter for tool execution,
  but that is not the same boundary as AI SDK Harness sandbox providers. For
  OpenAgents, Core tools should call into `openagents.sandbox.v1` or the
  OpenAgents sandbox/workroom API when they need workspace execution;
  harnesses should use the OpenAgents sandbox provider when the whole agent
  runtime must live inside a sandbox.

One minor modernization: new Khala code should consume `result.stream`; keep
`fullStream` support only for compatibility with opencode-style examples and
older AI SDK minor lines.

### What Khala should copy

Khala Code should add an opencode-style AI SDK Core runtime with these
boundaries:

1. Define or reuse a canonical OpenAgents stream event schema for Khala model
   turns. It should cover text, reasoning, step boundaries, tool input, tool
   call/result/error, provider metadata, usage, finish reason, file changes,
   compaction, and private raw sidecar refs. AI SDK parts must not become the
   public transcript schema.
2. Add a `khala-ai-sdk-core-runtime` service that prepares OpenAgents messages,
   tools, headers, provider options, and telemetry, calls `streamText`, and
   converts AI SDK `TextStreamPart`s into OpenAgents events.
3. Add a provider catalog / transform layer, probably in a shared package
   rather than the desktop client. Start smaller than opencode, but keep the
   same shape: model metadata, provider package loader, providerOptions key
   mapping, reasoning variants, prompt cache keys, schema lowering, and
   provider-specific headers.
4. Bridge OpenAgents tools into AI SDK `tool()` definitions. Tool bodies must
   re-enter Effect, enforce the compiled OpenAgents tool policy, ask
   permissions through Khala/Pylon authority, and use OpenAgents
   sandbox/workroom execution APIs for workspace effects.
5. Keep raw provider chunks and raw agent events private. Use AI SDK `raw`
   chunks only for narrow metadata extraction or private archives, never as a
   public proof or user-visible transcript.
6. Use a runtime selector with explicit lanes:
   - `ai_sdk_core` for normal provider/model calls.
   - `codex_app_server` for today's local Codex app-server path.
   - `claude_pylon` for today's local Claude path.
   - `ai_sdk_harness_sandbox` for Codex/Claude harness experiments inside an
     OpenAgents sandbox.
   - optional `native_direct` only if we later build a native request executor.
7. Make all lanes emit the same OpenAgents event stream. This is the key
   opencode lesson: once the processor owns a canonical event contract, runtime
   replacement becomes a routing decision, not a UI rewrite.

### Feasibility impact

This makes the upgrade more viable, not less. The harness/sandbox path is still
needed for full Codex and Claude Code runtimes, but Khala can adopt AI SDK Core
first without forking AI SDK and without waiting for an OpenAgents sandbox
provider.

The practical order should be:

1. Build the AI SDK Core stream adapter locally against one low-risk model
   provider and fixture tool.
2. Render its OpenAgents events in Khala Code's transcript alongside existing
   Codex/Pylon events.
3. Move provider and tool transforms into shared packages once the shape is
   proven.
4. Bind tool execution to `openagents.sandbox.v1` / OpenAgents sandbox APIs
   for workspace effects.
5. Add AI SDK Harnesses for Codex/Claude only after the OpenAgents sandbox
   provider can be the containment boundary.

## Khala Sync Mobile/Desktop Roadmap

This pass changes the practical roadmap. The earlier Fable analysis correctly
identified cross-device chat as the dogfood milestone, but parts of it are now
stale: Khala Sync packages are no longer empty, the Postgres-backed sync engine
is live, `@openagentsinc/khala-sync-client` exists, the TanStack DB collection
adapter exists, and the Expo Khala mobile destination is now the TypeScript
surface while SwiftUI remains the interim shipping/native-reference app.

### Current implementation state

- Khala Sync is the right substrate for this upgrade: it already has typed
  scopes, dense cursors, named mutators, in-band rejections, local SQLite
  state, optimistic overlay/rebase, HTTP/WS transport, owner-private chat
  mutators, fleet mutators, and scope authorization.
- Desktop currently exposes `khalaSyncChatThreads`,
  `khalaSyncChatCreateThread`, `khalaSyncChatRenameThread`,
  `khalaSyncFleetState`, and `khalaSyncFleetMutate`. The local active diff
  adding `khalaSyncChatAppendMessage` is directionally correct: append is a
  control intent, not a UI-only RPC.
- The desktop service already builds chat mutators, tracks in-band rejections,
  keeps a durable SQLite store under `~/.khala-code`, and surfaces real sync
  phases (`idle`, `bootstrapping`, `catching_up`, `live`, `must_refetch`,
  `denied`) instead of fabricating liveness.
- Mobile now has a Tailnet health dot that probes the desktop health beacon
  (`:50099/health`) and resolves a `KhalaCodeConnectionProfile` for simulator
  loopback vs physical-device Tailnet routing. It also has a durable
  `KhalaSyncSession` runtime over Expo SQLite, confirmed chat projection reads,
  keychain-only auth loading, and typed chat create/append intents with
  public-safe pending/rejection state.
- The server already has `chat.createThread`, `chat.appendMessage`, and
  `chat.renameThread`, with `scope.user.<owner>` carrying thread metadata and
  `scope.thread.<threadId>` carrying message bodies. That is the exact shape
  we should extend for runtime turn/control events.

### OpenCode-shaped target

OpenCode's current architecture has two useful layers. The older path calls
AI SDK `streamText`, adapts `fullStream` parts into `@opencode-ai/llm`
`LLMEvent`s, and lets the session processor consume only `LLMEvent`. The newer
`packages/core` runner moves that seam deeper: `llm.stream(request)` emits
normalized `LLMEvent`s, then a publisher turns them into durable typed session
events while local tool settlement remains outside provider transport.

Khala should copy that ownership pattern:

1. AI SDK stream parts are adapter input only.
2. OpenAgents runtime events are the canonical product/session/sync contract.
3. Khala Sync carries those canonical events and typed control intents across
   mobile, desktop, web, and server.
4. Tool execution re-enters OpenAgents policy and sandbox/workroom authority.
5. Harnesses are a later runtime lane, not the foundation for mobile sync.

In other words, mobile should not call "AI SDK" and desktop should not project
"AI SDK parts." Mobile submits OpenAgents control intents through Khala Sync;
desktop or the server-side runtime executes through the selected lane; every
lane emits OpenAgents runtime events; Khala Sync replicates the authorized
read model back to mobile and desktop.

### Roadmap issues

The whole process is now filed as GitHub issues:

| Issue | Workstream |
| --- | --- |
| [#8363](https://github.com/OpenAgentsInc/openagents/issues/8363) | Define the AI SDK-shaped OpenAgents runtime event and control schema. |
| [#8364](https://github.com/OpenAgentsInc/openagents/issues/8364) | Finish the desktop mobile chat/control bridge, including typed append. |
| [#8365](https://github.com/OpenAgentsInc/openagents/issues/8365) | Connect the mobile Tailnet health dot to a durable Khala Sync client. |
| [#8370](https://github.com/OpenAgentsInc/openagents/issues/8370) | Add server runtime control/event scopes, owner policy, and idempotency. |
| [#8373](https://github.com/OpenAgentsInc/openagents/issues/8373) | Build the OpenCode-style AI SDK Core stream adapter to OpenAgents events. |
| [#8374](https://github.com/OpenAgentsInc/openagents/issues/8374) | Implement local and OpenAgents AI SDK sandbox providers. |
| [#8375](https://github.com/OpenAgentsInc/openagents/issues/8375) | Prove the mobile-to-desktop AI SDK-shaped runtime dogfood flow. |

Issue #8363 implementation status: the canonical contract now lives in
`@openagentsinc/agent-runtime-schema` as
`openagents.khala_runtime_event.v1` and
`openagents.khala_runtime_control_intent.v1`. The schema includes the runtime
lanes named in this audit, stable turn/message/control/tool/chunk IDs,
structural mappers from existing `AgentRuntimeEvent` records and AI SDK
`TextStreamPart`-shaped objects, golden fixtures, and conformance tests for
raw sidecar privacy and required tool authority. The remaining roadmap items
should consume this package instead of defining their own message/turn/tool
contract.

Issue #8364 implementation status: the desktop bridge now has typed
`khalaSyncChatMessages` and `khalaSyncChatAppendMessage` RPCs, per-thread
`chat_message` TanStack collections over `scope.thread.<threadId>`, and a
renderer path that hydrates Khala Sync chat rows into the existing transcript
instead of starting a Codex turn. The bridge uses client-generated message IDs,
returns public-safe disabled/auth/rejection states, keeps message bodies out of
the owner personal scope, and reads sidebar thread metadata from the Khala Sync
overlay so cross-scope append updates stay visible. The shared Sync session now
returns the exact `MutationId` assigned by `mutate`, which lets collection
adapters match in-band server rejections without guessing from a per-collection
pending queue. Remaining runtime controls should build on this as additional
typed control intents rather than as ad hoc desktop RPCs.

Issue #8365 implementation status: the Expo mobile app now opens a real
`KhalaSyncSession` through `openKhalaMobileSyncRuntime()`, with auth loaded
from the Khala keychain adapter and an Expo SQLite implementation of
`KhalaSyncLocalStore` for durable cursors, confirmed rows, client identity, and
pending mutation intents. The home-screen chat panel consumes confirmed
`chat_thread` and `scope.thread.<threadId>` `chat_message` projections instead
of the old raw JSON demo feed, while surfacing sync phase, pending count, and
public-safe rejections separately. The health resolver now produces a
connection profile that distinguishes simulator loopback (`127.0.0.1:50099`)
from physical-device Tailnet host routing and normalizes the Khala Sync base
URL independently from the health beacon. Mobile tests cover connection
profiles, SQLite checkpoint/store behavior, fake-session create/append,
app-restart cursor resume without duplicate messages, and rejection handling
without retaining rejected private bodies in the confirmed read model.

### Sequenced implementation path

P0 should be the event/control schema (#8363). Without this, each bridge will
invent its own version of "message," "turn," "tool," and "runtime state," and
we will lose the main OpenCode benefit. The schema should cover text,
reasoning, tool input/call/result/error, usage, provider metadata, finish
reasons, file changes, compaction, interruption, private raw-event refs, and
stable IDs for turns/messages/control intents/tool calls.

P1 is now landed for the chat subset (#8364): create/rename/append/read cover
the first desktop/mobile control bridge. Treat append as the first control
intent in a growing vocabulary. Start-turn, interrupt, continue/resume, retry,
and close remain the runtime subset. Those controls should return typed
public-safe results and surface rejections as state, matching Khala Sync's
queue-never-blocks model.

P2 is now landed for the mobile chat subset (#8365): the health dot feeds
connection discovery, the app opens a durable sync session, Expo SQLite stores
confirmed sync state and pending mutation intents, `chat_thread` plus
`scope.thread.<threadId>` projections drive the UI, and chat create/append use
client-generated IDs. The key UX rule remains honesty: stale, offline,
pending, denied, and rejected are visible states, never coerced into
"connected." Pairing/auth UX beyond the keychain token loader remains product
work, not a Sync substrate blocker.

P3 should add runtime state on the server side (#8370). The existing chat
mutators prove the shape, but runtime events need their own typed rows or
projection entities, owner/thread/team scope routing, idempotent mutators, and
scope-auth coverage. Message bodies and runtime content stay in authenticated
owner/thread scopes; public evidence gets only refs, counts, route names,
latency buckets, and issue/build refs.

P4 should add the AI SDK Core lane (#8373). This can work locally before the
harness sandbox work: one model fixture, one tool fixture, one providerOptions
fixture, and one raw privacy fixture. Its output must be OpenAgents runtime
events, not AI SDK stream parts. This is the fastest way to get "AI SDK-like"
runtime shape into Khala Code without waiting for the harness provider.

P5 should implement the sandbox provider path (#8374). Start with a clearly
unsafe local provider for owner-local fixtures, then move to
`@openagentsinc/ai-sdk-sandbox-openagents` over the real OpenAgents
sandbox/workroom API. Claude should go first because its AI SDK adapter
already supports approvals/filtering; Codex should wait until the OpenAgents
sandbox enforces policy under the adapter's full-access posture.

P6 should close with a public-safe dogfood receipt (#8375). The proof should
show a mobile-created control intent appearing in desktop without restart, a
runtime event appearing back on mobile after catch-up/resume, and at least one
offline/reconnect or app-restart cursor-resume case. The evidence validator
must reject raw prompts, chat bodies, provider chunks, local paths, tokens, and
secrets.

### Local feasibility answer

Yes, we can get this working locally in increments:

1. Fake or local Khala Sync transport now proves desktop chat create, append,
   rename, thread-scope read convergence, pending retry visibility, and
   public-safe rejection handling.
2. The mobile runtime now proves simulator loopback vs physical-device Tailnet
   connection profiles, Expo SQLite durable cursor/checkpoint resume, and a
   fake-session chat create/append flow without duplicate messages after app
   restart.
3. AI SDK Core can be proven with a fixture provider and one low-risk model
   without any harness sandbox provider.
4. The local unsafe harness provider can prove Codex/Claude bridge mechanics
   without Vercel, but it must remain owner-local until the OpenAgents sandbox
   provider is real.
5. The real dogfood proof ties them together: mobile control intent -> Khala
   Sync -> desktop/runtime lane -> OpenAgents runtime events -> Khala Sync ->
   mobile/desktop/web projections.

The main risk is not "can we call AI SDK?" The main risk is accidentally
letting AI SDK, a mobile client, or a desktop RPC become the authority for
tools, workspace effects, secrets, or transcript truth. Keep the authority in
OpenAgents schemas, mutators, policy, and sandbox/workroom services, and this
is a viable upgrade.

## What The AI SDK Code Actually Requires

`HarnessV1SandboxProvider` is small and direct:

- `specificationVersion: "harness-sandbox-v1"`
- `providerId`
- optional `bridgePorts`
- `createSession({ sessionId, identity, onFirstCreate, abortSignal })`
- optional `resumeSession({ sessionId, abortSignal })`

The returned `HarnessV1NetworkSandboxSession` must provide:

- `id`
- `defaultWorkingDirectory`
- `ports`
- `getPortUrl({ port, protocol })`
- `run`
- `spawn`
- file read/write methods from `Experimental_SandboxSession`
- `restricted()`
- `stop()`
- optional `destroy()`, `setNetworkPolicy()`, and `setPorts()`

This is good news. An OpenAgents sandbox provider does not need to pretend to
be a Vercel sandbox. It only needs to satisfy the contract above.

`@ai-sdk/sandbox-vercel` proves the provider layer is thin. It wraps
`@vercel/sandbox`, forwards `runCommand`, `readFileToBuffer`, `writeFiles`,
port domains, network policy updates, and sandbox `stop/delete`. Its template
path uses `identity` plus `onFirstCreate` to build a persistent prepared
snapshot and then forks per-session sandboxes from it. That maps naturally to
our `.agents/setup` plus post-setup snapshot plan.

The AI SDK bootstrap model also helps us: adapters provide a bootstrap recipe
with files and commands; the framework hashes the recipe and writes an
idempotent marker under the sandbox. For OpenAgents, that hash can become part
of the OpenAgents sandbox snapshot key alongside repo ref, `.agents/setup`,
lockfiles, base image, and toolchain version.

## Codex Adapter Findings

The published Codex adapter is bridge-backed:

- `createCodex().getBootstrap()` writes bridge files into
  `/tmp/harness/codex` and runs `pnpm install --frozen-lockfile` inside the
  sandbox.
- `doStart()` requires a sandbox with an exposed port, spawns
  `node /tmp/harness/codex/bridge.mjs`, waits for a bridge-ready record, then
  opens a WebSocket through `sandboxSession.getPortUrl(..., protocol: "ws")`.
- Resume is built around bridge coordinates, event-log replay, and Codex
  `threadId`.
- It forwards auth through env (`CODEX_API_KEY`, `OPENAI_BASE_URL`,
  `OPENAI_ORGANIZATION`, `OPENAI_PROJECT`, AI Gateway vars, etc.).

The hard limits:

- `supportsBuiltinToolApprovals: false`.
- Built-in tool filtering throws immediately.
- Any `permissionMode` other than `allow-all` throws immediately.
- The bridge starts Codex with `sandboxMode: "danger-full-access"` and
  `approvalPolicy: "never"`.
- Host tools are not clean MCP yet. The bridge documents an upstream Codex
  issue where MCP tools do not surface in the Codex SDK path, so it adds a
  CLI-relay workaround that the model invokes through its bash tool.
- It emits normalized tool/text/reasoning/file-change/usage parts, but not the
  full raw Codex SDK event stream. The shared `raw` stream part exists in
  `@ai-sdk/harness`, but this adapter does not currently use it.
- The bridge dependency lock pins `@openai/codex-sdk` to `0.130.0`, behind both
  current npm latest and our existing Pylon dependency range.

This means upstream Codex can be used behind an OpenAgents sandbox, but cannot
by itself replace Pylon's current authority model. Our current Pylon Codex
executor also uses owner-local full access and approval policy `never`, but it
additionally has OpenAgents-specific account homes, raw event chunk archives,
exact usage ingest, quota/auth health ledgers, post-hoc workspace escape
blocking, SCM credential scans, and closeout semantics. The AI SDK adapter
would need a fork or wrapper to preserve those behaviors.

One local doc/code drift matters: `apps/pylon/docs/codex-bridge.md` still says
the bounded assignment path disables network access, while
`apps/pylon/src/codex-agent-executor.ts` now passes `networkAccessEnabled: true`
for the live runner and documents the owner-local danger posture in code. The
audit conclusion is therefore stronger: network policy must live below the
adapter in the OpenAgents sandbox/workroom profile, not in stale docs or
model-facing settings.

## Claude Code Adapter Findings

The Claude adapter is a better fit for early experimentation:

- `supportsBuiltinToolApprovals: true`.
- `supportsBuiltinToolFiltering: true`.
- The bridge uses the Claude Agent SDK's `canUseTool` hook to ask/allow/deny
  based on AI SDK `permissionMode`.
- `allow-all` maps to Claude bypass permissions; `allow-edits` maps to accept
  edits; read/edit/bash distinctions are enforced through generated permission
  rules.
- Host tools are exposed through an SDK MCP server rather than the Codex CLI
  relay workaround.
- Compaction is observed through Claude compact-boundary messages and a
  `PostCompact` hook.
- Usage and `total_cost_usd` are projected into harness metadata.

The remaining gaps are still real:

- It also installs a bridge package inside the sandbox and pins older Claude
  Agent SDK / Claude Code package versions.
- It forwards selected auth env and can read the host `~/.claude/settings.json`
  `apiKeyHelper` in auto-detect mode; OpenAgents would need explicit account
  home and credential-source routing instead of ambient host defaults.
- It normalizes events rather than preserving a full private raw SDK archive.
- Our Pylon Claude runner already has a stricter workspace-specific
  `PreToolUse` guard and OpenAgents turn reporting. The AI SDK path must prove
  parity before replacing that runner.

## Can We Maintain A Fork?

Yes, if the fork is scoped. No, if the fork means owning all of AI SDK.

Viable fork boundary:

1. `@openagentsinc/ai-sdk-sandbox-local` or similar local provider for tests
   and owner-local desktop experiments.
2. `@openagentsinc/ai-sdk-sandbox-openagents` implementing `HarnessV1SandboxProvider`
   over OpenAgents workrooms/sandboxes.
3. A shallow `@openagentsinc/harness-codex` fork only for:
   - Codex SDK version control.
   - account-home routing (`CODEX_HOME`, short-lived auth material, no default
     global home).
   - private raw event side-channel or `raw` stream parts.
   - OpenAgents failure taxonomy.
   - removal or hardening of the prompt-mediated CLI tool relay where possible.
4. A shallow `@openagentsinc/harness-claude-code` fork only for:
   - `CLAUDE_CONFIG_DIR` / per-account OAuth token routing.
   - exact usage and cost metadata preservation.
   - private raw event capture if needed.
   - OpenAgents-specific permission presets.

Avoid forking:

- `ai` core.
- `@ai-sdk/harness` core unless we hit a missing extension point that cannot be
  wrapped. If that happens, open an upstream issue/PR first and carry the
  smallest possible patch.

Fork maintenance is feasible because the adapter packages are small: the Codex
adapter is about 2.6k source lines and the Claude adapter is about 2.8k. The
danger is not size; it is churn in Codex/Claude runtime packages and bridge
lockfiles. Any fork needs automated drift checks against npm tarballs and a
fixture matrix that fails when upstream package updates change event shapes.

## Can We Get It Working Locally?

Yes. There are now two different "working locally" targets:

- AI SDK Core provider calls can work locally first, without a harness sandbox
  provider, if Khala adapts AI SDK stream parts into OpenAgents events.
- AI SDK Harnesses can work locally next, through a deliberately unsafe local
  sandbox-provider spike, before moving into real OpenAgents sandboxes.

For harnesses, the local path still has two stages.

### Stage 1: unsafe local provider for proof only

Build a local provider that implements `HarnessV1SandboxProvider` by creating a
temp workspace, starting bridge processes as host child processes, exposing a
fixed localhost port, and implementing `run`, `spawn`, read/write, stop, and
destroy. On macOS it can optionally wrap commands with the existing
`packages/khala-tools/src/process-sandbox-macos.ts` Seatbelt service.

This is enough to prove:

- `HarnessAgent` can run Codex and Claude without Vercel.
- Khala Code can consume AI SDK stream parts.
- We can run a public sum-repair fixture and verify file changes.
- We understand the package bootstrap and port behavior.

This stage must be owner-local only. macOS Seatbelt writes-limited-to-workspace
is useful, but it still is not the production security boundary for Codex full
access, network egress, package installs, or hostile repositories.

### Stage 2: real OpenAgents sandbox provider

Build an OpenAgents sandbox/workroom provider over the OpenAgents sandbox plan:

| AI SDK requirement | OpenAgents mapping |
| --- | --- |
| `createSession({ sessionId, identity, onFirstCreate })` | Create or resume a workroom/sandbox, keyed by thread/session id and snapshot identity. |
| `identity` | Hash of AI SDK bootstrap recipe plus `.agents/setup`, lockfiles, base image, and toolchain version. |
| `onFirstCreate` | Run AI SDK bridge bootstrap and repo setup before snapshot. |
| `run` / `spawn` | Workroom exec/session API over `oa-workroomd` or `openagents.sandbox.v1`. |
| file read/write | Workroom filesystem API, artifact adapter, or bounded rsync channel. |
| `ports` / `getPortUrl` | Managed preview ingress with hashed endpoint tokens. |
| `setNetworkPolicy` | Sandbox profile plus capability gateways. |
| `restricted()` | Tool-safe view with no lifecycle, port, or network-policy authority. |
| `stop()` | Pause/stop workroom and stop metering. |
| `destroy()` | Closeout/archive/destroy with artifact and receipt checks. |
| `resumeSession()` | Reattach to the same workroom by session id. |

This is a strong fit with the sandbox-inspiration doc:

- thread -> workroom binding,
- `.agents/setup` / `.agents/resume`,
- post-setup snapshots with TTL,
- pause/resume economics,
- `.openagents/dev-ports.json`,
- PTY/preview gateway,
- lane-transparent events into Khala Code.

It also fits the sandbox-platform audit's public contract plan:
`openagents.sandbox.v1`, tiered isolation, Firecracker for untrusted/heavy,
Cloudflare Containers for light/web, content-addressed artifacts, capability
gateways, metadata endpoint, and metered receipts.

## How It Should Tie Into OpenAgents Sandboxes

The AI SDK sandbox provider should become one consumer of the OpenAgents sandbox platform, not
the OpenAgents sandbox platform itself.

Recommended layering:

1. `openagents.sandbox.v1` / sandbox API owns lifecycle, isolation, snapshots,
   ports, filesystem, receipts, and metering.
2. `@openagentsinc/ai-sdk-sandbox-openagents` adapts that API to
   `HarnessV1SandboxProvider`.
3. AI SDK Codex/Claude adapters run their bridges inside the OpenAgents sandbox.
4. Khala Code consumes normalized stream parts for UI, but OpenAgents sidecars
   still own raw private event archives, exact usage ledgers, account health,
   SCM credential scans, and closeout receipts.

This lets us use AI SDK where it is strongest, as a compatibility stream and
runtime bridge, while keeping OpenAgents authority in the pieces we already
care about.

Do not invert that layering. If AI SDK becomes the authority for sandbox
lifecycle, secrets, or closeout, we lose the exact invariants the OpenAgents
sandbox plan exists to enforce.

## Recommended Pathway

### P0: Khala Sync mobile/desktop event contract

Run the Khala Sync/mobile path in parallel with the AI SDK Core lane. The
canonical runtime event/control schema (#8363), desktop append/control bridge
(#8364), and mobile durable Sync session (#8365) are now the base. Next, extend
server-side runtime scopes/policy (#8370). This is the path that makes
mobile-to-desktop Khala Code feel AI SDK-shaped without letting AI SDK own the
product state.

Required tests:

- schema fixtures that cover the OpenAgents event/control contract,
- desktop RPC tests for append/control intent success, rejection, and disabled
  states,
- mobile fake-session tests for durable checkpoint/resume and pending/rejected
  state,
- scope-auth/mutator tests proving runtime content remains owner/thread scoped.

Gate:

- mobile, desktop, and web consume the same OpenAgents runtime event/control
  contract over Khala Sync; no surface persists raw AI SDK stream parts as
  canonical state.

### P0a: opencode-style AI SDK Core runtime

Build a narrow `ai_sdk_core` runtime for Khala before the harness work. It
should call `streamText`, consume `result.stream`, and convert AI SDK
`TextStreamPart`s into a canonical OpenAgents event stream.

Required tests:

- one model-provider fixture that streams text, reasoning if available, usage,
  and finish reason,
- one tool fixture that proves OpenAgents permission/tool policy stays outside
  AI SDK,
- one providerOptions fixture for an OpenAI-family model,
- one raw-chunk fixture proving raw provider data is private or discarded,
- one transcript fixture proving Khala UI consumes OpenAgents events rather
  than AI SDK parts directly.

Gate:

- the existing Codex/Pylon transcript path and the new AI SDK Core model path
  render through the same Khala event consumer.

### P0b: mirror and test the published package code

Add a small internal mirror or patch workspace that can import the exact npm
tarballs, run type checks, and diff the source files we patch. This is not a
product package yet; it is fork hygiene.

Required tests:

- package version drift check,
- bridge dependency lockfile drift check,
- Codex/Claude event-shape fixture check,
- adapter bootstrap hash check.

### P1: local AI SDK sandbox provider spike

Implement a local provider sufficient for a public sum-repair fixture. It can
use a temp directory, local child processes, a reserved localhost WebSocket
port, and optional macOS Seatbelt.

Gates:

- Codex and Claude `HarnessAgent.stream()` both run without Vercel.
- No read/write outside the temp workspace in the fixture.
- Stream parts render through the Khala Code transcript adapter.
- The proof is clearly labeled owner-local and unsafe for untrusted work.

### P2: Claude first, Codex second

Claude should be the first real adapter experiment because its AI SDK adapter
already supports built-in approvals and filtering. Codex should wait until the
OpenAgents sandbox provider can enforce the sandbox profile underneath
`danger-full-access`.

Gates:

- Claude account-home routing through explicit env, not ambient host defaults.
- Claude tool approval/filtering parity with current Pylon bounded runner.
- Exact usage parity with current `/api/pylon/claude/turns` expectations.

### P3: OpenAgents sandbox provider over the sandbox runtime

Build `@openagentsinc/ai-sdk-sandbox-openagents` against the real or fixture-backed
`openagents.sandbox.v1` surface.

Gates:

- create/resume/stop/destroy contract tests,
- WebSocket bridge port through managed preview ingress,
- `.agents/setup` snapshot key proves cache hit/miss behavior,
- network policy and capability-gateway tests,
- public-safe receipt refs only.

### P4: Codex fork with raw/private event side-channel

Fork only what we need in the Codex adapter.

Required deltas:

- update or parameterize `@openai/codex-sdk`,
- expose raw Codex SDK events privately, either as `raw` stream parts gated by
  settings or as an OpenAgents side-channel,
- preserve exact token and failure metadata,
- route account auth through explicit `CODEX_HOME`/short-lived material,
- remove or constrain prompt-mediated host-tool CLI relay in cloud lanes,
- map Codex failures to existing account health/quota ledgers.

Gate:

- run the same public git fixture with current Pylon runner and AI SDK sandbox
  runner, compare usage rows, file-change projection, closeout refs, and typed
  failure behavior.

### P5: upstream extension points

Open upstream issues/PRs for generic needs:

- adapter raw event passthrough,
- bridge package dependency override or fresher pin cadence,
- explicit auth home/env routing hooks,
- Codex built-in tool policy once the Codex SDK supports it,
- documented non-Vercel sandbox provider examples.

The goal is to delete fork patches, not grow them.

## Promotion Gates

Do not promote AI SDK Harnesses beyond experiment until all of these pass.
Promote the AI SDK Core lane only as a transport adapter; the first two gates
are hard requirements for that lane.

| Gate | Required proof |
| --- | --- |
| Core event adapter | AI SDK Core stream parts are converted into OpenAgents events; Khala does not persist AI SDK stream parts as canonical transcript state. |
| Tool authority | AI SDK Core tools re-enter OpenAgents Effect policy and permission checks before side effects. |
| Local non-Vercel proof | Codex and Claude run through `HarnessAgent` in a local provider without Vercel. |
| Sandbox proof | Same fixture runs inside an OpenAgents sandbox/workroom with managed port ingress. |
| Account isolation | `CODEX_HOME` and `CLAUDE_CONFIG_DIR` are selected account homes, never default ambient homes. |
| Raw private archive | Pylon/Khala can retain ordered raw runtime events privately, or the old runner remains the authority for flows needing them. |
| Exact usage | Usage rows match current Codex/Claude ingestion semantics; estimates never move public counters. |
| Workspace boundary | Codex is contained by OpenAgents sandbox policy plus post-hoc file-change validation; Claude retains pre-tool denial or equivalent. |
| Network policy | Public/untrusted lanes cannot rely on adapter settings; egress is enforced by OpenAgents sandbox/workroom profile. |
| SCM credential scan | Workspace and selected account homes are scanned before verification/PR publication. |
| Resume | Detach, suspend, stop, destroy, and resume behave across host process restarts. |
| Product honesty | No public claim that AI SDK harnesses are default until the gates above are receipt-backed. |

## Bottom Line

The feasible upgrade is not "replace Khala Code with AI SDK Harnesses." The
feasible upgrade has three lanes:

1. Make Khala Sync the mobile/desktop transport for OpenAgents runtime events
   and control intents, starting with the filed roadmap issues
   [#8363](https://github.com/OpenAgentsInc/openagents/issues/8363) through
   [#8375](https://github.com/OpenAgentsInc/openagents/issues/8375).
2. Copy opencode's AI SDK Core pattern now: use `streamText` and provider
   packages as a transport layer, but convert every part into OpenAgents-owned
   events.
3. Make OpenAgents sandboxes a first-class AI SDK sandbox provider, then run
   Codex/Claude harness bridges inside that provider where whole-agent runtime
   compatibility helps.

Maintain a fork only at the adapter edge, and only for OpenAgents authority
needs. Use upstream AI SDK core as long as it can stay unmodified. Get the
Core stream adapter working first, get the local unsafe harness provider
working second, move Claude through the OpenAgents sandbox path next, and let
Codex become default only after the OpenAgents sandbox is the real containment
boundary.
