# Codex app-server client support: T3 Code, OpenCode, and OpenAgents

**Date:** 2026-07-15

**Question:** What does each open-source product mean by “Codex support,” how
much of Codex app-server does it actually consume, and what would “full
app-server support” require in OpenAgents?

## Executive conclusion

T3 Code and OpenCode sit on opposite sides of a boundary that product copy
often hides:

- **T3 Code is an app-server host.** It launches Codex app-server, generates a
  large typed client from a pinned upstream schema, and translates a small
  exercised subset into T3's provider-neutral thread model.
- **OpenCode is not an app-server client.** It obtains ChatGPT/Codex OAuth
  credentials and calls the Codex Responses backend from OpenCode's own agent
  engine. OpenCode owns its sessions, prompt loop, tools, permissions, MCP,
  compaction, persistence, steering, and queueing.
- **Neither demonstrates full app-server support.** T3 has broad transport
  declarations but narrow behavioral coverage. OpenCode has zero app-server
  protocol coverage; similar-looking capabilities are independent
  reimplementations.
- **OpenAgents already has a real app-server path**, including start/resume,
  start/steer/interrupt, native skills, selected notifications, approvals,
  user questions, and dynamic tools. It is still a purpose-built workroom
  slice, not a lossless implementation of the whole protocol.

The architectural consequence is important: “the generated type exists,” “the
message parses,” “the provider has an analogous feature,” and “the product
fully supports the app-server capability” are four different claims.
OpenAgents should track those claims separately for every method and event.

## Scope and source snapshots

This is a source audit, not a runtime certification. Counts refer to these
local snapshots:

| Project | Snapshot | Role in this analysis |
| --- | --- | --- |
| OpenAI Codex | `1bbdb32789e1f79932df44941236ea3658f6e965` (2026-07-15) | Current app-server contract and core turn behavior |
| T3 Code | `c1ec1915fc16f3dc1ec5d47d9a97f6210a574526` (2026-07-12) | Generated client, Codex adapter, orchestration, composer |
| T3 generated Codex schema | upstream Codex `b39f943a634a6e7ba86c3d6e8cf6d5f35e612566` (2026-06-10) | The app-server revision T3 says its generated package represents |
| OpenCode | `d3459eb7403cbb33c197621777409954e9a1312f` (2026-07-05) | Codex OAuth/backend integration and OpenCode-owned runtime |
| OpenAgents | `3346167c538c9dee375114dbe31eea2789047e37` plus this documentation change | Existing Desktop app-server client and workroom projection |

The official protocol overview is the
[Codex app-server README](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md).
The Rust protocol definitions are the full count authority; the export logic
and emitted JSON/TypeScript schemas explain which experimental members ordinary
generation omits. [public] [source] [schema]

## 1. “Supports Codex” names three different architectures

```text
T3 Code
UI -> T3 orchestration -> provider-neutral adapter -> codex app-server -> Codex core

OpenCode
UI -> OpenCode session/tool engine -> Responses request -> Codex model backend

OpenAgents Desktop today
UI -> typed workroom/runtime gateway -> codex app-server -> Codex core
```

| Property | T3 Code | OpenCode | OpenAgents today |
| --- | --- | --- | --- |
| Launches `codex app-server` | Yes | No | Yes |
| Speaks app-server JSON-RPC | Yes | No | Yes |
| Uses Codex Thread/Turn/Item engine | Yes, then normalizes it | No | Yes, then projects a selected workroom vocabulary |
| Uses ChatGPT/Codex OAuth | Through ordinary Codex installation/session | Directly in OpenCode's plugin | Through ordinary compatible Codex installation/session |
| Owns the model/tool loop | Codex | OpenCode | Codex |
| Owns queue semantics above the model loop | T3 has no durable user queue | OpenCode V2 | OpenAgents workroom/runtime contracts |
| Claims broad provider neutrality | Yes | Yes | No for this lane; app-server is the explicit engine |

