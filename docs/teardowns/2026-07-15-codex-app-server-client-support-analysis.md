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
  start/steer/interrupt, selected notifications, approvals, and user questions.
  Skill registration and ProductSpec dynamic-tool code exist and are tested,
  but production deliberately disables that path. The result is a purpose-built
  workroom slice, not a lossless implementation of the whole protocol.
- **Literal 100% parity requires replacing the current per-turn app-server
  process before adding feature wrappers.** OpenAgents currently calls 6 of 87
  stable-schema / 90 ungated-runtime / 126 full request methods, recognizes 8 of 72 notification names
  only partially, and has no generated 11-method reverse-RPC registry. The
  ordered program is: protocol manifest → long-lived supervisor → generated
  decoding/native event plane → reverse-RPC safety → account/policy control
  plane → thread/history repair → full turn/item fidelity → ecosystem → privileged and
  experimental runtime surfaces.

The architectural consequence is important: “the generated type exists,” “the
message parses,” “the provider has an analogous feature,” and “the product
fully supports the app-server capability” are four different claims.
OpenAgents should track those claims separately for every method and event.

### Implementation status: CAP-00

CAP-00 now establishes `packages/codex-app-server-protocol` as that tracking
authority. Like T3 Code, it programmatically transforms pinned Codex JSON
Schema into generated Effect schemas and method maps; unlike T3's single
snapshot, it keeps current source and Desktop's exact bundled `0.144.1`
executable in separate manifests. The current-source ledger accounts for
126/1/11/72 runtime members and the 87 generated + 3 deprecated compatibility
+ 36 gated request partition. The bundled ledger records its own
125/1/11/69 denominator and the reviewed Darwin arm64 executable SHA-256.

The JSON/TypeScript generator asymmetry is explicit: the three deprecated
requests and runtime-only raw-response notifications are compatibility entries,
not silently borrowed from another artifact. CI verifies generated digests,
refs, counts, duplicate-free inventories, and every member's disposition.
Desktop now advertises `experimentalApi: false` and rejects an executable whose
target/version/hash tuple has no matching manifest before returning it to a
thread-start consumer. This is the protocol foundation only; handler and native
projection fields intentionally remain pending for the sequenced CAP issues.

### Implementation status: CAP-01

CAP-01 replaces production's one-process-per-turn ownership with a host-owned
`CodexAppServerSupervisor`. Its pool identity includes the reviewed executable
hash, effective account/`CODEX_HOME`, and host target. Preflight and ordinary
Desktop work share this supervisor; turn leases release their listeners and
reverse routes while the initialized app-server remains alive between turns.

Each connection generation installs all 11 bundled reverse-request methods
before its one initialize handshake. Safe methods have typed deny fallbacks;
methods without a valid deny-shaped result fail as JSON-RPC errors. Active
thread/turn IDs route approvals and questions to the owning lease. Writes are
byte-bounded and requests support cancellation, timeout, overload, malformed
stream shutdown, and bounded stderr evidence.

Unexpected transport close immediately publishes degraded/repairing state,
increments the generation before replacement, rejects stale notifications,
reinitializes, and resumes registered non-ephemeral visible threads. The
failed RPC is never retained or replayed. A real bundled-0.144.1 smoke proves
two simultaneous thread starts over one initialized generation; deterministic
fault tests cover stale-generation fencing, bounded reconnect, resumption,
reverse routing, and idempotent shutdown.

### Implementation status: CAP-02

CAP-02 adds a second generated artifact beside each reviewed Effect schema: a
small runtime JSON Schema document for every response, reverse request, and
notification. Desktop enables strict decoding on its production supervisor,
so semantic provider payloads cannot reach the portable workroom projection
until the generated boundary accepts them. The current fixture corpus replays
all 72 notifications and all 18 `ThreadItem` variants; the bundled real-binary
smoke runs with strict decoding enabled.

Accepted messages first enter a private `CodexNativeEnvelope` plane carrying
the exact connection generation, request ID, thread ID, turn ID, item ID,
method, and decoded payload. Existing Fable events are then derived from that
same decoded payload, preserving current timeline/tool/plan/usage/child-agent
behavior. Final `item/completed` state wins over starts and deltas while every
known but unpresented envelope remains privately queryable.

Durable disk state contains only bounded causal identity, item type, and
status. Stream deltas, process/audio output, and raw-response events use a
smaller in-memory transient spool; raw provider text, workspace content,
absolute paths, auth/MCP/attestation material, and credentials are never
journaled. Decode failures and unknown future methods are quarantined as a
deduplicated compatibility receipt and projected as one bounded visible lane
notice. Reopening the journal restores enough exact IDs and terminal status to
reconcile without inventing completion.

## Scope and source snapshots

This is a source audit, not a runtime certification. Counts refer to these
local snapshots:

