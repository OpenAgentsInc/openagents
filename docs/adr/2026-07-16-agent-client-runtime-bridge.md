# Agent Client Protocol runtime bridge

Status: accepted and implemented for the shared bridge boundary in [#8891](https://github.com/OpenAgentsInc/openagents/issues/8891).

Here ACP means Zed's **Agent Client Protocol**: the JSON-RPC protocol used by `grok agent stdio` and Cursor `agent acp`. It does not mean Linux Foundation Agent Communication Protocol and it does not mean A2A.

## Decision

`@openagentsinc/agent-client-runtime-bridge` is the only protocol-to-domain boundary. Transport owns bytes, request correlation, deadlines, and process generation. Peer profiles own launch, authentication, optional methods, and namespaced extensions. The bridge owns bounded native evidence, canonical identity/projection, reverse-request authority, interactions, and refs-only receipts.

A provider session ID is scoped by `(profile, connection, process generation, provider session ID)`. It is an attachment, never an OpenAgents Thread or Turn ID. Work Unit references are attached only when a fleet launch already supplied one.

Every validated update is admitted to the private native evidence store before canonical delivery. The native envelope records profile/version, connection/generation, method, request or receive/update ID, provider session, timestamps, discriminant, extension namespace, validated payload, digest, and byte length. UI/log/persistence projections receive only bounded canonical fields and native references.

## Stable mapping

| Native input                  | Canonical projection                                     | Merge rule                                                                   |
| ----------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `user_message_chunk`          | user Item state plus private content reference           | append accepted chunks in wire order                                         |
| `agent_message_chunk` text    | `text.delta`, then one `text.completed`                  | repeated equal chunks are legitimate; duplicate envelope is not              |
| `agent_thought_chunk` text    | `reasoning.delta`, then one `reasoning.completed`        | same channel FSM as message text                                             |
| non-text content              | `raw.sidecar_ref` plus degraded/attachment state         | never stringify blobs into text                                              |
| `tool_call`                   | one canonical tool item and `tool.call`                  | provider report is evidence, not OpenAgents execution authority              |
| `tool_call_update`            | patch metadata; terminal `tool.result` or `tool.error`   | absent retains, replace-on-present content/locations, terminal states absorb |
| `plan`                        | atomic plan snapshot                                     | full-list replacement; informative, not approval                             |
| commands, mode, config        | provider/session metadata snapshots                      | complete replacement; IDs remain generation scoped                           |
| session info                  | provider/session metadata                                | partial patch; explicit null clears                                          |
| usage                         | cumulative context/cost snapshot                         | replace, never sum or mislabel as generation tokens                          |
| prompt response               | exactly one `turn.finished` with native reason reference | standard/private completion races settle once                                |
| unknown stable/vendor variant | private sidecar plus degraded state                      | no canonical state mutation or crash                                         |

Accepted stop mappings are `end_turn → stop`, `max_tokens → length`, `cancelled → cancelled`, and `refusal → content-filter`; unrecognized reasons map to `unknown` while preserving a private native reference. `max_turn_requests` remains `unknown` rather than asserting tool-call semantics.

Admission IDs use `(connection, generation, receive sequence)`, not payload hashes: adjacent identical text chunks must survive. Reusing an admitted envelope is a no-op. Lower/out-of-order sequence, old generation, unowned session, terminal tool regression, and post-turn updates are quarantined visibly. Tool states follow `absent → pending → in_progress → completed | failed`, with terminal states absorbing.

## Authority and capabilities

Reverse methods never execute provider-supplied effects directly. The bridge accepts injected session authority, Runtime Interaction, workspace, terminal, MCP launch-material, and receipt ports. Capabilities are immutable for a connection generation and true only when all of handler installed, session-scoped grant, tested implementation, and current health were asserted. Calls recheck negotiated capability, connection/generation/session ownership, authentication, scope, and broker health.

The Node broker adapter implements canonical workspace containment (including realpath/symlink checks), UTF-8 and byte/line limits, atomic writes, command/cwd/environment policy, session-owned process lifetimes, bounded redacted output, cancellation, and refs-only evidence. Reverse transport registration validates both request and response schemas and derives idempotency from the native JSON-RPC request ID.

| Reverse request              | Authority path                                                                             | Success shape                                                       |
| ---------------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------- |
| `session/request_permission` | canonical policy plus durable `RuntimeInteraction` decision                                | exact selected offered option, or cancelled                         |
| filesystem read/write        | normalized workspace broker with containment, symlink, encoding, byte, cancellation policy | protocol response plus refs-only receipt                            |
| terminal lifecycle           | command/cwd/env policy and owned process broker with bounded output/cancellation           | protocol response plus refs-only receipt                            |
| MCP material                 | scoped, expiring capability-broker reference resolved only for authorized session creation | native launch material is callback-scoped; receipt stores refs only |
| Grok/Cursor questions        | `provider_question` interaction bound to generation/session/request/deadline               | profile codec returns exact native response                         |
| Cursor plan request          | `plan_review` interaction; plan notifications remain snapshots                             | profile codec returns exact native response                         |
| Cursor todos                 | namespaced work/plan snapshot                                                              | notification only; never implicit authority                         |

Permission refusal selects one peer-offered rejection option; cancellation/expiry returns the protocol's cancelled outcome. Filesystem and terminal faults use bounded structured failures and never copy paths, commands, contents, output, environment, or secrets into error data. MCP credentials, headers, environment, and raw launch material must not enter native evidence, logs, renderer state, or durable receipts.

While a permission/question/plan interaction is pending, ordinary user input queues a later prompt. Only a typed interaction decision may steer the active reverse request. Renderer callbacks are never authority.

## Redaction and evidence

Native payloads remain in the private bounded store. Canonical events contain safe summaries and refs. Receipts contain method, scoped identity, outcome, fault code, timestamps, and safe evidence refs only. Prompt text, filesystem content, terminal output, auth material, MCP material, and provider extension bodies are redacted at log, receipt, persistence, and renderer boundaries.

The Grok compatibility harness delegates update projection to this shared bridge. Provider profile, real-binary admission, Desktop interaction rendering, recovery, and registry expansion remain separately gated by #8893–#8897.
