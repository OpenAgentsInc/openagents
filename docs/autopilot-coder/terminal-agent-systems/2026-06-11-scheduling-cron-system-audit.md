# Scheduling And Cron System Audit

Date: 2026-06-11

This is system #45 from the Bun/Effect terminal-agent systems list. It defines
how terminal-agent work can be scheduled, repeated, resumed, and continued
without granting unattended authority beyond explicit policy.

## Target

Build a scheduling system for delayed runs, recurring checks, overnight tasks,
retry windows, budget-aware continuation, and maintenance work.

Scheduled execution should be boring and inspectable. The user should know
what will run, when, with which budget, and under which approval policy.

## User-Visible Capability

Users should be able to:

- Schedule a run for a future time.
- Queue overnight work.
- Create recurring checks or maintenance jobs.
- Pause, resume, edit, or delete schedules.
- See next-run time, last-run result, and failure state.
- Set budget, workspace, provider, and notification policy.
- Require reapproval before risky recurring actions.

The schedule should not hide whether the next execution is allowed to write,
spend, push, deploy, or contact an external provider.

## Schedule Record Model

Each schedule should include:

- Schedule ref.
- Owner and team refs.
- Goal or work-order template ref.
- Workspace or repo ref.
- Trigger kind.
- Timezone.
- Next and last run timestamps.
- Budget policy.
- Permission policy.
- Provider or adapter preference.
- Notification policy.
- Retention policy.
- Status and blocker refs.

Each fired run should create a run receipt linked back to the schedule ref.

## Bun/Effect Boundary

Use Effect services for:

- `ScheduleRegistryService`: create, update, pause, delete, and list
  schedules.
- `SchedulerService`: computes due work and enqueues runs.
- `ContinuationPolicyService`: decides whether a stopped run may continue.
- `RecurringApprovalService`: checks whether authority must be refreshed.
- `ScheduleProjectionService`: shows schedule status and freshness.
- `ScheduleReceiptService`: records fired, skipped, failed, and cancelled
  occurrences.

Use Schedule for retry and recurrence logic. Use Queue for due work. Use
Schema for trigger types, recurrence, timezone, and permission policies.

## Safety Rules

- Unattended runs cannot gain new authority at fire time.
- Expired credentials, missing budget, revoked workspace, or changed policy
  skip the run with a receipt.
- Recurring jobs must have a maximum budget and cancellation path.
- Push, deploy, spend, payout, and public-post actions need explicit policy.
- Schedules must survive restart without double-firing.
- Schedule projections must show staleness.
- Timezone changes are auditable.

## OpenAgents Translation Notes

As of 2026-06-11, the unified Autopilot roadmap identifies scheduled launches
and auto-continuation as missing product work. The terminal-agent README does
not yet include a scheduling/cron audit.

Related open issue anchors:

- #4764 scheduled launches and auto-continuation policy.
- #4768 overnight unattended run proof smoke.
- #4770 team budgets and spend-to-evidence join.
- #4773 API parity contract.
- #4786 Autopilot MVP ladder.

No claim should say queue-the-night-before or autonomous recurring execution is
live until schedule creation, fire, skip, continuation, budget, and notification
receipts exist.

## Tests

Minimum coverage:

- Create one-shot and recurring schedules.
- Fire due schedules exactly once.
- Skip when budget, credential, workspace, or policy is invalid.
- Pause, resume, edit, and delete schedules.
- Preserve timezone behavior across daylight-saving boundaries.
- Require reapproval for risky recurring actions.
- Record fired, skipped, failed, and cancelled receipts.
- Recover after process restart without duplicate runs.

## Decision

Scheduling should enqueue ordinary typed runs under predeclared policy. It
should not become a hidden automation channel with broader authority than the
foreground terminal.