| Project | Snapshot | Role in this analysis |
| --- | --- | --- |
| OpenAI Codex | `1bbdb32789e1f79932df44941236ea3658f6e965` (2026-07-15) | Current app-server contract and core turn behavior |
| T3 Code | `c1ec1915fc16f3dc1ec5d47d9a97f6210a574526` (2026-07-12) | Generated client, Codex adapter, orchestration, composer |
| T3 generated Codex schema | upstream Codex `b39f943a634a6e7ba86c3d6e8cf6d5f35e612566` (2026-06-10) | The app-server revision T3 says its generated package represents |
| OpenCode | `d3459eb7403cbb33c197621777409954e9a1312f` (2026-07-05) | Codex OAuth/backend integration and OpenCode-owned runtime |
| OpenAgents | `2f9a8ee9780a2f62b92afbd042fcb4e2fb299a6b` plus this documentation change | Existing Desktop app-server client and workroom projection; bundled Codex dependency `0.144.1` |

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
| Owns queue semantics above the model loop | T3 has no durable user queue | OpenCode V2 | OpenAgents runtime contracts; the direct local Codex implementation is still in-memory |
| Claims broad provider neutrality | Yes | Yes | No for this lane; app-server is the explicit engine |

OpenCode is still a valuable comparison for durable admission and host UX, but
it cannot answer whether an app-server notification, approval request, plugin
operation, or thread mutation is correctly handled. It does not receive those
messages. [source]

## 2. App-server is a bidirectional host protocol, not a model API

The current audited protocol exposes three directions plus one client
notification. “Current” needs three counts because runtime declarations,
method gating, and generated stable schemas are not identical:

| Direction | Full runtime declarations | Ungated at runtime | Stable emitted TypeScript union | T3 generated | T3 production behavior |
| --- | ---: | ---: | ---: | ---: | ---: |
| Client requests | 126 | 90 | 87 | 87 | 11 invoked |
| Client notifications | 1 | 1 | 1 | 1 | 1 invoked (`initialized`) |
| Server requests | 11 | 10 | 10 | 10 | 3 handled |
| Server notifications | 72 | 58 | 72 | 67 | 67 generically forwarded; 4 update session state |

The 126 client declarations are 90 ungated plus 36 method-gated requests. The
stable generator emits only 87 because it also strips three deprecated but
runtime-accepted v1 methods: `getConversationSummary`, `gitDiffToRemote`, and
`getAuthStatus`. Notification generation behaves differently: the stable
notification union still contains all 72 variants even though transport
filtering gates 14 of them. A generated union is therefore not itself a
negotiated capability manifest. [source] [schema]

This distinction matters because T3 advertises `experimentalApi: true`; before
CAP-00 OpenAgents did too, but it now fails closed with `experimentalApi: false`.
A client cannot use the stable-only generated request
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

### 2.1 The gated surface hidden by stable generation

The 36 method-gated client requests omitted from Codex's stable generated
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
undifferentiated experimental capability and call the stable export “all.” A
literal runtime-parity package must also add explicit compatibility schemas
for the three deprecated ungated v1 requests that the generator intentionally
omits. [source] [schema]

## 3. T3 Code: schema-broad, behavior-narrow

### 3.1 The generated package is real, but generated is not implemented

T3's `effect-codex-app-server` generator fetches upstream JSON schema from a
hard-coded Codex commit, runs an Effect generator, and supplements generation
with manual legacy schemas. The generated metadata records upstream ref
`b39f943...`, 908 Codex commits behind the audited HEAD. Both T3 and the current
stable export contain 87 client request names, but their sets differ:

- missing versus current Codex:
  `account/rateLimitResetCredit/consume`,
  `account/workspaceMessages/read`, and
  `externalAgentConfig/import/readHistories`; and
- retained by T3 but intentionally removed from the current stable generator:
  `getConversationSummary`, `gitDiffToRemote`, and `getAuthStatus`.

Those three remain accepted by current runtime compatibility paths. T3's
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

### 3.2 T3 invokes only 11 of 87 stable-schema / 90 ungated / 126 full client methods

The audited product path invokes:

| Family | T3 methods actually called |
| --- | --- |
| Session bootstrap | `initialize`, then `initialized` notification |
| Thread | `thread/start`, `thread/resume`, `thread/read`, `thread/rollback` |
| Turn | `turn/start`, `turn/interrupt` |
| Discovery | `model/list`, `skills/list`, `account/read` |
| MCP maintenance | `config/mcpServer/reload` |

That is **11/87 stable-schema methods**, **11/90 ungated runtime methods**, or
**11/126** when “all server stuff” includes the gated API T3 advertises. The
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

### 3.3 T3 answers only 3 of 10 stable / 11 full server requests

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
3/10 of the stable reverse-RPC surface and 3/11 of the full advertised
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
| Durable next-turn queue | No | Own V2 queue | OpenAgents-owned admission contract, but the direct local Codex queue is currently in-memory |
| Item lifecycle | Selected normalized projection | Own events | Selected typed workroom projection |
| Command/file approvals | Handled | Own permission engine | Handled and surfaced |
| User questions | Handled | Own mechanism | Handled and surfaced |
| Permission approval / MCP elicitation / auth refresh / attestation | Generated, not handled | None; own unrelated flows | Mostly explicit unsupported/decline today |
| Dynamic tool server requests | Generated, not handled | Own tools | Dormant ProductSpec-only code; production returns unsupported |
| Skills | List used | Own instruction/agent system | Dormant registration code; production does not call the skills API |
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
and dispatches server requests. The production path currently calls:

