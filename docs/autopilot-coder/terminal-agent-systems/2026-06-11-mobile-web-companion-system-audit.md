# Mobile And Web Companion System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #40 from the Bun/Effect terminal-agent systems list. It defines
the mobile and web companion surfaces for monitoring, approving, steering, and
reviewing terminal-agent work.

## Target

Build companion surfaces that expose the right slice of a running session to a
browser, phone, or lightweight web app while preserving terminal runtime
authority, private-data boundaries, and receipt-backed completion.

## User-Visible Capability

Users should be able to:

- See active runs, paused runs, blockers, and waiting decisions.
- Approve or deny explicit prompts.
- Read public-safe progress without opening a terminal.
- Inspect artifacts and closeout refs.
- Cancel, pause, or resume when policy permits.
- Add a bounded instruction or answer a question.
- Receive notifications when the agent needs attention.
- Continue from mobile without exposing raw private session material by
  default.

The companion UI should be fast, responsive, and status-oriented. It should
not pretend to be a full terminal unless a private-channel policy explicitly
allows that.

## State Model

The companion projection should include:

- Run id.
- Goal or mission ref.
- Current status.
- Waiting decision refs.
- Latest public-safe progress message.
- Artifact refs and visibility.
- Cost/budget status where applicable.
- Last update timestamp and staleness declaration.
- Available actions for the current user.

The surface should render unavailable data honestly. Missing artifacts,
stale projections, and private-only events should appear as scoped caveats, not
empty success states.

## Companion Transport And Resume Model

The companion surface should read from a typed event stream and write through
typed action requests. It should not scrape terminal output or replay arbitrary
commands.

Each projected row and event should include:

- Node or runtime ref.
- Run/session ref.
- Event id and stream sequence.
- Projection level and redaction class.
- Generated timestamp and received timestamp.
- Staleness status.
- Available action refs for the current pairing.

Mobile and web clients should persist the last accepted sequence per stream and
reconnect with that cursor. The server should replay missing lossless events or
return a fresh snapshot with an explicit lag caveat when the cursor is outside
retention. Duplicate events and duplicate action submissions should be ignored
by idempotency key.

Use two delivery tiers:

- Lossless: decision prompts, decision cancellation/resolution, visible
  progress deltas, run/session completion, artifact availability, and
  capability or pairing changes.
- Best effort: verbose progress, high-volume logs, heartbeats, and transient
  telemetry.

The UI should expose stale/offline/read-only state in the chrome of the page or
screen, and action controls should be disabled from capability refs rather than
hidden by ad hoc client logic.

## Bun/Effect Boundary

Use Effect services for:

- `CompanionProjectionService`: derives mobile/web status rows.
- `DecisionQueueService`: lists and resolves action-required records.
- `CompanionActionService`: validates pause, resume, cancel, answer, and
  approval requests.
- `NotificationPreferenceService`: controls push, email, local, and quiet
  hours.
- `CompanionReceiptService`: writes action and delivery receipts.

Use Schema for projection rows, actions, notification preferences, and action
receipts. Use Stream for live updates. Use Schedule for retrying notification
delivery.

## Authority Rules

Companion actions must check:

- User identity and session pairing.
- Team or mission membership.
- Capability refs for the requested action.
- Freshness of the decision prompt.
- Idempotency key.
- Budget and spend boundaries.
- Whether the action has direct effects or only records intent.

Approving a PR draft, a shell command, a provider-spend action, or a payout
must remain separate actions with separate policy refs.

## Decision And Action Handling

Decision cards should be derived from explicit server-originated decision
requests. A decision request should include a request id, action ref, effect
summary, expiry, allowed verbs, required capability ref, and public-safe
evidence refs. The companion may display private evidence only when the current
projection level allows it.

Responses should carry the original request id, action ref, idempotency key,
chosen verb, optional bounded answer text, and current pairing ref. Late,
duplicate, cancelled, or already-resolved responses should return typed results
that the UI can render without retry loops.

Free-form user messages are separate from approvals. A bounded instruction can
be queued only while the run/session accepts steering and the pairing has the
specific steering capability. Interrupt, cancel, pause, resume, and spawn are
distinct actions with distinct refs and receipts.

Offline action queues should be conservative. Queue only actions that remain
valid after reconnect, attach idempotency keys, and discard actions whose
decision expiry or capability freshness has passed. Approval prompts should
prefer "must be fresh" semantics over delayed send.

## OpenAgents Translation Notes

As of 2026-06-11, the OpenAgents unified roadmap identifies decision records
and live sync scopes as ready substrate, but states that user-facing mobile
decision surfaces remain missing. The terminal-agent README has no imported
mobile/web companion audit.

Related open issue anchors:

- #4765 decision queue and notifications is the direct MVP issue.
- #4768 overnight unattended run requires companion status and attention
  events to be verified across both lanes.
- #4773 API parity contract requires every companion action to have an API
  peer.
- #4770 team budgets and spend-to-evidence join affects companion budget
  display.

Do not claim mobile control or web companion approval is live for terminal work
until the decision queue, notifications, and API parity receipts exist.

## Tests

Minimum coverage:

- Project running, blocked, waiting, completed, and failed runs.
- Render stale projections with explicit timestamps.
- Resolve decisions with idempotency.
- Reject action attempts from non-members.
- Keep private artifacts out of public companion views.
- Deliver notification events according to preferences.
- Preserve action receipts for pause, cancel, answer, and approval.
- Verify mobile-width rendering for decision actions.
- Reconnect from a stored event cursor and deduplicate replayed events.
- Render a lag caveat when the server must fall back to a fresh snapshot.
- Keep read-only paired clients from invoking interrupt, cancel, approval,
  spawn, or steering actions.
- Disable expired or externally resolved decision cards after receiving a
  decision cancellation/resolution event.
- Reject duplicate action submissions by idempotency key.
- Keep queued offline actions from sending after decision expiry or capability
  revocation.
- Surface overloaded or unsupported action responses as user-visible status
  rows rather than indefinite spinners.

## Decision

The companion system should be a workroom and decision projection, not a
parallel terminal. It should make remote supervision useful while keeping
effectful authority in the typed runtime.