OpenCode is still a valuable comparison for durable admission and host UX, but
it cannot answer whether an app-server notification, approval request, plugin
operation, or thread mutation is correctly handled. It does not receive those
messages. [source]

## 2. App-server is a bidirectional host protocol, not a model API

The current audited protocol exposes three directions plus one client
notification. “Current” needs two counts because Codex's default schema export
intentionally filters experimental request methods while its Rust protocol
source still defines them:

| Direction | Full Codex source | Default emitted TypeScript union | T3 generated | T3 production behavior |
| --- | ---: | ---: | ---: | ---: |
| Client requests | 126 = 90 stable + 36 experimental | 90 | 87 | 11 invoked |
| Client notifications | 1 | 1 | 1 | 1 invoked (`initialized`) |
| Server requests | 11 = 10 stable + 1 experimental | 10 | 10 | 3 handled |
| Server notifications | 72 = 58 stable + 14 experimental | 72 | 67 | 67 generically forwarded; 4 update session state |

This distinction matters because both T3 and OpenAgents advertise
`experimentalApi: true`. A client cannot use the stable-only generated request
union as its definition of complete experimental coverage. [source] [schema]

A host is incomplete if it only sends requests. It must also preserve server
notifications and answer server-initiated requests correctly, including cases
that block a turn. A generic JSON-RPC client proves framing, correlation, and
timeouts; it does not prove method semantics.

The surface is larger than chat:

| Family | Current app-server responsibility |
| --- | --- |
| Initialization/capabilities | Client identity, advertised experimental behavior, version/capability boundary |
| Threads | Start/resume/read/list/fork/archive/unarchive/delete/unsubscribe/name, goals, metadata/settings, compaction, rollback, shell/background processes, injected items |
| Turns | Start, same-turn steer, interrupt, settings and environment selection |
| Items | Typed lifecycle for messages, reasoning, commands, file changes, MCP/dynamic tools, web search, plans, review, collaboration agents, images and other response items |
| Host decisions | Command/file/permission approvals, user questions, dynamic tool calls, MCP elicitation, auth refresh, attestation, legacy approval compatibility |
| Catalogs | Models/providers, skills, plugins, apps/connectors, marketplace, hooks, experimental features, permission profiles |
| MCP | Server reload/status, OAuth, resource reads, tool calls, elicitation |
| Account/config | Login/logout/status, rate limits, usage/reset credit, workspace messages, config read/write and managed requirements |
| Host utilities | Filesystem metadata/read/write/watch/search, command/process lifecycle, fuzzy search, Windows sandbox setup |
| Advanced runtime | Reviews, realtime audio/transcripts/SDP, remote-control/environment status, external-agent history import |

“Full” should mean behavioral coverage of the applicable contract, not that
every dangerous operation is exposed to every user. Protocol completeness and
product authority are separate: a host may decode and explicitly deny an
operation according to policy while still handling it correctly.

### 2.1 The experimental surface hidden by default generation

The 36 source-defined client requests omitted from Codex's ordinary generated
request union are not one speculative feature. They span most of the rich-host
work needed by a “full” client:

- 16 thread methods: elicitation increment/decrement,
  `thread/settings/update`, `thread/memoryMode/set`, three background-terminal
  operations, thread search, turn/item pagination, and six
  `thread/realtime/*` controls;
- seven remote-control methods: enable/disable/status, pairing start/status,
  and client list/revoke;
- `environment/add`, `environment/info`, and `environment/status`;
- `process/spawn`, `process/writeStdin`, `process/kill`, and
  `process/resizePty`;
- three stateful fuzzy-file-search session methods;
- `memory/reset` and `collaborationMode/list`; and
- test-only `mock/experimentalMethod`.

