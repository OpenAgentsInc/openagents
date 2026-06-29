# Notifications And Attention System Audit

Date: 2026-06-11

This is system #25 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should represent local notifications, terminal
attention signals, background completion, waiting-for-user states, quiet hours,
and status handoff.

## Target

Build an attention system that separates user-facing notifications from runtime
state.

The runtime should publish typed attention events. A notification coordinator
should decide when, where, and how to notify the user based on priority,
terminal capability, user preferences, privacy, and current focus state.

## User-Visible Capability

The user should be able to:

- Know when the agent is waiting for input.
- Know when a permission, approval, or question needs attention.
- Know when a long-running foreground task completes.
- Know when background tasks finish, fail, or need review.
- Choose terminal bell, terminal-native notification, desktop notification, or
  no external notification.
- Avoid duplicate notifications for the same event.
- Suppress low-value notifications during active typing.
- Respect quiet hours and focus modes.
- See current waiting reason in a status surface.

Notifications should be helpful, sparse, and private by default.

## Attention Model

Represent attention as typed events:

- Idle completion.
- Waiting for user input.
- Waiting for approval.
- Waiting for a worker or background task decision.
- Waiting for sandbox or workspace policy.
- Dialog or modal open.
- Background task completed.
- Background task failed.
- Remote session disconnected.
- External service needs login.
- Diagnostic warning.
- Session resumed or forked.

Each event should include priority, dedupe key, invalidation keys, optional
folding behavior, timeout, private-data classification, and preferred channel.

## Core Design

Define a `NotificationCoordinatorService` that owns attention queueing,
deduplication, channel selection, and delivery.

Suggested service boundary:

```ts
interface NotificationCoordinatorService {
  publish(event: AttentionEvent): Effect.Effect<NotificationReceipt, NotificationError>
  current(): Effect.Effect<AttentionSnapshot, NotificationError>
  dismiss(request: NotificationDismissRequest): Effect.Effect<void, NotificationError>
  configure(request: NotificationConfigRequest): Effect.Effect<NotificationConfigReceipt, NotificationError>
}
```

Runtime services should publish attention events. They should not write raw
terminal escape sequences, call desktop APIs, or decide quiet-hour policy.

## Queue Semantics

The queue should support:

- Priorities: low, medium, high, immediate.
- Dedupe by stable key.
- Invalidation by related keys.
- Folding duplicate events into a single updated event.
- Requeueing interrupted lower-priority events.
- Expiring stale notifications.
- Delivery receipts.
- Current attention snapshot for status bars.

Immediate events may preempt lower-priority events. Non-immediate events should
not spam the user when a newer event invalidates them.

## Channel Selection

Supported channel classes:

- In-terminal status message.
- Terminal bell.
- Terminal-native notification escape sequence.
- Desktop notification bridge.
- External hook.
- Disabled notification.

Channel selection should consider:

- User preference.
- Terminal capability.
- SSH or multiplexer environment.
- Operating-system permission status.
- Focus mode or quiet hours.
- Privacy class of the message.
- Whether the user is actively typing.

If the preferred channel fails, the coordinator should fall back safely or
record a delivery failure without crashing the agent.

## Status Handoff

The agent should publish a compact status snapshot for external monitors:

- Session id ref.
- Workspace ref.
- State: idle, busy, waiting, failed, closed.
- Waiting reason.
- Active task count.
- Last activity timestamp.
- Public-safe current phase.

Status snapshots should never include prompts, raw file paths, secrets, or
private transcript content.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for notification coordination and status snapshots.
- `Schema` for attention events, queue entries, channel configs, and receipts.
- `Queue` for runtime attention events.
- `Ref` for current event, quiet-hour config, and delivery history.
- `Stream` for status updates.
- `Schedule` for notification timeout, idle threshold, and quiet-hour refresh.
- `Layer` for terminal, desktop, hook, and disabled channel implementations.

Delivery should be best-effort. Attention state itself should remain typed and
queryable even if every external channel is disabled.

## Privacy Rules

- Notification titles and bodies must be public-safe by default.
- Do not include prompts, file contents, raw paths, tokens, or customer data in
  external notifications.
- Do not persist raw desktop-notification payloads.
- Do not send private diagnostics through external hooks without policy.
- Do not leak internal task names if the current surface is public.
- Do not trigger attention for low-priority streaming noise.

## Safety Rules

- Do not let notification delivery block the agent runtime.
- Do not let duplicate background events spam the user.
- Do not notify after a session is closed unless explicitly requested.
- Do not emit terminal escape sequences from general runtime code.
- Do not assume a terminal supports a notification protocol.
- Do not treat a delivery failure as task failure.
- Do not leave stale waiting status after the user responds.
- Do not bypass quiet hours for non-urgent events.

## Tests

Minimum regression coverage:

- Publish and order low, medium, high, and immediate events.
- Dedupe events by key.
- Fold duplicate background-completion events.
- Invalidate current and queued events.
- Requeue lower-priority events after immediate preemption.
- Respect quiet hours and disabled channels.
- Fall back when a terminal channel is unsupported.
- Keep external notification payloads public-safe.
- Update waiting reason for approval, input, dialog, and task states.
- Clear waiting status after user response.
- Emit status snapshots without raw private data.
- Continue runtime execution when notification delivery fails.

## OpenAgents Translation Notes

When promoted, map attention events to OpenAgents operator status, Pylon task
state, approval refs, public/private projection boundaries, and receipt
notifications. Verify live issue state before claiming notifications, quiet
hours, or external status handoff are implemented.

## Decision

Notifications should be a policy-governed attention layer over typed runtime
events. The agent should expose clear waiting states and sparse completion
signals without leaking private context or letting delivery failures affect
work execution.
