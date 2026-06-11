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

## Decision

The remote bridge should be a projection and scoped-action layer over the local
runtime, not a second runtime. Local policy, local workspace boundaries, and
normal approval semantics remain authoritative.

