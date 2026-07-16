# T3 Code Agent Client Protocol implementation teardown — 2026-07-16

Read-only implementation audit of T3 Code's **Agent Client Protocol**
integration, with an OpenAgents plan for controlling Grok, Cursor, and other
compatible coding agents. The T3 Code source was inspected at commit
[`bde0a4c0dd2d420a5fd71f39448d5db1bab078da`](https://github.com/pingdotgg/t3code/tree/bde0a4c0dd2d420a5fd71f39448d5db1bab078da)
on 2026-07-16. The local OpenAgents comparison was made at
`61ebc9b9bc196ee0b3c9dada1f694ed5e170ec84` before this document was written.

This is a focused continuation of the
[whole-product T3 Code teardown](./2026-07-13-t3-code-teardown.md), the
[T3 Code/OpenAgents gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md),
the [Grok Build teardown](./2026-07-15-grok-build-teardown.md), and the
[OpenAgents adaptation analysis](./2026-07-10-openagents-product-adaptation-analysis.md).

## Terminology and exact scope

In this document, **ACP means Agent Client Protocol** because that is the
protocol implemented by T3 Code, Cursor, and Grok's `grok agent stdio`
command. It standardizes communication between a coding agent and a client
such as an editor or IDE. The protocol is bidirectional: the client calls the
agent for session work, while the agent calls the client for permissions,
filesystem, terminal, and other negotiated capabilities.

The similarly named Agent Communication Protocol at
`agentcommunicationprotocol.dev` is a different REST protocol. It is not used
by T3 or Grok, cannot drive `grok agent stdio`, and is out of scope for this
implementation plan. This document does not propose support for that protocol.

Evidence labels:

- **[source]** — observed directly in the commit-pinned source;
- **[schema]** — encoded in a generated schema or method manifest;
- **[test]** — encoded in a checked-in test or live-probe harness;
- **[history]** — supported by the repository's Git history;
- **[public]** — supported by an official ACP source;
- **[inferred]** — an architectural conclusion drawn from several observations;
- **[decision]** — the recommended OpenAgents disposition; and
- **[limitation]** — a boundary on what source inspection proves.

No T3 Code source, user state, credentials, or live provider account was
modified. The checked-in real-CLI probes were inspected but not armed.

## Executive conclusion

T3 Code has a serious ACP implementation, not a thin JSON-RPC helper. Its
private `effect-acp` package provides:

- generated Effect schemas from a pinned official ACP release;
- typed client and agent APIs over one bidirectional NDJSON/stdio connection;
- generic extension requests and notifications in both directions;
- structured transport, parse, process-exit, and request errors;
- subprocess lifecycle integration;
- bounded diagnostic logging that avoids copying failed payloads; and
- fixture coverage for routing, cancellation, extensions, late responses,
  process exit, error correlation, and redaction.

Above that package, T3 Code adds a provider-independent `AcpSessionRuntime`.
That runtime spawns Cursor or Grok, initializes and authenticates the peer,
creates or restores one root session, serializes prompts, tracks configuration
and mode state, normalizes session updates, gates replay during restore, and
turns permission callbacks into T3 provider events. Cursor and Grok then add
small launch/auth/model policies plus private extension handlers.

The architecture is:

```text
Cursor "agent acp"              Grok "grok agent stdio"
        \                              /
         \ provider launch/auth/model /
          +--------------------------+
                         |
                  AcpSessionRuntime
       start / resume / prompt / cancel / config
       replay gate / event barrier / state tracking
                         |
                    effect-acp client
      typed agent RPC + reverse client RPC + extensions
                         |
             patched bidirectional Effect RPC
                  NDJSON over child stdio
                         |
             normalized provider runtime events
        plans / tools / content / permissions / lifecycle
```

The lesson for OpenAgents is to copy the **layering**, not the package
verbatim. T3's generated protocol snapshot is behind the current official
Agent Client Protocol schema stream, its transport depends on unstable Effect
RPC internals, it leaves several protocol methods unimplemented, and its
product projection intentionally drops much of the protocol event vocabulary.

The required OpenAgents product is **Agent Client Protocol client support**:
OpenAgents hosts `grok agent stdio` and other compatible coding agents as
foreign workers. That is the implementation priority and the meaning of
“ACP support” in this plan. Exposing OpenAgents itself as an ACP agent is a
separate product and is out of scope.

The client adapter should use one generated, versioned protocol package and one
bidirectional transport. ACP remains an adapter rather than the canonical
OpenAgents domain. Thread, Turn, Item, Work Unit, Runtime Interaction,
authority, evidence, and Receipt remain canonical.

## Implementation tracking

The execution plan is tracked by
[#8887 — Full Agent Client Protocol integration for Grok and Cursor](https://github.com/OpenAgentsInc/openagents/issues/8887).
Its child issues turn the phases below into dependency-ordered deliverables:

| Issue                                                            | Deliverable                                                                            |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| [#8888](https://github.com/OpenAgentsInc/openagents/issues/8888) | pin `schema-v1.19.0`; generate codecs, manifests, provenance, and drift checks         |
| [#8889](https://github.com/OpenAgentsInc/openagents/issues/8889) | bounded bidirectional stdio JSON-RPC transport and exact process lifecycle             |
| [#8890](https://github.com/OpenAgentsInc/openagents/issues/8890) | stable wire-v1 conformance, Grok/Cursor fixtures, faults, and compatibility artifacts  |
| [#8891](https://github.com/OpenAgentsInc/openagents/issues/8891) | native/canonical event projection and reverse-request authority bridge                 |
| [#8892](https://github.com/OpenAgentsInc/openagents/issues/8892) | session lifecycle, prompt/update drain, replay/live gating, cancellation, and recovery |
| [#8893](https://github.com/OpenAgentsInc/openagents/issues/8893) | required Grok profile over `grok agent stdio`                                          |
| [#8894](https://github.com/OpenAgentsInc/openagents/issues/8894) | required Cursor profile over `agent acp`                                               |
| [#8895](https://github.com/OpenAgentsInc/openagents/issues/8895) | Grok/Cursor install, auth, configuration, interaction, failure, and recovery UX        |
| [#8896](https://github.com/OpenAgentsInc/openagents/issues/8896) | trusted peer-profile schema and registry/admission path                                |
| [#8897](https://github.com/OpenAgentsInc/openagents/issues/8897) | pinned Grok and Cursor live compatibility and release-claim gate                       |

Implementation status on 2026-07-16: #8888 delivered the pinned generated
protocol authority, #8889 delivered the bounded bidirectional stdio transport,
and #8890 delivered the hermetic
[`agent-client-protocol-conformance`](../../packages/agent-client-protocol-conformance/README.md)
package. #8891 then delivered the native/canonical projection and reverse
authority bridge, and #8892 delivered the shared race-safe
[session runtime](../adr/2026-07-16-agent-client-session-runtime.md). The
#8893 Grok edge now composes those layers behind admitted `grok agent stdio`,
uses documented cached-token/API-key negotiation, and records an exact 0.2.101
candidate probe in the
[Grok peer ADR](../adr/2026-07-16-grok-agent-client-protocol-peer.md). Its
production compatibility facade no longer uses the raw JSON-RPC fixture client.
The #8897 claim validator is now implemented as a checked
[machine-readable matrix](../../packages/agent-client-protocol-conformance/compatibility/release-matrix.json)
with a [human proof ledger](../qa/2026-07-16-acp10-release-proof/README.md).
It independently derives an experimental verdict for both pinned peers because
required live and cross-platform rows remain unresolved; it does not turn
fixture coverage into supported language. Grok and Cursor are now registered
main-owned provider lanes, and an isolated production Desktop build completed a
real Grok Full Auto turn. A checked, opt-in production runner now reproduces a
redacted two-peer candidate receipt without claim authority. #8897 remains open
for the unobserved credential, permission, extension, and platform proof;
packaged interruption/restart journeys for both pinned peers are now checked,
and Grok now has live broker-only MCP custody evidence with a complete bounded
post-shutdown scan of its exact session/configuration persistence surfaces.
Cursor client-side login cancellation is also live-proven using the ordinary
HOME and stopping before `authenticate`, without mutating login/keychain state.
The conformance package's exact
23-method coverage report, compatibility matrix, bounded
fault inventory, independently versioned peer provenance, MCP reference
custody cases, and opt-in probes are checked artifacts. The Grok and Cursor
fixtures are deliberately labeled source-derived synthetic evidence; they do
not substitute for #8897's independent pinned live-binary release proof.

Grok and Cursor are independent required release peers. Passing the shared
wire suite or one provider's live probe does not establish support for the
other. The epic deliberately keeps their auth, extensions, version ranges,
and recovery behavior in separate peer profiles.

#8895 now supplies the Desktop/service control boundary over those profiles.
Main owns probing, admission, validated alternate-path persistence, runtime
startup/auth, workspace-bound session creation, cancellation, repair, receipts,
and sanitized support export. Preload exposes only schema-checked status and a
closed provider/action pair. Settings renders distinct Grok and Cursor entries,
advertised auth methods and terminal auth states, exact session/process state,
stable versus peer-extension configuration provenance, broker-backed authority
truth, and non-color accessible status labels. Zero-option Grok/Cursor questions
now accept bounded free-form text through the same canonical durable interaction
decision envelope as option answers. No token-file or environment presence is
presented as authentication proof, and release labels remain gated by the
#8897 matrix.

## 1. Snapshot identity and protocol drift

| Field                                    | T3 Code observation                                            |
| ---------------------------------------- | -------------------------------------------------------------- |
| Repository revision                      | `bde0a4c0dd2d420a5fd71f39448d5db1bab078da`, 2026-07-16         |
| ACP package                              | private workspace package `packages/effect-acp`                |
| Generated schema artifact                | ACP `v0.11.3` unstable schema                                  |
| Negotiated wire version                  | `1`                                                            |
| Generated schema size                    | 10,375 lines plus a 35-line method manifest                    |
| Hand-written package source/tests        | roughly 6,000 lines including tests and generator              |
| Product consumers                        | Cursor and Grok provider adapters                              |
| Current Grok implementation              | Rust `agent-client-protocol` 0.10.4 + unstable, schema 0.11.4  |
| Current official TypeScript SDK          | `@agentclientprotocol/sdk` 1.2.1                               |
| Initial ACP commit                       | `9c64f12e`, 2026-04-17, “Add ACP support with Cursor provider” |
| Grok addition                            | `38ea6d48`, 2026-06-09                                         |
| Latest ACP-specific hardening in history | `52b04b94`, replay-idle load readiness, 2026-06-26             |

T3's generator pins
[release `v0.11.3`](https://github.com/agentclientprotocol/agent-client-protocol/releases/tag/v0.11.3)
and downloads `schema.unstable.json` and `meta.unstable.json`. At the time of
this audit, the official repository's latest schema release is
[`schema-v1.19.0`](https://github.com/agentclientprotocol/agent-client-protocol/releases/tag/schema-v1.19.0),
published 2026-07-06. Both describe ACP wire protocol version 1, but the method
and schema artifacts are not equivalent. [source] [schema] [public]

That distinction matters. ACP wire compatibility is negotiated with
`protocolVersion`; schema artifact versions describe the generated API and may
change without a wire-version bump. Optional behavior is capability-gated.
OpenAgents must therefore pin both the artifact and the wire version, retain a
generated method manifest, and compare every upgrade rather than treating
“protocol version 1” as a frozen method set.

The current stable `schema-v1.19.0` agent surface includes:

- `initialize` and `authenticate`;
- `session/new`, `session/load`, `session/list`, `session/delete`,
  `session/resume`, `session/close`, `session/prompt`, and `session/cancel`;
- `session/set_mode` and `session/set_config_option`; and
- `logout`.

Its stable reverse-client surface includes permission, session update,
filesystem, and terminal methods. It also defines protocol-level
`$/cancel_request`. The current unstable artifact adds provider selection,
MCP tunneling, editor document notifications, next-edit suggestions,
`session/fork`, and elicitation. T3's older unstable snapshot instead contains
`session/set_model` and older `session/elicitation*` names. [schema] [public]

**Disposition:** use current stable ACP as the first OpenAgents compatibility
target. Put unstable families behind explicit generated capability flags and
upgrade receipts. Do not copy T3's `v0.11.3` generated files. [decision]

### 1.1 Grok compatibility verdict

The current public Grok source was independently checked at
[`c68e39f60462f28d9be5e683d9cbe2c57b1a5027`](https://github.com/xai-org/grok-build/tree/c68e39f60462f28d9be5e683d9cbe2c57b1a5027),
published 2026-07-16. Grok does not use the TypeScript SDK. Its Cargo workspace
pins:

```toml
agent-client-protocol = { version = "0.10.4", features = ["unstable"] }
```

The lockfile resolves `agent-client-protocol-schema` 0.11.4. xAI then wraps
the crate in the workspace package `xai-acp-lib` for typed bidirectional
channels, gateways, messages, line buffering, and normalization. The agent
implementation is an `impl acp::Agent for MvpAgent`, and the public stdio path
negotiates `protocolVersion: 1`. [source]

The current `schema-v1.19.0` stable artifact also describes wire protocol
version 1. Its stable method table includes the entire documented Grok hello
path: `initialize`, `authenticate`, `session/new`, `session/prompt`, and
`session/update`. Its initialize codecs deliberately default missing optional
capabilities and authentication arrays, preserving wire-version-1
compatibility with older agents. [schema]

**Verdict:** a client generated from `schema-v1.19.0` should consume the
documented `grok agent stdio` path. It is the right schema family and wire
version. That conclusion is source-backed but still requires a pinned
real-binary compatibility fixture before a shipped claim. [inferred]

It does not mean one schema artifact automatically supports every behavior of
every ACP agent. The official protocol's own versioning rule is:

- negotiate compatibility with `protocolVersion`;
- inspect capabilities before every optional method;
- use the schema/SDK artifact version for generated API compatibility; and
- handle extensions by explicit namespace and version.

Accordingly, `schema-v1.19.0` is the correct broad stable baseline for the
current Agent Client Protocol registry, which includes Cursor, Codex, Claude,
Gemini, GitHub Copilot, OpenCode, Qwen Code, Goose, and many other coding
agents. Each still needs its own launch, installation, authentication,
capability, extension, and conformance profile. Grok is not currently listed
in that curated registry, but its source and public documentation implement
the same protocol and wire version. [public] [decision]

For Grok specifically, the first profile must:

1. spawn `grok agent stdio` with stdout reserved for NDJSON and stderr kept
   separate;
2. initialize with wire version 1 and truthful client capabilities;
3. select an actually advertised auth method, including `cached_token` or
   `xai.api_key` for the documented headless path;
4. create a session with an absolute `cwd` and `mcpServers`;
5. retain and project `session/update` while `session/prompt` is pending;
6. answer reverse permission/filesystem/terminal calls only when the matching
   handlers and authority grants exist;
7. preserve unknown `x.ai/*` extensions as bounded native evidence and invoke
   only extensions needed by the Grok profile; and
8. implement session cancellation before treating process kill as the normal
   interrupt path.

## 2. The `effect-acp` package

### 2.1 Generated schema authority

`packages/effect-acp/scripts/generate.ts` is the protocol authority:

1. download the pinned official schema and method metadata;
2. decode both inputs with Effect Schema;
3. normalize nullable JSON Schema forms that the generator cannot consume
   directly;
4. sort definitions for deterministic output;
5. generate Effect codecs with `@effect/openapi-generator`;
6. emit `schema.gen.ts` and `meta.gen.ts`; and
7. format the generated directory.

The package exports the generated schema, method manifest, RPC declarations,
client, agent, protocol, terminal facade, and error types through explicit
subpath exports. It is private to the monorepo, so T3 does not claim it as a
general ACP SDK. [source]

This is a good authority pattern: source schemas are pinned, generation is
deterministic, generated code is checked in, and method names are not repeated
as hand-written string literals throughout the application. The weakness is
upgrade cadence: the generator's hard-coded artifact remained at `v0.11.3`
while the official schema stream advanced. [source] [inferred]

### 2.2 Typed method groups

`rpc.ts` turns generated request, response, and error codecs into two Effect
RPC groups:

- `AgentRpcs` for client-to-agent requests; and
- `ClientRpcs` for agent-to-client reverse requests.

Notifications are handled outside those RPC groups because ACP uses JSON-RPC
notifications for streaming session updates and cancellation.

The package allocates typed RPC request IDs starting at `2^32` and extension
request IDs starting at 1. Tests assert that the two ranges do not collide.
[source] [test]

One material omission is visible in the manifest-to-RPC comparison:
`session/set_mode` is generated but absent from `AgentRpcs`, `AcpClient`, and
`AcpAgent`. T3's higher runtime implements `setMode` by writing a configuration
option named `mode` through `session/set_config_option`. That works for the
providers T3 inspected, but it is not literal method coverage. [schema]

### 2.3 Why the transport is “patched”

Effect RPC normally treats one side as the request client and the other as the
request server. ACP requires both roles on the same byte stream: while a
client's `session/prompt` request is outstanding, the agent can ask the client
for permission, file access, terminal work, or elicitation.

`makeAcpPatchedProtocol` therefore constructs both an Effect RPC client
protocol and an Effect RPC server protocol over one NDJSON parser and one stdio
writer. It owns separate queues for:

- requests routed to the local typed server;
- responses routed to the local typed client;
- notifications;
- disconnects;
- outgoing encoded messages; and
- generic extension requests awaiting responses.

Incoming JSON objects are classified as requests, responses, protocol control
messages, typed notifications, or extensions. Known reverse-request methods go
to the typed server. Unknown requests go to the extension handler or receive a
method-not-found error. Responses are correlated either with a typed Effect RPC
request or the separate extension pending map. [source]

The outgoing path logs a decoded structure, encodes one JSON object per line,
optionally logs the raw line, and offers it to the writer queue. The incoming
path logs raw and decoded forms, converts parse/schema failures to structured
ACP errors, and routes every decoded object. [source]

The patch is effective but should not be copied blindly:

- all hot queues are unbounded;
- `supportsAck` is reported to Effect RPC even though ACP/stdio provides no
  durable acknowledgement contract;
- generic extension responses cannot stream;
- extension requests have interrupt cleanup but no transport-owned deadline;
- individual notification callback failures are swallowed;
- unknown notifications are ignored when no fallback is registered;
- server sends turn transport failure into an Effect defect with `orDie`; and
- protocol-level `$/cancel_request` is not implemented in the pinned method
  table or router.

These are tractable tradeoffs for a private adapter, not an OpenAgents
interoperability contract. [source] [inferred]

### 2.4 Client API

`AcpClient` is the side T3's Cursor and Grok integrations consume. It exposes:

- typed initialization, authentication, logout, session create/load/list/fork/
  resume/close, prompt, model selection, configuration, and session cancel;
- reverse-request handler registration for permission, elicitation, files, and
  terminals;
- session-update and elicitation-complete notification handlers;
- typed and fallback extension request/notification handlers; and
- raw request, notification, and notification-stream access.

Notifications that arrive before a handler is registered are buffered. The
first registration flushes the buffer, after which notifications are
dispatched live. Multiple notification handlers run; one handler's failure is
caught so later handlers still receive the notification. Core reverse-request
handlers are single mutable slots, while extension handlers are keyed maps.
Later registration replaces an earlier handler, and there is no deregistration
handle. [source] [test]

The child-process layer connects child stdout to protocol input and protocol
output to child stdin. Child stderr is drained but not surfaced by this package.
Termination errors retain the child PID and exact exit status. [source] [test]

### 2.5 Agent API

`AcpAgent` is the mirror image. It can:

- register typed handlers for the supported client-to-agent methods;
- receive `session/cancel` notifications;
- call the client's permission, elicitation, filesystem, and terminal methods;
- send session updates and elicitation completion;
- send or handle typed and unknown extensions; and
- expose a terminal handle whose operations route back to the ACP client.

T3's product does not use this half to expose T3 itself as an ACP agent. It is
nevertheless a valuable implementation proof that the underlying transport is
bidirectional and not Cursor-specific. [source]

### 2.6 Structured errors and diagnostics

The package defines distinct schema-tagged errors for spawn failure, exact
process exit, protocol parse/encode failure, transport failure, input-stream
end, and JSON-RPC request failure. Request errors retain method, request ID,
code, upstream data, operation, and structured cause where available.

The June hardening series is especially relevant:

- transport errors became structured;
- request failures gained correlation;
- child termination retained PID and exit status; and
- schema/native diagnostics stopped copying raw invalid values into ordinary
  logs.

Parse diagnostics record issue count, issue kinds, and maximum path depth
instead of the failed payload. T3's native ACP logger similarly records value
type, field count, array count, byte length, structural method/tag, status, and
error tag—not raw prompts, files, credentials, or wire bodies. [source]

This is the right default for OpenAgents protocol observability. Raw native
payload retention, if required for lossless internal replay, must live in a
separately classified private evidence store with bounded size, redaction,
retention, and export policy. It must not leak into ordinary logs or public
receipts. [decision]

## 3. T3's method coverage

### 3.1 Client-to-agent methods in T3's pinned schema

| Method                      | Typed in `effect-acp` | Used by T3 Cursor/Grok runtime | Finding                                                                  |
| --------------------------- | --------------------: | -----------------------------: | ------------------------------------------------------------------------ |
| `initialize`                |                   yes |                            yes | Always sends wire version 1, client capabilities, and client info.       |
| `authenticate`              |                   yes |                            yes | Cursor uses `cursor_login`; Grok chooses API key or cached token.        |
| `logout`                    |                   yes |                             no | Library surface only.                                                    |
| `session/new`               |                   yes |                            yes | Creates one root session per child runtime.                              |
| `session/load`              |                   yes |                            yes | Used for provider resume IDs.                                            |
| `session/list`              |                   yes |                             no | Not surfaced by `AcpSessionRuntime`.                                     |
| `session/fork`              |                   yes |                             no | Not surfaced by `AcpSessionRuntime`.                                     |
| `session/resume`            |                   yes |                             no | T3 restore uses `session/load` instead.                                  |
| `session/close`             |                   yes |                             no | Child scope teardown owns product close.                                 |
| `session/prompt`            |                   yes |                            yes | Serialized per runtime.                                                  |
| `session/cancel`            |          notification |                            yes | Sends real session cancel, then interrupts the local prompt fiber.       |
| `session/set_config_option` |                   yes |                            yes | Cursor mode/model/options; local value validation and no-op suppression. |
| `session/set_model`         |         yes, unstable |                           Grok | Used when the requested Grok model differs.                              |
| `session/set_mode`          |        generated only |                             no | Replaced with config option `mode`.                                      |

The runtime unconditionally calls `authenticate` after `initialize` rather than
making authentication conditional on negotiated methods/capabilities. That is
provider policy embedded in a “shared” start sequence. A general ACP client
must negotiate optional auth honestly and support agents that require none.
[source] [inferred]

### 3.2 Agent-to-client methods

| Method                         | Typed in `effect-acp` | Used by T3 provider adapters | Finding                                                |
| ------------------------------ | --------------------: | ---------------------------: | ------------------------------------------------------ |
| `session/update`               |          notification |                          yes | Core streaming input.                                  |
| `session/request_permission`   |                   yes |                          yes | Mapped to provider approval events and runtime policy. |
| `session/elicitation`          |         yes, unstable |            exposed, not used | Cursor/Grok use vendor question extensions instead.    |
| `session/elicitation/complete` |          notification |            exposed, not used | Old unstable naming.                                   |
| `fs/read_text_file`            |                   yes |                           no | Runtime defaults capability to false.                  |
| `fs/write_text_file`           |                   yes |                           no | Runtime defaults capability to false.                  |
| `terminal/create`              |                   yes |                           no | Runtime defaults capability to false.                  |
| `terminal/output`              |                   yes |                           no | Library surface only.                                  |
| `terminal/wait_for_exit`       |                   yes |                           no | Library surface only.                                  |
| `terminal/kill`                |                   yes |                           no | Library surface only.                                  |
| `terminal/release`             |                   yes |                           no | Library surface only.                                  |

T3's shared runtime gets one security-critical detail right: it advertises file
read, file write, and terminal as false unless a provider integration
explicitly enables and implements them. The current Cursor capability override
is for parameterized model selection, not ambient filesystem or terminal
access. [source]

### 3.3 Capability enforcement gap

The generated initialize types include client filesystem, terminal, auth, and
elicitation capabilities, plus agent prompt, MCP, load, and session
capabilities. T3 records the initialize response but does not centrally guard
every optional call against the negotiated capability set. Provider code
mostly knows which peer it launched and calls the expected method.

A general OpenAgents adapter cannot rely on that closed-world assumption.
Every optional method must have one of three outcomes:

1. capability proven and call allowed;
2. capability absent and a typed unsupported result returned; or
3. peer violated the negotiation and the connection records a protocol fault.

## 4. `AcpSessionRuntime`: the reusable product layer

### 4.1 One process, one root session

The runtime owns a scoped child process and one root ACP session, even though
ACP permits several concurrent sessions on one connection. It accepts spawn
argv/environment, cwd, optional resume session ID, auth method, client
capabilities/info, MCP servers, load timing, request logging, and protocol
logging.

`start` is concurrency-safe:

- `NotStarted` creates one startup fiber;
- concurrent callers await the same deferred;
- success is retained as `Started`; and
- failure returns to `NotStarted` so startup can be retried.

Startup performs `initialize`, `authenticate`, then either `session/new` or
`session/load`. It snapshots the returned modes and configuration options.
[source]

This is a strong lifecycle shape. OpenAgents should preserve idempotent shared
startup and explicit retryability, but should not assume one process per
session. The runtime owner should model connection and session generations
separately so it can support both single-session CLIs and multi-session agents.
[decision]

### 4.2 Restore and the replay-idle gate

Grok can stream replay `session/update` notifications during `session/load`
while leaving the load request pending. T3 works around this with a gate:

1. mark load active before sending the request;
2. observe replay activity without projecting it into the live turn;
3. race the actual load response against a two-second replay-idle gap;
4. synthesize a load response from model/mode state found in initialize
   metadata if replay becomes idle first; and
5. fail after a 90-second overall timeout.

Notifications for child sessions are also rejected rather than flattened into
the root stream. [source] [test]

The workaround is provider-specific but the principle is general: restore
needs an explicit replay/live barrier and session lineage. OpenAgents should
model accepted restore, replay complete, projection caught up, and live-ready
as different states. An idle heuristic may be a compatibility fallback, never
the canonical durability contract. [decision]

### 4.3 Prompt and cancellation semantics

Prompts are serialized with a semaphore. Before each prompt, the runtime closes
any active assistant segment. It forks the typed prompt RPC, stores the active
fiber, converts a pure interruption into a `cancelled` prompt response, and
closes the final assistant segment on completion.

Cancellation:

- interrupts the local prompt fiber;
- sends a real `session/cancel` notification in the runtime scope; and
- deliberately does not wait for a response because cancellation is a
  notification.

The provider adapters add generation and active-turn checks so stale
completion cannot settle a newer turn. Grok also drains an event-stream barrier
before final turn settlement, preventing queued deltas from appearing after
completion. [source] [test]

This is stronger than OpenAgents' current Grok fixture, which kills and
restarts the entire process because it does not implement session cancel.

### 4.4 Session update projection

T3's runtime parses these update families:

- `agent_message_chunk` with text content;
- `tool_call` and `tool_call_update`;
- `plan`; and
- `current_mode_update`.

It normalizes snake-case ACP statuses to T3's statuses, merges partial tool
updates by tool-call ID, derives command/detail presentation, suppresses
uninformative placeholder updates, segments assistant content around tool
calls, and emits stream barriers for deterministic draining.

The provider event adapter maps:

- ACP execute/read/edit/delete/move permission kinds to T3 approval classes;
- ACP tool kinds to command, file-change, web-search, or dynamic-tool items;
- plan entries to canonical turn-plan events;
- assistant segments to item lifecycle; and
- text chunks to content deltas.

Every projected plan/tool/content/permission event retains raw provenance with
an ACP source and method. [source]

The projection is intentionally incomplete. It drops or does not canonically
project user message chunks, thought chunks, non-text assistant content,
available-command changes, config-option updates, session-info updates, usage
updates, and several rich tool-content forms. Some raw tool fields survive in
item data, but that is not lossless protocol support. [source]

### 4.5 Modes, models, and configuration

The runtime snapshots typed configuration options and validates writes:

- booleans must receive booleans;
- select values must appear in the advertised option list;
- an already-current value becomes a local no-op; and
- successful writes replace the local configuration snapshot.

Cursor:

- launches `agent acp`;
- authenticates with `cursor_login`;
- uses a model configuration option plus separate provider options;
- maps product plan/approval modes to advertised ACP mode IDs; and
- discovers models through `cursor/list_available_models`.

Grok:

- launches `grok agent stdio`;
- sets an OAuth referrer;
- selects `xai.api_key` when an API key is present, otherwise `cached_token`;
- reads current model state from session setup; and
- uses unstable `session/set_model` only when the model changes.

The contrast is a warning against hard-coding model and mode methods. Current
stable ACP prefers generalized session configuration. OpenAgents should render
advertised configuration options and use stable `session/set_config_option`,
with `session/set_mode` where the current stable capability requires it.
Provider-specific selectors belong behind an adapter flag. [decision]

### 4.6 Provider extensions

Cursor registers:

- `cursor/ask_question`;
- `cursor/create_plan`;
- `cursor/update_todos`; and
- `cursor/list_available_models`.

Grok registers:

- `x.ai/ask_user_question` and `_x.ai/ask_user_question`; and
- a private prompt-completion notification fallback used when the standard
  prompt request does not settle correctly.

Extensions are decoded with provider-specific schemas, mapped to T3's existing
user-input/plan/model abstractions, and logged with distinct raw sources. They
do not become new orchestration aggregates. [source]

That is the right domain boundary. The caveat is interoperability: an extension
that fixes a provider's prompt completion or supplies core user interaction is
part of that adapter's compatibility contract and needs a version/capability
matrix, not an unversioned best effort.

## 5. Verification depth

The package and runtime tests cover more than happy-path prompting:

- typed client and agent round trips;
- reverse permission requests;
- buffered notifications and handler-failure isolation;
- typed and generic extensions;
- distinct request ID spaces;
- invalid payload diagnostics without payload copying;
- zero-valued JSON-RPC IDs;
- interrupted requests and late responses;
- process exit propagation and single termination;
- pending-request failure on child exit;
- initialize capability merging;
- prompt sequencing and cancellation;
- child-session filtering;
- assistant segmentation around tool calls;
- config validation and no-op writes;
- restore replay suppression and replay-idle readiness;
- bounded native diagnostic logging;
- Cursor and Grok extension parsing; and
- environment-gated probes against real Cursor and Grok CLIs.

The remaining verification gaps are exactly the areas OpenAgents must add:

- bounded queue overload and backpressure;
- multiple sessions sharing one connection;
- capability violations;
- protocol-level request cancellation;
- partial writes and writer failure;
- malformed/oversized line policy;
- handler registration teardown;
- authentication-optional agents;
- stable-schema conformance at the latest pinned release;
- reconnect/restart repair; and
- full projection coverage for every retained update variant.

## 6. OpenAgents' existing ACP code

OpenAgents is not starting from zero. `packages/grok-harness` contains:

- a subprocess JSON-RPC client for `grok agent stdio`;
- initialize/auth selection;
- session creation and prompting;
- a minimal session-update projector;
- a mock stdio server and in-process client;
- a local desktop-to-Grok session mapping; and
- an environment-gated live smoke.

That code proves Grok can be exercised through ACP and provides useful fixtures.
It is not a reusable ACP compatibility layer.

Material limitations:

1. it implements only outbound requests and `session/update` notifications;
2. it cannot answer bidirectional permission/filesystem/terminal requests;
3. despite that, initialize advertises filesystem read/write and terminal as
   true;
4. it ignores parse failures, unknown messages, stderr, structured error code/
   data, stdin write failure, and process signals;
5. only one update handler exists and later registration replaces it;
6. restore, list, resume, close, delete, configuration, logout, and protocol
   cancellation are absent;
7. interrupt kills the entire process, starts a new one, and loses the active
   ACP session;
8. the projector retains only text chunks and shallow tool status;
9. plan, thought, rich content, mode/config/session info, usage, permission,
   and provenance are lost; and
10. the mock covers initialize → authenticate → new → prompt, so it cannot
    catch the missing bidirectional surface.

The advertised-capability mismatch should be corrected when the implementation
packet begins: unimplemented file and terminal capabilities must be false.
Until then, the package should continue to be described as a Grok fixture, not
general ACP support. [source] [decision]

## 7. OpenAgents architecture decision

### 7.1 One shared client foundation, peer profiles above it

Create one generated ACP protocol package with:

- pinned stable and explicitly selected unstable schema artifacts;
- checked-in artifact digests, wire version, method manifest, and capability
  manifest;
- codecs for every supported request, response, notification, and error;
- one bidirectional connection service;
- typed outbound agent-method calls plus reverse client-method handlers;
- bounded pending-request and outgoing/incoming queues;
- deadlines, interruption, late-response handling, and `$/cancel_request`;
- structured spawn/transport/protocol/request/peer-exit errors;
- private raw-event capture separated from safe diagnostics; and
- connection/session generations with scoped teardown.

Prefer the official TypeScript SDK as the wire implementation if a pinned
spike proves it supports the required outbound calls, reverse requests,
cancellation, and stdio semantics. Wrap it in Effect lifecycle and errors. If
it cannot satisfy those contracts, implement the missing transport locally
against the generated schemas. Do not couple the public contract to Effect's
unstable RPC wire model or report acknowledgements the transport does not
provide. [decision]

The required consumer is the outbound client adapter:

| Direction               | ACP role   | OpenAgents owner                                          |
| ----------------------- | ---------- | --------------------------------------------------------- |
| Host foreign ACP agents | ACP client | Provider/runtime adapter behind the existing runtime host |

### 7.2 Canonical identity mapping

| ACP concept                  | OpenAgents mapping                                                                                        |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- |
| Connection/process           | runtime generation, peer implementation/version, negotiated capability set                                |
| `sessionId`                  | opaque provider/adapter attachment ID, never the OpenAgents Thread ID                                     |
| `session/new`                | create or attach through the canonical thread/session command                                             |
| `session/load` / `resume`    | repairable attachment to an existing canonical thread, with replay/live barrier                           |
| `session/list`               | bounded projection of sessions the authenticated adapter is allowed to see                                |
| `session/delete` / `close`   | explicit archive/delete/close intent with existing authority and evidence rules                           |
| `session/prompt`             | canonical turn admission; steering/queue behavior must remain explicit                                    |
| `session/cancel`             | cancel the owned active turn/session generation, idempotently                                             |
| `session/update`             | projection from or into canonical Item/Turn/Interaction events, with native provenance retained privately |
| `session/request_permission` | Runtime Interaction plus authority compiler; never direct renderer or peer authority                      |
| filesystem/terminal calls    | brokered workspace capabilities, rooted and scoped to the negotiated session grant                        |
| MCP server config            | capability reference or short-lived broker material; never durable raw credential storage                 |
| `_meta` and extensions       | bounded native metadata with namespace, version, redaction, and retention policy                          |

ACP must not collapse OpenAgents' queue-versus-steer contract. If the standard
method cannot express the distinction, the adapter should use a safe default
and optionally advertise a namespaced extension. It must not infer user intent
from concurrent prompt timing. [decision]

### 7.3 Authority rules

Negotiated capability is not authorization. It means only that the peer can
speak a method. Every privileged reverse request still requires:

- authenticated peer and current connection/session generation;
- workspace root and canonical path validation;
- method-specific grant intersection;
- an exact Runtime Interaction when user judgment is required;
- bounded arguments and output;
- cancellation/deadline propagation;
- one terminal outcome; and
- private evidence plus an appropriate public-safe receipt.

An ACP client offering terminal capability does not grant an ACP agent ambient
shell access. An ACP agent requesting file write does not bypass OpenAgents'
workspace capability broker. Full-access product mode may compile to a broader
grant, but the effective grant and containment outcome remain explicit.

### 7.4 Event fidelity

Retain the full decoded native ACP envelope before portable projection:

- peer and connection/session generation;
- direction, method, request/notification ID, and time;
- schema artifact and wire version;
- capability snapshot;
- native payload classification;
- projection result or explicit unsupported reason; and
- redaction/retention class.

Do not write raw wire bodies to ordinary logs. Do not smuggle raw prompts,
files, terminal output, headers, or credentials into generic error `data` or a
public receipt. A stable variant unknown to the pinned codecs must produce a
typed protocol-drift/degraded outcome and cannot mutate canonical state; retain
only its bounded private raw envelope for diagnosis and upgrade work. Unknown
extension metadata follows the same private retention bounds and cannot acquire
behavior without an explicit namespaced codec and peer profile.

## 8. Ordered implementation packet

### Phase ACP-0 — name and authority

1. Use “Agent Client Protocol” in contracts, docs, metrics, and UI.
2. Record outbound agent-method and reverse client-method coverage separately.
3. Pin the current stable schema artifact and wire version.
4. Decide which unstable families, if any, are required by the first peer.
5. Add the method/capability manifest and upgrade-diff check.

### Phase ACP-1 — protocol package

1. Generate checked-in schemas and manifests.
2. Implement or wrap one bidirectional stdio JSON-RPC connection.
3. Add bounded queues and pending maps.
4. Add deadlines, `$/cancel_request`, late-response policy, and exact process
   termination.
5. Add safe diagnostic and private native-event sinks.
6. Provide typed outbound calls and reverse-request handlers with scoped
   registration.

### Phase ACP-2 — conformance fixtures

**Implemented by #8890.** The checked package covers the exact stable manifest,
known variant inventories, a production-transport-backed scripted peer,
concurrent sessions and reverse requests, deterministic redacted transcripts,
MCP reference refusal/redaction, fault and compatibility artifacts, and inert
by default Grok/Cursor live probes. It also includes an explicitly armed
two-peer production runner whose closed artifact is validated by
`check:release` but cannot promote the release matrix. Product-native projection and reverse
authority are implemented by [#8891](https://github.com/OpenAgentsInc/openagents/issues/8891)
and specified in the [runtime bridge ADR](../adr/2026-07-16-agent-client-runtime-bridge.md).
The provider-independent lifecycle now single-flights startup, capability-gates
stable optional methods, serializes prompts, drains accepted inbound frames,
fences replay from live work, separates cancellation sources, observes process
exit, and performs bounded generation-safe recovery. Grok real-binary
admission is implemented by #8893. Cursor's separate `agent acp` composer,
`cursor_login` negotiation, modes/configuration surface, and four directional
extension gates are implemented by #8894. The #8897 matrix now owns final
release claims and currently denies a general supported claim for both peers.
#8895 adds the main-owned provider control host, shared main/preload/renderer
contract, Desktop Settings state/actions, validated alternate executable path,
canonical free-form question path, and refs-only support artifact. The host
still derives “supported” exclusively from admission evidence; this UI work
does not promote a peer or generalize proof to another registry agent.

1. Exercise every stable method and notification.
2. Assert capability-gated call refusal.
3. Test reverse permission, filesystem, and terminal requests while a prompt is
   outstanding.
4. Test multiple sessions on one connection.
5. Test fragmented, malformed, oversized, and partial lines.
6. Test overload, interruption, timeout, child exit, writer failure, late
   response, restart, and teardown.
7. Test redaction against prompts, file contents, terminal output, headers, and
   error data.
8. Keep live Cursor/Grok/official-example probes opt-in and outside hermetic
   ordinary CI. Ad hoc probe results are diagnostic, not release authority; the
   pinned Grok/Cursor matrix required by the release gate is authoritative for
   provider claims.

### Phase ACP-3 — ACP client adapter and Grok/Cursor proof

**Implemented structurally by #8893 and #8894; release evidence is gated by
#8897's checked matrix.** The Cursor implementation deliberately resolves only a PATH candidate
whose real basename is `cursor-agent`, pins the launcher plus every regular file
in its installation closure and the normalized date version, rechecks that
closure before spawn, supplies only `HOME` plus a fixed `/usr/bin:/bin` launcher PATH,
and never treats another vendor's `agent` shim as Cursor. It negotiates only an
advertised `cursor_login` through a typed external-browser interaction. Stable
session modes and config options come from the peer; model discovery is a
versioned, bounded `cursor/list_available_models` response with explicit
provenance. The parameterized model-picker `_meta` flag, reverse authority, and
all Cursor extensions require fresh evidence bound to the admitted version and
digest. The observed version remains experimental because the #8897 matrix
does not promote it.
Initialize-only and incomplete prompt probes remain diagnostic and cannot mint
their own feature evidence.

1. Replace `grok-harness`'s raw client with the shared package.
2. Correct advertised capabilities before enabling any peer.
3. Map all native updates losslessly into the provider runtime and canonical
   projections.
4. Route permissions and elicitation through Runtime Interactions.
5. Add brokered filesystem/terminal capabilities only after their grants and
   tests exist.
6. Prove cancel, resume, restart repair, session lineage, and event drain.
7. Implement separate Grok and Cursor launch/auth/model/extension profiles.
8. Prove Cursor login, modes/configuration, model discovery, and
   `cursor/ask_question`, `cursor/create_plan`, `cursor/update_todos`, and
   `cursor/list_available_models` independently from Grok.
9. Prove Grok cached-token/API-key auth, update streaming, question extensions,
   and any unstable model/completion compatibility fallback independently from
   Cursor.

This phase proves Grok and Cursor control through the shared client. It adds
protocol support without making wire-version compatibility a blanket product
claim. Each additional registry agent still needs an explicit product
decision, launch/install/auth profile, capability matrix, UX, and release
evidence.

### Phase ACP-4 — registry discovery and additional agents

1. Consume the curated registry's versioned v1 JSON and validate it against the
   registry schema.
2. Persist the registry version and digest used for every install decision.
3. Treat registry entries as discovery metadata, not executable authority or
   proof of artifact integrity.
4. Lower `binary`, `npx`, and `uvx` distributions through an explicit installer
   broker; never interpolate registry strings into a shell.
5. Verify platform, archive/package identity, executable path, and launch
   arguments before first run.
6. Require an initialize/authentication conformance probe before admitting an
   installed peer.
7. Build a separate peer profile for launch, authentication, capabilities,
   stable/unstable methods, extensions, update policy, and known deviations.
8. Admit agents one at a time behind product and release gates even when the
   generic wire conformance suite passes.

### Phase ACP-5 — extensions and remote transport

1. Add namespaced extensions only for a demonstrated semantic gap.
2. Version every extension schema and capability.
3. Keep current official unstable families separately gated.
4. Treat HTTP/WebSocket remote ACP as a later authenticated transport profile,
   not a free consequence of stdio support.

## 9. Acceptance gates

ACP support is not complete until:

- the exact schema artifact, wire version, stable/unstable status, and method
  coverage are generated and inspectable;
- both JSON-RPC directions operate concurrently;
- every advertised capability has a real handler and authority path;
- no unadvertised optional call is made;
- queues, messages, outputs, and pending requests are bounded;
- request and session cancellation are distinct and tested;
- connection and session generations prevent stale completion;
- restart and replay/live reconciliation have deterministic outcomes;
- full native events are retained privately and portable projection loss is
  explicit;
- ordinary diagnostics contain no raw private payloads;
- filesystem, terminal, MCP, and permission paths are brokered and receipted;
- provider extensions remain edge adapters;
- pinned live Grok and Cursor compatibility matrices pass independently; and
- README/product language distinguishes implemented, experimental, and planned
  support.

## 10. What to copy and what to refuse

### Copy

- generated, pinned schema authority;
- typed outbound and reverse-request facades over one connection;
- one provider-independent session runtime;
- idempotent shared startup;
- prompt serialization and event-drain barriers;
- structured errors with exact process termination;
- replay/live gating and child-session filtering;
- capability defaults of false;
- provider-specific launch/auth/model/extension layers;
- native provenance on projected events; and
- payload-free ordinary diagnostics.

### Refuse

- copying the stale `v0.11.3` unstable artifact;
- treating wire version 1 as a frozen method table;
- unbounded protocol queues;
- unsupported acknowledgement claims;
- swallowing callback failures without a health signal;
- mutable handler registration without scoped teardown;
- unconditional authentication;
- calling optional methods without central capability guards;
- one process per session as a universal law;
- translating standard mode control into a provider convention;
- losing unprojected ACP update variants;
- provider extensions as canonical product state; and
- advertising filesystem or terminal capability before the handler and
  authority path exist.

## Final assessment

T3 Code provides the best TypeScript/Effect reference in the audited set for
**hosting ACP agents**. Its strength is not merely its generated protocol
package; it is the separation between wire protocol, session lifecycle,
provider quirks, and canonical runtime events. Cursor and Grok share the hard
parts while retaining thin peer-specific adapters.

The implementation also shows why ACP should remain an edge protocol.
T3 supports only a subset of its pinned schema in product behavior, uses
extensions for important provider interactions, and normalizes only the event
families its workbench currently needs. OpenAgents requires a more explicit
method manifest, bounded transport, loss accounting, authority compilation,
and receipt boundary.

The correct OpenAgents target is therefore:

> one current, generated, bidirectional Agent Client Protocol foundation; an
> outbound client adapter that proves `grok agent stdio` and Cursor `agent acp`
> independently, then admits other registry agents through capability-gated
> peer profiles; and no second domain model, authority path, or event writer.

## Primary source map

### T3 Code

- [`effect-acp` package](https://github.com/pingdotgg/t3code/tree/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/packages/effect-acp)
- [schema generator](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/packages/effect-acp/scripts/generate.ts)
- [patched protocol](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/packages/effect-acp/src/protocol.ts)
- [client facade](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/packages/effect-acp/src/client.ts)
- [agent facade](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/packages/effect-acp/src/agent.ts)
- [structured errors](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/packages/effect-acp/src/errors.ts)
- [session runtime](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/apps/server/src/provider/acp/AcpSessionRuntime.ts)
- [runtime model](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/apps/server/src/provider/acp/AcpRuntimeModel.ts)
- [runtime-event mapping](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/apps/server/src/provider/acp/AcpCoreRuntimeEvents.ts)
- [native diagnostics](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/apps/server/src/provider/acp/AcpNativeLogging.ts)
- [Cursor adapter](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/apps/server/src/provider/Layers/CursorAdapter.ts)
- [Grok adapter](https://github.com/pingdotgg/t3code/blob/bde0a4c0dd2d420a5fd71f39448d5db1bab078da/apps/server/src/provider/Layers/GrokAdapter.ts)

### Grok

- [current Grok source](https://github.com/xai-org/grok-build/tree/c68e39f60462f28d9be5e683d9cbe2c57b1a5027)
- [workspace protocol dependency](https://github.com/xai-org/grok-build/blob/c68e39f60462f28d9be5e683d9cbe2c57b1a5027/Cargo.toml)
- [`xai-acp-lib`](https://github.com/xai-org/grok-build/tree/c68e39f60462f28d9be5e683d9cbe2c57b1a5027/crates/codegen/xai-acp-lib)
- [Grok ACP agent implementation](https://github.com/xai-org/grok-build/blob/c68e39f60462f28d9be5e683d9cbe2c57b1a5027/crates/codegen/xai-grok-shell/src/agent/mvp_agent/acp_agent.rs)

### Agent Client Protocol

- [official ACP repository](https://github.com/agentclientprotocol/agent-client-protocol)
- [architecture](https://agentclientprotocol.com/get-started/architecture)
- [protocol schema](https://agentclientprotocol.com/protocol/schema)
- [extensibility](https://agentclientprotocol.com/protocol/extensibility)
- [current schema release](https://github.com/agentclientprotocol/agent-client-protocol/releases/tag/schema-v1.19.0)
- [official TypeScript SDK](https://github.com/agentclientprotocol/typescript-sdk)
- [official registry](https://agentclientprotocol.com/get-started/registry)

### OpenAgents

- `packages/grok-harness/src/acp-client.ts`
- `packages/grok-harness/src/chat-runtime.ts`
- `packages/grok-harness/src/event-projector.ts`
- `packages/grok-harness/src/mock-acp-server.ts`
- `apps/openagents-desktop/src/runtime-gateway.ts`
- `apps/openagents-desktop/src/provider-runtime-host.ts`
- `packages/agent-runtime-schema`
