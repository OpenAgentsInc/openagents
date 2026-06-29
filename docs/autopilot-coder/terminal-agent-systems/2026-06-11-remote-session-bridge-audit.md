# Remote Session Bridge Audit

Date: 2026-06-11

This is system #39 from the Bun/Effect terminal-agent systems list. It defines
how a local terminal session can be observed, resumed, steered, or approved
from another OpenAgents surface without turning the terminal into an unbounded
remote-control endpoint.

## Target

Build a remote session bridge that exposes selected session events and selected
control actions through capability refs, signed connection records, and scoped
approval policy.

The bridge is for continuity and supervision. It must not grant blanket file,
shell, provider, wallet, repository, or desktop authority to a remote client.

## User-Visible Capability

Users should be able to:

- Pair a companion surface with a running terminal session.
- See current run status, waiting-for-user prompts, and public-safe progress.
- Approve, deny, or answer explicit permission requests.
- Send a bounded instruction into a paused or waiting session.
- Disconnect a device or revoke a pairing.
- Inspect which remote clients are connected.
- Continue locally after remote control ends.

The terminal should show when a remote bridge is active and what authority it
has.

## Core Contract

A remote bridge session should include:

- Local session id.
- Pairing id.
- Remote client id and device class.
- Capability refs.
- Expiration time.
- Approval channel refs.
- Event projection level.
- Revocation state.
- Public-safe connection receipt.

Remote events should be projections of the local event log. Remote clients
should not receive raw shell output, raw prompts, raw provider payloads, private
repo content, secrets, or local paths unless the user grants a private-channel
policy for that exact run.

## Protocol Shape

The bridge should expose a versioned, typed protocol rather than a generic
remote command tunnel. A minimal first version should include:

- `bridge.pair`, `bridge.revoke`, and `bridge.clients.list`.
- `session.subscribe`, `session.unsubscribe`, `session.snapshot`, and
  `session.history`.
- `turn.start`, `turn.steer`, and `turn.interrupt`.
- `decision.resolve` and `decision.cancel`.
- `artifact.read`.
- `capability.list`.

The notification stream should use explicit event names, not log scraping:

- `session.status.changed`.
- `turn.started` and `turn.completed`.
- `item.progress.delta` for public-safe progress.
- `decision.requested`, `decision.cancelled`, and `decision.resolved`.
- `artifact.available`.
- `bridge.client.connected`, `bridge.client.disconnected`, and
  `bridge.client.revoked`.
- `stream.heartbeat` and `stream.lagged`.

Each request should carry a client request id, session/run ref when applicable,
capability ref, idempotency key, and caller pairing ref. Mutating requests
should serialize by local session/run id. Read-only list/search/snapshot
requests may run concurrently, but long-running read requests need cancellation
tokens so stale mobile views do not hold queues open.

Each event should carry an event id, monotonically increasing stream sequence,
session/run ref, generated timestamp, projection level, and redaction class.
The sequence is the resume cursor; the event id is the duplicate-detection key.

## Transport And Auth Model

Support the same protocol over multiple transports:

- Local loopback HTTP/SSE or a Unix socket for same-machine desktop attachment.
- Tailnet-reachable WebSocket or SSE-read plus POST-write for phone and tablet
  companions.
- A future relay transport using the same envelope and capability checks.

SSE-read plus POST-write is a strong default for mobile because it survives app
lifecycle changes and can resume from `Last-Event-ID` or an explicit sequence.
WebSocket is appropriate when the client needs lower-latency interactive
control. Both transports must share the same request, notification, and
server-originated decision schemas.

Any non-loopback listener must require a scoped credential. The first QR or
copy-paste pairing payload should be a short-lived bootstrap secret, not the
node's all-purpose operator token. The node should exchange that bootstrap for
a capability-scoped credential with issuer, audience, expiry, client id,
device class, pairing id, nonce or `jti`, and projection level. Store raw
credentials only on the client; the node should store token hashes or signing
material plus revocation state.

## Bun/Effect Boundary

Use Effect services for:

- `RemoteBridgeService`: pairing, connection lifecycle, revocation, and
  heartbeat.
- `RemoteProjectionService`: converts session events into public-safe,
  team-safe, or private-channel views.
- `RemoteApprovalService`: relays permission prompts and resolves exactly one
  answer.
- `RemoteInstructionService`: accepts typed remote messages when policy allows
  them.
- `RemoteAuditService`: writes connection and action receipts.

Use Stream for remote event feeds and heartbeats. Use Queue for inbound remote
actions. Use Scope to close sockets and invalidate pending resolvers on
disconnect.

## Authority Model

Separate capabilities:

- Observe public-safe progress.
- Observe private session detail.
- Answer approval prompts.
- Send typed user messages.
- Request cancellation.
- Resume or pause.
- Download artifacts.

