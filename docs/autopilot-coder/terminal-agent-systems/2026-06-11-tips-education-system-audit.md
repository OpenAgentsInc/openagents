# Tips And Education System Audit

Date: 2026-06-11

This is system #58 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should teach users about capabilities, limitations,
commands, privacy, payments, providers, and receipts without cluttering the
main workflow or making unsupported claims.

## Target

Build an education system that surfaces contextual help, first-run tips,
capability caveats, and safety explanations only when useful and only when
they match live behavior.

## User-Visible Capability

Users should be able to:

- Discover commands and workflows.
- Understand why approval is required.
- Learn what a capability can and cannot do.
- See payment, provider, and privacy caveats before risky workflows.
- Dismiss tips.
- Reopen a help topic later.
- Use the terminal without repeated tutorial noise.

Education should be contextual, short, and grounded in current capability
state.

## Education Model

Each tip should include:

- Tip id.
- Topic.
- Trigger condition.
- Audience and scope.
- Capability refs.
- Required live-state refs.
- Dismissal state.
- Expiration or version.
- Link to detailed docs where useful.

Tips should be rendered by policy, not hardcoded in scattered UI branches.

## Bun/Effect Boundary

Use Effect services for:

- `EducationCatalogService`: stores available tips and docs topics.
- `TipTriggerService`: decides when a tip is relevant.
- `DismissalStateService`: records user dismissal.
- `CapabilityEducationService`: filters tips based on live capability state.
- `HelpProjectionService`: exposes searchable help topics.

Use Schema for tip records, triggers, dismissal state, and capability refs.

## Safety Rules

- Do not show tips that imply planned capabilities are live.
- Do not hide policy caveats behind dismissible tips when they are required
  warnings.
- Dismissal of education does not dismiss approval prompts.
- Payment, provider, payout, and settlement education must use exact claim
  boundaries.
- Tips must not include secrets, private refs, or raw run data.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has public docs, product-promise registry,
agent-readable onboarding, and capability caveats. The terminal-agent README
does not yet include a tips/education audit.

Related anchors:

- #4772 MVP exit review for public-copy readiness.
- #4773 API parity contract for agent-readable help.
- #4785 settlement visibility law for payout and settlement education.

No education surface should teach users that a workflow is available until the
corresponding receipt-backed capability is available.

## Tests

Minimum coverage:

- Show first-run tips once.
- Filter tips by capability state.
- Preserve required warnings after dismissing optional tips.
- Search help topics.
- Reject tips with unsupported capability claims.
- Keep payment and settlement caveats exact.
- Sync dismissal state across sessions.
- Render tips in non-interactive mode as documentation refs only.

## Decision

Tips should teach the runtime honestly and sparingly. They should reduce
surprise without becoming product copy that outruns evidence.