```text
initialize
thread/start or thread/resume
turn/start
turn/steer
turn/interrupt
```

It projects selected notifications including thread start, agent-message
deltas, token usage, plan updates, item start/completion, errors, and turn
completion. It surfaces command/file approvals and user questions. Skill
registration and ProductSpec dynamic-tool scaffolding exist in source/tests,
but `productSpecEnabled()` is hard-coded false in production. The production
server-request callback also bypasses the low-level client's method-specific
fallback declines, so every request other than those three becomes a generic
JSON-RPC `-32000` unsupported error. [source] [test]

This is stronger than T3 on explicit steering and less broad than T3's
generated transport declaration. The client still uses
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

Install handlers for all 11 source-defined server-request methods, including
`currentTime/read` even when experimental advertising is off because the
audited server can still route it. Every method needs one of:

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

## 10. Assume the target is literal 100% app-server parity

Under that assumption, the target is not “everything needed for chat.” It is:

- every one of the **126 source-defined client requests** is known and typed;
- all **87 stable-export requests**, all **36 method-gated requests**, and the
  **3 deprecated ungated compatibility requests** can be issued through one
  typed client;
- the sole `initialized` client notification is correct;
- all **11 server-initiated requests** have method-correct, policy-correct,
  timeout-safe handlers;
- all **72 server notifications** decode and reach a native consumer or an
  explicit bounded sink—none disappears because a switch lacks a case;
- every `ThreadItem`, item delta, lifecycle transition, and child-thread edge
  remains reconstructable;
- every non-test capability has an OpenAgents product/authority disposition;
  and
- compatibility is proven against the exact app-server binary shipped or
  accepted by the app.

The source-defined request count includes experimental
`mock/experimentalMethod`. Literal protocol parity means it remains generated
and fixture-tested; it does **not** mean shipping a user-facing mock button.
Deprecated methods likewise need compatibility adapters and tests, not new
first-class UI. Product parity applies to the 125 non-test request methods;
wire parity applies to all 126. [source] [schema]

This is still **app-server parity**, not Codex TUI parity. If a Codex TUI
behavior has no app-server operation, it remains an upstream protocol gap or a
clearly separate OpenAgents feature. The historical
[`docs/khala-code/2026-07-01-codex-app-server-gap-matrix.md`](../khala-code/2026-07-01-codex-app-server-gap-matrix.md)
identified examples such as richer TUI preference pickers and side-agent
controls. Those do not belong in the denominator until app-server exposes a
contract.

### 10.1 “100%” has three gates

| Gate | Required result |
| --- | --- |
| Wire parity | 126/126 client requests, 1/1 client notifications, 11/11 server requests, and 72/72 notifications are generated, decoded, and compatibility-tested |
| Semantic parity | Request lifecycle, response meaning, reverse-RPC decisions, subscriptions, Thread/Turn/Item state, and restart repair match app-server behavior |
| Product parity | Every non-test capability is either surfaced with typed authority or intentionally unavailable under an explicit product policy; no supported operation exists only as a generic raw-RPC escape |

A raw “send any method” console can help development, but it satisfies none of
these gates by itself.

Do not mistake “stable” for “safe” when ordering the work. Some of the largest
authority surfaces are ungated: `thread/shellCommand`, absolute-path `fs/*`
mutation, persistent command-policy amendments, session permission grants,
thread delete/rollback, model-visible item injection, config/plugin/account
mutation, MCP calls, and auth-token refresh. Experimental `process/spawn` adds
another intentionally unsandboxed path, but it is not the first security
boundary the program encounters.

## 11. What OpenAgents is missing today

### 11.1 The current coverage numbers

The present Desktop path has a compact, useful workroom implementation:

| Direction | Current OpenAgents behavior | Strict 100%-parity status |
| --- | --- | --- |
| Client requests | Production calls 6 names: `initialize`; `thread/start`, `thread/resume`; and all 3 `turn/*` methods | 6/87 stable schema; 6/90 ungated runtime; 6/126 full |
| Client notifications | Sends `initialized` | 1/1 |
| Server requests | Product handlers for command approval, file approval, and user input; production disables ProductSpec dynamic tools and bypasses the low-level fallback decline helper | 3/10 ungated handled; 0/1 gated; the other 8 full-surface methods return generic unsupported errors |
| Server notifications | String-switch recognizes 8 names: `thread/started`, `thread/tokenUsage/updated`, `turn/plan/updated`, `turn/completed`, `item/started`, `item/completed`, `item/agentMessage/delta`, and `error` | 8/72 recognized, all partially projected; 0/72 losslessly retained as a complete native stream |
| Item types | Selected projection for agent/reasoning messages, command/file/MCP/dynamic/web/collaboration activity, and child state | Several item types and nearly all item-specific deltas are omitted |
| Wire schemas | `Record<string, unknown>` plus manual field tests | 0/126 generated request/result contracts; 0/11 generated reverse-RPC contracts; 0/72 generated notification contracts |

