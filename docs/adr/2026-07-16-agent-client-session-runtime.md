# Agent Client Protocol session runtime

Status: accepted and implemented for the shared lifecycle boundary in [#8892](https://github.com/OpenAgentsInc/openagents/issues/8892).

This decision concerns Zed's **Agent Client Protocol** over JSON-RPC/stdio. It does not concern Linux Foundation Agent Communication Protocol or A2A.

## Decision

`AcpSessionRuntime` is the provider-independent lifecycle owner above the bounded stdio transport and runtime bridge. Grok and Cursor supply admitted launch, authentication selection, extension allowlists, and peer/version gates; neither provider owns a second session state machine.

Runtime, provider session, canonical Thread, and Turn identity are separate:

| Identity             | Scope                      | Rule                                                               |
| -------------------- | -------------------------- | ------------------------------------------------------------------ |
| transport generation | one owned child process    | every callback captures it; an exit closes only its own generation |
| session generation   | one attach attempt         | current Grok/Cursor policy permits one root session per process    |
| peer `sessionId`     | opaque provider attachment | map to a canonical Thread; never use it as the Thread ID           |
| turn generation      | one serialized prompt      | exactly one terminal outcome; late content is quarantined          |

Start is single-flight. It creates one transport, validates wire-v1 initialize request/response, records peer info/capabilities/auth IDs/extensions, performs only an advertised and locally authorized authentication method, and optionally bootstraps one session. Empty auth methods skip authentication; non-empty methods without an authorized intersection fail closed.

Stable optional operations are called only when advertised: load, list, delete, resume, close, and logout. Modes and configuration values are session-advertised allowlists; invalid values are refused locally and no-op changes are suppressed. `session/fork` uses the unstable definition codecs only when an exact peer-version profile explicitly enables it.

## State and ordering

The runtime state is `idle → starting → ready → recovering | stopping → stopped`, with `failed` entered on protocol/start/process failure. A session is `replay → live → closed`; new sessions open directly into live after their accepted-frame barrier. A turn is queued on a per-session promise tail, becomes active for one `session/prompt`, drains every frame already accepted by the transport, and settles once.

`session/update` is the content stream. The `session/prompt` result contributes only validated completion metadata and `stopReason`. The transport exposes an accepted-frame barrier: after the response it requires two consecutive `setImmediate` check turns with an unchanged accepted-frame watermark and an empty router (within a hard 16-turn bound). This covers a coalesced next stream read without polling arbitrary “stable text.” Peer profiles and conformance evidence must treat frames beyond that explicit bound as late. Content accepted before the barrier belongs to the active turn. Content arriving after terminal settlement is retained only as a quarantined late-update record and cannot mutate the completed turn.

Load/resume create the binding in replay phase before their response barrier. Replay updates and returned mode/config snapshots merge while the live gate is closed. The gate opens once after the accepted-frame barrier, so the first new prompt cannot race replay.

Cancellation sources remain distinct: user stop, transport abort, protocol `session/cancel`, shutdown, and restart. Cancel is idempotent before, during, and after settlement. It sends at most one session cancellation for an active turn; transport request abort and process termination have their own outcomes. Assistant/reasoning channels and unfinished tools terminalize once on completion, refusal, cancellation, timeout, process exit, or protocol failure.

## Recovery and capability material

Recovery disposes the failed generation, applies bounded exponential backoff, initializes a fresh admitted transport, and selects only an advertised resume or load path. Receipts distinguish reattached, new-session-required, missing binary, auth loss, incompatible version, missing session, protocol drift, crash loop, and cancellation. An exited process is never reported as running.

MCP inputs are bounded, scoped, expiring references. They are resolved just in time for the intended runtime/session generation and `session/new`, supported load, or supported resume call. The materializer must return the exact ordered ref/transport identities it resolved; mismatches fail before a session request. Durable snapshots retain only reference receipts; the materializer's dispose callback completes before a successful attach outcome and also runs on refusal/failure paths. Missing, expired, out-of-scope, duplicate, malformed, unavailable, or mismatched references fail before a session request.

## Deterministic race matrix

| Race                                        | Outcome                                                                   |
| ------------------------------------------- | ------------------------------------------------------------------------- |
| concurrent start calls                      | one spawn/initialize/auth/bootstrap flight                                |
| update before prompt response               | applied to the active turn in receive order                               |
| prompt response before final accepted frame | accepted-frame barrier applies the frame before terminal settlement       |
| update after barrier                        | quarantined as `late-after-turn`                                          |
| prompt overlap                              | serialized per session; unrelated reverse requests remain transport-owned |
| cancel during request or drain              | one cancellation source and one terminal turn                             |
| process exit while idle/active              | generation becomes failed; sessions close; active turn terminalizes       |
| replay and first new prompt                 | replay gate opens exactly once after load/resume drain                    |
| stale generation callback                   | ignored without mutating the replacement generation                       |
| restart loop                                | bounded attempts/backoff, then typed crash-loop outcome                   |

## Consequences

The runtime is usable by the required Grok and Cursor profiles without embedding provider names or auth IDs. It deliberately enforces single-root ownership until a peer profile and conformance evidence prove multi-session safety. Peer-specific live compatibility and product UX remain gated by #8893–#8897.

The executable identity pin is produced by the trusted admission path; production profile wiring must launch the admitted path rather than repeat a caller-controlled lookup. Pre-session MCP authority must use a provisional canonical/session-generation lease and atomically bind the returned opaque peer session ID; it must never invent a future provider ID.
