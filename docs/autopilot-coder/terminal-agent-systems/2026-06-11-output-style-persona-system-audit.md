# Output Style And Persona System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #56 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should handle tone, verbosity, formatting, persona,
domain style, and product voice without letting style override policy or
truthfulness.

## Target

Build an output-style system that composes user preferences, project
instructions, command modes, domain requirements, accessibility needs, and
product defaults into a typed style policy.

## User-Visible Capability

Users should be able to:

- Choose concise, normal, or detailed responses.
- Set formatting preferences.
- Use task-specific modes such as review, planning, implementation, or status.
- Keep project-required language and safety rules intact.
- See when the agent is constrained by policy rather than preference.
- Override style for a single turn.

The agent should sound consistent but not theatrical when doing engineering
work. Style should support clarity, not become a hidden objective.

## Style Policy Model

Each resolved style policy should include:

- Verbosity.
- Formatting rules.
- Persona constraints.
- Domain mode.
- Audience.
- Accessibility requirements.
- Disallowed claims.
- Citation or evidence requirements.
- Final-answer expectations.
- Conflict-resolution refs.

The policy should be included in context as structured instruction, not as a
pile of untracked prose.

## Bun/Effect Boundary

Use Effect services for:

- `StylePreferenceService`: stores and resolves user preferences.
- `PersonaPolicyService`: applies product and managed style constraints.
- `OutputFormatterService`: formats final and intermediate messages.
- `StyleContextService`: creates bounded context fragments.
- `StyleAuditService`: records style decisions when they affect output.

Use Schema for style modes, formatting preferences, and conflict outcomes.

## Safety Rules

- Style cannot override safety, privacy, approval, or product-promise policy.
- Persona text cannot claim capabilities that are not live.
- A requested voice cannot expose private data or hidden chain state.
- Style changes must not alter tool authority.
- Public copy claims still require receipts.
- Accessibility settings take precedence over decorative style.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has public agent templates, product-promise copy
gates, and terminal-agent style expectations in docs, but the terminal-agent
README does not yet include an output-style/persona audit.

Related issue anchors:

- #4773 API parity contract, because style and action availability must match
  across UI and API.
- #4772 MVP exit review, because public copy must match actual receipts.

No persona or copy layer should be allowed to promote planned behavior into
live capability language.

## Tests

Minimum coverage:

- Resolve style preferences with project and managed policy.
- Enforce review-mode and implementation-mode output shape.
- Prevent style from overriding safety instructions.
- Preserve accessibility formatting settings.
- Reject capability claims without receipt refs.
- Verify final-answer formatting across simple and long tasks.
- Record style conflicts when policy wins.

## Decision

Output style should be a typed presentation layer. It can make work clearer,
but it cannot change runtime authority, privacy boundaries, or product truth.