The denominator is deliberately strict. A notification does not count as
semantically complete merely because its method string appears in a branch.
For example, root `thread/started` is not retained as complete thread state,
and `item/started`/`item/completed` project only selected item fields.

There are also **two version denominators** that must not be blended. The
126/11/72 target above is current Codex source at the pinned audit commit.
OpenAgents Desktop currently bundles Codex `0.144.1`, whose protocol snapshot
has 90 ungated plus 35 method-gated client requests and 58 ungated plus 12
method-gated notifications. The first executable release gate is 100% of the
exact bundled binary; upgrading that binary and regenerating the manifest is a
separate, explicit change. Otherwise a dashboard can falsely report progress
by testing schemas from one revision against a different executable.

### 11.2 The process lifetime prevents parity

`runCodexAppServerTurn` documents and implements **one app-server process per
active turn**. It initializes, starts or resumes one thread, runs one turn, and
closes the client in `finally`. The persisted provider thread id allows a later
process to call `thread/resume`, but the connection itself is not durable.
[source]

That lifecycle cannot faithfully host capabilities whose state outlives one
turn or whose notifications arrive while no turn is active:

- thread subscriptions/status/name/archive/delete/goal/settings events;
- background terminals and standalone process or command streams;
- filesystem watches and fuzzy-search sessions;
- browser/device login and MCP OAuth completion;
- plugin installation, app catalog, hooks, skill changes, and external import
  progress;
- remote-control enrollment/status and remote environment connections;
- realtime audio/transcript sessions; and
- account, model safety, warning, and configuration notifications.

The first implementation change therefore cannot be “add the other 120
request wrappers.” They would be wrappers around a process that exits before
many operations can complete.

Reconnect is repair, not replay. Subscriptions are connection-local, and the
protocol provides no notification acknowledgement, sequence number, or replay
cursor. A replacement connection must initialize again, resume every visible
thread, restore subscriptions, read durable thread/turn/item state, and then
reconcile it with the native journal. Pending server requests can be replayed
on resume and must be deduplicated by request identity. Transient deltas may be
irrecoverable; filesystem watches, command/process sessions, and similar host
resources are connection-owned and must be explicitly recreated or closed.
Ephemeral threads cannot promise the same repair semantics as persisted ones.

### 11.3 The transport is framing-correct but not a protocol kernel

The hand-written client provides numeric request correlation, one fixed
30-second timeout, JSONL parsing, notification listeners, reverse-request
dispatch, and child-process closure. Missing for parity:

- generated params, results, errors, server requests, and notifications;
- capture of the `initialize` result (`userAgent`, `codexHome`, platform
  identity) and an explicit supported-binary/schema decision;
- stable and experimental generated artifacts (`generate-ts --experimental`
  or equivalent) tied to the same Codex build;
- method-specific timeout/cancellation policies for login, import, process,
  OAuth, and realtime operations;
- JSON-RPC error-code/data preservation instead of reducing every failure to a
  message string;
- bounded write/backpressure handling and overload retry for WebSocket mode;
- listener isolation, unknown-message quarantine, and observable decode or
  handler failures;
- transport authentication and connection ownership for WebSocket/Unix remote
  app-server targets; and
- restart supervision with generation IDs so stale responses/events cannot
  mutate the replacement runtime.

The current client advertises `experimentalApi: true` while possessing none of
the 36 experimental request bindings and only partial knowledge of
experimental notifications/fields. During migration, OpenAgents should either
turn that flag off or gate it on the generated experimental compatibility
manifest. [source] [schema]

### 11.4 Native state is projected too early and retained too narrowly

The app-server turn adapter immediately converts selected native events into
the smaller `FableLocalEvent` vocabulary. The local turn journal retains
OpenAgents thread/turn refs, provider session ref, bounded assistant text,
phase, cursor, and recovery disposition. It does not retain the native
Thread/Turn/Item graph or the app-server envelope stream. [source]

Consequences:

- restart repair cannot prove which native items completed before disconnect;
- unknown/new notification fields cannot be projected later;
- command output, file patch updates, plan/reasoning deltas, review markers,
  warnings, model reroutes, and moderation/safety state can disappear;
- child topology is reconstructed from selected collaboration items instead
  of retained from every native thread edge and lifecycle event; and
- app-server thread truth and OpenAgents workroom truth cannot be reconciled by
  exact IDs/cursors after a crash.

The existing history UI reads Codex rollout/session files through
`codex-history.ts`; the active app-server client does not use `thread/list`,
`thread/read`, or experimental turn/item pagination. For 100% app-server
parity, app-server must become the primary history/lifecycle API. Rollout-file
parsing can remain a bounded repair or migration input, not the normal product
contract.

### 11.5 Missing client-request families