The six realtime controls are start, append audio/text/speech, stop, and voice
listing. The 14 experimental notifications are likewise product-significant:
environment connect/disconnect, thread settings, two process events,
moderation metadata, and eight realtime events. A product may deliberately
exclude an experimental family, but then it should not advertise an
undifferentiated experimental capability and call the default export “all.”
[source] [schema]

## 3. T3 Code: schema-broad, behavior-narrow

### 3.1 The generated package is real, but generated is not implemented

T3's `effect-codex-app-server` generator fetches upstream JSON schema from a
hard-coded Codex commit, runs an Effect generator, and supplements generation
with manual legacy schemas. The generated metadata records upstream ref
`b39f943...`, 908 Codex commits behind the audited HEAD. At the audited
snapshot, the package declares 87 of the current default export's 90 client
request names:

- missing versus current Codex:
  `account/rateLimitResetCredit/consume`,
  `account/workspaceMessages/read`, and
  `externalAgentConfig/import/readHistories`.

No T3-generated method name was removed from the current default union; the
problem is additions and changed schemas, not stale method names alone. T3's
generated server-request names match the stable 10-name export but omit the
experimental `currentTime/read`. Its generated notification set has 67 names
versus current Codex's 72:

- missing current notifications:
  `thread/environment/connected`, `thread/environment/disconnected`,
  `rawResponse/completed`, `externalAgentConfig/import/progress`, and
  `model/safetyBuffering/updated`.

Method-name drift understates the compatibility gap. Since T3's pin, 69 of the
generator input schemas changed (61 modified and 8 added). Direct method-schema
drift affects 26/87 generated client requests, 4/10 server requests, and 17/67
notifications. Seven of T3's eleven invoked client methods have drifted,
including `initialize`, `account/read`, thread start/resume/read/rollback, and
`turn/start`. Two of its three implemented reverse handlers use changed
schemas: current command approval adds required `environmentId`, while current
user input adds `autoResolutionMs`. All four notifications with dedicated T3
session-state logic—`error`, `thread/started`, `turn/started`, and
`turn/completed`—also changed. [source] [schema]

This is ordinary protocol drift, but it proves why a pinned ref and generated
files cannot stand in for a compatibility test against the binary actually
launched. [source] [schema]

### 3.2 T3 invokes only 11 of 90 default / 126 full client methods

The audited product path invokes:

| Family | T3 methods actually called |
| --- | --- |
| Session bootstrap | `initialize`, then `initialized` notification |
| Thread | `thread/start`, `thread/resume`, `thread/read`, `thread/rollback` |
| Turn | `turn/start`, `turn/interrupt` |
| Discovery | `model/list`, `skills/list`, `account/read` |
| MCP maintenance | `config/mcpServer/reload` |

That is **11/90 stable client request methods**, or **11/126** when “all server
stuff” includes the source-defined experimental API T3 advertises. The
generated schema also contains
`turn/steer`, but T3's `ProviderAdapterShape`, orchestration command union, and
composer do not expose it. T3 sends `turn/start` through a manual schema
extension to recover a `collaborationMode` field the generator did not emit,
another example of compile-time coverage diverging from product behavior.
[source] [schema]

The largest unexercised groups include:

- thread fork/archive/unarchive/delete/unsubscribe/name/goal/metadata/settings,
  compaction, shell/background terminal, guardian approval, loaded-thread
  inspection, and item injection;
- review start and explicit `turn/steer`;
- plugin, app/connector, marketplace, and hook catalogs and mutations;
- skills configuration and extra-root management;
- permission profiles, experimental features, and model-provider capabilities;
- MCP OAuth/status/resource/tool operations;
- account login/logout/rate/usage/reset-credit/workspace-message operations;
- filesystem, process/command, fuzzy-search, external-agent import, config,
  managed requirements, and Windows sandbox operations.

### 3.3 T3 answers only 3 of 10 default / 11 full server requests

T3's Codex runtime has concrete handlers for:

- `item/commandExecution/requestApproval`;
- `item/fileChange/requestApproval`; and
- `item/tool/requestUserInput`.

The runtime's unknown-request path returns method-not-found for the other seven
stable/generated requests:

- `mcpServer/elicitation/request`;
- `item/permissions/requestApproval`;
- `item/tool/call`;
- `account/chatgptAuthTokens/refresh`;
- `attestation/generate`;
- legacy `applyPatchApproval`; and
- legacy `execCommandApproval`.

The full source-defined API adds experimental `currentTime/read`, which is not
in T3's generated set and also receives method-not-found. Thus T3 implements
3/10 of the default reverse-RPC surface and 3/11 of the full advertised
experimental surface.

Several can suspend or determine a live turn. Merely decoding their params is
not support; a product that advertises the relevant capability and then sends
method-not-found has a behavioral hole. [source]

### 3.4 T3 listens broadly, then discards semantics at normalization

`CodexSessionRuntime` subscribes to every server notification known by its
generated package and initially emits a raw provider event. Only four also
update T3's session state directly. `CodexAdapter` then maps provider events
into T3's shared runtime vocabulary. The audit found only 38 of 67 generated
notification names represented in that canonical adapter; 29 are not
projected into shared runtime events:

```text
thread/deleted
skills/changed
thread/goal/updated, thread/goal/cleared, thread/settings/updated
hook/started, hook/completed
item/autoApprovalReview/started, item/autoApprovalReview/completed
rawResponseItem/completed
command/exec/outputDelta, process/outputDelta, process/exited
item/fileChange/patchUpdated
mcpServer/startupStatus/updated
app/list/updated, remoteControl/status/changed
externalAgentConfig/import/completed
fs/changed, model/verification, model/safety events at newer revisions
turn/moderationMetadata
warning, guardianWarning
fuzzyFileSearch/sessionUpdated, fuzzyFileSearch/sessionCompleted
thread/realtime/transcript/delta, thread/realtime/transcript/done,
thread/realtime/sdp
account/login/completed
```

Raw events can be written to NDJSON diagnostics, but events that map to an
empty canonical list do not reach T3's shared projections or product UI. Raw
logging is observability, not product support. [source]

The drift failure mode is worse than a visible “unsupported” state. Production
does not install an unknown-notification handler, so the five current methods
absent from T3's generated map are silently dropped. Known-notification decode
and handler failures are wrapped in a blanket catch returning `Effect.void`,
so changed schemas can also disappear silently. By contrast, unhandled server
requests fail promptly with JSON-RPC method-not-found (`-32601`). [source]

Normalization also changes structure. T3 suppresses selected child
conversation lifecycle notifications and attaches child activity to the
parent turn, flattening some of Codex's explicit thread/agent topology. That is
a legitimate provider-neutral design choice, but it prevents a claim of
lossless Codex support. [source] [inferred]

### 3.5 T3's composer reveals why exact turn semantics matter

T3 has no durable follow-up prompt queue. It supports one editable local draft
while a turn runs, swaps the pointer-visible send control for interrupt, and
has a keyboard path that can still issue another `thread.turn.start`. Current
Codex core treats `turn/start` received during an active regular turn as
additional pending input for that active turn. Thus the accidental second send
can act like steering without the public `turn/steer` method's
`expectedTurnId` guard. Other T3 providers need not behave the same way.