Each capability should have its own policy ref. A mobile companion allowed to
approve a shell command is not automatically allowed to read raw command
output.

Remote approval answers must bind to a pending approval id, session id,
capability ref, and expiry. Free-form chat from a remote client is never an
approval.

## Resume, Backpressure, And Pending Requests

Subscribing to a running session should be atomic from the client's point of
view: the server captures a snapshot or history page, registers the connection,
and then replays still-pending decision requests before streaming new events.
This avoids a mobile client showing "no action required" while an approval was
already waiting.

Reconnect should be cursor-based. A client reconnects with its last accepted
sequence or event id; the server replays missing lossless events and ignores
duplicate client action ids. If the requested cursor is older than retention,
the server should send a fresh snapshot and an explicit `stream.lagged` caveat.

Event delivery should have two tiers:

- Lossless: decision request/cancel/resolve, assistant or public-safe progress
  deltas that form the visible transcript, turn/session completion, capability
  and pairing changes, artifact availability.
- Best effort: verbose progress, log chunks, heartbeat-only status, and other
  high-volume telemetry.

Bounded queues are required. If an inbound request queue is full, reject the
request with a typed overload error instead of accepting work that may never
run. If an outbound queue is full, preserve lossless events and drop only
best-effort events, emitting a lag marker with the number or range skipped.
Server-originated approval requests must never be silently dropped.

Pending decision requests should be stored by request id and action ref. If a
remote client disconnects, unresolved remote-only prompts either expire,
return to local handling, or are replayed to another authorized connection.
Unsupported remote request types should receive explicit error responses so the
local runtime does not wait forever.

## Safety Rules

- Pairings expire and can be revoked locally.
- The local terminal remains the primary authority unless explicitly delegated.
- Remote messages are schema-validated.
- Remote clients cannot introduce new tools.
- Remote clients cannot widen filesystem or shell authority.
- Remote approvals are single-use and idempotent.
- Bridge logs contain refs and statuses, not secrets or raw private payloads.
- Disconnection cancels unresolved remote-only prompts or returns them to local
  handling.
- Read-only viewers are a distinct capability class; they cannot interrupt,
  cancel, answer, spawn, or steer.
- `turn.steer`, `turn.interrupt`, `session.cancel`, and `session.pause/resume`
  are separate verbs with separate policy checks. A free-form instruction is
  not an interrupt or approval.
- Interrupts should bind to the active turn id or an explicit startup marker
  and should resolve only after the local runtime observes the interruption.

## Implementation Slices

Recommended implementation order:

1. Add the protocol envelope, schema fixtures, event cursor, and projection
   levels while keeping actions disabled.
2. Add pairing exchange, scoped credentials, active-client listing, and
   revocation receipts.
3. Add read-only subscribe/snapshot/history with cursor resume and lag markers.
4. Add server-originated decision requests, exactly-once response binding, and
   request cancellation.
5. Add bounded steering, pause/resume, interrupt, and cancel as separate
   capabilities.
6. Add artifact reads through artifact refs only.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has live Pylon and Autopilot surfaces that model
worker loops, assignments, public-safe refs, and companion-readable status, but
the terminal-agent README has no remote session bridge audit.

Related open issue anchors:

- #4765 decision queue and notifications: remote approval UX should reuse the
  same decision-action spine.
- #4768 overnight unattended proof smoke: remote session state is part of
  proving laptop-independent execution.
- #4773 API parity contract: bridge actions need agent-readable API peers.
- #4778 mission/work-order unification: bridge events should project from the
  same run and assignment record layer.

No live claim should say remote terminal control is shipped until there is a
pairing receipt, revocation receipt, remote approval test, and public/private
projection test.

## Tests

Minimum coverage:

- Pair and revoke a remote client.
- Reject expired and replayed pairing tokens.
- Project event streams at public-safe and private levels.
- Resolve one approval and reject duplicate answers.
- Deny remote tool-authority escalation.
- Recover local approval flow after disconnect.
- Preserve audit receipts without raw private data.
- Verify headless sessions can expose status without accepting remote input.
- Resume from a stream cursor and deduplicate replayed events.
- Force a fresh snapshot plus lag caveat when a cursor is outside retention.
- Reject overloaded inbound requests instead of hanging.
- Preserve lossless events while dropping best-effort events with lag markers.
- Replay pending decision prompts on subscribe to a running session.
- Return an explicit error for unsupported remote request types.
- Reject read-only viewer attempts to interrupt, cancel, answer, spawn, or
  steer.
- Resolve remote interrupts only after the local runtime observes the active
  turn interruption.

## Decision

The remote bridge should be a projection and scoped-action layer over the local
runtime, not a second runtime. Local policy, local workspace boundaries, and
normal approval semantics remain authoritative.