| Family | Current OpenAgents app-server use | Missing for parity |
| --- | --- | --- |
| Initialization | `initialize`; ignores typed result; advertises experimental unconditionally | Exact client version, result projection, capability negotiation, notification opt-out policy, attestation/form-elicitation capabilities, version/hash gate |
| Thread (36 methods) | `thread/start`, `thread/resume` | Fork, list/search/read/loaded list, turn/item pagination, metadata/settings/memory mode, name, goal, archive/unarchive/delete/unsubscribe, compact, rollback compatibility, item injection, shell command, guardian approval, elicitation counters, background terminals, realtime lifecycle |
| Turn (3 methods) | All three called | Complete input variants/settings/environments/permissions/output schema; exact turn state, idempotency, reconnect, review/compact steer rejection, terminal receipts |
| Review | None | `review/start`, inline/detached review state and review item projection |
| Models/features/permissions | None | Model catalog and provider capabilities, experimental feature list/mutation, collaboration modes, permission profiles |
| Account (9 methods) | None | Read, login/cancel/logout, rate limits, usage, reset credits, workspace messages, owner nudge; login completion projection |
| Config/requirements | None | Read, single/batch write, requirements projection, managed-policy enforcement, MCP reload |
| Skills/hooks | All three skill calls exist only in a dormant ProductSpec registration path | Production skill enablement, hooks list, skill-change projection, complete skill roots/config UX and authority |
| Plugins/apps/marketplace | None | Catalog/read/install/uninstall/share operations, marketplace add/remove/upgrade, app list/update, auth-required states |
| MCP | None | Status, OAuth, resource read, tool call, reload, startup progress, OAuth completion, elicitation |
| Filesystem (9 methods) | None | Read/write/create/metadata/list/remove/copy/watch/unwatch and `fs/changed` handling |
| Command/process | None | Sandboxed `command/exec` lifecycle and experimental unsandboxed `process/*` lifecycle, streaming output, PTY control, termination |
| Search/import | None | Legacy fuzzy search, experimental search sessions, external config detect/import/history and progress/completion |
| Platform/operations | None | Windows sandbox setup/readiness/events, feedback upload |
| Remote runtime | None | Environment add/info/status and connection events; remote-control enable/disable/status/pairing/client grants |
| Realtime | None | Start/append audio/text/speech/stop/list voices, WebSocket/WebRTC SDP, transcript/audio/item/error/closed events |
| Memory | None | Experimental memory eligibility and reset |
| Legacy/test | None | Generated compatibility wrappers/tests for deprecated methods and fixture-only mock method |

### 11.6 Missing notification and item fidelity

The 64 notification method names with no current branch are not all optional
decoration. They include:

- `turn/started`, turn diff, moderation, model reroute/verification/safety, and
  raw Responses completion;
- agent plan and reasoning deltas, command output, file patch/output updates,
  and temporary auto-approval review state;
- thread archive/unarchive/delete/close/status/name/goal/settings/environment
  transitions;
- config and generic warnings, Guardian warnings, server-request resolution,
  and Windows sandbox outcomes;
- skills, apps, hooks, MCP startup/OAuth, account login, external import,
  filesystem watch, fuzzy search, command/process, remote-control, and realtime
  events.

The current `ThreadItem` projection is also incomplete. Full parity must retain
at least user messages, agent messages, plans, reasoning, command execution,
file changes, MCP and collaboration tools, web search, image view, sleep,
review entry/exit, and context compaction, plus every current item-specific
delta and lifecycle field. Final `item/completed` remains authoritative while
deltas provide live presentation.

### 11.7 Missing reverse-RPC safety

All reverse methods must be owned before broad feature enablement:

| Server request | Current state | Required parity behavior |
| --- | --- | --- |
| Command approval | Surfaced | Generated payload including environment, actions, additional permissions, network context, available decisions, policy amendments; durable correlated decision receipt |
| File approval | Surfaced | Generated payload including grant root; exact once/session decision semantics |
| User input | Surfaced | Full question schema including auto-resolution timing and cancellation |
| Permission approval | Missing | Render requested network/filesystem profile against environment/cwd and return a method-correct grant/deny response |
| Dynamic tool call | ProductSpec-only code exists but production disables it | Namespace/authority registry for every declared dynamic tool; undeclared calls fail closed with typed result |
| MCP elicitation | Generic decline helper, not complete live workflow | Standard and OpenAI form elicitation UI, timeout/cancel, method-correct decline |
| ChatGPT auth-token refresh | Missing | Trusted credential broker with typed unavailable behavior when the selected auth mode cannot refresh |
| Attestation generation | Missing and not opted in | Generated handler always; advertise `requestAttestation` only when an opaque token broker and deadline path exist; never log tokens |
| Current-time read | Missing | External-clock service returning integer Unix seconds with timeout/cancellation coverage |
| Legacy exec/apply approvals | Generic decline only | Generated compatibility handlers and tests until upstream removal |

App-server can fan one reverse request out to multiple subscribers and accepts
the first response for its request id. OpenAgents therefore needs one host-side
arbiter across Desktop windows, web/mobile supervisors, and reconnect
generations. Renderers may propose a decision, but only the arbiter can commit
the exactly-once response; late or duplicate decisions become visible no-ops.
Install the `currentTime/read` handler even with experimental mode disabled:
the audited server can emit it despite the generated stable server-request
surface omitting it.