The detailed source trace and required queue model are in the
[T3 Code teardown](./2026-07-13-t3-code-teardown.md#41-composer-concurrency-draft-ahead-implicit-codex-steering-and-no-product-queue).

## 4. OpenCode: Codex backend support, zero app-server support

Repo-wide source search at the audited OpenCode commit found no app-server
launch, protocol schema, `@openai/codex` dependency, or app-server method use.
Its path is instead:

```text
OpenCode session engine
  -> Vercel AI SDK or OpenCode native Responses implementation
  -> OAuth request rewrite
  -> https://chatgpt.com/backend-api/codex/responses
```

### 4.1 What OpenCode actually borrows

The built-in OpenAI plugin implements PKCE/device OAuth using the Codex CLI's
simplified flow, refreshes and stores tokens in OpenCode's auth store, removes
the SDK authorization header, adds its bearer token and
`ChatGPT-Account-Id`, and rewrites Responses/chat-completions URLs to the
ChatGPT Codex Responses endpoint. An integration test asserts that rewrite and
those headers. Optional OpenCode-owned WebSocket code makes Responses sessions
persistent with HTTP fallback. [source] [test]

OpenCode model IDs containing `codex` select an OpenCode-owned Codex-style
system prompt. That is model/prompt compatibility, not reuse of the Codex
runtime. [source]

### 4.2 What OpenCode owns instead

OpenCode independently owns:

- the assistant/tool continuation loop and reasoning replay;
- session and message persistence in its SQLite schema;
- tool definitions, validation, execution, output bounding, and settlement;
- allow/ask/deny permissions and persisted “always allow” rules;
- MCP clients, OAuth, tools, prompts, and resources;
- compaction and context assembly; and
- V2 durable prompt admission, where `steer` inputs promote at safe boundaries
  and queued inputs promote one at a time when the session is idle.

The last item is a useful OpenAgents design reference precisely because it is
**above** a provider. It is not an implementation of Codex `turn/steer` or an
app-server queue. [source]

### 4.3 What OpenCode therefore does not inherit

OpenCode has no app-server coverage for initialization/capability negotiation;
Thread/Turn/Item requests or events; Codex rollout persistence; thread
resume/fork/archive/rollback/compaction; Codex approvals, sandbox, permission
profiles, or command/process services; Codex MCP lifecycle; skills/plugins/apps
and marketplace state; hooks; account/config APIs; realtime; collaboration
topology; or filesystem/external-agent import utilities.

OpenCode has analogues for some of these. They remain OpenCode contracts with
OpenCode behavior. Feature-name similarity is not wire or semantic
compatibility. [source] [inferred]

## 5. Comparative support matrix

The following uses deliberately strict labels:

- **generated**: a schema/type exists;
- **called/handled**: runtime behavior exists;
- **analogue**: the product independently implements a similar concept;
- **none**: no app-server relationship.

| Protocol area | T3 Code | OpenCode | OpenAgents current workroom slice |
| --- | --- | --- | --- |
| JSON-RPC lifecycle | Generated and used | None | Hand-written client, used |
| Thread create/resume | Used | Own sessions | Used |
| Thread read/list/fork/archive/delete/settings/goals/compaction | Mostly generated only; read used | Own analogues for a subset | Not in primary client path |
| Turn start | Used | Own Responses loop | Used |
| Turn steer | Generated; not product-wired; second start can implicitly steer Codex | Own durable V2 steer | Used with `expectedTurnId` |
| Turn interrupt | Used | Own cancellation | Used |
| Durable next-turn queue | No | Own V2 queue | OpenAgents-owned admission concern, not an app-server method |
| Item lifecycle | Selected normalized projection | Own events | Selected typed workroom projection |
| Command/file approvals | Handled | Own permission engine | Handled and surfaced |
| User questions | Handled | Own mechanism | Handled and surfaced |
| Permission approval / MCP elicitation / auth refresh / attestation | Generated, not handled | None; own unrelated flows | Mostly explicit unsupported/decline today |
| Dynamic tool server requests | Generated, not handled | Own tools | ProductSpec dynamic tools handled |
| Skills | List used | Own instruction/agent system | List plus extra roots/config used |
| Plugins/apps/marketplace/hooks | Generated, not exercised/projected completely | Own plugin system, not Codex's | Not in primary client path |
| MCP lifecycle | Reload only | Own MCP stack | Not in primary client path |
| Account/model/config | Account read and model list only | Own provider/auth config | Ordinary Codex session; broad APIs not projected |
| Filesystem/process/realtime/Windows sandbox | Generated, largely dropped | Own host implementations | Not in primary client path |
| Lossless native event journal | Optional raw NDJSON diagnostics, not product projection | Not applicable | No complete raw protocol journal yet |
| Explicit tested protocol revision | Generated package pinned, runtime compatibility can drift | Not applicable | Compatible-runtime checks exist, but client is not generated from the full schema |

## 6. OpenAgents' current starting point

The Desktop app-server client is intentionally small and workroom-oriented. It
spawns `codex app-server`, performs `initialize`/`initialized`, correlates
numeric JSON-RPC responses with timeouts, routes notifications to listeners,
and dispatches server requests. The main path currently calls:

```text
initialize
skills/extraRoots/set
skills/config/write
skills/list
thread/start or thread/resume
turn/start
turn/steer
turn/interrupt
```

It projects selected notifications including thread start, agent-message
deltas, token usage, plan updates, item start/completion, errors, and turn
completion. It surfaces command/file approvals and user questions, and routes
selected `item/tool/call` requests into product-owned dynamic tools. Its
fail-closed helper has typed negative responses for approvals, user input, MCP
elicitation, and dynamic tool calls, while unknown server requests error.
[source] [test]

This is stronger than T3 on explicit steering and product dynamic tools, and
less broad than T3's generated transport declaration. The client still uses
`Record<string, unknown>` at the wire boundary and string comparisons in the
projection. Most current request families and notifications have no product
path. Calling this “full Codex app-server support” today would be inaccurate.
[source]

That is not a defect in the accepted workroom scope. It is the baseline from
which a full-support program must be measured.

## 7. A rigorous definition of full support

For each upstream method or notification, record these independent states:

| Dimension | Proof required |
| --- | --- |
| Known | Present in a machine-generated manifest for the exact supported Codex revision |
| Decoded | Params/result/event pass generated runtime validation without `unknown` casts |
| Transported | Request correlation, cancellation, timeout, and reconnect behavior are tested |
| Handled | Server requests receive method-correct responses on every policy branch |
| Journaled | Native envelopes and causal IDs survive process/UI restart where required |
| Projected | Every semantic field is retained in a native projection, even if portable UI omits it |
| Presented | A user/operator can inspect and act on the capability where product policy allows |
| Authorized | Authority is typed, scoped, and defaults safely; dangerous support is not silent enablement |
| Repaired | Resume/reconnect reconciles app-server truth with host truth without duplication |
| Verified | Fixture and compatible real-binary tests cover success, rejection, interruption, crash, and drift |

“Not applicable by product policy” can be a valid Presented state. It cannot be
used for Decoded, Handled, or Journaled when the server can still emit the
message.

### Support tiers

1. **Protocol kernel:** generated request/result/server-request/notification
   types; exact upstream ref; binary/schema compatibility handshake; unknown
   message quarantine rather than silent discard.
2. **Bidirectional safety:** all server-request methods answered correctly,
   including explicit denial and capability-dependent methods.
3. **Lossless Codex model:** native Thread/Turn/Item graph, child topology,
   item variants, usage, settings, warnings, moderation, environments, and
   process state preserved before normalization.
4. **Control plane:** start/resume/read/list/fork/archive/rollback/compact,
   start/steer/interrupt/review, goals/settings/metadata, and exact receipt
   behavior.
5. **Capability plane:** models, permissions, config requirements, skills,
   plugins, apps, marketplace, hooks, MCP, account, and experimental features.
6. **Host services:** filesystem, process/command, search, realtime, external
   history import, remote-control/environment state, and platform-specific
   sandbox setup.
7. **Product completeness:** cross-surface UI, attention, recovery, audit,
   authority, and tested degradation for all enabled capabilities.

## 8. Recommended OpenAgents architecture

### 8.1 Keep a lossless native plane beside the portable product plane

Do not force every Codex event directly into a least-common-denominator agent
event. Preserve the decoded native envelope first:

```text
Codex JSON-RPC
      |
      v
generated protocol decoder + compatibility gate
      |
      +--> lossless Codex envelope journal/projection
      |        thread / turn / item / request / notification identity
      |
      +--> portable OpenAgents projection
               workroom timeline / questions / approvals / graph / receipts
```

T3 shows the failure mode: broad subscription followed by normalization to an
empty event list makes capabilities disappear. OpenAgents should be able to
add a future UI projection from retained native state without replaying raw
process logs or rerunning a task.

### 8.2 Generate the protocol and test it against the shipped binary

- Generate TypeScript/Effect schemas and method manifests from a pinned Codex
  app-server revision.
- Record the compatible Codex binary hash/version range in the signed app
  release.
- In CI, diff current supported schemas against upstream and require every
  added/removed method to receive an explicit support disposition.
- At startup, fail with a precise incompatible-runtime state if the binary is
  outside that tested set. Advertising `experimentalApi: true` must be tied to
  actual handler coverage.
- Keep narrow manual extensions only as reviewed, tested compatibility patches
  that name the upstream generator gap.

### 8.3 Give queueing and steering different owners

App-server provides same-turn `turn/steer`, guarded by `expectedTurnId`, and
separate interruption. It does not provide the durable product FIFO needed for
“run this after the current task.” OpenAgents should retain that queue above
Codex:

- `steer`: admitted against an exact active provider turn and translated to
  `turn/steer`;
- `queue`: durable OpenAgents intent promoted to `turn/start` only after the
  previous turn has a quiescence receipt;
- retry/reconnect: idempotent through stable client user-message and queue IDs;
- UI: draft, steering input, and queued future turn are visibly different.

OpenCode V2 is the better reference for admission/promotion. Codex app-server
is the authority for active-turn execution. T3's accidental second-start path
should not be copied.

### 8.4 Make server requests a first-class inbox

All 10 stable server-request methods—and experimental `currentTime/read` while
OpenAgents advertises the experimental API—need one of:

- a live owner workflow;
- an automatically authorized response under an explicit grant;
- a method-correct fail-closed response; or
- an initialization capability that guarantees the server will not issue it.

Unknown requests must produce an inspectable incompatibility receipt. They
must not hang the turn, receive a response shape borrowed from another method,
or vanish in logs.

### 8.5 Separate implementation completeness from exposure policy

Filesystem writes, command execution, account changes, marketplace/plugin
installation, auth refresh, and sandbox mutation are security-sensitive. Full
protocol support means OpenAgents can decode, authorize, execute or deny, and
audit them correctly. It does not mean every renderer receives those buttons
or every workspace grant allows the operation.

## 9. Suggested machine-readable gap matrix

The app should generate and test a row like this for every protocol member:

```ts
type CodexMethodSupport = {
  readonly method: string
  readonly upstreamRef: string
  readonly direction: "client_request" | "server_request" | "notification"
  readonly stability: "stable" | "experimental" | "legacy"
  readonly decode: "generated" | "manual" | "missing"
  readonly handler: "implemented" | "deny" | "not_applicable" | "missing"
  readonly nativeProjection: "lossless" | "partial" | "missing"
  readonly productSurface: readonly ("desktop" | "web" | "mobile" | "operator")[]
  readonly authorityRef: string | null
  readonly fixture: string | null
  readonly realBinaryProof: string | null
}
```

Release gates should derive from this manifest, not from a README checklist.
Useful aggregate numbers are then honest: request transport coverage,
server-request handling coverage, lossless notification coverage, presented
capability coverage, and real-binary verification coverage are separate
percentages.

## 10. Ordered path from the current workroom slice

1. Generate the exact current protocol package and a checked-in method/event
   manifest; add drift CI against the supported Codex ref.
2. Wrap the existing client in generated decoding while preserving its process
   supervision, timeout, and test fixture behavior.
3. Add a lossless native envelope journal and complete Thread/Turn/Item graph;
   project the existing workroom events from it.
4. Complete all server-request handlers before enabling more advertised
   capabilities.
5. Land lifecycle/control requests: read/list/fork/archive/unarchive/delete,
   goals/settings/metadata, compact/rollback/review, and process state.
6. Land capability catalogs and mutations in authority-sized groups: account
   and model; config/requirements/permissions; skills/plugins/apps/marketplace;
   MCP/hooks.
7. Add host utility and advanced-runtime groups with platform-specific grants
   and receipts.
8. Prove restart/reconnect, unknown-version, unknown-message, interrupted
   approval, queue promotion, and duplicate-client-message cases against both
   the fixture and the exact compatible real binary.

## Primary source map

### Codex

- `codex-rs/app-server/README.md`
- `codex-rs/app-server-protocol/schema/json/ClientRequest.json`
- `codex-rs/app-server-protocol/schema/json/ServerRequest.json`
- `codex-rs/app-server-protocol/schema/json/ServerNotification.json`
- `codex-rs/app-server-protocol/src/protocol/common.rs`
- `codex-rs/app-server-protocol/src/export.rs`
- `codex-rs/app-server/src/request_processors/turn_processor.rs`
- `codex-rs/core/src/session/handlers.rs`
- `codex-rs/core/src/session/input_queue.rs`

### T3 Code

- `packages/effect-codex-app-server/scripts/generate.ts`
- `packages/effect-codex-app-server/src/_generated/meta.gen.ts`
- `packages/effect-codex-app-server/src/_generated/schema.gen.ts`
- `apps/server/src/provider/Layers/CodexSessionRuntime.ts`
- `apps/server/src/provider/CodexAdapter.ts`
- `packages/contracts/src/provider.ts`
- `packages/contracts/src/orchestration.ts`
- `apps/web/src/components/ChatView.tsx`
- `apps/web/src/components/chat/ChatComposer.tsx`
- `apps/web/src/components/chat/ComposerPrimaryActions.tsx`

### OpenCode

- `packages/opencode/src/plugin/openai/codex.ts`
- `packages/opencode/test/plugin/codex.test.ts`
- `packages/opencode/src/session/llm.ts`
- `packages/opencode/src/session/prompt.ts`
- `packages/opencode/src/session/system.ts`
- `packages/opencode/src/session/prompt/codex.txt`
- `packages/llm/src/protocols/openai-responses.ts`
- `packages/core/src/session/input.ts`
- `packages/core/src/session/runner/llm.ts`
- `packages/core/src/permission.ts`
- `packages/opencode/src/mcp/index.ts`

### OpenAgents

- `apps/openagents-desktop/src/codex-app-server-client.ts`
- `apps/openagents-desktop/src/codex-app-server-turn.ts`
- `apps/openagents-desktop/src/codex-local-runtime.ts`
- `apps/openagents-desktop/src/product-spec-app-server-tools.ts`
- `apps/openagents-desktop/src/codex-app-server-client.test.ts`
- `apps/openagents-desktop/src/codex-app-server-smoke-fixture.test.ts`

## Final product judgment

T3 proves that wrapping Codex app-server behind a multi-provider host is
practical, but also that schema generation can create an illusion of breadth
while product normalization and missing reverse-RPC handlers discard much of
the protocol. OpenCode proves that Codex-authenticated model access can power a
fully independent open agent engine, but it says nothing about app-server
parity.

OpenAgents should take neither project's “Codex support” label at face value.
The right target is a generated, bidirectional, lossless, version-gated Codex
protocol plane; a separately typed OpenAgents authority and durable-admission
plane; and explicit projections that make every supported capability
inspectable without allowing the provider protocol to become product policy.
