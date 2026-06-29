# Permission And Approval System Audit

Date: 2026-06-11

This is system #6 from the Bun/Effect terminal-agent systems list. It covers
permission decisions, approval prompts, remembered trust, denials, permission
modes, hooks, unattended execution, classifier-assisted decisions, remote
approval channels, and audit records.

## Target

The permission system should be the single authority for deciding whether a
tool action can run. Tools can contribute context and suggested rules, but they
should not own the global approval model.

## User-Visible Capability

The user should be able to:

- Allow once.
- Allow for the session.
- Persist an allow rule.
- Deny once.
- Persist a deny rule.
- Force ask for matching future actions.
- Add or remove workspace directories.
- Switch permission mode.
- Review recent denials.
- Understand why an action asked, allowed, or denied.
- Approve remotely or through another control surface when configured.

## Core Design

Define a `PermissionService` that accepts normalized action requests and emits
decisions.

Suggested service boundary:

```ts
interface PermissionService {
  decide(request: PermissionRequest): Effect.Effect<PermissionDecision, PermissionError>
  requestApproval(request: ApprovalRequest): Effect.Effect<ApprovalDecision, PermissionError>
  applyUpdate(update: PermissionUpdate): Effect.Effect<PermissionContext, PermissionError>
  listRules(scope: PermissionScope): Effect.Effect<ReadonlyArray<PermissionRule>, PermissionError>
}
```

The interactive terminal, remote bridge, background worker, and non-interactive
runner should all use this service.

## Permission Context

The context should include:

- Current mode.
- Allow rules by source.
- Deny rules by source.
- Ask rules by source.
- Additional workspace directories.
- Whether prompts are available.
- Whether automated checks should complete before UI prompt.
- Sandbox policy.
- Local denial tracking state.
- Remote approval callbacks or channels.

Rule sources should be explicit:

- CLI argument.
- Session.
- Local settings.
- User settings.
- Project settings.
- Managed/policy settings.
- Command-provided temporary rule.
- Hook-provided decision.

## Decision Shape

Permission decisions should be typed:

- `allow`
- `deny`
- `ask`
- `passthrough`

Every decision should include:

- Decision reason.
- Source.
- Optional updated input.
- Suggested permission updates.
- Persistence destination, if applicable.
- Whether the decision was user-authored, config-authored, hook-authored,
  classifier-authored, or mode-authored.
- Redaction metadata for logs and public projections.

Decision reasons should include:

- Rule match.
- Mode.
- Hook.
- Classifier.
- Safety check.
- Working-directory boundary.
- Sandbox override.
- Async/headless no-prompt rule.
- External approval tool.
- Other typed reason.

## Permission Modes

Modes should be explicit and schema-backed:

- Default: ask when no rule proves safety.
- Plan: allow planning/read-only work, ask or deny mutation.
- Accept edits: allow workspace file edits that pass path/freshness policy.
- Bypass permissions: allow broad execution but still preserve audit records
  and non-bypassable hard safety checks.
- Do not ask/headless: deny or route to hooks/remote approval when prompting is
  unavailable.
- Auto/classifier mode: allow or deny through a classifier with fail-closed
  policy when configured.

Mode changes are policy changes inside the session and should be recorded.

## Approval Flow

1. Tool validates input.
2. Tool contributes permission metadata and suggested rules.
3. Permission service checks hard deny/safety rules.
4. Permission service checks always-ask rules.
5. Permission service checks allow rules.
6. Permission service checks mode-specific policy.
7. Hooks may approve, deny, or require prompt within their authority.
8. Classifier may approve/deny when enabled.
9. If still ask and prompts are available, enqueue an approval request.
10. If prompts are unavailable, deny unless an approved remote/hook path
   supplies a decision.
11. Apply any selected permission update.
12. Emit a decision audit event.

## Remembered Trust

Remembered trust should be represented as permission updates:

- Add allow rule.
- Add deny rule.
- Add ask rule.
- Replace rule set.
- Remove rule.
- Add workspace directory.
- Remove workspace directory.
- Set mode.

Only trusted destinations should persist. Session-level changes should die with
the session. Managed/policy rules should not be overwritten by user actions.

The UI may suggest a narrow reusable rule, but the service should validate that
the suggestion is not broader than the approved action.

## Headless And Background Rules

When a run cannot prompt the user:

- Run hooks and remote approval callbacks first if configured.
- Auto-deny if no non-interactive authority supplies a decision.
- Record the denial with a reason that prompts were unavailable.
- Do not hang waiting for a UI that cannot appear.
- Track repeated denials and stop loops that are clearly stuck.

Background agents may forward approval requests to a coordinator or mailbox,
but the forwarded decision must still become a normal permission decision.

## Classifier-Assisted Decisions

Classifier decisions are optional acceleration, not the source of truth.

Rules:

- Hard deny and safety checks beat classifier allow.
- Classifier unavailable behavior must be explicit: fail closed or fall back to
  prompt.
- Classifier denials should be tracked to prevent infinite deny/retry loops.
- If denial limits are hit, interactive sessions should fall back to user
  review; headless sessions should stop.
- Classifier prompts and raw transcripts should not enter public projections.

## Bun/Effect Boundary

Use:

- `Schema` for context, rule, update, request, decision, and audit events.
- `Effect.Service` for permission decisions.
- `Layer` for interactive UI, remote approval, hooks, classifier, and test
  implementations.
- `Queue` for approval requests.
- `Deferred` for pending approval decisions.
- `Ref` for mutable session context and denial tracking.
- `PubSub` for UI/remote surfaces that need prompt notifications.
- `Scope` for request lifetime and cancellation.

## Audit Events

Emit:

- `permission.requested`
- `permission.allowed`
- `permission.denied`
- `permission.asked`
- `permission.cancelled`
- `permission.update_applied`
- `permission.mode_changed`
- `permission.classifier_started`
- `permission.classifier_completed`
- `permission.denial_limit_hit`

Each event should include action refs, tool refs, decision reason, source, and
redaction class. Public projections should use summaries, not raw tool input
when the input may contain code, paths, prompts, or secrets.

## Safety Rules

- Deny rules beat allow rules.
- Hard safety checks beat user-saved allow rules.
- Prompt unavailable never means allow.
- Broad allow suggestions must be rejected or narrowed.
- Hooks cannot silently persist rules unless explicitly allowed.
- Remote decisions must be bound to a request id and expire.
- Approval after cancellation must be ignored.
- Permission decisions must be logged before execution begins.
- Policy/managed settings should not be modified by ordinary approval UI.
- Secrets and raw private paths should be redacted from portable audits.

## Tests

Minimum tests:

- Deny rule overrides allow rule.
- Ask rule forces prompt even when a broad allow exists.
- Hard safety check blocks a dangerous action despite allow mode.
- Allow-once does not persist a rule.
- Allow-session persists only in session context.
- Allow-always persists only to approved destination.
- Prompt cancellation produces denial/cancellation and no side effect.
- Headless no-prompt path denies without hanging.
- Remote approval resolves only the matching pending request.
- Late approval after cancellation is ignored.
- Classifier unavailable obeys fail-closed/fallback policy.
- Denial loop limit stops or falls back to review.
- Public audit redacts raw private inputs.

## Decision

The permission and approval system should be its own Effect service with typed
decisions, explicit modes, rule sources, update persistence, cancellation, and
audit events. Tool code can describe an action and suggest narrow rules, but
the permission service decides whether anything runs.