## 12. What to implement, in dependency order

The sequence below is intentionally not ordered by visual feature value. Each
phase creates a trustworthy substrate for the next one.

### Phase 0 — Freeze the parity contract and stop over-advertising

Implement first:

1. Add an owned generated protocol package, for example
   `packages/codex-app-server-protocol`, produced from the exact supported
   Codex binary/source ref in both stable and experimental modes, with explicit
   compatibility schemas for the three runtime-accepted deprecated methods
   that Codex generation omits.
2. Check in a machine-readable manifest for all 126/1/11/72 members, their
   stability, schemas, authority class, owner surface, and tests.
3. Make CI fail on added, removed, or schema-changed protocol members until the
   manifest receives an explicit disposition.
4. Bind the signed Desktop release to an exact binary hash/version or a tested
   compatibility window.
5. Until later phases pass, send `experimentalApi: false`; alternatively, make
   experimental opt-in conditional on a complete matching manifest and handler
   set.

Why first: every later wrapper otherwise targets a moving, partly unknown
protocol.

Exit gate: generated stable and experimental unions plus the three reviewed
compatibility adapters account for all 126 runtime declarations; the checked
binary regenerates byte-equivalent schemas; unknown binary or schema versions
fail before a thread starts.

### Phase 1 — Replace per-turn processes with a scoped app-server supervisor

Implement:

1. One long-lived app-server runtime per runtime identity—at minimum
   `(binary hash, CODEX_HOME/account, host target)`—owned by the Desktop host,
   not by a turn.
2. A scoped Effect service such as `CodexAppServerSupervisor` that owns process
   spawn, handshake, stderr/exit evidence, request IDs, subscriptions,
   backpressure, cancellation, restart generation, and teardown.
3. Install the complete reverse-request dispatch table before `initialize`;
   handlers may still be generated deny-only until their authority phase lands.
4. Multiplex many threads/turns over the same initialized connection.
5. Reconnect with exponential backoff where safe; reinitialize, resume visible
   persisted threads, restore connection-local subscriptions, and reconcile
   durable state. Never retry a mutating request without an idempotency or
   reconciliation rule.
6. Support stdio first. Add authenticated WebSocket/Unix transport behind the
   same protocol interface only after local custody is correct.

Why now: login, import, watches, processes, background terminals, realtime,
remote control, and idle notifications cannot work on a process closed after
each turn.

Exit gate: app restart, app-server crash, and process replacement cannot leak
stale events across generations; two simultaneous threads can stream through
one runtime; idle notifications remain observable.

### Phase 2 — Install generated decoding and a lossless native event plane

Implement:

1. Decode every response, reverse request, and notification before it enters
   product code. Remove `Record<string, unknown>` from the protocol boundary.
2. Preserve complete native identities and semantics in a
   `CodexNativeEnvelope` stream and native Thread/Turn/Item projection before
   producing portable OpenAgents events.
3. Add an unknown/decode-failure quarantine with bounded private retention and
   a visible compatibility receipt. Never silently catch and discard.
4. Define retention classes: durable semantic state is journaled; high-volume
   transient audio/process/raw-response data gets a bounded private spool and
   explicit expiry, not permanent general logs.
5. Keep credentials, attestation tokens, MCP secrets, auth payloads, absolute
   private paths, and raw workspace content out of public projections and
   ordinary logs.

Why before feature methods: otherwise each new method creates another lossy
manual parser and migration debt.

Exit gate: a fixture can replay all 72 notifications and every current
`ThreadItem`/delta through native projection with no unclassified loss; an
unknown future notification produces one compatibility receipt and does not
crash or disappear.

### Phase 3 — Complete all reverse-RPC handlers

Implement the 11-method server-request registry before enabling the feature
families that can emit them:

1. command/file approvals and user questions on generated schemas;
2. permission approval with environment-aware grants;
3. dynamic tool registry with exact namespace and grant ownership;
4. MCP elicitation, including optional OpenAI form support;
5. auth-token refresh behind a credential broker;
6. attestation behind explicit `requestAttestation` capability;
7. external current-time service;
8. legacy approval compatibility.

Every pending request needs a durable correlation record, owner attention
state, deadline, cancellation on turn/runtime shutdown, exactly-one response,
method-specific deny behavior, and a central arbiter that deduplicates
multi-subscriber and post-reconnect replays by request identity.

At this phase, requests whose authority dependencies land later may be
generated, correlated, and **deny-only** in production: permission escalation
waits for Phase 4 policy profiles, MCP elicitation waits for Phase 7 MCP UX,
and auth refresh/attestation wait for their credential brokers. The manifest
must say `deny` rather than `implemented`; those rows become fully implemented
when the owning phase lands.

Why before broad requests: a catalog or turn can activate reverse calls; a
host that cannot answer them can deadlock or abort otherwise valid work.

Exit gate: 11/11 success/deny/timeout/cancel/restart fixture cases pass; one
response wins under multi-window races; replayed pending requests cannot cause
a second decision; no pending reverse request survives its runtime generation;
real-binary tests exercise command, file, permission, user-input, dynamic-tool,
MCP, and current-time paths.

### Phase 4 — Account, model, configuration, and policy control plane

Implement in this order:

1. `account/read`, login/start/cancel/logout, login completion;
2. account rate limits, token usage, reset-credit consumption, workspace
   messages, and owner nudge;
3. model list and model-provider capabilities;
4. config read/single write/batch write and config warnings;
5. `configRequirements/read`, permission profiles, experimental features, and
   collaboration modes.

Managed requirements must constrain controls before the renderer offers them.
Config and auth mutations require explicit owner intent and audit receipts.

Why before complete turn controls, plugins/MCP, and dangerous host utilities:
those systems depend on identity, model/provider support, config layering,
permission profiles, and managed policy.

Exit gate: sign-in/out/device/browser flows survive restart; rate/usage truth
is visible; managed requirements remove or deny prohibited choices; config
writes reconcile loaded threads without making OpenAgents a second config
database.

### Phase 5 — Make app-server own thread/history truth

Implement the lifecycle and repair spine:

- `thread/list`, `thread/search`, `thread/loaded/list`, `thread/read`,
  experimental `thread/turns/list` and `thread/items/list`;
- start/resume/fork with all current fields and environment/permission roots;
- name, metadata, settings, goal, memory mode;
- archive/unarchive/delete/unsubscribe and lifecycle notifications;
- compaction, rollback compatibility, raw item injection, and Guardian denied
  action approval; and
- deterministic reconnect repair: reinitialize, resume, restore subscriptions,
  page durable state, and reconcile by native IDs without inventing a replay
  cursor the protocol does not provide.

Replace normal rollout-file parsing with these APIs. Keep rollout parsing only
for migration, diagnostics, or recovery when app-server cannot answer, and
label that evidence source explicitly.

Why before turn/ecosystem UI: every later capability attaches to a thread,
subscription, cwd, environment, or persisted lifecycle. Phase 4 establishes
the global policy that constrains those roots before the lifecycle becomes a
general product API.

Exit gate: create/resume/fork/archive/unarchive/delete/compact/restart journeys
round-trip through app-server; pagination has no gaps/duplicates; the complete
child graph and active statuses repair after host and app-server restarts;
transient gaps and ephemeral-thread limits are surfaced instead of fabricated.

### Phase 6 — Finish turn, item, review, steering, and queue semantics

Implement:

- complete `turn/start` input variants (text/image/local image, skills, apps,
  plugins), output schema, model/service tier/effort/personality,
  collaboration mode, permissions, environments, runtime roots, additional
  context, and client metadata;
- exact `turn/started` → item lifecycle/deltas → `turn/completed` projection;
- turn diff, plan/reasoning/command/file deltas, warnings, model safety/reroute/
  verification, moderation, raw completion, and usage;
- `review/start` inline and detached flows;
- `turn/steer` with active-turn compare-and-set and durable receipt;
- `turn/interrupt` completion based on terminal notification, not request ACK;
  and
- a durable OpenAgents next-turn queue above app-server, promoted to
  `turn/start` only after the prior turn's confirmed quiescence.

The current `followupQueue` is in-memory. Replace it with the existing durable
admission/confirmed-runtime machinery rather than extending the map.

Why here: this completes the core coding workroom only after native state,
restart repair, policy, and model/config truth are trustworthy.

Exit gate: all item variants and deltas render and replay; steer/interrupt race
tests are deterministic; queued messages survive Desktop/app-server restart
without duplicate `clientUserMessageId` values.

### Phase 7 — Skills, hooks, plugins, apps, marketplace, and MCP

Implement dependency order inside the ecosystem:

1. Complete skills list/config/extra roots plus `skills/changed`.
2. Hooks list and hook started/completed state.
3. Marketplace add/remove/upgrade and plugin catalog/read/install/uninstall/
   share operations.
4. App list/update and connector authentication states.
5. MCP reload/status/startup, OAuth completion, resources, tools, and
   elicitations.

Plugin install can create apps and MCPs, so the app/MCP projections and auth
workflow must be ready before install is exposed. Under-development plugin
methods remain behind explicit product flags even though the protocol layer is
complete.

Exit gate: install/uninstall/restart leaves one reconciled catalog; MCP OAuth
and elicitation resume the correct operation; skill/hook/plugin/app changes
propagate without process restart or secret leakage.

### Phase 8 — Filesystem, command, search, import, and platform services

Implement authority-sized services, not a generic privileged RPC button:

- filesystem read/write/create/list/metadata/remove/copy/watch/unwatch;
- sandboxed command exec/write/resize/terminate and output stream;
- fuzzy file search legacy request and experimental session API;
- external-agent detect/import/history with progress and completion;
- Windows sandbox readiness/setup events; and
- feedback upload with explicit attachment review.

All absolute paths must resolve through WorkContext grants. Filesystem and
command result sizes need bounded streaming/spooling. Windows behavior needs
real Windows CI or signed-host proof.

Why after policy: these methods directly mutate or reveal host state and must
consume the authority model built in Phase 4.

Exit gate: path-escape, symlink, oversized-output, watch-overflow,
process-cancel, partial-import, and Windows setup failure tests all produce
typed receipts and no silent partial success.

### Phase 9 — Experimental host runtime: environments, processes, terminals, realtime, and remote control

Implement last among protocol capabilities:

- remote environment add/info/status and connection lifecycle;
- standalone unsandboxed `process/*` with PTY/input/output/exit ownership;
- thread background-terminal list/clean/terminate;
- full realtime start/append/stop/voice/SDP/audio/transcript/item lifecycle;
- remote-control enable/disable/status/pairing/client grant revocation; and
- experimental memory reset and remaining experimental thread controls.

These features combine high privilege, long lifetime, remote identity, or
high-volume ephemeral media. They depend on every earlier runtime,
authorization, retention, and reconnect guarantee.

Exit gate: environment disconnect/reconnect, process ownership, terminal
cleanup, realtime WebSocket/WebRTC closure, remote-control pairing/revocation,
and memory reset have real-host tests and explicit owner attention states.

### Phase 10 — Product surfaces and certification

Only after protocol and semantic gates pass should every capability receive
its final Desktop/web/mobile/operator surface:

- thread library, graph, goals/settings, review, diffs, processes and terminal
  management;
- model/account/usage/config/permission catalogs;
- skills/plugins/apps/marketplace/MCP management;
- filesystem/search/import and platform setup;
- realtime and remote-control supervision; and
- compatibility, degraded, blocked, and unknown-version diagnostics.

The renderer consumes typed projections/intents only. It never receives a raw
RPC method string, credentials, attestation token, unrestricted absolute path,
or generic process authority.

Exit gate: the machine-readable manifest reports complete wire and semantic
coverage; every non-test capability names a product surface or explicit policy
state; cross-surface tests prove the same authority and recovery result.

## 13. Concrete first work packets

The first mergeable implementation sequence should be:

1. **Protocol generation:** generated stable + experimental schemas, complete
   manifest, drift checker, no production behavior change.
2. **Supervisor seam:** move the existing client under a long-lived scoped
   service while preserving the current six production request paths.
3. **Generated decode:** replace unknown records in current initialize/thread/
   turn methods, dormant skill scaffolding, eight notifications, and current
   reverse handlers.
4. **Native envelope store:** retain exact native lifecycle and project the
   existing workroom events from it with equivalence tests.
5. **Reverse-RPC completion:** reach 11/11 before adding any new feature call.
6. **Account/policy bootstrap:** identity, requirements, permission profiles,
   model/config truth, and safe catalog boundaries.
7. **Thread read/list/reconcile:** make app-server the history and restart
   repair path.
8. **Complete item/turn projection and review.**
9. **Then** proceed through ecosystem, host utilities, and
   experimental runtime phases above.

This ordering preserves the accepted workroom while replacing its narrow
kernel from underneath. A large settings/catalog PR before packets 1–7 would
create UI over an unversioned, ephemeral, lossy transport and should be
rejected.

## 14. Final 100%-parity acceptance gate

Do not call the program complete until one release proves all of the following:

- generated inventory: 126/126 requests, 1/1 client notifications, 11/11
  reverse requests, 72/72 notifications;
- no manual unknown-record parsing at the app-server wire boundary;
- no silently ignored known or unknown notification;
- all current ThreadItem variants and item deltas reconstruct from fixture
  replay;
- one long-lived runtime supports multiple simultaneous threads and survives
  app-server replacement;
- every reverse request resolves exactly once across approve/deny/timeout/
  cancel/restart;
- thread/turn/item state reconciles after host crash without duplicated user
  input, missing terminal state, or fabricated completion;
- stable and experimental capability gates match the generated binary;
- every dangerous method consumes typed authority and emits a private audit
  receipt;
- every non-test method has a product surface or explicit policy-owned
  unavailable state;
- deterministic fixtures cover every protocol member, while compatible real
  binaries cover each capability family on every applicable OS/transport; and
- protocol drift, schema drift, binary mismatch, overload, and unknown-message
  cases fail visibly before they can corrupt product state.

That is the difference between “OpenAgents can chat through Codex” and
“OpenAgents is a complete Codex app-server host.”

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

- `apps/openagents-desktop/package.json`
- `apps/openagents-desktop/src/main.ts`
- `apps/openagents-desktop/src/codex-app-server-client.ts`
- `apps/openagents-desktop/src/codex-app-server-turn.ts`
- `apps/openagents-desktop/src/codex-local-runtime.ts`
- `apps/openagents-desktop/src/product-spec-app-server-tools.ts`
- `apps/openagents-desktop/src/codex-app-server-client.test.ts`
- `apps/openagents-desktop/src/codex-app-server-smoke-fixture.test.ts`
- `apps/openagents-desktop/src/local-turn-journal.ts`
- `apps/openagents-desktop/src/local-turn-recovery.ts`
- `apps/openagents-desktop/src/codex-history.ts`
- `docs/khala-code/2026-07-01-codex-app-server-gap-matrix.md`

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
